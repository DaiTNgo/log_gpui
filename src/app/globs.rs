//! Logic: Live Globs of open Remote Log Spaces. A watcher per (Server,
//! folder) lists the folder; the app decides from each listing which files
//! the Live Globs of the folder add (see `remote::glob_plan`).

use super::AppView;
use crate::live::SourceStatus;
use crate::live::remote::GlobInodes;
use crate::remote::browse::DirEntry;
use crate::remote::glob_plan::{GlobPlan, Listing, SpaceFile, plan_glob};
use crate::remote::glob_watcher::{GlobEvent, GlobWatcher, WatchCommand, WatcherHandle};
use crate::workspace::log_space::{LiveGlob, LogSpaceKind};
use async_channel::{Receiver, Sender};
use gpui::Context;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// A folder as the watcher last listed it.
#[derive(Clone)]
pub struct StoredListing {
    pub entries: Vec<DirEntry>,
    pub inodes: HashMap<String, u64>,
    pub archives: HashSet<String>,
}

/// Live state of the Live Globs: not remembered when the app closes.
pub struct GlobRuntime {
    events: Sender<GlobEvent>,
    inbox: Receiver<GlobEvent>,
    /// A watcher per `(server id, folder)`, with the patterns it was given.
    pub watchers: HashMap<(String, String), (WatcherHandle, Vec<String>)>,
    pump_started: bool,
    /// The inodes each Live Glob has read, shared with its sources.
    pub inodes: HashMap<String, GlobInodes>,
    /// Live Globs matched against their folder since their space was opened.
    pub reconciled: HashSet<String>,
    /// Matching files each Live Glob did not add because of the limit.
    pub pending: HashMap<String, usize>,
    /// Folders that do not exist (yet).
    pub waiting_folder: HashSet<(String, String)>,
    last: HashMap<(String, String), StoredListing>,
    /// Live Globs whose group is collapsed in the sidebar.
    pub collapsed: HashSet<String>,
}

impl GlobRuntime {
    pub fn new() -> Self {
        let (events, inbox) = async_channel::unbounded();
        Self {
            events,
            inbox,
            watchers: HashMap::new(),
            pump_started: false,
            inodes: HashMap::new(),
            reconciled: HashSet::new(),
            pending: HashMap::new(),
            waiting_folder: HashSet::new(),
            last: HashMap::new(),
            collapsed: HashSet::new(),
        }
    }

    /// The inodes of a Live Glob, created on first use.
    pub fn inodes_of(&mut self, glob_id: &str) -> GlobInodes {
        self.inodes.entry(glob_id.to_string()).or_default().clone()
    }
}

impl Default for GlobRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// What to do with one Live Glob after a listing.
struct GlobAction {
    space_id: String,
    glob_id: String,
    server_name: String,
    /// Files of the glob that left its folder while the space was closed.
    prune: Vec<String>,
    add: Vec<String>,
}

impl AppView {
    /// The open Remote Log Spaces that are not paused.
    fn live_glob_spaces(&self) -> Vec<String> {
        let Some(project) = self.active_project() else { return Vec::new() };
        self.open_space_ids()
            .into_iter()
            .filter(|id| !self.live.paused_spaces.contains(id))
            .filter(|id| project.log_space(id).is_some_and(|s| s.kind == LogSpaceKind::Remote))
            .collect()
    }

