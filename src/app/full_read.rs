//! Logic: Open in full. A Remote File and the Rotated Files chosen are read
//! whole from the Server into a Remote Copy in an Imported Log Space. Lines
//! show as they come; the copy is on disk, so it opens again without a
//! connection.

use super::AppView;
use super::live::{FileDelta, now_label};
use crate::live::BreakKind;
use crate::remote::copy::{self, StopReason};
use crate::remote::full_read::{GenerationPlan, ReadControl, ReadEvent, ReadPlan, run};
use crate::workspace::log_space::{LogFile, LogSpaceKind};
use crate::space_view::ParsedFile;
use async_channel::Receiver;
use gpui::Context;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where the Remote Copy goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// A new Imported Log Space with this name.
    NewSpace(String),
    /// An Imported Log Space that exists.
    Existing(String),
}

/// What the Open in full dialog asks for, for one Remote File.
#[derive(Debug, Clone)]
pub struct FullReadRequest {
    pub server_id: String,
    pub server_name: String,
    pub path: String,
    /// Oldest Generation first.
    pub generations: Vec<GenerationPlan>,
    pub start: Option<f64>,
    pub to: Option<f64>,
    pub start_text: Option<String>,
    pub to_text: Option<String>,
    pub destination: Destination,
    /// The Full Read Limit of this read.
    pub limit_lines: usize,
    /// Read again: the file whose copy this read replaces when it ends.
    pub replace: Option<String>,
}

/// A Remote Copy being read.
pub struct CopyRead {
    pub rx: Receiver<ReadEvent>,
    pub ctl: Arc<ReadControl>,
    /// The Imported Log Space that holds the file.
    pub space_id: String,
    pub progress: Option<(String, u64, u64)>,
    /// The folder this read writes.
    pub dir: PathBuf,
    /// Read again: the folder of the copy this read replaces.
    pub replaces: Option<PathBuf>,
    /// The first line numbers could not be counted.
    pub numbers_unavailable: bool,
    /// Where reading the Generation of this file started in.
    finished: bool,
}

impl CopyRead {
    /// "Reading web1:/app/app.log.2.gz 45% (17 MB / 38 MB)".
    pub fn progress_text(&self, name: &str) -> String {
        let size = crate::live::buffer::size_text;
        match &self.progress {
            Some((what, done, total)) if *total > 0 => {
                format!("Reading {name} — {what} {}% ({} / {})", done.min(total) * 100 / total, size(*done), size(*total))
            }
            _ => format!("Reading {name}"),
        }
    }
}

impl AppView {
    /// The folder of the Remote Copies, next to the other app data.
    pub(crate) fn copies_root(&self) -> PathBuf {
        let base = self.remote.picker_state_path.parent().map(Path::to_path_buf).unwrap_or_default();
        copy::root_in(&base)
    }

    #[doc(hidden)]
    pub fn copies_root_for_test(&self) -> PathBuf {
        self.copies_root()
    }

    #[doc(hidden)]
    pub fn copy_dirs_in_use_for_test(&self) -> std::collections::HashSet<PathBuf> {
        self.copy_dirs_in_use()
    }

    /// The Full Read Limit in lines.
    pub fn full_read_limit(&self) -> usize {
        self.live.full_read_limit
    }

