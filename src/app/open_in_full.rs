//! Logic: the Open in full dialog. It lists the Rotated Files of the Remote
//! Files to read, takes a Starting Time and a To, tells how big the read will
//! be before anything is read, and starts the reads.

use super::AppView;
use super::full_read::{Destination, FullReadRequest};
use crate::live::line_time::newest_timestamp;
use crate::remote::browse::{dirname, join};
use crate::remote::compress::Compression;
use crate::remote::estimate::{self, Estimate};
use crate::remote::full_read::{
    GenerationEstimate, GenerationPlan, estimate_generations, fit_newest, total_estimate,
};
use crate::remote::rotated::{RotatedFile, list_rotated, sniff};
use crate::remote::starting_time::{StartError, resolve_start};
use crate::remote::{ConnectError, ServerStore};
use crate::workspace::log_space::LogSpaceKind;
use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// The time of the newest line in the last 64 KB of a plain file, on the
/// clock of the file (`None` when it has none, or the file is compressed).
pub(crate) async fn end_time(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
    size: u64,
    compression: Compression,
) -> Result<Option<f64>, String> {
    if compression.is_compressed() || size == 0 {
        return Ok(None);
    }
    let take = size.min(64 * 1024) as usize;
    let mut file = sftp.open(path.to_string()).await.map_err(|e| e.to_string())?;
    file.seek(std::io::SeekFrom::Start(size - take as u64)).await.map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; take];
    let mut done = 0;
    while done < take {
        let n = file.read(&mut buf[done..]).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        done += n;
    }
    let text = String::from_utf8_lossy(&buf[..done]).to_string();
    Ok(newest_timestamp(text.lines()))
}

/// One Remote File in the dialog.
pub struct FullFile {
    pub server_id: String,
    pub server_name: String,
    pub path: String,
    pub size: u64,
    pub compression: Compression,
    /// Time of the newest line in the end of the file (for spans).
    pub newest: Option<f64>,
    /// Newest first.
    pub rotated: Vec<RotatedFile>,
    pub ticked: Vec<bool>,
    pub loading: bool,
    pub error: Option<String>,
}

impl FullFile {
    pub fn name(&self) -> String {
        format!("{}:{}", self.server_name, self.path)
    }
}

/// What the estimate of the ticked files said.
#[derive(Debug, Clone)]
pub enum EstimateState {
    /// Waiting for the files to be listed or for the answer.
    Working,
    Ready { total: Estimate, per_file: Vec<Vec<GenerationEstimate>> },
    Failed(String),
}

pub struct OpenInFull {
    pub files: Vec<FullFile>,
    /// The Log Space the command was used on (names the new one).
    pub space_name: String,
    /// An existing Imported Log Space to add to; `None` makes a new one.
    pub existing: Option<String>,
    pub new_name: String,
    pub start_text: String,
    pub to_text: String,
    pub start_input: Option<Entity<InputState>>,
    pub to_input: Option<Entity<InputState>>,
    pub name_input: Option<Entity<InputState>>,
    pub estimate: EstimateState,
    /// Counts the questions asked of the Server; only the latest answer counts.
    request: u64,
    /// The user chose "Read newest N lines".
    pub read_newest: bool,
    /// "Narrow time…" moves the cursor to the Starting Time field.
    pub focus_start: bool,
    pub disk_error: Option<String>,
}

impl OpenInFull {
    pub fn start_error(&self) -> Option<String> {
        field_error(&self.start_text, self.files.iter().filter_map(|f| f.newest).next(), self.files.iter().any(|f| f.newest.is_some()))
    }

    pub fn to_error(&self) -> Option<String> {
        field_error(&self.to_text, self.files.iter().filter_map(|f| f.newest).next(), self.files.iter().any(|f| f.newest.is_some()))
    }

    /// Whether the estimate is above the Full Read Limit.
    pub fn over_limit(&self, limit: usize) -> bool {
        matches!(&self.estimate, EstimateState::Ready { total, .. } if total.over(limit))
    }

    /// The Rotated Files whose last write is before the Starting Time hold
    /// nothing of the range: they are not ticked and say so.
    pub fn outside_range(&self, file: usize, rotated: usize) -> bool {
        let Some(f) = self.files.get(file) else { return false };
        let Some(start) = resolve_start(&self.start_text, f.newest).ok().flatten() else { return false };
        f.rotated.get(rotated).and_then(|r| r.modified).is_some_and(|m| (m as f64) < start)
    }
}

fn field_error(text: &str, newest: Option<f64>, any_times: bool) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    match resolve_start(text, newest) {
        Ok(_) => None,
        // A span needs a time in the file; the file may not have loaded yet.
        Err(StartError::NoTimestamps) if !any_times => None,
        Err(e) => Some(e.to_string()),
    }
}

