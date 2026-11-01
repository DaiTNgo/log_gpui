//! Logic: reading local files in the background. A file the open tab needs
//! is read and parsed on a worker, with its progress shown on the canvas
//! and the tab, so choosing a tab with a large file never freezes the
//! window. When the tab's files are all read its view is built, with the
//! position, Filter and Focus Tabs it keeps. A file whose tab was left while
//! it was being read is dropped. While a file of a Live Log Space is
//! indexed, its last lines show first (L3): they are read on their own.
//! The Archive Entries of a tab are unpacked to the archive cache in the
//! background first, one pass per Archive, then read like local files.

use super::AppView;
use crate::filter_index::{FilterIndex, MergeFile};
use crate::index::file::IndexedFile;
use crate::index::indexing::{IndexOptions, Indexed, Outcome, index_into};
use crate::index::tail::{TAIL_BYTES, Tail, read_tail};
use crate::index::{ChainSource, DiskSource, HandleBudget, LineSource};
use crate::model::LogEntry;
use crate::space_view::ParsedFile;
use crate::workspace::log_space::{LogFile, StoredMarker};
use gpui::Context;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// How often the progress of a read is shown again.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);
/// How often the last lines of a file being indexed are read again.
const TAIL_EVERY: Duration = Duration::from_millis(500);
/// Most last lines shown while the files are indexed.
pub const TAIL_ROWS: usize = 500;

/// A file being read; dropping it stops the read.
pub struct FileLoad {
    pub name: String,
    /// What is read so far: bytes read and parsed, and the file's size.
    progress: Progress,
    /// The file's last lines, read before Indexing gets there: for a file
    /// of a Live Log Space.
    tail: Option<Arc<Mutex<Option<Tail>>>>,
    /// The file is read again (another Log Format or other Detected
    /// Fields) while the view shows it as read before.
    again: bool,
    cancel: Arc<AtomicBool>,
}

/// How far a read is: what Indexing has built so far, or for an Archive
/// Entry not unpacked yet, the bytes read of its Archive.
enum Progress {
    Indexed(Arc<Indexed>),
    Unpacking { read: Arc<AtomicU64>, total: u64 },
}

impl FileLoad {
    /// How far the read is, 0..=100.
    pub fn percent(&self) -> u8 {
        let (done, total) = match &self.progress {
            Progress::Indexed(indexed) => indexed.progress(),
            Progress::Unpacking { read, total } => (read.load(Ordering::Relaxed), *total),
        };
        if total == 0 {
            return 0;
        }
        (done.min(total) * 100 / total) as u8
    }

    /// What Indexing has built so far, for a file read through its Line
    /// Index.
    pub fn indexed(&self) -> Option<&Arc<Indexed>> {
        match &self.progress {
            Progress::Indexed(indexed) => Some(indexed),
            Progress::Unpacking { .. } => None,
        }
    }

    /// Whether the file is an Archive Entry still being unpacked.
    pub fn is_unpacking(&self) -> bool {
        matches!(self.progress, Progress::Unpacking { .. })
    }

    /// The file's name as its progress shows it.
    pub fn label(&self) -> String {
        if self.is_unpacking() { format!("{} (unpacking)", self.name) } else { self.name.clone() }
    }
}

/// A file to read in the background, and where its text is: its path, or
/// for an Archive Entry, its file in the archive cache.
#[derive(Debug, Clone)]
pub struct QueuedLoad {
    pub file: LogFile,
    pub read_path: String,
}