    /// Start reading: make the folder and the file, show the empty copy in its
    /// Imported Log Space, and read in the background.
    pub fn start_full_read(&mut self, request: FullReadRequest, cx: &mut Context<Self>) -> Option<String> {
        let root = self.copies_root();
        let dir = match copy::create_dir(&root) {
            Ok(dir) => dir,
            Err(e) => {
                self.notice = Some(format!("Could not make room for the copy: {e}"));
                cx.notify();
                return None;
            }
        };
        self.persist_active_session();
        let (file_id, display, space_id, replaces) = if let Some(file_id) = request.replace.clone() {
            // Read again: the file and its Log Space stay, the content is
            // replaced when the new read ends.
            let found = self.active_project().and_then(|p| {
                p.log_spaces.iter().find_map(|s| {
                    s.file(&file_id).filter(|f| f.is_remote_copy()).map(|f| (s.id.clone(), f.display_name.clone(), f.copy.clone()))
                })
            });
            let Some((space_id, display, old)) = found else {
                let _ = copy::remove(&dir);
                return None;
            };
            (file_id, display, space_id, old.map(PathBuf::from))
        } else {
            let file = LogFile::remote_copy(&dir.to_string_lossy(), &request.server_name, &request.path);
            let (file_id, display) = (file.file_id.clone(), file.display_name.clone());
            let space_id = match &request.destination {
                Destination::NewSpace(name) => {
                    let project = self.active_project_mut()?;
                    match project.create_log_space(name, LogSpaceKind::Imported) {
                        Ok(space) => {
                            let _ = space.add_remote_copy(file);
                            space.id.clone()
                        }
                        Err(e) => {
                            let _ = copy::remove(&dir);
                            self.notice = Some(e.message().to_string());
                            cx.notify();
                            return None;
                        }
                    }
                }
                Destination::Existing(id) => {
                    let added = self
                        .active_project_mut()
                        .and_then(|p| p.log_space_mut(id))
                        .filter(|s| s.kind == LogSpaceKind::Imported)
                        .map(|space| space.add_remote_copy(file).is_ok())
                        .unwrap_or(false);
                    if !added {
                        let _ = copy::remove(&dir);
                        return None;
                    }
                    id.clone()
                }
            };
            (file_id, display, space_id, None)
        };
        let mut parsed = ParsedFile::empty();
        parsed.live = Some(crate::live::LiveBuffer::command(&display));
        self.parsed_files.insert(file_id.clone(), parsed);
        // Generations the copy will hold: the file moves up by one for each.
        if let Some(space) = self.active_project_mut().and_then(|p| p.log_space_mut(&space_id)) {
            if let Some(f) = space.file_mut(&file_id) {
                f.generation = 0;
            }
        }
        self.expanded_spaces.insert(space_id.clone());
        self.open_log_space_tab(&space_id, cx);
        // Opened with Follow off, at the first line read.
        self.live.follow = false;
        self.live.unseen = 0;

        let plan = ReadPlan {
            server_id: request.server_id.clone(),
            server_name: request.server_name.clone(),
            path: request.path.clone(),
            generations: request.generations,
            start: request.start,
            to: request.to,
            start_text: request.start_text,
            to_text: request.to_text,
            limit_lines: request.limit_lines,
            dir: dir.clone(),
        };
        let hub = self.remote_hub(cx);
        let (tx, rx) = async_channel::unbounded();
        let ctl = Arc::new(ReadControl::default());
        if self.live.paused_spaces.contains(&space_id) {
            ctl.set_paused(true);
        }
        let servers = self.remote.shared_servers.clone();
        let (run_hub, run_ctl) = (hub.clone(), ctl.clone());
        hub.spawn(async move { run(run_hub, servers, plan, tx, run_ctl).await });
        self.live.copy_reads.insert(
            file_id.clone(),
            CopyRead { rx, ctl, space_id, progress: None, dir, replaces, numbers_unavailable: false, finished: false },
        );
        self.schedule_session_save(cx);
        cx.notify();
        Some(file_id)
    }

    /// Apply what the reads of Remote Copies sent since the last time.
    pub(crate) fn pump_copy_reads(&mut self, cx: &mut Context<Self>) {
        if self.live.copy_reads.is_empty() {
            return;
        }
        let mut batches: Vec<(String, Vec<ReadEvent>)> = Vec::new();
        let mut budget = 200_000usize;
        for (file_id, read) in &self.live.copy_reads {
            let mut events = Vec::new();
            while budget > 0 {
                match read.rx.try_recv() {
                    Ok(event) => {
                        budget = budget.saturating_sub(match &event {
                            ReadEvent::Lines { lines, .. } => lines.len().max(1),
                            _ => 1,
                        });
                        events.push(event);
                    }
                    Err(_) => break,
                }
            }
            if !events.is_empty() {
                batches.push((file_id.clone(), events));
            }
        }
        if batches.is_empty() {
            return;
        }
        let mut deltas: Vec<(String, FileDelta)> = Vec::new();
        for (file_id, events) in batches {
            let delta = self.apply_copy_events(&file_id, events);
            deltas.push((file_id, delta));
        }
        self.apply_deltas(deltas, cx);
        self.live.copy_reads.retain(|_, read| !read.finished);
        self.schedule_session_save(cx);
    }

