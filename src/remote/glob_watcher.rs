//! `GlobWatcher`: lists one folder of one Server on a timer, for the Live
//! Globs that point at it, and tells the app what it found. One watcher
//! serves every Live Glob of the same (Server, folder).

use super::browse::{DirEntry, glob_match, is_glob, list_dir};
use super::hub::RemoteSession;
use super::{RemoteHub, ServerStore};
use async_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use russh_sftp::client::{RawSftpSession, SftpSession};
use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncRead, AsyncWrite};

/// A folder with more entries than this is listed at most every 30 seconds.
pub const BIG_FOLDER: usize = 10_000;
pub const BIG_FOLDER_INTERVAL: Duration = Duration::from_secs(30);
/// Names asked about (inode, first bytes) per scan.
const MAX_PROBES: usize = 500;
/// Names per `stat` command.
const STAT_BATCH: usize = 100;
/// Files whose first bytes are asked for at the same time.
const PROBE_CONCURRENCY: usize = 16;
/// Bytes of a file the Archive check needs.
const HEAD_LEN: u32 = 512;

/// What a watcher tells the app.
#[derive(Debug, Clone)]
pub enum GlobEvent {
    /// The folder as it is now.
    Listing {
        server_id: String,
        dir: String,
        entries: Vec<DirEntry>,
        /// Inode by name, for the names that could be asked about.
        inodes: HashMap<String, u64>,
        /// Names whose first bytes say compressed file or Archive.
        archives: HashSet<String>,
    },
    /// The folder does not exist (yet): the Live Globs wait for it.
    FolderMissing { server_id: String, dir: String },
}

/// What the app tells a watcher.
#[derive(Debug, Clone)]
pub enum WatchCommand {
    /// The patterns of the Live Globs that point at the folder.
    SetPatterns(Vec<String>),
    SetInterval(Duration),
    /// List now (Resume, Resume watching).
    ScanNow,
    Stop,
}

#[derive(Debug, Clone)]
pub struct WatcherHandle {
    commands: Sender<WatchCommand>,
}

impl WatcherHandle {
    pub fn send(&self, command: WatchCommand) {
        let _ = self.commands.try_send(command);
    }

    pub fn stop(&self) {
        self.send(WatchCommand::Stop);
    }
}

/// How long to wait between two scans of a folder with `entries` entries.
pub fn scan_interval(configured: Duration, entries: usize) -> Duration {
    if entries > BIG_FOLDER { configured.max(BIG_FOLDER_INTERVAL) } else { configured }
}

pub struct GlobWatcher {
    pub hub: Arc<RemoteHub>,
    pub servers: Arc<Mutex<ServerStore>>,
    pub server_id: String,
    pub dir: String,
    pub patterns: Vec<String>,
    pub interval: Duration,
    pub events: Sender<GlobEvent>,
}

impl GlobWatcher {
    /// Start watching in the background; the first scan is at once.
    pub fn spawn(self) -> WatcherHandle {
        let (commands, command_rx) = async_channel::unbounded();
        let hub = self.hub.clone();
        hub.spawn(async move { self.run(command_rx).await });
        WatcherHandle { commands }
    }