impl Drop for FileLoad {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// Read the file at `path` through its Line Index: Indexing builds it into
/// `indexed`, and the file stays open to read its lines again.
fn index_file(
    path: &str,
    options: IndexOptions,
    indexed: Arc<Indexed>,
    budget: Arc<HandleBudget>,
    cancel: &AtomicBool,
) -> Option<std::io::Result<ParsedFile>> {
    // Read as a chain: a Live Log Space's file can take on a new Generation.
    let source = match DiskSource::stay_open(path, budget) {
        Ok(source) => Arc::new(ChainSource::new(Arc::new(source) as Arc<dyn LineSource>)),
        Err(e) => return Some(Err(e)),
    };
    match index_into(&*source, &options, &indexed, cancel) {
        Outcome::Done => Some(Ok(ParsedFile::from_indexed(IndexedFile::with_chain(source, indexed, options)))),
        Outcome::Cancelled => None,
        Outcome::Failed(error) => Some(Err(std::io::Error::other(error))),
    }
}

impl AppView {
    /// Read `file` (a local file read live) in the background, unless it is
    /// read or being read already.
    pub(crate) fn queue_file_load(&mut self, file: &LogFile) {
        if self.parsed_files.contains_key(&file.file_id) || self.is_queued_or_read(&file.file_id) {
            return;
        }
        let Some(path) = file.path.clone() else { return };
        self.load_queue.push(QueuedLoad { file: file.clone(), read_path: path });
    }

    /// Whether `file_id` is being read, or waits to be read or unpacked.
    pub(crate) fn is_queued_or_read(&self, file_id: &str) -> bool {
        self.loads.contains_key(file_id)
            || self.load_queue.iter().any(|q| q.file.file_id == file_id)
            || self.archive_queue.iter().any(|(_, files)| files.iter().any(|f| f.file_id == file_id))
    }

    /// How `file` is read: the id of its Log Format, and the options with
    /// that format and the Detected Fields turned on.
    pub(crate) fn index_options_for(&mut self, file: &LogFile) -> (Option<String>, IndexOptions) {
        let format_id = self
            .active_space()
            .cloned()
            .and_then(|space| self.wanted_formats(&space).into_iter().find(|(id, _)| *id == file.file_id)?.1);
        let format = format_id.as_deref().and_then(|id| self.formats.compiled(id));
        (format_id, IndexOptions { format, config: self.line_config(), ..IndexOptions::default() })
    }

    /// Read again, in the background, the files of the open Log Space read
    /// through their Line Index with another Log Format or other Detected
    /// Fields than they are to be read with now. Whether there were any.
    pub(crate) fn read_indexed_again_if_stale(&mut self) -> bool {
        let Some(space) = self.active_space().cloned() else { return false };
        let config = self.line_config();
        let wanted: std::collections::HashMap<String, Option<String>> = self.wanted_formats(&space).into_iter().collect();
        let stale: Vec<LogFile> = space
            .files
            .iter()
            .filter(|f| {
                self.parsed_files.get(&f.file_id).is_some_and(|p| {
                    p.indexed.as_ref().is_some_and(|indexed| {
                        !Arc::ptr_eq(&indexed.options().config, &config)
                            || wanted.get(&f.file_id).is_some_and(|w| *w != p.format_id)
                    })
                })
            })
            .cloned()
            .collect();
        if stale.is_empty() {
            return false;
        }
        // The view is built again from the tab once they are read: what it
        // shows now (its Filter, its Focus Tabs) goes to the tab first.
        self.persist_active_session();
        for file in &stale {
            // Meanwhile the rows stay, parsed the new way when shown (task
            // 6.3); Filter's values of a new column come with Indexing.
            let (format_id, options) = self.index_options_for(file);
            if let Some(parsed) = self.parsed_files.get_mut(&file.file_id)
                && let Some(indexed) = parsed.indexed.clone()
            {
                parsed.indexed = Some(Arc::new(indexed.read_with(options)));
                parsed.format_id = format_id;
            }
            self.queue_read_again(file);
        }
        true
    }

    /// Read `file` again in the background, from its start; the view shows
    /// it as read before until then. A read of it under way stops.
    pub(crate) fn queue_read_again(&mut self, file: &LogFile) {
        if let Some(load) = self.loads.remove(&file.file_id) {
            load.cancel.store(true, Ordering::Release);
        }
        self.load_queue.retain(|q| q.file.file_id != file.file_id);
        let Some(path) = file.path.clone() else { return };
        self.load_queue.push(QueuedLoad { file: file.clone(), read_path: path });
    }

    /// Whether a file the open tab shows is still being read.
    pub fn is_reading(&self) -> bool {
        let reads = self.active_file_ids();
        self.loads.keys().any(|id| reads.contains(id))
    }

