//! Logic: live reading. A source per file sends the lines written after the
//! Log Space was opened; the app gathers what arrived in about 100 ms and
//! applies it once: parse, place by timestamp, update Filter, Search and the
//! Focus Pane, and follow the end when Follow is on.

use crate::rows::Rows as _;
use super::AppView;
use crate::focus::LineRef;
use crate::live::{
    BreakKind, LineSource, LocalFileSource, SourceCommand, SourceEvent, SourceHandle,
    SourceStatus, Trimmed,
};
use crate::model::LogEntry;
use crate::workspace::log_space::{LogSpaceKind, RemoteTarget};
use crate::workspace::working_set::{TabTarget, ViewMode};
use gpui::{Context, Pixels, point};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::time::Duration;

/// How long the app gathers new lines before applying them.
pub const LIVE_BATCH_INTERVAL: Duration = Duration::from_millis(100);

/// How long one pump spends applying lines before it leaves the rest for the
/// next one, so a burst of lines never holds up a frame.
pub const PUMP_BUDGET: Duration = Duration::from_millis(8);

/// The most lines of one file one pump takes: a bigger `Append` is cut.
pub const LINES_PER_FILE_PER_PUMP: usize = 8_000;

/// A batch of this many lines or more is read on a worker thread before the
/// app applies it.
pub const PREPARE_MIN_LINES: usize = 1_500;

/// Default Live Window: the newest lines kept for each Remote File.
pub const DEFAULT_LIVE_WINDOW: usize = 100_000;

/// The most lines a Remote File holds once Load More made its window longer
/// (the Full Read Limit; `remote-full-read` will make it a setting).
pub const FULL_READ_LIMIT: usize = 2_000_000;

/// A running source and what it last said.
pub struct LiveSourceState {
    pub handle: SourceHandle,
    /// Which source this is: a file whose source was replaced (edited, read
    /// again) has a new number. `pump_live` drains only the sources it holds,
    /// each through its own channel, so a late event of a closed source never
    /// reaches the file's new lines.
    pub seq: u64,
    pub status: SourceStatus,
    /// Name shown for the file in the status bar list.
    pub name: String,
    /// Whether the file is on a Server (Live Window applies).
    pub remote: bool,
    /// Whether it is a Remote Command (its output cannot be read again).
    pub command: bool,
    /// Where a remote source's real line numbers could not be counted.
    pub numbers_unavailable: bool,
    /// Reconnects or Runs, last error and last line: shown in Connection
    /// Details only, and not remembered.
    pub health: crate::live::SourceHealth,
}

/// Live state of the app: not remembered when the app closes.
pub struct LiveRuntime {
    pub sources: HashMap<String, LiveSourceState>,
    /// Log Spaces whose files are not read for now.
    pub paused_spaces: HashSet<String>,
    /// The view scrolls to the newest line.
    pub follow: bool,
    /// Lines that arrived while Follow was off.
    pub unseen: usize,
    /// Live Window: newest lines kept per Remote File.
    pub window: usize,
    /// Lines one Load More reads when the user gives no number.
    pub load_more_step: usize,
    /// Most lines one Open in full reads, and the most a Remote File holds
    /// once Load More made its window longer.
    pub full_read_limit: usize,
    /// Most files one Live Glob reads at once.
    pub glob_limit: usize,
    /// Seconds between two checks of a Live Glob's folder.
    pub glob_interval: u64,
    /// The number the next source gets (`LiveSourceState::seq`).
    next_source_seq: u64,
    /// A Remote Command that was edited onto another Server: the Server it
    /// left, whose connection closes when its old Run has ended.
    pub release_after_replace: HashMap<String, String>,
    /// Files with a Load More being read.
    pub loading_more: HashSet<String>,
    /// The next Rotated File each file could load above its lines.
    pub earlier: HashMap<String, crate::live::source::EarlierFile>,
    /// Files with a Rotated File being loaded.
    pub loading_earlier: HashSet<String>,
    /// Long reads under way, by file.
    pub progress: HashMap<String, crate::live::source::ReadProgress>,
    /// Remote Copies being read, by file.
    pub copy_reads: HashMap<String, super::full_read::CopyRead>,
    /// The Live Globs' watchers and what they found.
    pub globs: super::globs::GlobRuntime,
    pub pump_started: bool,
    /// Events taken from a source that did not fit one pump, per file, in
    /// order, to be applied before what the source sends next.
    pub(crate) backlog: HashMap<String, std::collections::VecDeque<SourceEvent>>,
    /// Files whose batch is being read on a worker thread, by the `seq` of
    /// their source; their events wait until it is applied.
    parsing: HashMap<String, u64>,
    /// Where the next pump starts among the files, so every file gets turns.
    pump_cursor: usize,
    /// When the files of an open Static Log Space were last looked at.
    static_watched: Option<std::time::Instant>,
    /// Files of the open Static Log Space that are no longer on disk: their
    /// lines stay, read through the handle still open, until a file is at
    /// the path again.
    pub static_gone: HashSet<String>,
    /// Local files of a Live Log Space deleted from disk: when a file is at
    /// the path again, its lines are a new Generation below a Break
    /// "replaced".
    pub local_deleted: HashSet<String>,
    /// The Break above the first line of a local file read again from its
    /// start (it was cut, or written over), until it is read.
    pub first_marks: HashMap<String, std::sync::Arc<crate::live::BreakMark>>,
    /// The handles the local files read through their Line Index may hold
    /// open: each Generation of a file kept readable holds one.
    pub handle_budget: std::sync::Arc<crate::index::HandleBudget>,
    /// A batch of this many lines is read on a worker thread.
    pub preparse_min: usize,
    /// Batches applied since the allocator's cache was last given back.
    batches_since_release: usize,
}

impl LiveRuntime {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            paused_spaces: HashSet::new(),
            follow: true,
            unseen: 0,
            window: DEFAULT_LIVE_WINDOW,
            load_more_step: 10_000,
            full_read_limit: FULL_READ_LIMIT,
            glob_limit: 50,
            glob_interval: 5,
            next_source_seq: 0,
            release_after_replace: HashMap::new(),
            loading_more: HashSet::new(),
            earlier: HashMap::new(),
            loading_earlier: HashSet::new(),
            progress: HashMap::new(),
            copy_reads: HashMap::new(),
            globs: super::globs::GlobRuntime::new(),
            pump_started: false,
            backlog: HashMap::new(),
            parsing: HashMap::new(),
            pump_cursor: 0,
            static_watched: None,
            static_gone: HashSet::new(),
            local_deleted: HashSet::new(),
            first_marks: HashMap::new(),
            handle_budget: crate::index::HandleBudget::shared(),
            preparse_min: PREPARE_MIN_LINES,
            batches_since_release: 0,
        }
    }
}

impl Default for LiveRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// A file read live while a tab of its Log Space is open.
struct NeededFile {
    path: Option<String>,
    name: String,
    remote: Option<crate::workspace::log_space::RemoteRef>,
    /// The Live Glob that brought the file, if any.
    from_glob: Option<String>,
    spaces: Vec<String>,
}

/// The row at the top of the view before rows moved.
pub(crate) struct TopAnchor {
    line: LineRef,
    /// Its position in the rows.
    pub(crate) row: usize,
    /// How far the view is scrolled into the row.
    within: Pixels,
    row_height: Pixels,
}

/// What a batch did to one file.
#[derive(Default, Clone)]
pub(crate) struct FileDelta {
    /// Positions in the file's entries of the lines appended at the end.
    pub(crate) appended: Option<Range<usize>>,
    /// Lines were inserted above, dropped, or renumbered.
    pub(crate) structural: bool,
    pub(crate) trimmed: Option<Trimmed>,
    pub(crate) renumbered: Option<isize>,
    pub(crate) generation: u32,
    /// The last line already shown was replaced by its finished version.
    pub(crate) replaced_last: bool,
    /// Something the app shows may have changed (lines, a status, a
    /// progress, a notice): a batch with none needs no new frame when the
    /// open view holds none of its files.
    pub(crate) shown: bool,
}