    async fn run(mut self, commands: Receiver<WatchCommand>) {
        // What was learned about a name, for the modification time it had.
        let mut probed: HashMap<String, Probe> = HashMap::new();
        // The SFTP sessions are kept from scan to scan.
        let mut conn: Option<Conn> = None;
        let mut next_wait = Duration::ZERO;
        loop {
            let deadline = tokio::time::Instant::now() + next_wait;
            let mut scan_now = next_wait.is_zero();
            while !scan_now {
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => scan_now = true,
                    cmd = commands.recv() => match cmd {
                        Ok(WatchCommand::Stop) | Err(_) => return,
                        Ok(WatchCommand::ScanNow) => scan_now = true,
                        Ok(WatchCommand::SetPatterns(p)) => {
                            self.patterns = p;
                            scan_now = true;
                        }
                        Ok(WatchCommand::SetInterval(i)) => {
                            self.interval = i;
                            // The new pace starts now.
                            scan_now = true;
                        }
                    },
                }
            }
            if self.events.is_closed() {
                return;
            }
            let entries = self.scan(&mut probed, &mut conn).await;
            next_wait = scan_interval(self.interval, entries);
        }
    }

    /// One listing. Returns the number of entries seen.
    async fn scan(&self, probed: &mut HashMap<String, Probe>, conn: &mut Option<Conn>) -> usize {
        let store = self.servers.lock().unwrap().clone();
        let Ok(session) = self.hub.session(&store, &self.server_id).await else {
            *conn = None;
            return 0;
        };
        if conn.as_ref().is_some_and(|c| !Arc::ptr_eq(&c.session, &session) || session.is_closed()) {
            *conn = None;
        }
        if conn.is_none() {
            let (Ok(sftp), Ok(stream)) = (session.open_sftp().await, session.open_sftp_stream().await) else { return 0 };
            let Ok(head) = HeadProbe::new(stream).await else { return 0 };
            *conn = Some(Conn { session: session.clone(), sftp, head: Arc::new(head) });
        }
        let live = conn.as_ref().unwrap();
        let (sftp, head) = (&live.sftp, live.head.clone());
        let entries = match list_dir(sftp, &self.dir).await {
            Ok(entries) => entries,
            Err(message) => {
                *conn = None;
                if message.contains("does not exist") {
                    let _ = self
                        .events
                        .send(GlobEvent::FolderMissing { server_id: self.server_id.clone(), dir: self.dir.clone() })
                        .await;
                }
                return 0;
            }
        };
        let count = entries.len();
        // Names a probe is worth: matching files, newest first, not seen as
        // they are now.
        let mut wanted: Vec<&DirEntry> = entries
            .iter()
            .filter(|e| !e.is_dir && self.patterns.iter().any(|p| matches_pattern(p, &e.name)))
            .collect();
        wanted.sort_by_key(|e| std::cmp::Reverse(e.modified));
        wanted.retain(|e| probed.get(&e.name).is_none_or(|p| p.modified != e.modified || p.size != e.size));
        wanted.truncate(MAX_PROBES);
        if !wanted.is_empty() {
            let names: Vec<&str> = wanted.iter().map(|e| e.name.as_str()).collect();
            let inodes = read_inodes(&session, &self.dir, &names).await;
            let paths: Vec<String> = wanted.iter().map(|e| super::browse::join(&self.dir, &e.name)).collect();
            let archives: HashSet<String> =
                head.are_archives(&paths).await.into_iter().filter(|(_, a)| *a).map(|(p, _)| p).collect();
            for (entry, path) in wanted.iter().zip(&paths) {
                probed.insert(
                    entry.name.clone(),
                    Probe {
                        modified: entry.modified,
                        size: entry.size,
                        inode: inodes.get(&entry.name).copied(),
                        archive: archives.contains(path),
                    },
                );
            }
        }
        forget_missing(probed, &entries);
        let inodes = probed.iter().filter_map(|(n, p)| p.inode.map(|i| (n.clone(), i))).collect();
        let archives = probed.iter().filter(|(_, p)| p.archive).map(|(n, _)| n.clone()).collect();
        let _ = self
            .events
            .send(GlobEvent::Listing { server_id: self.server_id.clone(), dir: self.dir.clone(), entries, inodes, archives })
            .await;
        count
    }
}

struct Probe {
    modified: Option<u64>,
    size: u64,
    inode: Option<u64>,
    archive: bool,
}