    /// The files the open tab shows that are being read, with how far.
    pub fn reading(&self) -> Vec<(String, u8)> {
        let reads = self.active_file_ids();
        let mut out: Vec<(String, u8)> =
            self.loads.iter().filter(|(id, _)| reads.contains(*id)).map(|(_, l)| (l.label(), l.percent())).collect();
        out.sort();
        out
    }

    /// Start reading the files queued.
    pub(crate) fn start_queued_loads(&mut self, cx: &mut Context<Self>) {
        self.start_queued_unpacking(cx);
        let queued = std::mem::take(&mut self.load_queue);
        if queued.is_empty() {
            return;
        }
        let live = self.active_space_is_live();
        for QueuedLoad { file, read_path: path } in queued {
            // An Archive Entry is static: no last lines to show first.
            let live = live && file.entry.is_none();
            let cancel = Arc::new(AtomicBool::new(false));
            let read_cancel = cancel.clone();
            let name = file.display_name.clone();
            let (format_id, options) = self.index_options_for(&file);
            let indexed = Arc::new(Indexed::default());
            let budget = self.live.handle_budget.clone();
            let tail = live.then(|| self.read_tail_while_indexing(&path, &options, &cancel, cx));
            let progress = Progress::Indexed(indexed.clone());
            let again = self.parsed_files.contains_key(&file.file_id);
            let load = FileLoad { name: name.clone(), progress, tail, again, cancel: cancel.clone() };
            self.loads.insert(file.file_id.clone(), load);
            let task = cx.background_executor().spawn(async move {
                let read = index_file(&path, options, indexed, budget, &read_cancel);
                read.map(|r| r.map(|parsed| ParsedFile { format_id, ..parsed }))
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| this.file_read(&file, &cancel, result, cx)).ok();
            })
            .detach();
        }
        self.show_read_progress(cx);
        cx.notify();
    }

    /// Read the last lines of the file at `path` now and again until the
    /// read is stopped or done (`cancel`): a live file grows meanwhile.
    fn read_tail_while_indexing(
        &self,
        path: &str,
        options: &IndexOptions,
        cancel: &Arc<AtomicBool>,
        cx: &mut Context<Self>,
    ) -> Arc<Mutex<Option<Tail>>> {
        let (cancel, budget) = (cancel.clone(), self.live.handle_budget.clone());
        // Opened for each read and closed after it: the handle the file
        // is indexed through is the one it keeps.
        let path = path.to_string();
        let open = move || DiskSource::on_demand(&path, budget.clone());
        let mut parser = crate::parser::text::LineParser::new();
        parser.set_format(options.format.clone());
        parser.set_config(options.config.clone());
        // The first read is here: 64 KiB, for a first screen at once.
        let first = read_tail(&open(), TAIL_BYTES, &mut parser).ok();
        let mut read_to = first.as_ref().map(|t| t.end);
        let tail = Arc::new(Mutex::new(first));
        let out = tail.clone();
        let executor = cx.background_executor().clone();
        executor
            .clone()
            .spawn(async move {
                loop {
                    executor.timer(TAIL_EVERY).await;
                    if cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let source = open();
                    let len = source.len().ok();
                    if len.is_some() && len != read_to {
                        if let Ok(read) = read_tail(&source, TAIL_BYTES, &mut parser) {
                            read_to = Some(read.end);
                            *out.lock().unwrap() = Some(read);
                        }
                    }
                }
            })
            .detach();
        tail
    }

    /// The last lines of the open tab's files while they are indexed, by
    /// time, oldest first; their line numbers are not known yet. Empty
    /// once a file of the tab is read (its rows show) or for a Static Log
    /// Space.
    pub fn reading_tail(&self) -> Vec<LogEntry> {
        if !self.view.is_empty() {
            return Vec::new();
        }
        let reads = self.active_file_ids();
        let mut ids: Vec<&String> = self.loads.keys().filter(|id| reads.contains(*id)).collect();
        ids.sort();
        // Each line by the time of the nearest timed line at or above it.
        let mut lines: Vec<(f64, LogEntry)> = Vec::new();
        for id in ids {
            let Some(tail) = self.loads[id].tail.as_ref() else { continue };
            let Some(tail) = tail.lock().unwrap().as_ref().map(|t| t.lines.clone()) else { continue };
            let source: Option<Arc<str>> = (reads.len() > 1).then(|| self.loads[id].name.as_str().into());
            let mut at = f64::NEG_INFINITY;
            for mut entry in tail {
                if !entry.is_continuation() {
                    at = entry.timestamp;
                }
                if source.is_some() {
                    entry.source = source.clone();
                }
                lines.push((at, entry));
            }
        }
        lines.sort_by(|a, b| a.0.total_cmp(&b.0));
        let skip = lines.len().saturating_sub(TAIL_ROWS);
        lines.into_iter().skip(skip).map(|(_, entry)| entry).collect()
    }

