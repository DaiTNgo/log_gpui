//! Logic: reading local files in the background. A file the open tab needs
//! is read and parsed on a worker, with its progress shown on the canvas
//! and the tab, so choosing a tab with a large file never freezes the
//! window. When the tab's files are all read its view is built, with the
//! position, Filter and Focus Tabs it keeps. A file whose tab was left while
//! it was being read is dropped.

use super::AppView;
use crate::index::file::IndexedFile;
use crate::index::indexing::{IndexOptions, Indexed, Outcome, index_into};
use crate::index::{DiskSource, HandleBudget, LineSource};
use crate::live::LiveBuffer;
use crate::space_view::ParsedFile;
use crate::workspace::log_space::LogFile;
use gpui::Context;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// How often the progress of a read is shown again.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// A file being read; dropping it stops the read.
pub struct FileLoad {
    pub name: String,
    /// What is read so far: bytes read and parsed, and the file's size.
    progress: Progress,
    cancel: Arc<AtomicBool>,
}

/// How far a read is.
enum Progress {
    /// Parsed whole (a file held in memory): bytes done, of all.
    Parsed { done: Arc<AtomicU64>, total: Arc<AtomicU64> },
    /// Read through its Line Index.
    Indexed(Arc<Indexed>),
}

impl FileLoad {
    /// How far the read is, 0..=100.
    pub fn percent(&self) -> u8 {
        let (done, total) = match &self.progress {
            Progress::Parsed { done, total } => (done.load(Ordering::Relaxed), total.load(Ordering::Relaxed)),
            Progress::Indexed(indexed) => indexed.progress(),
        };
        if total == 0 {
            return 0;
        }
        (done.min(total) * 100 / total) as u8
    }
}

impl Drop for FileLoad {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// Read and parse the whole file at `path`, live from where it ends.
fn read_file(name: &str, path: &str, done: &AtomicU64, total: &AtomicU64, cancel: &AtomicBool) -> Option<std::io::Result<ParsedFile>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => return Some(Err(e)),
    };
    total.store(bytes.len() as u64, Ordering::Relaxed);
    let mut parsed = ParsedFile::from_bytes_with_progress(&bytes, |read| {
        done.store(read as u64, Ordering::Relaxed);
        !cancel.load(Ordering::Acquire)
    })?;
    done.store(bytes.len() as u64, Ordering::Relaxed);
    parsed.live = Some(LiveBuffer::loaded_from_bytes(name, &bytes, parsed.entries.len()));
    Some(Ok(parsed))
}

/// Read the file at `path` through its Line Index: Indexing builds it into
/// `indexed`, and the file stays open to read its lines again.
fn index_file(path: &str, options: IndexOptions, indexed: Arc<Indexed>, cancel: &AtomicBool) -> Option<std::io::Result<ParsedFile>> {
    let source: Arc<dyn LineSource> = match DiskSource::stay_open(path, HandleBudget::shared()) {
        Ok(source) => Arc::new(source),
        Err(e) => return Some(Err(e)),
    };
    match index_into(&*source, &options, &indexed, cancel) {
        Outcome::Done => Some(Ok(ParsedFile::from_indexed(IndexedFile::new(source, indexed, options)))),
        Outcome::Cancelled => None,
        Outcome::Failed(error) => Some(Err(std::io::Error::other(error))),
    }
}

impl AppView {
    /// Read `file` (a local file read live) in the background, unless it is
    /// read or being read already.
    pub(crate) fn queue_file_load(&mut self, file: &LogFile) {
        if self.parsed_files.contains_key(&file.file_id)
            || self.loads.contains_key(&file.file_id)
            || self.load_queue.iter().any(|f| f.file_id == file.file_id)
        {
            return;
        }
        self.load_queue.push(file.clone());
    }

    /// How `file` is read: the id of its Log Format, and the options with
    /// that format and the Detected Fields turned on.
    fn index_options_for(&mut self, file: &LogFile) -> (Option<String>, IndexOptions) {
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
            self.parsed_files.remove(&file.file_id);
            self.queue_file_load(file);
        }
        true
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
            self.loads.iter().filter(|(id, _)| reads.contains(*id)).map(|(_, l)| (l.name.clone(), l.percent())).collect();
        out.sort();
        out
    }

    /// Start reading the files queued.
    pub(crate) fn start_queued_loads(&mut self, cx: &mut Context<Self>) {
        let queued = std::mem::take(&mut self.load_queue);
        if queued.is_empty() {
            return;
        }
        for file in queued {
            let Some(path) = file.path.clone() else { continue };
            let cancel = Arc::new(AtomicBool::new(false));
            let name = file.display_name.clone();
            let task = if self.index_local_files {
                let (format_id, options) = self.index_options_for(&file);
                let indexed = Arc::new(Indexed::default());
                let load = FileLoad { name: name.clone(), progress: Progress::Indexed(indexed.clone()), cancel: cancel.clone() };
                self.loads.insert(file.file_id.clone(), load);
                cx.background_executor().spawn(async move {
                    let read = index_file(&path, options, indexed, &cancel);
                    read.map(|r| r.map(|parsed| ParsedFile { format_id, ..parsed }))
                })
            } else {
                let (done, total) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
                let progress = Progress::Parsed { done: done.clone(), total: total.clone() };
                self.loads.insert(file.file_id.clone(), FileLoad { name: name.clone(), progress, cancel: cancel.clone() });
                cx.background_executor().spawn(async move { read_file(&name, &path, &done, &total, &cancel) })
            };
            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| this.file_read(&file, result, cx)).ok();
            })
            .detach();
        }
        self.show_read_progress(cx);
        cx.notify();
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
    fn file_read(&mut self, file: &LogFile, result: Option<std::io::Result<ParsedFile>>, cx: &mut Context<Self>) {
        if std::env::var_os("LOG_GPUI_LIVE_TRACE").is_some() {
            eprintln!("read {}: {}", file.display_name, if result.is_some() { "done" } else { "stopped" });
        }
        if self.loads.remove(&file.file_id).is_none() {
            // Dropped meanwhile: what it read is not wanted.
            return;
        }
        let Some(result) = result else { return };
        // The tab left meanwhile: the file is read again when it is chosen.
        if !self.active_file_ids().contains(&file.file_id) || self.parsed_files.contains_key(&file.file_id) {
            return;
        }
        let parsed = result.unwrap_or_else(|e| {
            let error = super::tabs::read_failure(file, &e);
            self.notice = Some(error.clone());
            ParsedFile::failed(error)
        });
        self.parsed_files.insert(file.file_id.clone(), parsed);
        if !self.is_reading() {
            // Every file of the tab is read: its view, as the tab keeps it.
            self.restore_active_session(cx);
        }
    }

    /// Stop reading the files the open tab does not show.
    pub(crate) fn drop_dormant_loads(&mut self) {
        let reads = self.active_file_ids();
        self.loads.retain(|id, _| reads.contains(id));
        self.load_queue.retain(|f| reads.contains(&f.file_id));
    }
}
