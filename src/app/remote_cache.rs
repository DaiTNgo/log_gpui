//! Logic: the Remote Cache of each Remote File (openspec change
//! `remote-local-cache`, first stage). While a tab is open its Remote Files
//! hold their lines in memory as before; when the tab is left or the app
//! quits, what they hold up to the last Checkpoint is written to the cache.
//! Choosing the tab again, or opening the app, shows those lines at once
//! and reads on from the Server only what was written since (`Resume`).

use super::AppView;
use crate::live::LiveBuffer;
use crate::live::remote::ResumeFrom;
use crate::remote::cache::{self, CacheTarget, Checkpoint, RemoteCache, Snapshot};
use crate::space_view::ParsedFile;
use crate::workspace::log_space::RemoteTarget;
use std::path::PathBuf;

/// Lines a segment of the cache takes (the first stage writes a
/// Generation as one segment).
const SEGMENT_LINES: usize = 100_000;

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

    /// Write what the Remote File `file_id` holds, up to its last
    /// Checkpoint, to its cache. Nothing is written when the lines do not
    /// line up with the Checkpoint (it is then read anew next time).
    pub(crate) fn snapshot_remote_file(&mut self, file_id: &str) {
        if !self.remote_cache_enabled {
            return;
        }
        let Some((dir, server_id, target)) = self.remote_cache_dir(file_id) else { return };
        let Some(from) = self.live.checkpoints.get(file_id).cloned() else { return };
        let Some(parsed) = self.parsed_files.get(file_id) else { return };
        let Some(buffer) = parsed.live.as_ref() else { return };
        let Some(after) = buffer.lines_after(parsed, from.generation, from.server_end) else { return };
        let keep = parsed.entries.len() - after;
        let mut generations: Vec<Snapshot<'_>> = Vec::new();
        let mut last_generation = None;
        for (i, entry) in parsed.entries[..keep].iter().enumerate() {
            if last_generation != Some(entry.generation) {
                last_generation = Some(entry.generation);
                generations.push(Snapshot {
                    first_line_no: entry.index as u64,
                    lines: Vec::new(),
                    lens: Vec::new(),
                    checkpoint: Checkpoint::default(),
                });
            }
            let snap = generations.last_mut().expect("pushed above");
            snap.lines.push(entry.raw_line.as_ref());
            snap.lens.push(buffer.byte_len(i));
        }
        // Lines with no number yet (the total was not counted) are not kept.
        if generations.iter().any(|g| !crate::live::buffer::is_real_line_number(g.first_line_no as usize)) {
            return;
        }
        if let Some(newest) = generations.last_mut() {
            newest.checkpoint = Checkpoint {
                generation: from.generation,
                inode: from.inode,
                server_end: from.server_end,
                head_hash: Some(from.head_hash),
                tail_hash: Some(from.tail_hash),
            };
        }
        let extended = buffer.hold == crate::live::HoldMode::Extended;
        let written = RemoteCache::open(&dir, &server_id, target, SEGMENT_LINES)
            .and_then(|mut cache| cache.write_snapshot(&generations, extended));
        if let Err(e) = written
            && e.kind() != std::io::ErrorKind::WouldBlock
        {
            self.notice = Some(format!("Could not keep the lines of {file_id}: {e}"));
        }
        self.check_remote_cache_size();
    }

    /// Write the cache of every Remote File held: the app quits.
    pub(crate) fn snapshot_all_remote_files(&mut self) {
        let ids: Vec<String> = self.parsed_files.keys().filter(|id| self.live.checkpoints.contains_key(*id)).cloned().collect();
        for id in ids {
            self.snapshot_remote_file(&id);
        }
    }

    /// The Remote File `file_id` as its cache kept it, with where its
    /// source reads on from; `None` when it has no cache to show.
    pub(crate) fn remote_file_from_cache(&mut self, file_id: &str, name: &str) -> Option<ParsedFile> {
        if !self.remote_cache_enabled {
            return None;
        }
        let (dir, server_id, target) = self.remote_cache_dir(file_id)?;
        if !cache::has_lines(&dir) {
            return None;
        }
        let cache = RemoteCache::open(&dir, &server_id, target, SEGMENT_LINES).ok()?;
        let extended = cache.meta().extended;
        let read = cache.read_lines().ok()?;
        let newest = read.last()?.0.checkpoint.clone();
        let (Some(head_hash), Some(tail_hash)) = (newest.head_hash, newest.tail_hash) else { return None };
        let generations: Vec<(usize, Vec<String>, Vec<u32>)> =
            read.into_iter().map(|(g, lines, lens)| (g.first_line_no as usize, lines, lens)).collect();
        let mut parsed = ParsedFile::empty();
        let buffer =
            LiveBuffer::restore(name, &mut parsed, &generations, newest.generation, newest.server_end, newest.inode, extended);
        parsed.live = Some(buffer);
        self.live.resume_from.insert(
            file_id.to_string(),
            ResumeFrom {
                generation: newest.generation,
                inode: newest.inode,
                server_end: newest.server_end,
                head_hash,
                tail_hash,
            },
        );
        // Its Markers count Generations from the newest one kept.
        if let Some(project) = self.active_project_mut() {
            for space in &mut project.log_spaces {
                if let Some(f) = space.file_mut(file_id) {
                    f.generation = newest.generation;
                }
            }
        }
        Some(parsed)
    }
}

