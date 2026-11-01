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
        self.live.cache_failed.remove(file_id);
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
    let max_chars = file.indexed.data.read_recursive().max_chars.clone();
    ParsedFile {
        max_chars,
        raw_breaks: std::sync::Arc::new(cached.marks_by_position()),
        indexed: Some(file),
        ..ParsedFile::empty()
    }
}

// ── Cleanup (decision 10) ───────────────────────────────────────────────────

impl AppView {
    /// The cache folder of every Remote File and Remote Command an open tab
    /// shows, in any project: a Log Space tab shows its files, a File View
    /// its file. Only these caches are kept.
    pub(crate) fn remote_cache_dirs_in_use(&self) -> std::collections::HashSet<PathBuf> {
        use crate::workspace::working_set::TabTarget;
        let root = self.remote_cache_root();
        let mut dirs = std::collections::HashSet::new();
        for project in &self.workspace_state.projects {
            for tab in project.working_set.tabs() {
                let (space_id, only) = match &tab.target {
                    TabTarget::LogSpace { space_id } => (space_id, None),
                    TabTarget::FileView { space_id, file_id } => (space_id, Some(file_id)),
                    TabTarget::InvestigationBoard { .. } => continue,
                };
                let Some(space) = project.log_space(space_id) else { continue };
                for file in space.files.iter().filter(|f| only.is_none_or(|id| *id == f.file_id)) {
                    let Some(remote) = file.remote.as_ref() else { continue };
                    let (server_id, target) = cache_key(&file.file_id, remote);
                    dirs.insert(cache::dir_for(&root, &server_id, &target));
                }
            }
        }
        dirs
    }

    /// A tab was closed, a file left its Log Space, its Log Space or Server
    /// was deleted, or Edit… made it another file: the caches no open tab
    /// shows go. A file read through a cache that is not its own any more
    /// stops reading it first.
    pub(crate) fn drop_unused_remote_caches(&mut self) {
        let keep = self.remote_cache_dirs_in_use();
        let stale: Vec<String> = self
            .live
            .cached
            .iter()
            .filter(|(id, c)| {
                let own = self.remote_cache_dir(id).map(|d| d.0);
                own.as_deref() != Some(c.cache().dir()) || !keep.contains(c.cache().dir())
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            self.live.cached.remove(&id);
            self.live.cache_failed.remove(&id);
        }
        cache::sweep(&self.remote_cache_root(), &keep);
    }

    /// The Servers whose files (Remote Files, Remote Commands, Live Globs)
    /// an open tab shows, in any project.
    pub fn servers_in_open_tabs(&self) -> std::collections::HashSet<String> {
        use crate::workspace::working_set::TabTarget;
        let mut servers = std::collections::HashSet::new();
        for project in &self.workspace_state.projects {
            for tab in project.working_set.tabs() {
                let (space_id, only) = match &tab.target {
                    TabTarget::LogSpace { space_id } => (space_id, None),
                    TabTarget::FileView { space_id, file_id } => (space_id, Some(file_id)),
                    TabTarget::InvestigationBoard { .. } => continue,
                };
                let Some(space) = project.log_space(space_id) else { continue };
                for file in space.files.iter().filter(|f| only.is_none_or(|id| *id == f.file_id)) {
                    if let Some(remote) = &file.remote {
                        servers.insert(remote.server_id.clone());
                    }
                }
                if only.is_none() {
                    servers.extend(space.globs.iter().map(|g| g.server_id.clone()));
                }
            }
        }
        servers
    }

    /// The connections to Servers no open tab uses go (decision 10). A
    /// Server an Open in full is reading from, or one a Remote dialog may be
    /// browsing, keeps its connection. Switching tabs keeps every one.
    pub(crate) fn release_servers_without_open_tabs(&mut self) {
        let Some(hub) = self.remote.hub.clone() else { return };
        if self.remote.picker.is_some() || self.remote.download.is_some() || self.remote.download_options.is_some() {
            return;
        }
        let mut used = self.servers_in_open_tabs();
        used.extend(self.live.copy_reads.values().map(|r| r.server_id.clone()));
        // A Server reached through Jump Hosts keeps them connected too.
        let hops: Vec<String> = used
            .iter()
            .filter_map(|id| self.remote.servers.chain(id).ok())
            .flatten()
            .map(|s| s.id.clone())
            .collect();
        used.extend(hops);
        for server in &self.remote.servers.servers {
            if used.contains(&server.id) {
                continue;
            }
            let (sid, h) = (server.id.clone(), hub.clone());
            hub.spawn(async move { h.drop_session(&sid).await });
        }
    }

    /// Every Remote Cache goes: the app quits, or opens (what an app that
    /// stopped short left behind). A cache another app window uses stays.
    pub(crate) fn remove_all_remote_caches(&mut self) {
        self.live.cached.clear();
        self.live.cache_failed.clear();
        cache::sweep(&self.remote_cache_root(), &Default::default());
    }
}