pub(crate) fn now_label() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

impl AppView {
    // ── Which Log Space is live ─────────────────────────────────────────────

    /// The Log Space of the active tab: the only one read (the other tabs
    /// are Dormant Tabs).
    pub(crate) fn open_space_ids(&self) -> Vec<String> {
        self.active_tab_files().map(|(space, _)| vec![space]).unwrap_or_default()
    }

    /// Log Spaces of the active project that have a tab, active or not.
    pub(crate) fn tab_space_ids(&self) -> Vec<String> {
        let Some(project) = self.active_project() else { return Vec::new() };
        let mut ids: Vec<String> = Vec::new();
        for tab in project.working_set.tabs() {
            if let Some(id) = tab.target.space_id() {
                if !ids.iter().any(|i| i == id) {
                    ids.push(id.to_string());
                }
            }
        }
        ids
    }

    /// What the active tab reads: its Log Space, and the one file of a File
    /// View (`None`: every file of the space).
    pub(crate) fn active_tab_files(&self) -> Option<(String, Option<String>)> {
        let tab = self.active_project()?.working_set.active_tab()?;
        match &tab.target {
            TabTarget::LogSpace { space_id } => Some((space_id.clone(), None)),
            TabTarget::FileView { space_id, file_id } => Some((space_id.clone(), Some(file_id.clone()))),
            TabTarget::InvestigationBoard { .. } => None,
        }
    }

    /// Whether the tab at `index` is a Dormant Tab: it shows log lines but is
    /// not the active tab, so none of its files is read.
    pub fn is_dormant_tab(&self, index: usize) -> bool {
        let Some(working_set) = self.active_project().map(|p| &p.working_set) else { return false };
        working_set.active_tab_index() != Some(index)
            && working_set.tabs().get(index).is_some_and(|t| t.target.is_log_view())
    }

    /// The `file_id`s the active tab reads.
    pub(crate) fn active_file_ids(&self) -> HashSet<String> {
        let Some((space_id, only)) = self.active_tab_files() else { return HashSet::new() };
        let Some(space) = self.active_project().and_then(|p| p.log_space(&space_id)) else { return HashSet::new() };
        space
            .files
            .iter()
            .filter(|f| only.as_deref().is_none_or(|id| id == f.file_id))
            .map(|f| f.file_id.clone())
            .collect()
    }

    /// Let go of the lines of the files the active tab does not read: their
    /// tabs are Dormant and read their files again when chosen. Scratch
    /// edits are saved first.
    pub(crate) fn drop_dormant_files(&mut self) {
        let keep = self.active_file_ids();
        if self.parsed_files.keys().all(|id| keep.contains(id)) {
            return;
        }
        self.flush_scratch_saves();
        self.parsed_files.retain(|id, _| keep.contains(id));
    }

    /// Whether the open tab reads a Live Log Space: it has Live/Pause and
    /// Follow. A Static one shows its files as they are on disk.
    pub fn active_space_is_live(&self) -> bool {
        self.active_space().is_some_and(|s| s.kind == LogSpaceKind::Live)
    }

    /// The files of the Log Spaces with a tab open that are read live, by
    /// `file_id`: local path, name, Server and path for a Remote File, and
    /// the Log Spaces holding it.
    fn needed_files(&self) -> HashMap<String, NeededFile> {
        let open = self.open_space_ids();
        let reads = self.active_file_ids();
        let mut needed: HashMap<String, NeededFile> = HashMap::new();
        if let Some(project) = self.active_project() {
            for space_id in &open {
                let Some(space) = project.log_space(space_id) else { continue };
                // Only a Live Log Space is followed: a Static one is watched
                // for changes instead (`watch_static_files`).
                if space.kind != LogSpaceKind::Live {
                    continue;
                }
                // Archive Entries are static: they are read once, never live.
                // A File View reads its own file only.
                for file in space.files.iter().filter(|f| f.entry.is_none() && reads.contains(&f.file_id)) {
                    let entry = needed.entry(file.file_id.clone()).or_insert_with(|| NeededFile {
                        path: file.path.clone(),
                        name: file.display_name.clone(),
                        remote: file.remote.clone(),
                        from_glob: file.from_glob.clone(),
                        spaces: Vec::new(),
                    });
                    entry.spaces.push(space.id.clone());
                }
            }
        }
        needed
    }

    /// Start the sources of the files in open tabs and stop those of files no
    /// tab shows any more. Safe to call at any time.
    pub(crate) fn sync_live_sources(&mut self, cx: &mut Context<Self>) {
        let open = self.open_space_ids();
        let needed = self.needed_files();
        // Stop the sources nobody needs.
        let stale: Vec<String> = self
            .live
            .sources
            .keys()
            .filter(|id| !needed.contains_key(*id))
            .cloned()
            .collect();
        for file_id in stale {
            self.live.backlog.remove(&file_id);
            if let Some(source) = self.live.sources.remove(&file_id) {
                source.handle.close();
            }
            // What was received is not kept after the Log Space closes.
            self.parsed_files.remove(&file_id);
        }
        // A Dormant Tab's space stays paused for when it is chosen again.
        let tabs = self.tab_space_ids();
        self.live.paused_spaces.retain(|id| open.contains(id) || tabs.contains(id));
        self.drop_dormant_files();
        self.drop_dormant_loads();
        self.start_missing_sources(cx);
        self.sync_glob_watchers(cx);
    }

    /// Start a source for every needed file that has none.
    pub(crate) fn start_missing_sources(&mut self, cx: &mut Context<Self>) {
        for (file_id, file) in self.needed_files() {
            if self.live.sources.contains_key(&file_id) {
                continue;
            }
            let Some(parsed) = self.parsed_files.get(&file_id) else { continue };
            // A file read through its Line Index follows from what is indexed.
            let local_offset = match (&parsed.indexed, parsed.live.as_ref()) {
                (Some(indexed), _) => indexed.indexed.data.read().unwrap().file.lines.end(),
                (None, Some(buffer)) => buffer.local_offset,
                (None, None) => continue,
            };
            let handle = match (&file.remote, &file.path) {
                (Some(remote), _) => {
                    let spawned = match &remote.target {
                        RemoteTarget::File { path } => self.spawn_remote_source(
                            &remote.server_id,
                            path,
                            &file.name,
                            file.from_glob.as_deref(),
                            remote.start_time.clone(),
                            cx,
                        ),
                        RemoteTarget::Command { .. } => {
                            self.spawn_command_source(&remote.server_id, &remote.target, &file.name, cx)
                        }
                    };
                    match spawned {
                        Some(handle) => handle,
                        None => continue,
                    }
                }
                (None, Some(path)) => LocalFileSource::new(path, local_offset).spawn(),
                _ => continue,
            };
            // A source starts a new run: Markers of older Generations are gone.
            if let Some(project) = self.active_project_mut() {
                for space in &mut project.log_spaces {
                    if let Some(f) = space.file_mut(&file_id) {
                        f.markers.retain(|m| m.generations_ago == 0);
                        // Another Run prints other lines: the Markers of a
                        // Remote Command do not come back.
                        if f.is_remote_command() {
                            f.markers.clear();
                        }
                        f.generation = 0;
                    }
                }
            }
            if file.spaces.iter().any(|s| self.live.paused_spaces.contains(s)) {
                handle.send(SourceCommand::Pause);
            }
            self.live.next_source_seq += 1;
            let seq = self.live.next_source_seq;
            // Events left over from a source this one replaces are not its.
            self.live.backlog.remove(&file_id);
            self.live.sources.insert(
                file_id,
                LiveSourceState {
                    handle,
                    seq,
                    status: SourceStatus::Connecting,
                    name: file.name,
                    remote: file.remote.is_some(),
                    command: file.remote.as_ref().is_some_and(|r| r.is_command()),
                    numbers_unavailable: false,
                    health: Default::default(),
                },
            );
        }
    }