    /// Start the watchers that are needed and stop those that are not: a
    /// folder needs one while an open, running Log Space has a Live Glob on
    /// it that is not stopped (or not yet matched since the space opened).
    pub(crate) fn sync_glob_watchers(&mut self, cx: &mut Context<Self>) {
        let mut want: HashMap<(String, String), Vec<String>> = HashMap::new();
        let mut open_globs: HashSet<String> = HashSet::new();
        if let Some(project) = self.active_project() {
            for space_id in self.open_space_ids() {
                let Some(space) = project.log_space(&space_id) else { continue };
                let running = !self.live.paused_spaces.contains(&space_id);
                for glob in &space.globs {
                    open_globs.insert(glob.glob_id.clone());
                    if running && (!glob.stopped || !self.live.globs.reconciled.contains(&glob.glob_id)) {
                        let patterns = want.entry((glob.server_id.clone(), glob.dir.clone())).or_default();
                        if !patterns.contains(&glob.pattern) {
                            patterns.push(glob.pattern.clone());
                        }
                    }
                }
            }
        }
        // A closed space is matched again when it is reopened.
        self.live.globs.reconciled.retain(|id| open_globs.contains(id));
        self.live.globs.pending.retain(|id, _| open_globs.contains(id));
        self.live.globs.inodes.retain(|id, _| open_globs.contains(id));
        let gone: Vec<(String, String)> =
            self.live.globs.watchers.keys().filter(|k| !want.contains_key(*k)).cloned().collect();
        for key in gone {
            if let Some((handle, _)) = self.live.globs.watchers.remove(&key) {
                handle.stop();
            }
            self.live.globs.last.remove(&key);
            self.live.globs.waiting_folder.remove(&key);
        }
        if want.is_empty() {
            return;
        }
        let interval = Duration::from_secs(self.live.glob_interval);
        for (key, patterns) in want {
            match self.live.globs.watchers.get_mut(&key) {
                Some((handle, current)) => {
                    if *current != patterns {
                        handle.send(WatchCommand::SetPatterns(patterns.clone()));
                        *current = patterns;
                    }
                }
                None => {
                    let hub = self.remote_hub(cx);
                    let handle = GlobWatcher {
                        hub,
                        servers: self.remote.shared_servers.clone(),
                        server_id: key.0.clone(),
                        dir: key.1.clone(),
                        patterns: patterns.clone(),
                        interval,
                        events: self.live.globs.events.clone(),
                    }
                    .spawn();
                    self.live.globs.watchers.insert(key, (handle, patterns));
                }
            }
        }
        self.start_glob_pump(cx);
    }