// ── Stage 2: Remote Files read through their Remote Cache ───────────────────

impl AppView {
    /// Lines a segment of a Remote File's cache takes: a tenth of the Live
    /// Window, so the window's cut drops whole segments most of the time.
    fn cache_segment_lines(&self) -> usize {
        (self.live.window / 10).max(1_000)
    }

    /// The Remote File `file_id` read through its Remote Cache, as it was
    /// kept or new; `None` when the cache cannot be used (stage 2 off,
    /// another app window holds it): it then holds its lines in memory.
    pub(crate) fn open_cached_remote_file(&mut self, file_id: &str, name: &str) -> Option<ParsedFile> {
        if !self.remote_cache_enabled || !self.remote_cache_index {
            return None;
        }
        let (dir, server_id, target) = self.remote_cache_dir(file_id)?;
        let file = self.workspace_state.projects.iter().flat_map(|p| p.log_spaces.iter()).find_map(|s| s.file(file_id)).cloned()?;
        let (_, options) = self.index_options_for(&file);
        let budget = self.live.handle_budget.clone();
        let lines = self.cache_segment_lines();
        let cache = RemoteCache::open(&dir, &server_id, target.clone(), lines).ok()?;
        let cached = match crate::live::cached::CachedFile::restore(name, cache, options.clone(), budget.clone()) {
            Ok(Some((cached, from))) => {
                self.live.resume_from.insert(file_id.to_string(), from);
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
            Ok(None) => {
                let cache = RemoteCache::open(&dir, &server_id, target, lines).ok()?;
                crate::live::cached::CachedFile::new(name, cache, options, budget).ok()?
            }
            Err(_) => return None,
        };
        let parsed = cached_parsed(&cached);
        self.live.cached.insert(file_id.to_string(), cached);
        Some(parsed)
    }

    /// The Remote Command `file_id` read through its Remote Cache: the
    /// output of its last Run kept, its command run again below it, or
    /// nothing yet. `None` when the cache cannot be used.
    pub(crate) fn open_cached_remote_command(&mut self, file_id: &str, name: &str) -> Option<ParsedFile> {
        if !self.remote_cache_enabled || !self.remote_cache_index {
            return None;
        }
        let (dir, server_id, target) = self.remote_cache_dir(file_id)?;
        let file = self.workspace_state.projects.iter().flat_map(|p| p.log_spaces.iter()).find_map(|s| s.file(file_id)).cloned()?;
        let (_, options) = self.index_options_for(&file);
        let budget = self.live.handle_budget.clone();
        let lines = self.cache_segment_lines();
        let cache = RemoteCache::open(&dir, &server_id, target.clone(), lines).ok()?;
        let cached = match crate::live::cached::CachedFile::restore_command(name, cache, options.clone(), budget.clone()) {
            Ok(Some((cached, previous))) => {
                self.live.previous_run.insert(file_id.to_string(), previous);
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
            Ok(None) => {
                let cache = RemoteCache::open(&dir, &server_id, target, lines).ok()?;
                crate::live::cached::CachedFile::command(name, cache, options, budget).ok()?
            }
            Err(_) => return None,
        };
        let parsed = cached_parsed(&cached);
        self.live.cached.insert(file_id.to_string(), cached);
        Some(parsed)
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
