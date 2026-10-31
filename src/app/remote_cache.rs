//! Logic: the Remote Cache of each Remote File and Remote Command (openspec
//! change `remote-local-cache`). Their lines are written to it and read
//! through a Line Index while their tab is open; when the tab is left or
//! the app quits, it ends at the last Checkpoint. Choosing the tab again,
//! or opening the app, shows those lines at once and reads on from the
//! Server only what was written since (`Resume`).

use super::AppView;
use crate::remote::cache::{self, CacheTarget, RemoteCache};
use crate::space_view::ParsedFile;
use crate::workspace::log_space::RemoteTarget;
use std::path::PathBuf;

impl AppView {
    /// The folder of Remote Caches, next to the Remote Copies.
    pub(crate) fn remote_cache_root(&self) -> PathBuf {
        let base = self.remote.picker_state_path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        cache::root_in(&base)
    }

    #[doc(hidden)]
    pub fn remote_cache_root_for_test(&self) -> PathBuf {
        self.remote_cache_root()
    }

    /// The cache folder of the Remote File or Remote Command `file_id`:
    /// `None` for a local file.
    fn remote_cache_dir(&self, file_id: &str) -> Option<(PathBuf, String, CacheTarget)> {
        let file = self.workspace_state.projects.iter().flat_map(|p| p.log_spaces.iter()).find_map(|s| s.file(file_id))?;
        let (server_id, target) = cache_key(file_id, file.remote.as_ref()?);
        Some((cache::dir_for(&self.remote_cache_root(), &server_id, &target), server_id, target))
    }
}

// ── Remote Files and Remote Commands read through their Remote Cache ──────

impl AppView {
    /// Lines a segment of a Remote File's cache takes: a tenth of the Live
    /// Window, so the window's cut drops whole segments most of the time.
    fn cache_segment_lines(&self) -> usize {
        (self.live.window / 10).max(1_000)
    }

    /// The Remote File or Remote Command `file_id` read through its Remote
    /// Cache: what it kept from last time, shown at once (a command's last
    /// Run, which runs again below it), or nothing yet. An error when the
    /// cache cannot be used: another app window reads the file
    /// (`WouldBlock`), or the disk fails.
    pub(crate) fn open_cached(&mut self, file_id: &str, name: &str, is_command: bool) -> std::io::Result<ParsedFile> {
        use crate::live::cached::CachedFile;
        let not_remote = || std::io::Error::new(std::io::ErrorKind::NotFound, "not a remote file");
        let (dir, server_id, target) = self.remote_cache_dir(file_id).ok_or_else(not_remote)?;
        let file = self
            .workspace_state
            .projects
            .iter()
            .flat_map(|p| p.log_spaces.iter())
            .find_map(|s| s.file(file_id))
            .cloned()
            .ok_or_else(not_remote)?;
        let (_, options) = self.index_options_for(&file);
        let budget = self.live.handle_budget.clone();
        let lines = self.cache_segment_lines();
        let cache = RemoteCache::open(&dir, &server_id, target.clone(), lines)?;
        let restored = if is_command {
            CachedFile::restore_command(name, cache, options.clone(), budget.clone())?.map(|(cached, previous)| {
                self.live.previous_run.insert(file_id.to_string(), previous);
                cached
            })
        } else {
            CachedFile::restore(name, cache, options.clone(), budget.clone())?.map(|(cached, from)| {
                self.live.resume_from.insert(file_id.to_string(), from);
                cached
            })
        };
        let cached = match restored {
            Some(cached) => {
                // Its Markers count Generations from the newest one kept.
                let generation = cached.generation;
                if let Some(project) = self.active_project_mut() {
                    for space in &mut project.log_spaces {
                        if let Some(f) = space.file_mut(file_id) {
                            f.generation = generation;
                        }
                    }
                }
                cached
            }
            None => {
                let cache = RemoteCache::open(&dir, &server_id, target, lines)?;
                if is_command {
                    CachedFile::command(name, cache, options, budget)?
                } else {
                    CachedFile::new(name, cache, options, budget)?
                }
            }
        };
        let parsed = cached_parsed(&cached);
        self.live.cached.insert(file_id.to_string(), cached);
        Ok(parsed)
    }

    /// The Remote File's lines as the view reads them now.
    pub(crate) fn refresh_cached_parsed(&mut self, file_id: &str) {
        let Some(cached) = self.live.cached.get(file_id) else { return };
        let parsed = cached_parsed(cached);
        let format_id = self.parsed_files.get(file_id).and_then(|p| p.format_id.clone());
        self.parsed_files.insert(file_id.to_string(), ParsedFile { format_id, ..parsed });
    }

    /// The tab of the Remote File is left: its cache ends at its last
    /// Checkpoint, for next time.
    pub(crate) fn close_cached(&mut self, file_id: &str) {
        let Some(cached) = self.live.cached.remove(file_id) else { return };
        let closed = if cached.is_command() { cached.close_command(&super::live::now_label()) } else { cached.close() };
        if let Err(e) = closed {
            self.notice = Some(format!("Could not keep the lines of {file_id}: {e}"));
        }
    }

    pub(crate) fn close_all_cached(&mut self) {
        let ids: Vec<String> = self.live.cached.keys().cloned().collect();
        for id in ids {
            self.close_cached(&id);
        }
    }
}