fn matches_pattern(pattern: &str, name: &str) -> bool {
    if is_glob(pattern) { glob_match(pattern, name) } else { pattern == name }
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Inode of each name, one `stat` command per batch (GNU `stat -c`, else BSD
/// `stat -f`). Names it could not tell about are left out.
pub async fn read_inodes(session: &RemoteSession, dir: &str, names: &[&str]) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    for batch in names.chunks(STAT_BATCH) {
        let args: Vec<String> = batch.iter().map(|n| shell_quote(n)).collect();
        let args = args.join(" ");
        let command = format!(
            "cd {dir} 2>/dev/null && {{ stat -c '%i %n' -- {args} 2>/dev/null || stat -f '%i %N' {args} 2>/dev/null; }}",
            dir = shell_quote(dir),
        );
        let Ok(result) = session.exec(&command).await else { continue };
        out.extend(parse_inodes(&String::from_utf8_lossy(&result.stdout)));
    }
    out
}

/// Lines of `<inode> <name>`.
pub fn parse_inodes(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter_map(|line| {
            let (inode, name) = line.split_once(' ')?;
            Some((name.to_string(), inode.parse::<u64>().ok()?))
        })
        .collect()
}

/// Drops what was learned about names that are no longer in the folder.
fn forget_missing(probed: &mut HashMap<String, Probe>, entries: &[DirEntry]) {
    let present: HashSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    probed.retain(|name, _| present.contains(name.as_str()));
}

/// What a watcher keeps between scans.
struct Conn {
    session: Arc<RemoteSession>,
    sftp: SftpSession,
    head: Arc<HeadProbe>,
}

/// Asks remote files for their first bytes only: a raw SFTP session, so the
/// read size is ours (`File` would ask for a full packet).
pub struct HeadProbe {
    raw: RawSftpSession,
}

impl HeadProbe {
    pub async fn new<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(stream: S) -> Result<Self, String> {
        let raw = RawSftpSession::new(stream);
        raw.init().await.map_err(|e| e.to_string())?;
        Ok(Self { raw })
    }

    /// Whether the first bytes of each path say Archive or compressed
    /// (several files at a time). An empty or unreadable file is not one.
    pub async fn are_archives(self: &Arc<Self>, paths: &[String]) -> Vec<(String, bool)> {
        let mut out = Vec::with_capacity(paths.len());
        let mut tasks = tokio::task::JoinSet::new();
        let mut next = paths.iter();
        loop {
            while tasks.len() < PROBE_CONCURRENCY {
                let Some(path) = next.next() else { break };
                let (probe, path) = (self.clone(), path.clone());
                tasks.spawn(async move {
                    let archive = probe.is_archive(&path).await;
                    (path, archive)
                });
            }
            match tasks.join_next().await {
                Some(Ok(done)) => out.push(done),
                Some(Err(_)) => {}
                None => break,
            }
        }
        out
    }

    async fn is_archive(&self, path: &str) -> bool {
        let Ok(handle) = self.raw.open(path, OpenFlags::READ, Default::default()).await else { return false };
        let mut head: Vec<u8> = Vec::new();
        while head.len() < HEAD_LEN as usize {
            match self.raw.read(handle.handle.clone(), head.len() as u64, HEAD_LEN - head.len() as u32).await {
                Ok(data) if !data.data.is_empty() => head.extend_from_slice(&data.data),
                _ => break,
            }
        }
        let _ = self.raw.close(handle.handle).await;
        crate::archive::looks_like_archive(&head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inodes_parse_with_spaces_in_names() {
        let parsed = parse_inodes("123 app.log\n456 my file.log\nbad line\n");
        assert_eq!(parsed.get("app.log"), Some(&123));
        assert_eq!(parsed.get("my file.log"), Some(&456));
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn forgetting_vanished_names_keeps_the_present_ones() {
        let entry = |name: &str| DirEntry { name: name.into(), is_dir: false, size: 1, modified: Some(1) };
        let probe = || Probe { modified: Some(1), size: 1, inode: None, archive: false };
        let mut probed: HashMap<String, Probe> = ["a", "b", "c"].iter().map(|n| (n.to_string(), probe())).collect();
        forget_missing(&mut probed, &[entry("b"), entry("z")]);
        assert_eq!(probed.keys().collect::<Vec<_>>(), ["b"]);
    }

    #[test]
    fn big_folders_scan_slowly() {
        let five = Duration::from_secs(5);
        assert_eq!(scan_interval(five, 100), five);
        assert_eq!(scan_interval(five, 10_001), Duration::from_secs(30));
        assert_eq!(scan_interval(Duration::from_secs(60), 20_000), Duration::from_secs(60));
    }
}