    /// Run the loop that applies what the sources sent, every 100 ms.
    pub(crate) fn start_live_pump(&mut self, cx: &mut Context<Self>) {
        if self.live.pump_started {
            return;
        }
        self.live.pump_started = true;
        cx.spawn(async move |this, cx| {
            let mut wait = LIVE_BATCH_INTERVAL;
            loop {
                cx.background_executor().timer(wait).await;
                // With lines left over the next pump comes at once, a frame
                // later, instead of after the whole interval.
                match this.update(cx, |this, cx| this.pump_live(cx)) {
                    Ok(more) => wait = if more { Duration::from_millis(16) } else { LIVE_BATCH_INTERVAL },
                    Err(_) => break,
                }
            }
        })
        .detach();
    }

    // ── Follow and Pause ────────────────────────────────────────────────────

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        if !self.active_space_is_live() {
            return;
        }
        self.set_follow(!self.live.follow, cx);
    }

    pub fn set_follow(&mut self, on: bool, cx: &mut Context<Self>) {
        self.live.follow = on;
        if on {
            self.live.unseen = 0;
            self.scroll_to_newest();
        }
        cx.notify();
    }

    pub(crate) fn scroll_to_newest(&mut self) {
        match self.view_mode {
            ViewMode::Parsed => self.scroll_handle.scroll_to_bottom(),
            ViewMode::Raw => self.raw.scroll_to_end = true,
        }
    }

    /// Whether the open Log Space is paused.
    pub fn active_space_paused(&self) -> bool {
        self.active_space().is_some_and(|s| self.live.paused_spaces.contains(&s.id))
    }

    /// Pause or resume every file of the open Log Space together.
    pub fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        if !self.active_space_is_live() {
            return;
        }
        let Some(space) = self.active_space().cloned() else { return };
        let pause = !self.live.paused_spaces.contains(&space.id);
        if pause {
            self.live.paused_spaces.insert(space.id.clone());
        } else {
            self.live.paused_spaces.remove(&space.id);
        }
        for file in &space.files {
            if let Some(source) = self.live.sources.get(&file.file_id) {
                source.handle.send(if pause { SourceCommand::Pause } else { SourceCommand::Resume });
            }
        }
        self.set_copy_reads_paused(&space.id, pause);
        // Paused, a Live Glob adds nothing; resumed, its folder is listed at once.
        self.sync_glob_watchers(cx);
        cx.notify();
    }

    /// Every live file is told the new Live Window.
    pub fn set_live_window(&mut self, window: usize, cx: &mut Context<Self>) {
        self.live.window = window.max(1_000);
        self.workspace_state.live_window = self.live.window;
        self.schedule_session_save(cx);
        for source in self.live.sources.values().filter(|s| s.remote) {
            source.handle.send(SourceCommand::SetWindow(self.live.window));
        }
        let window = self.live.window;
        let mut trimmed_any = false;
        let remote_ids: Vec<String> = self
            .live
            .sources
            .keys()
            .cloned()
            .collect();
        for file_id in remote_ids {
            if let Some(parsed) = self.parsed_files.get_mut(&file_id) {
                if let Some(mut buffer) = parsed.live.take() {
                    // A smaller window drops at once, not after 10% more.
                    let limit = buffer.limit(window, self.live.full_read_limit);
                    let trimmed = buffer.trim_to_window(parsed, limit);
                    parsed.live = Some(buffer);
                    if let Some(trimmed) = trimmed {
                        self.drop_markers_for(&file_id, &trimmed);
                        trimmed_any = true;
                    }
                }
            }
        }
        if trimmed_any {
            self.rebuild_open_view_keeping_position(cx);
        }
        cx.notify();
    }

    // ── Applying a batch ────────────────────────────────────────────────────

    /// Apply what the sources sent, for as long as the budget lasts. Returns
    /// whether lines are left for another pump.
    pub fn pump_live(&mut self, cx: &mut Context<Self>) -> bool {
        self.pump_copy_reads(cx);
        self.watch_static_files(cx);
        if self.live.sources.is_empty() {
            return false;
        }
        self.update_follow_from_scroll();

        let started = std::time::Instant::now();
        let mut ids: Vec<String> = self.live.sources.keys().cloned().collect();
        ids.sort();
        let turn = self.live.pump_cursor % ids.len();
        self.live.pump_cursor = self.live.pump_cursor.wrapping_add(1);
        ids.rotate_left(turn);

        let mut deltas: Vec<(String, FileDelta)> = Vec::new();
        for file_id in ids {
            if !deltas.is_empty() && started.elapsed() >= PUMP_BUDGET {
                break;
            }
            if self.live.parsing.contains_key(&file_id) {
                continue;
            }
            let events = self.drain_events(&file_id);
            if events.is_empty() {
                continue;
            }
            let events = match self.start_preparing(&file_id, events, cx) {
                Ok(()) => continue,
                Err(events) => events,
            };
            let delta = self.apply_file_events(&file_id, events);
            deltas.push((file_id, delta));
        }
        let more = !self.live.parsing.is_empty()
            || self.live.backlog.values().any(|q| !q.is_empty())
            || self.live.sources.values().any(|s| !s.handle.events.is_empty());
        if deltas.is_empty() {
            return more;
        }

        let parsed_in = started.elapsed();
        self.apply_deltas(deltas, cx);
        self.ask_again_after_auth_failure(cx);
        if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
            eprintln!(
                "live batch: parse {:?}, view {:?}, rows {}",
                parsed_in,
                started.elapsed() - parsed_in,
                self.view.len()
            );
        }
        more
    }

    /// The events of `file_id` waiting to be applied, up to
    /// `LINES_PER_FILE_PER_PUMP` lines: an `Append` that is longer is cut,
    /// and what is left of it waits for the next pump.
    fn drain_events(&mut self, file_id: &str) -> Vec<SourceEvent> {
        let Some(source) = self.live.sources.get(file_id) else { return Vec::new() };
        let incoming = source.handle.events.clone();
        let mut events = Vec::new();
        let mut taken = 0usize;
        while taken < LINES_PER_FILE_PER_PUMP {
            let waiting = self.live.backlog.get_mut(file_id).and_then(|q| q.pop_front());
            let Some(mut event) = waiting.or_else(|| incoming.try_recv().ok()) else { break };
            let room = LINES_PER_FILE_PER_PUMP - taken;
            if let SourceEvent::Append { lines, bytes, streams } = &mut event {
                if lines.len() > room {
                    let rest_lines = lines.split_off(room);
                    let rest_bytes = bytes.split_off(room.min(bytes.len()));
                    let rest_streams = streams.as_mut().map(|s| s.split_off(room));
                    self.live.backlog.entry(file_id.to_string()).or_default().push_front(SourceEvent::Append {
                        lines: rest_lines,
                        bytes: rest_bytes,
                        streams: rest_streams,
                    });
                }
            }
            taken += match &event {
                SourceEvent::Tail { lines, .. }
                | SourceEvent::Backfill { lines, .. }
                | SourceEvent::Older { lines, .. }
                | SourceEvent::Earlier { lines, .. }
                | SourceEvent::Append { lines, .. } => lines.len().max(1),
                _ => 1,
            };
            events.push(event);
        }
        events
    }

    /// Read the lines of a big batch on a worker thread, and apply the batch
    /// when they are read, so the frames go on meanwhile. A small batch is
    /// given back for the caller to apply.
    fn start_preparing(
        &mut self,
        file_id: &str,
        events: Vec<SourceEvent>,
        cx: &mut Context<Self>,
    ) -> Result<(), Vec<SourceEvent>> {
        let lines: usize = events
            .iter()
            .map(|e| match e {
                SourceEvent::Tail { lines, .. }
                | SourceEvent::Backfill { lines, .. }
                | SourceEvent::Older { lines, .. }
                | SourceEvent::Append { lines, .. } => lines.len(),
                _ => 0,
            })
            .sum();
        let (Some(source), Some(parsed)) = (self.live.sources.get(file_id), self.parsed_files.get(file_id)) else {
            return Err(events);
        };
        let Some(buffer) = parsed.live.as_ref() else { return Err(events) };
        if lines < self.live.preparse_min {
            return Err(events);
        }
        let job = buffer.prepare_job(parsed.entries.first().map(|e| e.generation));
        let seq = source.seq;
        self.live.parsing.insert(file_id.to_string(), seq);
        let file_id = file_id.to_string();
        cx.spawn(async move |this, cx| {
            let (events, prepared) = cx
                .background_executor()
                .spawn(async move {
                    let prepared = job.run(&events);
                    (events, prepared)
                })
                .await;
            this.update(cx, |this, cx| this.finish_preparing(&file_id, seq, events, prepared, cx)).ok();
        })
        .detach();
        Ok(())
    }

    fn finish_preparing(
        &mut self,
        file_id: &str,
        seq: u64,
        events: Vec<SourceEvent>,
        prepared: std::collections::VecDeque<crate::live::PreparedLines>,
        cx: &mut Context<Self>,
    ) {
        self.live.parsing.remove(file_id);
        // A source that was closed or replaced meanwhile has no say any more.
        if self.live.sources.get(file_id).map(|s| s.seq) != Some(seq) {
            return;
        }
        if let Some(buffer) = self.parsed_files.get_mut(file_id).and_then(|p| p.live.as_mut()) {
            buffer.set_prepared(prepared);
        }
        let delta = self.apply_file_events(file_id, events);
        if let Some(buffer) = self.parsed_files.get_mut(file_id).and_then(|p| p.live.as_mut()) {
            buffer.set_prepared(Default::default());
        }
        self.apply_deltas(vec![(file_id.to_string(), delta)], cx);
        self.ask_again_after_auth_failure(cx);
    }

    /// Follow turns off when the user scrolled away from the newest line, and
    /// on when they scrolled back to it.
    fn update_follow_from_scroll(&mut self) {
        if self.view_mode != ViewMode::Parsed || !self.active_space_is_live() {
            return;
        }
        let state = self.scroll_handle.0.borrow();
        // A scroll we asked for has not been laid out yet.
        if state.deferred_scroll_to_item.is_some() {
            return;
        }
        drop(state);
        let at_end = self.scroll_handle.is_scrolled_to_end().unwrap_or(true);
        if at_end && !self.live.follow {
            self.live.follow = true;
            self.live.unseen = 0;
        } else if !at_end && self.live.follow {
            self.live.follow = false;
        }
    }

    fn apply_file_events(&mut self, file_id: &str, events: Vec<SourceEvent>) -> FileDelta {
        let mut delta = FileDelta::default();
        let window = self.live.window;
        let time = now_label();
        let Some(mut parsed) = self.parsed_files.remove(file_id) else { return delta };
        if parsed.indexed.is_some() {
            return self.apply_indexed_events(file_id, parsed, events);
        }
        let Some(mut buffer) = parsed.live.take() else {
            self.parsed_files.insert(file_id.to_string(), parsed);
            return delta;
        };
        for event in events {
            if let Some(source) = self.live.sources.get_mut(file_id) {
                source.health.observe(&event, std::time::SystemTime::now());
            }
            if !matches!(event, SourceEvent::Inode(_)) {
                delta.shown = true;
            }
            match event {
                SourceEvent::Tail { lines, generation, bytes, end_offset } => {
                    buffer.apply_tail_with(&mut parsed, &lines, generation, &bytes, end_offset);
                    delta.structural = true;
                }
                SourceEvent::Backfill { lines, bytes, .. } => {
                    buffer.apply_backfill_with(&mut parsed, &lines, &bytes);
                    delta.structural = true;
                }
                SourceEvent::Older { lines, bytes, reached_start, before_start_time, .. } => {
                    self.live.loading_more.remove(file_id);
                    if buffer.apply_older_with(&mut parsed, &lines, &bytes, reached_start, before_start_time)
                        && !lines.is_empty()
                    {
                        delta.structural = true;
                    }
                }
                SourceEvent::Earlier { start, lines, bytes, last_line_no, reached_start } => {
                    if reached_start || start.is_none() && lines.is_empty() {
                        self.live.loading_earlier.remove(file_id);
                    }
                    if buffer.apply_earlier(&mut parsed, start.as_ref(), &lines, &bytes, last_line_no) {
                        self.shift_generations_for(file_id);
                    }
                    // More than the Live Window now: the file keeps it.
                    if parsed.entries.len() > window {
                        buffer.hold = crate::live::HoldMode::Extended;
                    }
                    delta.structural = true;
                }
                SourceEvent::EarlierAvailable(file) => {
                    self.live.loading_earlier.remove(file_id);
                    // "Open in full…" only when nothing more fits in memory.
                    let full = parsed.entries.len() >= self.live.full_read_limit;
                    buffer.set_earlier_available(&mut parsed, file.as_ref(), full);
                    match file {
                        Some(file) => self.live.earlier.insert(file_id.to_string(), file),
                        None => self.live.earlier.remove(file_id),
                    };
                    delta.structural = true;
                }
                SourceEvent::StartTimeOverflow { lines, since } => {
                    buffer.set_start_overflow(&mut parsed, lines, &since);
                    delta.structural = true;
                }
                SourceEvent::Progress(progress) => {
                    match progress {
                        Some(p) => self.live.progress.insert(file_id.to_string(), p),
                        None => self.live.progress.remove(file_id),
                    };
                }
                SourceEvent::OlderUnavailable => {
                    self.live.loading_more.remove(file_id);
                    buffer.mark_changed(&mut parsed);
                    delta.structural = true;
                }
                SourceEvent::Inode(inode) => buffer.set_inode(inode),
                SourceEvent::Append { lines, bytes, streams } => {
                    if buffer.replaces_last_line() && !lines.is_empty() {
                        delta.replaced_last = true;
                    }
                    let range = buffer.apply_append_streams(&mut parsed, &lines, &bytes, streams.as_ref());
                    delta.appended = Some(match delta.appended.take() {
                        Some(old) => old.start..range.end,
                        None => range,
                    });
                }
                SourceEvent::TotalLines(total) => match total {
                    Some(total) => {
                        if let Some(d) = buffer.apply_total(&mut parsed, Some(total)) {
                            delta.structural = true;
                            delta.renumbered = Some(d);
                            delta.generation = buffer.generation;
                        }
                    }
                    None => {
                        if let Some(source) = self.live.sources.get_mut(file_id) {
                            source.numbers_unavailable = true;
                        }
                    }
                },
                SourceEvent::Break { kind } => {
                    buffer.apply_break(kind, &time);
                    if kind.starts_generation() {
                        self.begin_generation_for(file_id);
                    }
                    if matches!(kind, BreakKind::Disconnected | BreakKind::Reconnected) {
                        // Only the label changes; rows do not.
                    }
                }
                SourceEvent::CommandChanged { from, to } => {
                    buffer.apply_command_changed(&from, &to, &time);
                    self.begin_generation_for(file_id);
                    if let Some(old) = self.live.release_after_replace.remove(file_id) {
                        self.release_unused_server(&old);
                    }
                    if let Some(source) = self.live.sources.get_mut(file_id) {
                        source.name = to;
                    }
                }
                SourceEvent::Problem(message) => self.notice = Some(message),
                SourceEvent::Run { .. } => {}
                SourceEvent::Status(status) => {
                    if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
                        eprintln!("live status: {file_id}: {}", status.label());
                    }
                    if let Some(source) = self.live.sources.get_mut(file_id) {
                        source.status = status;
                    }
                }
            }
        }
        {
            let limit = buffer.limit(window, self.live.full_read_limit);
            // Not structural: the oldest rows of the file leave the view in
            // place (`trim_open_view`).
            if let Some(trimmed) = buffer.trim_to_window(&mut parsed, limit) {
                delta.trimmed = Some(trimmed);
            }
        }
        parsed.live = Some(buffer);
        self.parsed_files.insert(file_id.to_string(), parsed);
        if let Some(trimmed) = delta.trimmed.clone() {
            self.drop_markers_for(file_id, &trimmed);
        }
        delta
    }

    /// Events of a local file read through its Line Index. Its source only
    /// tells that the file grew (what grew is read from the file), was
    /// rotated, cut or deleted, and how it is.
    fn apply_indexed_events(&mut self, file_id: &str, mut parsed: crate::space_view::ParsedFile, events: Vec<SourceEvent>) -> FileDelta {
        let mut delta = FileDelta::default();
        let mut grew = false;
        let mut again = None;
        for event in events {
            if let Some(source) = self.live.sources.get_mut(file_id) {
                source.health.observe(&event, std::time::SystemTime::now());
            }
            match event {
                SourceEvent::Append { .. } => grew = true,
                // Another file at the path: its lines follow the old ones',
                // which stay readable.
                SourceEvent::Break { kind: kind @ (BreakKind::Rotated | BreakKind::Replaced) } => {
                    if !self.begin_local_generation(file_id, &mut parsed, kind, &mut delta) {
                        again = Some(kind);
                    }
                }
                // The lines stay, read through the handle still open.
                SourceEvent::Break { kind: BreakKind::Deleted } => {
                    self.live.local_deleted.insert(file_id.to_string());
                }
                // The text of the lines is gone from disk: read the file again.
                SourceEvent::Break { kind } if kind.starts_generation() => again = Some(kind),
                SourceEvent::Problem(message) => self.notice = Some(message),
                SourceEvent::Status(status) => {
                    let back = status == SourceStatus::Live && self.live.local_deleted.remove(file_id);
                    if let Some(source) = self.live.sources.get_mut(file_id) {
                        source.status = status;
                    }
                    if back && !self.begin_local_generation(file_id, &mut parsed, BreakKind::Replaced, &mut delta) {
                        again = Some(BreakKind::Replaced);
                    }
                }
                _ => {}
            }
        }
        self.index_written_since(file_id, parsed, grew, again, delta)
    }

    /// A local file of a Live Log Space has another file at its path: the
    /// new file's lines are a new Generation of it, below a Break. False
    /// when the new file cannot be opened, or the file is not read live:
    /// it is then read again from its start, its old Generations closed
    /// (when the app holds as many files open as it may, that is how
    /// handles come free).
    fn begin_local_generation(
        &mut self,
        file_id: &str,
        parsed: &mut crate::space_view::ParsedFile,
        kind: BreakKind,
        delta: &mut FileDelta,
    ) -> bool {
        let Some(file) = self.active_space().and_then(|s| s.file(file_id).cloned()) else { return false };
        let (Some(indexed), Some(path)) = (parsed.indexed.clone(), file.path.as_deref()) else { return false };
        let source = match crate::index::DiskSource::stay_open(path, self.live.handle_budget.clone()) {
            Ok(source) => source,
            Err(_) => {
                if self.live.handle_budget.open() >= self.live.handle_budget.limit() {
                    self.notice = Some(format!("Older lines of {} were closed: too many files are open", file.display_name));
                }
                return false;
            }
        };
        let mark = std::sync::Arc::new(crate::live::BreakMark {
            kind,
            label: format!("{} {} {}", file.display_name, kind.label(), now_label()),
        });
        let generation = file.generation + 1;
        match indexed.begin_generation(std::sync::Arc::new(source), generation, mark.clone()) {
            Some(Ok(_)) => {}
            _ => return false,
        }
        self.begin_generation_for(file_id);
        if let Some(start) = indexed.generations().last() {
            std::sync::Arc::make_mut(&mut parsed.raw_breaks).insert(start.pos, mark);
        }
        delta.shown = true;
        delta.structural = true;
        true
    }

    /// Index what was written to an indexed file since (`grew`), or read it
    /// again from its start in the background (`again`, with why: another
    /// file, or one cut or rewritten). A local file of a Live Log Space read
    /// again is a new Generation, with the Break above its first line.
    fn index_written_since(
        &mut self,
        file_id: &str,
        parsed: crate::space_view::ParsedFile,
        grew: bool,
        mut again: Option<BreakKind>,
        mut delta: FileDelta,
    ) -> FileDelta {
        if grew && again.is_none() {
            use crate::index::file::Extended;
            let read = parsed.indexed.as_ref().map(|f| f.extend());
            match read {
                Some(Ok(Extended::Appended { range, replaced_last })) => {
                    delta.shown = true;
                    delta.replaced_last = replaced_last;
                    delta.appended = Some(range);
                }
                Some(Ok(Extended::Same)) => {}
                Some(Ok(Extended::Shrunk)) => again = Some(BreakKind::Truncated),
                Some(Ok(Extended::Rewritten)) | Some(Err(_)) | None => again = Some(BreakKind::Rewritten),
            }
        }
        if let Some(kind) = again {
            let live = self.active_space().is_some_and(|s| s.kind == LogSpaceKind::Live);
            let name = self.active_space().and_then(|s| s.file(file_id)).map(|f| f.display_name.clone());
            if let (true, Some(name)) = (live, name) {
                let label = format!("{name} {} {}", kind.label(), now_label());
                self.live.first_marks.insert(file_id.to_string(), std::sync::Arc::new(crate::live::BreakMark { kind, label }));
                self.begin_generation_for(file_id);
            }
            // Read the file at the path from its start, in the background;
            // its source goes on from where it is. The open tab's Filter
            // and Focus Tabs are kept first: the view is built again from them.
            self.persist_active_session();
            delta.shown = true;
            delta.structural = true;
            let file = self.active_space().and_then(|s| s.file(file_id).cloned());
            if let Some(file) = file {
                self.queue_file_load(&file);
            }
            return delta;
        }
        self.parsed_files.insert(file_id.to_string(), parsed);
        delta
    }

    /// A Static Log Space has no live source: the files its open tab reads
    /// are looked at every `LOCAL_POLL_INTERVAL`, read on when written on
    /// and read again when another file, or a rewritten one, is at the path.
    /// The view stays where it is (a Static space has no Follow).
    fn watch_static_files(&mut self, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        if self.live.static_watched.is_some_and(|at| now.duration_since(at) < crate::live::source::LOCAL_POLL_INTERVAL) {
            return;
        }
        self.live.static_watched = Some(now);
        let Some(space) = self.active_space().filter(|s| s.kind == LogSpaceKind::Static).cloned() else { return };
        let reads = self.active_file_ids();
        self.live.static_gone.retain(|id| reads.contains(id));
        let mut deltas = Vec::new();
        for file in space.files.iter().filter(|f| reads.contains(&f.file_id) && f.entry.is_none() && f.copy.is_none()) {
            let Some(path) = &file.path else { continue };
            // A file still being read has no index to compare yet.
            let Some(indexed) = self.parsed_files.get(&file.file_id).and_then(|p| p.indexed.clone()) else { continue };
            let (grew, again) = match indexed.changed_on_disk(std::path::Path::new(path)) {
                Ok(crate::index::file::OnDisk::Appended) => (true, None),
                // Another file at the path, or one back where it was deleted.
                Ok(crate::index::file::OnDisk::Replaced) => (false, Some(BreakKind::Replaced)),
                Ok(crate::index::file::OnDisk::Gone) => {
                    if self.live.static_gone.insert(file.file_id.clone()) {
                        cx.notify();
                    }
                    continue;
                }
                // Unchanged or unreadable for now: looked at again later.
                _ => continue,
            };
            self.live.static_gone.remove(&file.file_id);
            let Some(parsed) = self.parsed_files.remove(&file.file_id) else { continue };
            let delta = self.index_written_since(&file.file_id, parsed, grew, again, FileDelta::default());
            deltas.push((file.file_id.clone(), delta));
        }
        if !deltas.is_empty() {
            self.apply_deltas(deltas, cx);
        }
    }

    /// The file has a new Generation in every Log Space that holds it.
    pub(crate) fn begin_generation_for(&mut self, file_id: &str) {
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(file) = space.file_mut(file_id) {
                    file.begin_generation();
                }
            }
        }
    }

    /// The file gained an earlier Generation below all it held: its own count
    /// moves up with the buffer's, and Markers keep their distance.
    pub(crate) fn shift_generations_for(&mut self, file_id: &str) {
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(file) = space.file_mut(file_id) {
                    file.begin_earlier_generation();
                }
            }
        }
    }

    /// Lines left the Live Window: their Markers go with them.
    pub(crate) fn drop_markers_for(&mut self, file_id: &str, trimmed: &Trimmed) {
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(file) = space.file_mut(file_id) {
                    file.drop_markers_before(trimmed.generation, trimmed.first_kept);
                }
            }
        }
    }

    /// Show a batch in the open view.
    pub(crate) fn apply_deltas(&mut self, deltas: Vec<(String, FileDelta)>, cx: &mut Context<Self>) {
        // Numbers that moved: stored Markers and current lines follow them.
        for (file_id, delta) in &deltas {
            if let Some(by) = delta.renumbered {
                self.renumber_file(file_id, delta.generation, by);
            }
        }

        self.queue_raw_updates(&deltas);

        let Some(target) = self.active_target() else {
            return;
        };
        let space_files: HashSet<String> = match (&target, self.active_space()) {
            (TabTarget::LogSpace { .. }, Some(space)) => space.files.iter().map(|f| f.file_id.clone()).collect(),
            (TabTarget::FileView { file_id, .. }, _) => HashSet::from([file_id.clone()]),
            _ => return,
        };
        let relevant: Vec<&(String, FileDelta)> = deltas
            .iter()
            .filter(|(id, d)| space_files.contains(id) && (d.appended.is_some() || d.structural || d.trimmed.is_some()))
            .collect();
        if relevant.is_empty() {
            // Background files: the sidebar's status and counts are what a
            // new frame would show.
            if deltas.iter().any(|(_, d)| d.shown || d.appended.is_some() || d.structural || d.trimmed.is_some()) {
                cx.notify();
            }
            return;
        }

        let before = self.view.len();
        let structural = relevant.iter().any(|(_, d)| d.structural);
        let trimmed_any = relevant.iter().any(|(_, d)| d.trimmed.is_some());
        let in_place = !structural && self.apply_trims_and_appends(&relevant, cx);
        if !in_place {
            self.rebuild_open_view_keeping_position(cx);

            if let Some((file_id, d)) = relevant.iter().find(|(_, d)| d.trimmed.is_some()).map(|(f, d)| (f.as_str(), d)) {
                if let Some(trimmed) = &d.trimmed {
                    self.move_current_lines_off_dropped(file_id, trimmed);
                }
            }
        }
        // Lines trimmed off a file the open view does not show were never
        // taken out of a view: they go now.
        for parsed in self.parsed_files.values_mut() {
            if !parsed.trimmed_off.is_empty() {
                parsed.trimmed_off = Vec::new();
            }
        }
        // Rows are copied and freed in large blocks all the time: give the
        // allocator's cache back now and then, or it grows to gigabytes.
        self.live.batches_since_release += 1;
        if structural || trimmed_any || self.live.batches_since_release >= 50 {
            self.live.batches_since_release = 0;
            release_freed_memory();
        }
        let grew = self.view.len().saturating_sub(before);
        if self.live.follow {
            self.live.unseen = 0;
            self.scroll_to_newest();
        } else if !relevant.iter().all(|(id, _)| self.live.copy_reads.contains_key(id)) {
            // A Remote Copy being read is not news: it is all there to read.
            self.live.unseen += grew;
        }
        cx.notify();
    }

    /// Show a batch without building the view again: the oldest rows of the
    /// files that were trimmed leave in place, then the new lines are added.
    /// Returns false when the view has to be built again.
    fn apply_trims_and_appends(&mut self, relevant: &[&(String, FileDelta)], cx: &mut Context<Self>) -> bool {
        let mut adjusted: Vec<(String, FileDelta)> = Vec::new();
        for (file_id, delta) in relevant.iter().copied() {
            let mut delta = delta.clone();
            if let Some(trimmed) = delta.trimmed.clone() {
                if !self.trim_open_view(file_id, &trimmed) {
                    return false;
                }
                // The new lines are where they were, less the lines that left.
                delta.appended = delta
                    .appended
                    .take()
                    .filter(|r| r.end > trimmed.removed)
                    .map(|r| r.start.saturating_sub(trimmed.removed)..r.end - trimmed.removed);
            }
            adjusted.push((file_id.clone(), delta));
        }
        let appended: Vec<&(String, FileDelta)> = adjusted.iter().filter(|(_, d)| d.appended.is_some()).collect();
        appended.is_empty() || self.append_to_open_view(&appended, cx)
    }

    /// Tell the Raw View's editors what changed: lines at the end are added to
    /// the text, anything else builds the editor again.
    fn queue_raw_updates(&mut self, deltas: &[(String, FileDelta)]) {
        for (file_id, delta) in deltas {
            if !self.raw.editors.contains_key(file_id) {
                continue;
            }
            if delta.structural || delta.replaced_last {
                self.raw.stale.insert(file_id.clone());
                continue;
            }
            // The oldest lines leave the editor's text; lines of this batch
            // that were trimmed at once were never in it.
            let removed = delta.trimmed.as_ref().map_or(0, |t| t.removed);
            if let Some(trimmed) = &delta.trimmed {
                if delta.appended.as_ref().is_some_and(|r| r.start < trimmed.removed) {
                    self.raw.stale.insert(file_id.clone());
                    continue;
                }
                *self.raw.pending_trims.entry(file_id.clone()).or_default() += trimmed.text_bytes;
            }
            let Some(range) = delta.appended.clone() else { continue };
            let Some(parsed) = self.parsed_files.get(file_id) else { continue };
            let held = range.start - removed..range.end - removed;
            let Some(lines) = parsed.lines.get(held) else {
                self.raw.stale.insert(file_id.clone());
                continue;
            };
            let mut text = lines.join("\n");
            if range.start > 0 {
                text.insert(0, '\n');
            }
            self.raw.pending_appends.entry(file_id.clone()).or_default().push_str(&text);
        }
    }

    /// The fast case: lines appended in time order go straight to the end of
    /// the rows. Returns false when the view has to be built again.
    fn append_to_open_view(&mut self, relevant: &[&(String, FileDelta)], cx: &mut Context<Self>) -> bool {
        let Some(target) = self.active_target() else { return false };
        let is_space = matches!(target, TabTarget::LogSpace { .. });
        let Some(space) = self.active_space().cloned() else { return false };
        let order: HashMap<&str, usize> = space
            .ordered_files()
            .iter()
            .enumerate()
            .map(|(i, f)| (f.file_id.as_str(), i))
            .collect();
        // The files whose lines have no time: their rows come after every
        // row with one, each file's rows together in file order. A File View
        // of a file read so far without a time is one of them.
        let untimed: HashSet<String> = if is_space {
            self.no_timestamp_files.clone()
        } else if self.filter_index.timed_lines() == 0 {
            relevant.iter().map(|(id, _)| id.clone()).collect()
        } else {
            HashSet::new()
        };

        // The Detected Fields of the new lines, read in their file.
        let config = self.line_config();
        if !config.is_empty() {
            for (file_id, delta) in relevant {
                let (Some(range), Some(parsed)) = (delta.appended.clone(), self.parsed_files.get_mut(file_id.as_str()))
                else {
                    continue;
                };
                for entry in parsed.entries.get_mut(range).into_iter().flatten() {
                    entry.extra = config.extra_spans(entry);
                }
            }
        }

        // New rows with the time they sort by and where they came from.
        struct NewRow {
            /// 0 for a row sorted by its time, 1 for a row of a file with none.
            class: u8,
            ts: f64,
            /// The file's place in the Log Space's file order.
            file: usize,
            /// The row, packed: its file in the view and its position there.
            packed: u64,
            pos: usize,
        }
        let mut rows: Vec<NewRow> = Vec::new();
        for (file_id, delta) in relevant {
            let Some(range) = delta.appended.clone() else { continue };
            let Some(parsed) = self.parsed_files.get(file_id) else { return false };
            if space.file(file_id).is_none() {
                return false;
            }
            let Some(view_file) = self.view.file_index(file_id) else { return false };
            let file_ix = order.get(file_id.as_str()).copied().unwrap_or(0);
            let file_untimed = untimed.contains(file_id);
            // A file whose time goes backwards is not a sorted lane.
            let mut lane_ts = f64::NEG_INFINITY;
            if let (Some(before), false) = (range.start.checked_sub(1), file_untimed) {
                let Some(prev) = parsed_effective_ts(parsed, before) else { return false };
                lane_ts = prev;
            }
            for pos in range {
                if pos >= parsed.len() {
                    return false;
                }
                let ts = if file_untimed {
                    // A line with a time in a file that had none: the file
                    // sorts another way now.
                    if own_timestamp(parsed, pos).is_some() {
                        return false;
                    }
                    0.0
                } else {
                    let Some(ts) = parsed_effective_ts(parsed, pos) else { return false };
                    if ts < lane_ts {
                        return false;
                    }
                    lane_ts = ts;
                    ts
                };
                let packed = crate::index::view_rows::pack(view_file, pos);
                rows.push(NewRow { class: u8::from(file_untimed), ts, file: file_ix, packed, pos });
            }
        }
        if rows.is_empty() {
            return true;
        }
        // Lines of one file stay in order; files interleave by time; files
        // with no time come last.
        rows.sort_by(|a, b| {
            a.class
                .cmp(&b.class)
                .then(a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.file.cmp(&b.file))
                .then(a.pos.cmp(&b.pos))
        });
        // The view is sorted by (class, time, file): the new rows go where
        // the first of them belongs, and only the rows from there on move. A
        // line that replaced the last one already shown, or a file whose time
        // goes backwards, needs the view built again.
        if relevant.iter().any(|(_, d)| d.replaced_last) {
            return false;
        }
        let first_key = (rows[0].class, rows[0].ts, rows[0].file);
        let ts_index = &self.filter_index;
        let view_rows = self.rows();
        let key_of = |r: usize| {
            let file = view_rows.file_id(r).and_then(|id| order.get(id).copied()).unwrap_or(0);
            match ts_index.effective_timestamp(r) {
                Some(ts) => (0u8, ts, file),
                None => (1u8, 0.0, file),
            }
        };
        let from = partition_rows(view_rows.len(), |r| key_of(r) <= first_key);
        let tail_keys: Vec<(u8, f64, usize)> = (from..view_rows.len()).map(key_of).collect();
        let selected = self.selected_row_pos.filter(|pos| *pos >= from).and_then(|pos| self.line_ref_of_entry(pos));
        let anchor = self.top_row_anchor().filter(|a| a.row >= from);

        // Merge the rows below `from` with the new ones, by the same key a
        // full merge uses. Continuation Lines share their Parent's key and
        // follow it in line order, so they stay under it.
        // Rows that all sort after the last one only add to the view.
        let at_end = from == self.view.len();
        let tail: Vec<u64> = self.view.rows.to_mut().split_off(from);
        {
            let bound = self.view.bind(&self.parsed_files);
            self.filter_index.truncate(&bound, &bound.picked(&tail));
        }
        let mut merged: Vec<u64> = Vec::with_capacity(tail.len() + rows.len());
        let mut fresh: Vec<u64> = Vec::with_capacity(rows.len());
        let mut old = tail.into_iter().zip(tail_keys).peekable();
        let mut new = rows.into_iter().peekable();
        loop {
            let take_old = match (old.peek(), new.peek()) {
                (Some((_, key)), Some(n)) => *key <= (n.class, n.ts, n.file),
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            if take_old {
                merged.push(old.next().map(|(e, _)| e).unwrap());
            } else {
                let row = new.next().unwrap();
                fresh.push(row.packed);
                merged.push(row.packed);
            }
        }
        {
            let bound = self.view.bind(&self.parsed_files);
            let new_rows = bound.picked(&fresh);
            for r in 0..fresh.len() {
                if let Some(entry) = new_rows.get(r) {
                    let source = if is_space { new_rows.source(r) } else { None };
                    self.column_layout.max_chars.record_row(&entry, source);
                }
            }
        }
        let grew_from = self.view.len();
        if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
            eprintln!("live insert: {} rows from row {grew_from} of {}", merged.len(), grew_from + merged.len());
        }
        self.view.rows.to_mut().extend(merged);
        {
            let added_rows: Vec<u64> = self.view.rows.iter().skip(grew_from).collect();
            let bound = self.view.bind(&self.parsed_files);
            let added = bound.picked(&added_rows);
            self.filter_index
                .append_marking_untimed(&added, |r| added.file_id(r).is_some_and(|id| untimed.contains(id)));
        }
        if at_end {
            self.apply_rows_appended(grew_from, cx);
        } else {
            self.apply_filters_and_refresh(cx);
        }

        if let Some(line) = selected {
            self.selected_row_pos = self.entry_of_line_ref(&line);
        }
        self.restore_top_anchor(anchor);
        true
    }

    /// Build the rows again from the cache, keeping the reading position:
    /// the row at the top of the view stays at the top.
    pub fn rebuild_open_view_keeping_position(&mut self, cx: &mut Context<Self>) {
        self.rebuild_open_view_without(&HashSet::new(), cx);
    }

    /// The first line after row `row` that is not of a file in `gone`, or
    /// the last one before it when none follows.
    fn next_surviving_line(&self, row: usize, gone: &HashSet<String>) -> Option<LineRef> {
        let found = super::live_more::next_surviving_row(
            self.view.len(),
            |ix| self.view.get(ix).and_then(|(file, _)| self.view.files.get(file)).map(|f| &*f.file_id),
            row,
            gone,
        );
        found.and_then(|ix| self.line_ref_of_entry(ix))
    }

    /// `rebuild_open_view_keeping_position` for rows of files that were
    /// cleared: the top row, the selected line and the current line of every
    /// Focus Tab that were in them move to the line right after them.
    pub(crate) fn rebuild_open_view_without(&mut self, gone: &HashSet<String>, cx: &mut Context<Self>) {
        self.refreshes.rebuilds += 1;
        let mut anchor = self.top_row_anchor();
        let mut selected = self
            .selected_row_pos
            .and_then(|pos| self.line_ref_of_entry(pos));
        let mut focus_current: Vec<Option<LineRef>> =
            self.focus.tabs.iter().map(|t| t.current.clone()).collect();
        if !gone.is_empty() {
            let in_gone = |line: &LineRef| gone.contains(&*line.file_id);
            if let Some(a) = anchor.as_mut() {
                if in_gone(&a.line) {
                    match self.next_surviving_line(a.row, gone) {
                        Some(line) => {
                            a.line = line;
                            a.within = Pixels::ZERO;
                        }
                        None => anchor = None,
                    }
                }
            }
            if let Some(line) = selected.as_ref().filter(|l| in_gone(l)) {
                let row = self.entry_of_line_ref(line);
                selected = row.and_then(|row| self.next_surviving_line(row, gone));
            }
            for current in focus_current.iter_mut() {
                if let Some(line) = current.as_ref().filter(|l| in_gone(l)) {
                    let row = self.entry_of_line_ref(line);
                    *current = row.and_then(|row| self.next_surviving_line(row, gone));
                }
            }
        }
        let Some(target) = self.active_target() else { return };
        // The old rows go first: both lists at once would double the memory
        // for a moment.
        self.view = Default::default();
        self.filter_index = Default::default();
        let Some(open) = self.build_view_for(&target) else { return };
        self.install_live_rows(open);
        self.start_queued_loads(cx);
        self.apply_filters_and_refresh(cx);
        if let Some(line) = selected {
            self.selected_row_pos = self.entry_of_line_ref(&line);
        }
        if !gone.is_empty() {
            for (tab, current) in self.focus.tabs.iter_mut().zip(focus_current) {
                tab.current = current;
            }
        }
        self.restore_top_anchor(anchor);
    }

    /// The row at the top of the Parsed View, as a line that survives rows
    /// being inserted above it, and how far the view is scrolled into it.
    pub(crate) fn top_row_anchor(&self) -> Option<TopAnchor> {
        if self.view_mode != ViewMode::Parsed {
            return None;
        }
        let state = self.scroll_handle.0.borrow();
        let base = &state.base_handle;
        let (row, scrolled) = base.logical_scroll_top();
        let row_height = base.bounds_for_item(row)?.size.height;
        drop(state);
        let line = self.line_ref_of_entry(row)?;
        Some(TopAnchor { line, row, within: (-scrolled).max(Pixels::ZERO), row_height })
    }

    /// Scroll so the anchor's line is at the same pixel offset from the top as
    /// before, wherever the rows around it moved. Does nothing while Follow
    /// is on (the view goes to the newest line) or when the line is gone.
    pub(crate) fn restore_top_anchor(&mut self, anchor: Option<TopAnchor>) {
        let Some(anchor) = anchor else { return };
        if self.live.follow {
            return;
        }
        let Some(ix) = self.entry_of_line_ref(&anchor.line) else { return };
        let top = anchor.row_height * ix as f32 + anchor.within;
        let state = self.scroll_handle.0.borrow();
        let mut offset = state.base_handle.offset();
        offset.y = -top;
        state.base_handle.set_offset(point(offset.x, offset.y));
    }

    /// Stored Markers and the current lines of Focus Tabs of a file whose
    /// numbers moved by `by`.
    fn renumber_file(&mut self, file_id: &str, generation: u32, by: isize) {
        let shift = |n: usize| (n as isize + by).max(0) as usize;
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(file) = space.file_mut(file_id) {
                    let ago = file.generations_ago(generation);
                    for m in file.markers.iter_mut().filter(|m| m.generations_ago == ago) {
                        m.line = shift(m.line);
                    }
                }
                for tabs in std::iter::once(&mut space.focus).chain(space.file_focus.values_mut()) {
                    for tab in tabs.tabs.iter_mut() {
                        if let Some(line) = tab.current.as_mut() {
                            if &*line.file_id == file_id && line.generation == generation {
                                line.line = shift(line.line);
                            }
                        }
                    }
                }
            }
        }
        for tab in self.focus.tabs.iter_mut() {
            if let Some(line) = tab.current.as_mut() {
                if &*line.file_id == file_id && line.generation == generation {
                    line.line = shift(line.line);
                }
            }
        }
        if let Some((f, line)) = self.raw.selected.as_mut() {
            if f == file_id {
                *line = shift(*line + 1).saturating_sub(1);
            }
        }
    }

    /// The current line of a Focus Tab that left the Live Window becomes the
    /// oldest line still kept.
    pub(crate) fn move_current_lines_off_dropped(&mut self, file_id: &str, trimmed: &Trimmed) {
        let oldest = self.line_ref_of_entry(0);
        let Some(oldest) = oldest else { return };
        for tab in self.focus.tabs.iter_mut() {
            if let Some(line) = tab.current.as_ref() {
                let dropped = &*line.file_id == file_id
                    && (line.generation < trimmed.generation
                        || (line.generation == trimmed.generation && line.line < trimmed.first_kept));
                if dropped {
                    tab.current = Some(oldest.clone());
                }
            }
        }
    }

    // ── Status ──────────────────────────────────────────────────────────────

    /// The files of the open Log Space and what each source says.
    pub fn live_file_statuses(&self) -> Vec<(String, SourceStatus)> {
        let Some(space) = self.active_space() else { return Vec::new() };
        space
            .ordered_files()
            .into_iter()
            .filter_map(|f| {
                self.live
                    .sources
                    .get(&f.file_id)
                    .map(|s| (f.display_name.clone(), s.status.clone()))
            })
            .collect()
    }

    /// The worst status of the open Log Space's files with how many files
    /// have it: `("Reconnecting", 1, 3)` is "Reconnecting (1 of 3)".
    pub fn worst_live_status(&self) -> Option<(SourceStatus, usize, usize)> {
        let statuses = self.live_file_statuses();
        // A Gone file is not an error: it only counts when every file is Gone.
        let counted: Vec<&SourceStatus> =
            statuses.iter().map(|(_, s)| s).filter(|s| **s != SourceStatus::Gone).collect();
        if counted.is_empty() {
            let any = statuses.first()?.1.clone();
            return Some((any, statuses.len(), statuses.len()));
        }
        let worst = (*counted.iter().max_by_key(|s| s.severity())?).clone();
        // An Exited member counts whatever its exit code.
        let count = counted.iter().filter(|s| s.bar_label() == worst.bar_label()).count();
        Some((worst, count, statuses.len()))
    }
}

