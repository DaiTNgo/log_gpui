//! Logic: reading local files in the background. A file the open tab needs
//! is read and parsed on a worker, with its progress shown on the canvas
//! and the tab, so choosing a tab with a large file never freezes the
//! window. When the tab's files are all read its view is built, with the
//! position, Filter and Focus Tabs it keeps. A file whose tab was left while
//! it was being read is dropped.

use super::AppView;
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
    /// Bytes read and parsed so far, and the file's size.
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
}

impl FileLoad {
    /// How far the read is, 0..=100.
    pub fn percent(&self) -> u8 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        (self.done.load(Ordering::Relaxed).min(total) * 100 / total) as u8
    }
}

impl Drop for FileLoad {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// Read and parse the file at `path`, live from where it ends.
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
            let load = FileLoad {
                name: file.display_name.clone(),
                done: Arc::default(),
                total: Arc::default(),
                cancel: Arc::default(),
            };
            let (done, total, cancel) = (load.done.clone(), load.total.clone(), load.cancel.clone());
            let name = file.display_name.clone();
            self.loads.insert(file.file_id.clone(), load);
            let task =
                cx.background_executor().spawn(async move { read_file(&name, &path, &done, &total, &cancel) });
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