    fn start_glob_pump(&mut self, cx: &mut Context<Self>) {
        if self.live.globs.pump_started {
            return;
        }
        self.live.globs.pump_started = true;
        let inbox = self.live.globs.inbox.clone();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = inbox.recv().await {
                if this.update(cx, |this, cx| this.handle_glob_event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn handle_glob_event(&mut self, event: GlobEvent, cx: &mut Context<Self>) {
        match event {
            GlobEvent::FolderMissing { server_id, dir } => {
                if self.live.globs.watchers.contains_key(&(server_id.clone(), dir.clone())) {
                    self.live.globs.waiting_folder.insert((server_id, dir));
                    cx.notify();
                }
            }
            GlobEvent::Listing { server_id, dir, entries, inodes, archives } => {
                let key = (server_id, dir);
                // A listing that arrives after its watcher was stopped is stale.
                if !self.live.globs.watchers.contains_key(&key) {
                    return;
                }
                self.live.globs.waiting_folder.remove(&key);
                let listing = StoredListing { entries, inodes, archives };
                self.live.globs.last.insert(key.clone(), listing.clone());
                self.apply_listing(&key.0, &key.1, &listing, cx);
            }
        }
    }

    /// Decide, for every Live Glob on the folder, what its listing means.
    fn apply_listing(&mut self, server_id: &str, dir: &str, listing: &StoredListing, cx: &mut Context<Self>) {
        let limit = self.live.glob_limit;
        let mut actions: Vec<GlobAction> = Vec::new();
        let mut pending: Vec<(String, usize)> = Vec::new();
        let mut reconciled: Vec<String> = Vec::new();
        for space_id in self.live_glob_spaces() {
            // Every glob of the space gets its inode set before the space is borrowed.
            let glob_ids: Vec<String> = self
                .active_project()
                .and_then(|p| p.log_space(&space_id))
                .map(|s| s.globs.iter().map(|g| g.glob_id.clone()).collect())
                .unwrap_or_default();
            let sets: HashMap<String, GlobInodes> =
                glob_ids.into_iter().map(|id| (id.clone(), self.live.globs.inodes_of(&id))).collect();
            let Some(space) = self.active_project().and_then(|p| p.log_space(&space_id)) else { continue };
            let globs: Vec<LiveGlob> = space
                .globs
                .iter()
                .filter(|g| g.server_id == server_id && g.dir == dir)
                .cloned()
                .collect();
            for glob in globs {
                let server_name = self.remote.servers.get(&glob.server_id).map(|s| s.name.clone()).unwrap_or_default();
                let known = sets[&glob.glob_id].clone();
                // What the glob already has tells which inodes are its own.
                {
                    let mut map = known.lock().unwrap();
                    for file in space.glob_files(&glob.glob_id) {
                        let Some(remote) = &file.remote else { continue };
                        let name = remote.path.rsplit('/').next().unwrap_or("");
                        if let Some(ino) = listing.inodes.get(name) {
                            map.entry(*ino).or_insert_with(|| remote.path.clone());
                        }
                    }
                }
                let present: HashSet<&str> =
                    listing.entries.iter().filter(|e| !e.is_dir).map(|e| e.name.as_str()).collect();
                let first = !self.live.globs.reconciled.contains(&glob.glob_id);
                let mut prune = Vec::new();
                if first {
                    // Files that are no longer there (or no longer match) when
                    // the space is opened again are not brought back.
                    for file in space.glob_files(&glob.glob_id) {
                        let Some(remote) = &file.remote else { continue };
                        let name = remote.path.rsplit('/').next().unwrap_or("");
                        if !present.contains(name) || !crate::remote::browse::glob_match(&glob.pattern, name) {
                            prune.push(file.file_id.clone());
                        }
                    }
                    reconciled.push(glob.glob_id.clone());
                }
                let files: Vec<SpaceFile> = space
                    .files_on_server(&glob.server_id)
                    .into_iter()
                    .filter(|f| !prune.contains(&f.file_id))
                    .filter_map(|f| {
                        let remote = f.remote.as_ref()?;
                        Some(SpaceFile {
                            path: remote.path.clone(),
                            from_glob: f.from_glob.as_deref() == Some(glob.glob_id.as_str()),
                            gone: self
                                .live
                                .sources
                                .get(&f.file_id)
                                .is_some_and(|s| s.status == SourceStatus::Gone),
                        })
                    })
                    .collect();
                let read = known.lock().unwrap().clone();
                let plan: GlobPlan = plan_glob(
                    &glob,
                    &files,
                    &Listing { entries: &listing.entries, inodes: &listing.inodes, archives: &listing.archives },
                    &read,
                    limit,
                );
                pending.push((glob.glob_id.clone(), plan.pending));
                if !prune.is_empty() || !plan.add.is_empty() {
                    actions.push(GlobAction {
                        space_id: space_id.clone(),
                        glob_id: glob.glob_id.clone(),
                        server_name,
                        prune,
                        add: plan.add,
                    });
                }
            }
        }
        for id in reconciled {
            self.live.globs.reconciled.insert(id);
        }
        for (glob_id, count) in pending {
            if count == 0 {
                self.live.globs.pending.remove(&glob_id);
            } else {
                self.live.globs.pending.insert(glob_id, count);
            }
        }
        if !actions.is_empty() {
            self.persist_active_session();
            for action in actions {
                if let Some(space) = self.active_project_mut().and_then(|p| p.log_space_mut(&action.space_id)) {
                    space.drop_glob_files(&action.prune);
                    space.add_glob_files(&action.glob_id, &action.server_name, &action.add);
                }
                for id in &action.prune {
                    self.forget_file(id);
                }
            }
            self.refresh_after_space_change(cx);
        }
        // A stopped glob matched once for reopening needs no watcher after.
        self.sync_glob_watchers(cx);
        cx.notify();
    }

    /// Match every folder's last listing again (the limit changed).
    fn replan_globs(&mut self, cx: &mut Context<Self>) {
        let listings: Vec<((String, String), StoredListing)> =
            self.live.globs.last.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for ((server_id, dir), listing) in listings {
            self.apply_listing(&server_id, &dir, &listing, cx);
        }
    }

    pub fn set_glob_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        self.live.glob_limit = limit.clamp(1, 500);
        self.workspace_state.live_glob_limit = self.live.glob_limit;
        self.schedule_session_save(cx);
        self.replan_globs(cx);
        cx.notify();
    }

    pub fn set_glob_interval(&mut self, seconds: u64, cx: &mut Context<Self>) {
        self.live.glob_interval = seconds.clamp(2, 300);
        self.workspace_state.live_glob_interval = self.live.glob_interval;
        self.schedule_session_save(cx);
        let interval = Duration::from_secs(self.live.glob_interval);
        for (handle, _) in self.live.globs.watchers.values() {
            handle.send(WatchCommand::SetInterval(interval));
        }
        cx.notify();
    }

    /// "Stop watching": the files it has keep reading, no new file is added.
    pub fn stop_watching_glob(&mut self, space_id: &str, glob_id: &str, cx: &mut Context<Self>) {
        self.set_glob_stopped(space_id, glob_id, true, cx);
    }

    /// "Resume watching": the folder is listed at once.
    pub fn resume_watching_glob(&mut self, space_id: &str, glob_id: &str, cx: &mut Context<Self>) {
        self.set_glob_stopped(space_id, glob_id, false, cx);
        let key = self
            .active_project()
            .and_then(|p| p.log_space(space_id))
            .and_then(|s| s.glob(glob_id))
            .map(|g| (g.server_id.clone(), g.dir.clone()));
        if let Some((handle, _)) = key.and_then(|k| self.live.globs.watchers.get(&k)) {
            handle.send(WatchCommand::ScanNow);
        }
    }

    fn set_glob_stopped(&mut self, space_id: &str, glob_id: &str, stopped: bool, cx: &mut Context<Self>) {
        self.persist_active_session();
        if let Some(space) = self.active_project_mut().and_then(|p| p.log_space_mut(space_id)) {
            space.set_glob_stopped(glob_id, stopped);
        }
        if stopped {
            self.live.globs.pending.remove(glob_id);
        }
        self.schedule_session_save(cx);
        self.sync_glob_watchers(cx);
        cx.notify();
    }

    /// "Remove Live Glob": its files go, files added by path stay.
    pub fn remove_live_glob(&mut self, space_id: &str, glob_id: &str, cx: &mut Context<Self>) {
        self.persist_active_session();
        let removed = self
            .active_project_mut()
            .and_then(|p| p.log_space_mut(space_id))
            .map(|s| s.remove_live_glob(glob_id))
            .unwrap_or_default();
        for file in &removed {
            self.forget_file(&file.file_id);
        }
        self.live.globs.pending.remove(glob_id);
        self.live.globs.inodes.remove(glob_id);
        self.refresh_after_space_change(cx);
        self.sync_glob_watchers(cx);
    }

    /// What the status bar says about matching files that are not read.
    pub fn glob_pending_notes(&self) -> Vec<String> {
        let Some(space) = self.active_space() else { return Vec::new() };
        space
            .globs
            .iter()
            .filter(|g| !g.stopped)
            .filter_map(|g| {
                let n = *self.live.globs.pending.get(&g.glob_id)?;
                let server = self.remote.servers.get(&g.server_id).map(|s| s.name.as_str()).unwrap_or("?");
                Some(format!(
                    "{}: {n} matching file{} not read (limit {})",
                    g.label(server),
                    if n == 1 { "" } else { "s" },
                    self.live.glob_limit
                ))
            })
            .collect()
    }
}

impl AppView {
    /// Collapse or expand a Live Glob's group in the sidebar.
    pub fn toggle_glob_collapsed(&mut self, glob_id: &str, cx: &mut Context<Self>) {
        if !self.live.globs.collapsed.remove(glob_id) {
            self.live.globs.collapsed.insert(glob_id.to_string());
        }
        cx.notify();
    }

    /// What the sidebar needs to draw Live Globs.
    pub fn sidebar_globs(&self) -> crate::ui::sidebar::SidebarGlobs {
        let mut out = crate::ui::sidebar::SidebarGlobs {
            collapsed: self.live.globs.collapsed.clone(),
            ..Default::default()
        };
        out.gone_files = self
            .live
            .sources
            .iter()
            .filter(|(_, s)| s.status == SourceStatus::Gone)
            .map(|(id, _)| id.clone())
            .collect();
        for project in &self.workspace_state.projects {
            for space in &project.log_spaces {
                for glob in &space.globs {
                    let server = self.remote.servers.get(&glob.server_id).map(|s| s.name.as_str()).unwrap_or("?");
                    out.labels.insert(glob.glob_id.clone(), glob.label(server));
                    if self.live.globs.waiting_folder.contains(&(glob.server_id.clone(), glob.dir.clone())) {
                        out.waiting.insert(glob.glob_id.clone());
                    }
                }
            }
        }
        out
    }
}