/// What listing one Remote File found.
struct Listing {
    size: u64,
    compression: Compression,
    newest: Option<f64>,
    rotated: Vec<RotatedFile>,
}

impl AppView {
    /// "Open in full…" on one Remote File.
    pub fn open_in_full_file(&mut self, space_id: &str, file_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else { return };
        let files: Vec<_> = space.files.iter().filter(|f| f.file_id == file_id).cloned().collect();
        self.open_in_full_dialog(&space.name, files, cx);
    }

    /// "Open in full…" on a Remote Log Space: every Remote File of it.
    pub fn open_in_full_space(&mut self, space_id: &str, cx: &mut Context<Self>) {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)).cloned() else { return };
        let files: Vec<_> = space.files.iter().filter(|f| !f.is_remote_command()).cloned().collect();
        self.open_in_full_dialog(&space.name, files, cx);
    }

    /// The command palette and the Break button: the file of the selected
    /// line, or the only Remote File of the open Log Space.
    pub fn request_open_in_full(&mut self, file_id: &str, cx: &mut Context<Self>) {
        let space_id = self.active_project().and_then(|p| {
            p.log_spaces.iter().find(|s| s.kind == LogSpaceKind::Remote && s.has_file(file_id)).map(|s| s.id.clone())
        });
        match space_id {
            Some(space_id) => self.open_in_full_file(&space_id, file_id, cx),
            None => {
                self.notice = Some("Open in full reads a Remote File".into());
                cx.notify();
            }
        }
    }

    pub fn open_in_full_current(&mut self, cx: &mut Context<Self>) {
        if let Some(file_id) = self.current_remote_file() {
            self.request_open_in_full(&file_id, cx);
        }
    }

    fn open_in_full_dialog(
        &mut self,
        space_name: &str,
        files: Vec<crate::workspace::log_space::LogFile>,
        cx: &mut Context<Self>,
    ) {
        let files: Vec<FullFile> = files
            .into_iter()
            .filter_map(|f| {
                let remote = f.remote?;
                let path = remote.path()?.to_string();
                let server_name = self.remote.servers.get(&remote.server_id)?.name.clone();
                Some(FullFile {
                    server_id: remote.server_id,
                    server_name,
                    path,
                    size: 0,
                    compression: Compression::None,
                    newest: None,
                    rotated: Vec::new(),
                    ticked: Vec::new(),
                    loading: true,
                    error: None,
                })
            })
            .collect();
        if files.is_empty() {
            return;
        }
        let new_name = unique_name(self, &format!("{space_name} (full)"));
        self.remote.full_dialog = Some(OpenInFull {
            files,
            space_name: space_name.to_string(),
            existing: None,
            new_name,
            start_text: String::new(),
            to_text: String::new(),
            start_input: None,
            to_input: None,
            name_input: None,
            estimate: EstimateState::Working,
            request: 0,
            read_newest: false,
            focus_start: false,
            disk_error: None,
        });
        let count = self.remote.full_dialog.as_ref().map_or(0, |d| d.files.len());
        for index in 0..count {
            self.load_full_file(index, cx);
        }
        cx.notify();
    }

    pub fn close_open_in_full(&mut self, cx: &mut Context<Self>) {
        self.remote.full_dialog = None;
        cx.notify();
    }

    /// Create the fields of a freshly opened dialog.
    pub fn sync_open_in_full(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.remote.full_dialog.as_mut() else { return };
        if dialog.start_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("From: 1h, 30m, 2d, 10:42, 2026-10-19 10:00"));
            cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _w, cx| {
                if let InputEvent::Change = event {
                    let text = input.read(cx).value().to_string();
                    if let Some(d) = this.remote.full_dialog.as_mut() {
                        d.start_text = text;
                        d.read_newest = false;
                    }
                    this.estimate_open_in_full(cx);
                }
            })
            .detach();
            dialog.start_input = Some(input);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("To (optional)"));
            cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _w, cx| {
                if let InputEvent::Change = event {
                    let text = input.read(cx).value().to_string();
                    if let Some(d) = this.remote.full_dialog.as_mut() {
                        d.to_text = text;
                        d.read_newest = false;
                    }
                    this.estimate_open_in_full(cx);
                }
            })
            .detach();
            dialog.to_input = Some(input);
            let initial = dialog.new_name.clone();
            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder("Name of the new Log Space");
                state.set_value(gpui::SharedString::from(initial), window, cx);
                state
            });
            cx.subscribe_in(&input, window, |this: &mut Self, input, event: &InputEvent, _w, cx| {
                if let InputEvent::Change = event {
                    let text = input.read(cx).value().to_string();
                    if let Some(d) = this.remote.full_dialog.as_mut() {
                        d.new_name = text;
                        d.existing = None;
                    }
                    cx.notify();
                }
            })
            .detach();
            dialog.name_input = Some(input);
        }
        if dialog.focus_start {
            dialog.focus_start = false;
            if let Some(input) = dialog.start_input.clone() {
                input.update(cx, |state, cx| state.focus(window, cx));
            }
        }
    }

    /// List the Rotated Files of one file and read the end of it.
    fn load_full_file(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(dialog) = self.remote.full_dialog.as_ref() else { return };
        let Some(file) = dialog.files.get(index) else { return };
        let (server_id, path) = (file.server_id.clone(), file.path.clone());
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.remote.servers.clone();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let session = hub.session(&store, &server_id).await.map_err(|e| e.message())?;
                let sftp = session.open_sftp().await?;
                let size = sftp.metadata(path.clone()).await.map_err(|e| format!("{path}: {e}"))?.len();
                let compression = sniff(&sftp, &path).await?;
                let newest = end_time(&sftp, &path, size, compression).await?;
                let rotated = list_rotated(&sftp, &path).await?;
                Ok::<_, String>(Listing { size, compression, newest, rotated })
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let Some(dialog) = this.remote.full_dialog.as_mut() else { return };
                let Some(file) = dialog.files.get_mut(index) else { return };
                file.loading = false;
                match result {
                    Ok(Ok(listing)) => {
                        file.size = listing.size;
                        file.compression = listing.compression;
                        file.newest = listing.newest;
                        file.ticked = vec![false; listing.rotated.len()];
                        file.rotated = listing.rotated;
                    }
                    Ok(Err(message)) => file.error = Some(message),
                    Err(e) => file.error = Some(format!("Could not reach the server: {e}")),
                }
                this.estimate_open_in_full(cx);
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_full_rotated(&mut self, file: usize, rotated: usize, cx: &mut Context<Self>) {
        let Some(dialog) = self.remote.full_dialog.as_mut() else { return };
        let Some(f) = dialog.files.get_mut(file) else { return };
        if f.rotated.get(rotated).is_none_or(|r| !r.compression.is_supported()) {
            return;
        }
        if let Some(t) = f.ticked.get_mut(rotated) {
            *t = !*t;
        }
        dialog.read_newest = false;
        self.estimate_open_in_full(cx);
    }

    pub fn choose_full_destination(&mut self, existing: Option<String>, cx: &mut Context<Self>) {
        if let Some(dialog) = self.remote.full_dialog.as_mut() {
            dialog.existing = existing;
        }
        cx.notify();
    }

    /// "Narrow time…": the cursor goes to the Starting Time.
    pub fn full_narrow_time(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.remote.full_dialog.as_mut() {
            dialog.focus_start = true;
        }
        cx.notify();
    }

    /// "Read newest N lines".
    pub fn full_read_newest(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.remote.full_dialog.as_mut() {
            dialog.read_newest = true;
        }
        self.confirm_open_in_full(cx);
    }

    /// The Generations the dialog would read for one file, oldest first.
    fn full_plan_for(file: &FullFile) -> Vec<GenerationPlan> {
        let dir = dirname(&file.path);
        let mut plan: Vec<GenerationPlan> = file
            .rotated
            .iter()
            .enumerate()
            .filter(|(i, _)| file.ticked.get(*i).copied().unwrap_or(false))
            .map(|(i, r)| GenerationPlan {
                distance: i as u32 + 1,
                name: r.name.clone(),
                path: join(&dir, &r.name),
                size: r.size,
                modified: r.modified,
                compression: r.compression,
                start_byte: None,
                skip_lines: 0,
                older_dropped: false,
            })
            .collect();
        plan.reverse();
        plan.push(GenerationPlan {
            distance: 0,
            name: file.path.rsplit('/').next().unwrap_or(&file.path).to_string(),
            path: file.path.clone(),
            size: file.size,
            modified: None,
            compression: file.compression,
            start_byte: None,
            skip_lines: 0,
            older_dropped: false,
        });
        plan
    }

    /// Ask the Servers how big the read would be (a moment after the last
    /// change).
    pub fn estimate_open_in_full(&mut self, cx: &mut Context<Self>) {
        let hub = self.remote_hub(cx);
        let store: ServerStore = self.remote.servers.clone();
        let Some(dialog) = self.remote.full_dialog.as_mut() else { return };
        dialog.request += 1;
        dialog.disk_error = None;
        let request = dialog.request;
        if dialog.files.iter().any(|f| f.loading) || dialog.files.iter().all(|f| f.error.is_some()) {
            dialog.estimate = EstimateState::Working;
            cx.notify();
            return;
        }
        if dialog.start_error().is_some() || dialog.to_error().is_some() {
            dialog.estimate = EstimateState::Working;
            cx.notify();
            return;
        }
        dialog.estimate = EstimateState::Working;
        let jobs: Vec<(usize, String, Vec<GenerationPlan>, Option<f64>, Option<f64>)> = dialog
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.error.is_none())
            .map(|(i, f)| {
                let start = resolve_start(&dialog.start_text, f.newest).ok().flatten();
                let to = resolve_start(&dialog.to_text, f.newest).ok().flatten();
                (i, f.server_id.clone(), Self::full_plan_for(f), start, to)
            })
            .collect();
        let count = dialog.files.len();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                // A moment for the user to finish typing.
                tokio::time::sleep(Duration::from_millis(350)).await;
                let mut per_file: Vec<Vec<GenerationEstimate>> = vec![Vec::new(); count];
                for (index, server_id, plan, start, to) in jobs {
                    let session = hub.session(&store, &server_id).await.map_err(|e: ConnectError| e.message())?;
                    let sftp = session.open_sftp().await?;
                    per_file[index] = estimate_generations(&sftp, &plan, start, to).await?;
                }
                Ok::<_, String>(per_file)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let root = this.copies_root();
                let Some(dialog) = this.remote.full_dialog.as_mut() else { return };
                if dialog.request != request {
                    return;
                }
                dialog.estimate = match result {
                    Ok(Ok(per_file)) => {
                        let mut total = Estimate::default();
                        for ests in &per_file {
                            total.add(total_estimate(ests));
                        }
                        if let Some(free) = estimate::free_bytes(&root) {
                            if !estimate::disk_has_room(total.bytes, free) {
                                dialog.disk_error = Some(format!(
                                    "Not enough disk space: about {} needed, {} free",
                                    crate::live::buffer::size_text(total.bytes),
                                    crate::live::buffer::size_text(free)
                                ));
                            }
                        }
                        EstimateState::Ready { total, per_file }
                    }
                    Ok(Err(message)) => EstimateState::Failed(message),
                    Err(e) => EstimateState::Failed(format!("Could not reach the server: {e}")),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Open: read every file into a Remote Copy.
    pub fn confirm_open_in_full(&mut self, cx: &mut Context<Self>) {
        let limit = self.live.full_read_limit;
        let Some(dialog) = self.remote.full_dialog.as_ref() else { return };
        if dialog.start_error().is_some() || dialog.to_error().is_some() || dialog.disk_error.is_some() {
            return;
        }
        let (per_file_estimates, over) = match &dialog.estimate {
            EstimateState::Ready { total, per_file } => (per_file.clone(), total.over(limit)),
            _ => return,
        };
        if over && !dialog.read_newest {
            return;
        }
        let share = (limit / dialog.files.iter().filter(|f| f.error.is_none()).count().max(1)).max(1);
        let mut requests: Vec<FullReadRequest> = Vec::new();
        for (i, file) in dialog.files.iter().enumerate() {
            if file.error.is_some() {
                continue;
            }
            let start = resolve_start(&dialog.start_text, file.newest).ok().flatten();
            let to = resolve_start(&dialog.to_text, file.newest).ok().flatten();
            let mut generations = Self::full_plan_for(file);
            if dialog.read_newest {
                if let Some(ests) = per_file_estimates.get(i).filter(|e| e.len() == generations.len()) {
                    fit_newest(&mut generations, ests, share);
                }
            }
            requests.push(FullReadRequest {
                server_id: file.server_id.clone(),
                server_name: file.server_name.clone(),
                path: file.path.clone(),
                generations,
                start,
                to,
                start_text: Some(dialog.start_text.trim().to_string()).filter(|t| !t.is_empty()),
                to_text: Some(dialog.to_text.trim().to_string()).filter(|t| !t.is_empty()),
                destination: Destination::NewSpace(String::new()),
                replace: None,
                limit_lines: if dialog.read_newest || over { share } else { limit },
            });
        }
        let destination = match &dialog.existing {
            Some(id) => Destination::Existing(id.clone()),
            None => Destination::NewSpace(dialog.new_name.trim().to_string()),
        };
        if matches!(&destination, Destination::NewSpace(name) if name.is_empty()) {
            self.notice = Some("Name the new Log Space".into());
            cx.notify();
            return;
        }
        self.remote.full_dialog = None;
        let mut space_id: Option<String> = None;
        for mut request in requests {
            request.destination = match &space_id {
                Some(id) => Destination::Existing(id.clone()),
                None => destination.clone(),
            };
            if let Some(file_id) = self.start_full_read(request, cx) {
                if space_id.is_none() {
                    space_id = self.live.copy_reads.get(&file_id).map(|r| r.space_id.clone());
                }
            }
        }
        cx.notify();
    }
}

/// `base`, or `base 2`, `base 3`… when a Log Space has that name already.
fn unique_name(app: &AppView, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while app.active_project().is_some_and(|p| p.has_space_named(&name)) {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}