/// Which cache a remote file has: a Remote File by its Server and path; a
/// Remote Command by its file id, which stays when the command is edited
/// (its output stays, below a Break, as when it is held in memory).
fn cache_key(file_id: &str, remote: &crate::workspace::log_space::RemoteRef) -> (String, CacheTarget) {
    match &remote.target {
        RemoteTarget::File { path } => (remote.server_id.clone(), CacheTarget::File { path: path.clone() }),
        RemoteTarget::Command { .. } => (COMMANDS.to_string(), CacheTarget::Command { command: file_id.to_string() }),
    }
}

/// The folder of the caches of Remote Commands, in place of a Server's.
const COMMANDS: &str = "commands";

/// A `ParsedFile` reading the lines of `cached` through its Line Index.
fn cached_parsed(cached: &crate::live::cached::CachedFile) -> ParsedFile {
    let file = cached.file();
    let max_chars = file.indexed.data.read().unwrap().max_chars.clone();
    ParsedFile {
        max_chars,
        raw_breaks: std::sync::Arc::new(cached.marks_by_position()),
        indexed: Some(file),
        ..ParsedFile::empty()
    }
}

// ── Cleanup (decision 7) ────────────────────────────────────────────────────

/// A cache not opened for this long is removed at start.
const STALE_AFTER_SECS: u64 = 14 * 24 * 60 * 60;
/// The size of the caches is checked at most this often while lines come.
const SIZE_CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(10);

impl AppView {
    /// The cache folder of every Remote File and Remote Command a Log Space
    /// of any project holds.
    pub(crate) fn remote_cache_dirs_in_use(&self) -> std::collections::HashSet<PathBuf> {
        let root = self.remote_cache_root();
        self.workspace_state
            .projects
            .iter()
            .flat_map(|p| p.log_spaces.iter())
            .flat_map(|s| s.files.iter())
            .filter_map(|f| {
                let (server_id, target) = cache_key(&f.file_id, f.remote.as_ref()?);
                Some(cache::dir_for(&root, &server_id, &target))
            })
            .collect()
    }

    /// The caches of the files of open tabs: never removed to make room.
    fn open_cache_dirs(&self) -> std::collections::HashSet<PathBuf> {
        self.live.cached.values().map(|c| c.cache().dir().to_path_buf()).collect()
    }

    /// A file left its Log Space, its Log Space or Server was deleted, or
    /// Edit… made it another file: the caches nothing refers to any more
    /// go. A file read through a cache that is not its own any more stops
    /// reading it first.
    pub(crate) fn drop_unused_remote_caches(&mut self) {
        let stale: Vec<String> = self
            .live
            .cached
            .iter()
            .filter(|(id, c)| self.remote_cache_dir(id).map(|d| d.0).as_deref() != Some(c.cache().dir()))
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            self.live.cached.remove(&id);
        }
        cache::sweep(&self.remote_cache_root(), &self.remote_cache_dirs_in_use(), None);
    }

    /// At start: the caches no file refers to, and those not opened for 14
    /// days, go.
    pub(crate) fn sweep_remote_caches_at_start(&mut self) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        cache::sweep(&self.remote_cache_root(), &self.remote_cache_dirs_in_use(), Some(now.saturating_sub(STALE_AFTER_SECS)));
    }

    /// Lines were written to a cache: at most every 10 s, the caches opened
    /// longest ago go until all fit the limit.
    pub(crate) fn check_remote_cache_size(&mut self) {
        let now = std::time::Instant::now();
        if self.live.cache_checked.is_some_and(|t| now.duration_since(t) < SIZE_CHECK_EVERY) {
            return;
        }
        self.live.cache_checked = Some(now);
        self.trim_remote_caches();
    }

    /// The caches opened longest ago go until all fit the limit. When only
    /// the caches of open tabs are left and still go over, a notice says so
    /// once.
    pub fn trim_remote_caches(&mut self) {
        let trimmed = cache::trim_to_size(&self.remote_cache_root(), self.live.cache_limit, &self.open_cache_dirs());
        let over = trimmed.bytes > self.live.cache_limit;
        if over && !self.live.cache_over_told {
            self.notice = Some(format!(
                "The Remote Cache of the open tabs takes {}, more than its limit of {}",
                human_bytes(trimmed.bytes),
                human_bytes(self.live.cache_limit)
            ));
        }
        self.live.cache_over_told = over;
    }

    /// Bytes the Remote Caches take, for Settings.
    pub fn remote_cache_size(&self) -> u64 {
        cache::total_size(&self.remote_cache_root())
    }

    pub fn set_remote_cache_limit(&mut self, mb: u64, cx: &mut gpui::Context<Self>) {
        self.workspace_state.remote_cache_limit_mb = mb.clamp(256, 1 << 20);
        self.live.cache_limit = self.workspace_state.remote_cache_limit_mb << 20;
        self.live.cache_over_told = false;
        self.schedule_session_save(cx);
        self.trim_remote_caches();
        cx.notify();
    }

    /// "Clear Remote cache": every cache but those of open tabs goes; those
    /// files are read from their Server again next time.
    pub fn clear_remote_cache(&mut self, cx: &mut gpui::Context<Self>) {
        let root = self.remote_cache_root();
        let keep = self.open_cache_dirs();
        for info in cache::list(&root) {
            if !keep.contains(&info.dir) {
                let _ = cache::remove(&info.dir);
            }
        }
        cache::sweep(&root, &self.remote_cache_dirs_in_use(), None);
        cx.notify();
    }
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / f64::from(1u32 << 30))
    } else {
        format!("{} MB", bytes >> 20)
    }
}