    fn apply_copy_events(&mut self, file_id: &str, events: Vec<ReadEvent>) -> FileDelta {
        let mut delta = FileDelta::default();
        let Some(mut parsed) = self.parsed_files.remove(file_id) else { return delta };
        let Some(mut buffer) = parsed.live.take() else {
            self.parsed_files.insert(file_id.to_string(), parsed);
            return delta;
        };
        let limit = self.live.full_read_limit;
        // Read again ended: whether its copy replaces the old one.
        let mut replaced: Option<bool> = None;
        for event in events {
            match event {
                ReadEvent::GenerationStart { index, first_line_no, .. } => {
                    let label = format!("{} rotated", buffer.name);
                    buffer.begin_copy_generation(first_line_no, index == 0, &label);
                    if index > 0 {
                        self.begin_generation_for(file_id);
                    }
                }
                ReadEvent::FirstLineNo { first_line_no, .. } => match first_line_no {
                    Some(n) => {
                        if let Some(by) = buffer.set_first_line_no(&mut parsed, n) {
                            delta.structural = true;
                            delta.renumbered = Some(by);
                            delta.generation = buffer.generation;
                        }
                    }
                    None => {
                        if let Some(read) = self.live.copy_reads.get_mut(file_id) {
                            read.numbers_unavailable = true;
                        }
                    }
                },
                ReadEvent::Lines { lines, bytes } => {
                    let range = buffer.apply_append_with(&mut parsed, &lines, &bytes);
                    delta.appended = Some(match delta.appended.take() {
                        Some(old) => old.start..range.end,
                        None => range,
                    });
                }
                ReadEvent::Progress { what, done, total } => {
                    if let Some(read) = self.live.copy_reads.get_mut(file_id) {
                        read.progress = Some((what, done, total));
                    }
                }
                ReadEvent::Stopped(reason) => {
                    let kind = match reason {
                        StopReason::Cancelled => BreakKind::ReadCancelled,
                        StopReason::FullReadLimit => BreakKind::ReadLimit,
                        StopReason::RotatedWhileReading => BreakKind::ReadRotated,
                        StopReason::OlderNotRead => BreakKind::OlderNotRead,
                    };
                    buffer.mark_read_end(&mut parsed, kind, &reason.label(limit));
                    delta.structural = true;
                }
                ReadEvent::Done(meta) => {
                    if meta.generations.first().is_some_and(|g| g.older_not_read) {
                        buffer.mark_older_not_read(&mut parsed, &format!("{}: older lines were not read", meta.name()));
                        delta.structural = true;
                    }
                    if let Some(read) = self.live.copy_reads.get_mut(file_id) {
                        read.finished = true;
                        read.progress = None;
                    }
                    replaced = Some(true);
                }
                ReadEvent::Failed { message, .. } => {
                    self.notice = Some(format!("Open in full: {message}"));
                    if let Some(read) = self.live.copy_reads.get_mut(file_id) {
                        read.finished = true;
                        read.progress = None;
                    }
                    replaced = Some(false);
                }
            }
        }
        let _ = now_label;
        let generation = buffer.generation;
        if replaced == Some(true) {
            self.restore_markers_after_read_again(file_id, &parsed, generation);
        }
        parsed.live = Some(buffer);
        self.parsed_files.insert(file_id.to_string(), parsed);
        if let Some(done) = replaced {
            self.finish_read_again(file_id, done);
            delta.structural = true;
        }
        delta
    }