    /// Draw the progress of the reads again now and then while they run.
    fn show_read_progress(&mut self, cx: &mut Context<Self>) {
        if self.read_progress_shown {
            return;
        }
        self.read_progress_shown = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PROGRESS_EVERY).await;
                let reading = this
                    .update(cx, |this, cx| {
                        let reading = !this.loads.is_empty();
                        if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
                            eprintln!("reading: {:?}", this.loads.values().map(|l| (&l.name, l.percent())).collect::<Vec<_>>());
                        }
                        if reading {
                            this.show_values_read_so_far();
                            cx.notify();
                        } else {
                            this.read_progress_shown = false;
                        }
                        reading
                    })
                    .unwrap_or(false);
                if !reading {
                    break;
                }
            }
        })
        .detach();
    }

    /// The read of `file` ended (`None`: it was stopped).
    fn file_read(&mut self, file: &LogFile, cancel: &Arc<AtomicBool>, result: Option<std::io::Result<ParsedFile>>, cx: &mut Context<Self>) {
        // Another read of the file took its place meanwhile.
        if !self.loads.get(&file.file_id).is_some_and(|l| Arc::ptr_eq(&l.cancel, cancel)) {
            return;
        }
        if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
            eprintln!("read {}: {}", file.display_name, if result.is_some() { "done" } else { "stopped" });
        }
        // The last file of the open tab is read: what was set on its
        // Filter meanwhile goes to the tab before its view is built. It
        // goes while the tab still reads, so the row it was at stays.
        let ends_reading = result.is_some()
            && self.loads.contains_key(&file.file_id)
            && self.active_file_ids().contains(&file.file_id)
            && self.reading().len() == 1;
        if ends_reading {
            self.persist_active_session();
        }
        let Some(load) = self.loads.remove(&file.file_id) else {
            // Dropped meanwhile: what it read is not wanted.
            return;
        };
        let Some(result) = result else { return };
        // The tab left meanwhile: the file is read again when it is chosen.
        if !self.active_file_ids().contains(&file.file_id) || (self.parsed_files.contains_key(&file.file_id) && !load.again) {
            return;
        }
        let mut parsed = result.unwrap_or_else(|e| {
            let error = super::tabs::read_failure(file, &e);
            self.notice = Some(error.clone());
            ParsedFile::failed(error)
        });
        // Its lines are the file's current Generation, below the Break of
        // why it was read again, if it was.
        let mark = self.live.first_marks.remove(&file.file_id);
        if let Some(indexed) = &parsed.indexed {
            let generation = self.active_space().and_then(|s| s.file(&file.file_id)).map_or(0, |f| f.generation);
            indexed.set_first_generation(generation, mark.clone());
            if let Some(mark) = mark {
                Arc::make_mut(&mut parsed.raw_breaks).insert(0, mark);
            }
        }
        self.parsed_files.insert(file.file_id.clone(), parsed);
        self.drop_markers_that_no_longer_match(&file.file_id);
        if !self.is_reading() {
            // Every file of the tab is read: its view, as the tab keeps it,
            // with the picker left open on it.
            let picker = (self.filter_ui.picker_open, self.filter_ui.expanded.clone());
            self.restore_active_session(cx);
            (self.filter_ui.picker_open, self.filter_ui.expanded) = picker;
            // What Indexing held only while it read goes back to the system.
            super::live::release_freed_memory();
        } else {
            self.show_values_read_so_far();
        }
    }

    /// While the open tab shows no rows because its files are being read,
    /// its Filter offers the values of the lines read so far (task 5.2):
    /// they grow as Indexing reads on.
    pub fn show_values_read_so_far(&mut self) {
        if !self.view.is_empty() || !self.is_reading() {
            return;
        }
        let Some(space) = self.active_space() else { return };
        let shown = self.active_file_ids();
        // In file order: values first seen in the first file come first.
        let files: Vec<&LogFile> = space.files.iter().filter(|f| shown.contains(&f.file_id)).collect();
        let several = files.len() > 1;
        let mut parts: Vec<(Option<String>, Arc<Indexed>)> = Vec::new();
        for file in files {
            let indexed = match self.loads.get(&file.file_id) {
                Some(load) => load.indexed().cloned(),
                None => self.parsed_files.get(&file.file_id).and_then(|p| p.indexed.as_ref()).map(|f| f.indexed.clone()),
            };
            let Some(indexed) = indexed else { continue };
            parts.push((several.then(|| file.display_name.clone()), indexed));
        }
        let data: Vec<_> = parts.iter().map(|(_, indexed)| indexed.data.read_recursive()).collect();
        let files: Vec<MergeFile> = parts
            .iter()
            .zip(&data)
            .map(|((name, _), data)| MergeFile { index: &data.filter, source: name.as_deref() })
            .collect();
        let so_far = FilterIndex::values_so_far(&files);
        drop(files);
        drop(data);
        self.filter_index = so_far.into();
    }

    /// How far the open tab's files are read while its Filter shows the
    /// values read so far: `None` once they are read.
    pub fn filter_read_so_far(&self) -> Option<u8> {
        if !self.view.is_empty() {
            return None;
        }
        self.reading().into_iter().map(|(_, percent)| percent).min()
    }

    /// A file read from disk may have changed since its Markers were set: it
    /// was replaced or rewritten, or changed while the app was closed. A
    /// Marker stays on a line whose text still has the hash it was set on;
    /// the others are dropped, and the user is told how many.
    fn drop_markers_that_no_longer_match(&mut self, file_id: &str) {
        let Some(indexed) = self.parsed_files.get(file_id).and_then(|p| p.indexed.clone()) else { return };
        let hash_of = |line: usize| {
            line.checked_sub(1).and_then(|n| indexed.line(n)).map(|text| crate::workspace::log_space::line_hash(&text))
        };
        self.keep_markers_that_match(file_id, |m| m.generations_ago != 0 || hash_of(m.line) == m.line_hash);
    }

    /// A file was read again (from disk, or a Remote Copy from its Server):
    /// its Markers stay where `matches` says their line is the same, and the
    /// others are dropped, with a notice. A Marker saved without the hash of
    /// its line stays.
    pub(crate) fn keep_markers_that_match(&mut self, file_id: &str, matches: impl Fn(&StoredMarker) -> bool) {
        let mut dropped = 0;
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(f) = space.file_mut(file_id) {
                    let before = f.markers.len();
                    f.markers.retain(|m| m.line_hash.is_none() || matches(m));
                    dropped = dropped.max(before - f.markers.len());
                }
            }
        }
        if dropped > 0 {
            self.notice = Some(match dropped {
                1 => "1 Marker no longer matches the file".to_string(),
                n => format!("{n} Markers no longer match the file"),
            });
        }
    }

    /// Stop reading the files the open tab does not show.
    pub(crate) fn drop_dormant_loads(&mut self) {
        let reads = self.active_file_ids();
        self.loads.retain(|id, _| reads.contains(id));
        self.load_queue.retain(|q| reads.contains(&q.file.file_id));
        for (_, files) in &mut self.archive_queue {
            files.retain(|f| reads.contains(&f.file_id));
        }
        self.archive_queue.retain(|(_, files)| !files.is_empty());
    }

    /// Unpack in the background the Archives waiting to be: one pass over
    /// each for all its entries the tab shows, with its progress on them.
    fn start_queued_unpacking(&mut self, cx: &mut Context<Self>) {
        let queued = std::mem::take(&mut self.archive_queue);
        for (archive, files) in queued {
            let how = crate::archive_cache::Unpacking::default();
            let total = std::fs::metadata(&archive).map_or(0, |m| m.len());
            for file in &files {
                let progress = Progress::Unpacking { read: how.read.clone(), total };
                let load = FileLoad { name: file.display_name.clone(), progress, tail: None, again: false, cancel: how.cancel.clone() };
                self.loads.insert(file.file_id.clone(), load);
            }
            let root = self.archive_cache_root();
            let names: Vec<String> = files.iter().filter_map(|f| f.entry.clone()).collect();
            let (path, work) = (std::path::PathBuf::from(&archive), how.clone());
            let archive = archive.clone();
            let task = cx.background_executor().spawn(async move {
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                match crate::archive::detect(&path) {
                    Some(kind) => crate::archive_cache::unpack_with(&root, &path, kind, &names, crate::archive::MAX_UNCOMPRESSED_BYTES, &work),
                    None if path.exists() => Err(crate::archive::ArchiveError::Damaged("not an archive".into())),
                    None => Err(crate::archive::ArchiveError::NotFound),
                }
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| this.archive_unpacked(archive, files, &how.cancel, result, cx)).ok();
            })
            .detach();
        }
        self.show_read_progress(cx);
    }

    /// An Archive was unpacked (or could not be): its entries the tab still
    /// shows are read from the archive cache; those that failed are empty,
    /// with a notice. A cancelled unpacking leaves nothing behind.
    fn archive_unpacked(
        &mut self,
        archive: String,
        files: Vec<LogFile>,
        cancel: &Arc<AtomicBool>,
        result: Result<std::collections::HashMap<String, Result<std::path::PathBuf, crate::archive::ArchiveError>>, crate::archive::ArchiveError>,
        cx: &mut Context<Self>,
    ) {
        use crate::archive::ArchiveError;
        let reads = self.active_file_ids();
        let mut notices = Vec::new();
        let mut again = Vec::new();
        let failed = result.as_ref().is_err_and(|e| !matches!(e, ArchiveError::Cancelled));
        let mut entries = match result {
            Ok(entries) => Some(entries),
            Err(ArchiveError::Cancelled) => None,
            Err(e) => {
                let e = std::io::Error::from(e);
                for file in &files {
                    if self.loads.get(&file.file_id).is_some_and(|l| Arc::ptr_eq(&l.cancel, cancel)) && reads.contains(&file.file_id) {
                        let error = super::tabs::read_failure(file, &e);
                        notices.push(error.clone());
                        self.parsed_files.insert(file.file_id.clone(), ParsedFile::failed(error));
                    }
                }
                None
            }
        };
        for file in files {
            // Another read of the file took its place, or the tab left.
            if !self.loads.get(&file.file_id).is_some_and(|l| Arc::ptr_eq(&l.cancel, cancel)) {
                continue;
            }
            // The unpacking is over: its shared `cancel` stops nothing now.
            self.loads.remove(&file.file_id);
            if !reads.contains(&file.file_id) {
                continue;
            }
            let found = entries.as_mut().and_then(|e| file.entry.as_deref().and_then(|name| e.remove(name)));
            match found {
                Some(Ok(path)) => {
                    self.load_queue.push(QueuedLoad { file, read_path: path.to_string_lossy().into_owned() });
                }
                Some(Err(ArchiveError::Cancelled)) => {}
                Some(Err(e)) => {
                    let error = super::tabs::read_failure(&file, &std::io::Error::from(e));
                    notices.push(error.clone());
                    self.parsed_files.insert(file.file_id.clone(), ParsedFile::failed(error));
                }
                None if entries.is_some() => {
                    self.parsed_files.insert(file.file_id.clone(), ParsedFile::empty());
                }
                // Stopped for another entry of the Archive no longer shown:
                // this one is still wanted, so it is unpacked again.
                None if !failed => again.push(file),
                None => {}
            }
        }
        if !again.is_empty() {
            self.archive_queue.push((archive, again));
        }
        if !notices.is_empty() {
            self.notice = Some(notices.join("; "));
        }
        self.start_queued_loads(cx);
        if !self.is_reading() {
            self.restore_active_session(cx);
        }
        cx.notify();
    }
}