/// The time row `pos` of a file sorts by: its own, or its Parent Line's.
/// `None` when it has none (the rows then need the view built again).
/// The timestamp line `pos` of `parsed` has of its own, when it is usable.
fn own_timestamp(parsed: &crate::space_view::ParsedFile, pos: usize) -> Option<f64> {
    match &parsed.indexed {
        Some(file) => file
            .meta(pos)
            .filter(|m| m.flags & crate::index::meta::flags::OWN_TS != 0)
            .map(|m| crate::index::meta::seconds(m.ts_us)),
        None => parsed
            .entries
            .get(pos)
            .filter(|e| e.has_time && e.timestamp.is_finite() && e.timestamp > 0.0)
            .map(|e| e.timestamp),
    }
}

/// `effective_ts_at` for a file held or read through its Line Index.
fn parsed_effective_ts(parsed: &crate::space_view::ParsedFile, pos: usize) -> Option<f64> {
    match &parsed.indexed {
        Some(file) => {
            let meta = file.meta(pos)?;
            if meta.flags & crate::index::meta::flags::OWN_TS != 0 || meta.parent_dist > 0 {
                (meta.ts_us != crate::index::meta::NO_TIME).then(|| crate::index::meta::seconds(meta.ts_us))
            } else {
                None
            }
        }
        None => effective_ts_at(&parsed.entries, pos),
    }
}

fn effective_ts_at(entries: &[LogEntry], pos: usize) -> Option<f64> {
    let entry = entries.get(pos)?;
    if entry.has_time && entry.timestamp.is_finite() && entry.timestamp > 0.0 {
        return Some(entry.timestamp);
    }
    let parent = entry.parent?;
    let back = entry.index.checked_sub(parent)?;
    let parent_pos = pos.checked_sub(back)?;
    let parent_entry = entries.get(parent_pos)?;
    if parent_entry.index == parent && parent_entry.has_time {
        Some(parent_entry.timestamp)
    } else {
        None
    }
}

/// The number of leading rows of `0..len` for which `before` holds, given that
/// it holds for a prefix.
fn partition_rows(len: usize, before: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, len);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if before(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Return the memory the allocator keeps after large blocks were freed.
#[cfg(target_os = "macos")]
pub(crate) fn release_freed_memory() {
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }
    // SAFETY: a null zone means every zone; a goal of 0 means as much as can be.
    let freed = unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
    if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
        eprintln!("released {} MB", freed >> 20);
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn release_freed_memory() {}