    /// "Read again": read the same Generations again from the Server, with the
    /// same Starting Time and To, and replace the copy when it ends.
    pub fn read_again(&mut self, file_id: &str, cx: &mut Context<Self>) {
        if self.live.copy_reads.contains_key(file_id) {
            return;
        }
        let found = self.active_project().and_then(|p| {
            p.log_spaces.iter().find_map(|s| s.file(file_id).filter(|f| f.is_remote_copy()).map(|f| (s.id.clone(), f.clone())))
        });
        let Some((space_id, file)) = found else { return };
        let Some(dir) = file.copy.clone() else { return };
        let meta = match copy::read_meta(Path::new(&dir)) {
            Ok(meta) if !meta.server_id.is_empty() => meta,
            _ => {
                self.notice = Some(format!("{}: nothing saved says where it came from", file.display_name));
                cx.notify();
                return;
            }
        };
        if self.remote.servers.get(&meta.server_id).is_none() {
            self.notice = Some(format!("Server {} is no longer in the list", meta.server_name));
            cx.notify();
            return;
        }
        let hub = self.remote_hub(cx);
        let store = self.remote.servers.clone();
        let job_meta = meta.clone();
        let task = hub.spawn({
            let hub = hub.clone();
            async move {
                let session = hub.session(&store, &job_meta.server_id).await.map_err(|e| e.message())?;
                let sftp = session.open_sftp().await?;
                let folder = crate::remote::browse::dirname(&job_meta.path);
                let mut plans = Vec::new();
                let mut gone = Vec::new();
                for g in &job_meta.generations {
                    let path = if g.distance == 0 {
                        job_meta.path.clone()
                    } else {
                        crate::remote::browse::join(&folder, &g.source_name)
                    };
                    let Ok(attrs) = sftp.metadata(path.clone()).await else {
                        gone.push(g.source_name.clone());
                        continue;
                    };
                    let compression = crate::remote::rotated::sniff(&sftp, &path).await?;
                    plans.push(GenerationPlan {
                        distance: g.distance,
                        name: g.source_name.clone(),
                        path,
                        size: attrs.len(),
                        modified: None,
                        compression,
                        start_byte: None,
                        skip_lines: 0,
                        older_dropped: g.older_not_read && plans.is_empty(),
                    });
                }
                let newest = match plans.last() {
                    Some(last) => super::open_in_full::end_time(&sftp, &last.path, last.size, last.compression).await?,
                    None => None,
                };
                Ok::<_, String>((plans, newest, gone))
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let (plans, newest, gone) = match result {
                    Ok(Ok(found)) => found,
                    Ok(Err(message)) => {
                        this.notice = Some(format!("Read again: {message}"));
                        cx.notify();
                        return;
                    }
                    Err(e) => {
                        this.notice = Some(format!("Read again: could not reach the server ({e})"));
                        cx.notify();
                        return;
                    }
                };
                if plans.is_empty() {
                    this.notice = Some("Read again: the files are no longer on the Server".into());
                    cx.notify();
                    return;
                }
                if !gone.is_empty() {
                    this.notice = Some(format!("No longer on the Server: {}", gone.join(", ")));
                }
                let resolve = |text: &Option<String>| {
                    text.as_deref().and_then(|t| crate::remote::starting_time::resolve_start(t, newest).ok().flatten())
                };
                let request = FullReadRequest {
                    server_id: meta.server_id.clone(),
                    server_name: meta.server_name.clone(),
                    path: meta.path.clone(),
                    generations: plans,
                    start: resolve(&meta.start_text),
                    to: resolve(&meta.to_text),
                    start_text: meta.start_text.clone(),
                    to_text: meta.to_text.clone(),
                    destination: Destination::Existing(space_id),
                    limit_lines: meta.limit_lines.max(1),
                    replace: Some(file.file_id.clone()),
                };
                this.start_full_read(request, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Read again ended: the file takes the new folder and the old one goes
    /// (or, when it failed, the new one goes and the old one stays).
    fn finish_read_again(&mut self, file_id: &str, done: bool) {
        let Some(read) = self.live.copy_reads.get(file_id) else { return };
        let (new_dir, old_dir) = (read.dir.clone(), read.replaces.clone());
        let Some(old_dir) = old_dir else { return };
        if done {
            if let Some(project) = self.active_project_mut() {
                for space in &mut project.log_spaces {
                    if let Some(f) = space.file_mut(file_id) {
                        f.copy = Some(new_dir.to_string_lossy().to_string());
                    }
                }
            }
            let _ = copy::remove(&old_dir);
        } else {
            // What the old folder holds is shown again.
            let _ = copy::remove(&new_dir);
            self.parsed_files.remove(file_id);
        }
    }

    /// The Markers of the file go on lines that are the same (by Generation
    /// distance, number and content) in the new copy, and are dropped on the
    /// others.
    fn restore_markers_after_read_again(&mut self, file_id: &str, parsed: &ParsedFile, generation: u32) {
        let hash_at = |ago: u32, line: usize| -> Option<u64> {
            let wanted = generation.checked_sub(ago)?;
            let pos = parsed.entries.iter().position(|e| e.generation == wanted && e.index == line)?;
            parsed.lines.get(pos).map(|l| crate::workspace::log_space::line_hash(l))
        };
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(f) = space.file_mut(file_id) {
                    f.generation = generation;
                    f.markers.retain(|m| m.line_hash.is_none() || hash_at(m.generations_ago, m.line) == m.line_hash);
                }
            }
        }
    }

    /// Stop reading a Remote Copy; what was read stays.
    pub fn cancel_copy_read(&mut self, file_id: &str) {
        if let Some(read) = self.live.copy_reads.get(file_id) {
            read.ctl.cancel();
        }
    }

    /// Pause or resume one read (for tests).
    #[doc(hidden)]
    pub fn set_copy_reads_paused_for_test(&mut self, file_id: &str, paused: bool) {
        if let Some(read) = self.live.copy_reads.get(file_id) {
            read.ctl.set_paused(paused);
        }
    }

    /// Pause or resume the reads of the Remote Copies of a Log Space.
    pub(crate) fn set_copy_reads_paused(&mut self, space_id: &str, paused: bool) {
        for read in self.live.copy_reads.values().filter(|r| r.space_id == space_id) {
            read.ctl.set_paused(paused);
        }
    }

    /// Load a Remote Copy of an Imported Log Space from its folder.
    pub fn ensure_copy_loaded(&mut self, file: &LogFile) {
        if self.parsed_files.contains_key(&file.file_id) {
            return;
        }
        let Some(dir) = file.copy.as_deref() else { return };
        let parsed = match ParsedFile::from_remote_copy(Path::new(dir), &file.display_name) {
            Ok(parsed) => parsed,
            Err(e) => {
                let error = format!("{}: the saved copy could not be read ({e})", file.display_name);
                self.notice = Some(error.clone());
                ParsedFile::failed(error)
            }
        };
        let generation = parsed.live.as_ref().map(|b| b.generation).unwrap_or(0);
        self.parsed_files.insert(file.file_id.clone(), parsed);
        // Markers keep their distance: the newest Generation is 0 back.
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(f) = space.file_mut(&file.file_id) {
                    f.generation = generation;
                }
            }
        }
    }

    /// A Remote Copy leaves the app: its read stops and its folder goes.
    pub(crate) fn delete_copy_of(&mut self, file: &LogFile) {
        if let Some(read) = self.live.copy_reads.remove(&file.file_id) {
            read.ctl.cancel();
        }
        if let Some(dir) = &file.copy {
            let _ = copy::remove(Path::new(dir));
        }
    }

    /// The Remote Copies in `HashMap` by folder, for the sweep at start.
    pub(crate) fn copy_dirs_in_use(&self) -> std::collections::HashSet<PathBuf> {
        let mut dirs = std::collections::HashSet::new();
        for project in &self.workspace_state.projects {
            for space in &project.log_spaces {
                for file in &space.files {
                    if let Some(dir) = &file.copy {
                        dirs.insert(PathBuf::from(dir));
                    }
                }
            }
        }
        dirs
    }
}

/// Counts for a test or a tooltip: the folders of every Remote Copy by file.
pub fn copy_dirs(files: &[&LogFile]) -> HashMap<String, PathBuf> {
    files.iter().filter_map(|f| f.copy.as_ref().map(|d| (f.file_id.clone(), PathBuf::from(d)))).collect()
}
