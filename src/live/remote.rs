//! `RemoteFileSource`: a file on a Server read over SFTP by byte offset.
//!
//! Opening shows the last ~1,000 lines at once, then older lines are read in
//! the background up to the Live Window. New lines come from polling every
//! 500 ms and reading from the remembered offset, so a lost connection
//! carries on exactly where it stopped. Rotation is told by the inode (read
//! with `stat` through exec); a smaller size is a truncation.

use super::backward::{BackwardLines, cut_before_start};
use super::line_time::newest_timestamp;
use super::source::{
    BreakKind, EarlierFile, EarlierStart, LineSource, ReadProgress, SourceCommand, SourceEvent, SourceHandle,
    SourceStatus,
};
use crate::workspace::log_space::line_hash;
use super::splitter::LineSplitter;
use crate::remote::browse::{dirname, join};
use crate::remote::compress::{AsyncBridge, Compression, decoder};
use crate::remote::rotated::{RotatedFile, list_rotated};
use crate::remote::starting_time::resolve_start;
use crate::remote::{ConnectError, RemoteHub, RemoteSession, ServerStore, shell_quote};
use async_channel::{Receiver, Sender};
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::StatusCode;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// How often the file is checked for new bytes.
pub const REMOTE_POLL_INTERVAL: Duration = Duration::from_millis(500);
/// How often a Gone file's path is looked at again.
const GONE_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Lines shown at once when a file opens.
pub const TAIL_LINES: usize = 1_000;
/// Lines of one batch of older lines.
pub const BACKFILL_BATCH: usize = 5_000;
const BLOCK: usize = 64 * 1024;
/// A batch of new bytes read at once.
const READ_CHUNK: usize = 1024 * 1024;
/// The real number of lines is given up on after this long.
pub const WC_TIMEOUT: Duration = Duration::from_secs(30);
/// Reconnecting stops after this long and waits for the user.
pub const GIVE_UP_AFTER: Duration = Duration::from_secs(300);

pub struct RemoteFileSource {
    pub hub: Arc<RemoteHub>,
    /// The Servers as they are now (edits apply to the next connection).
    pub servers: Arc<Mutex<ServerStore>>,
    pub server_id: String,
    pub path: String,
    /// `<server>:<path>`.
    pub name: String,
    pub window: usize,
    /// Starting Time as the user typed it (a span such as `1h` or a time):
    /// older lines are not read. `None` reads the last lines as usual.
    pub start_time: Option<String>,
    /// For a file brought by a Live Glob: the inodes that glob has read. The
    /// file goes Gone when it is deleted, and when its path comes to hold a
    /// file the glob already read under another name.
    pub glob_inodes: Option<GlobInodes>,
}

/// The inodes a Live Glob has read, each with the path it was read at.
pub type GlobInodes = Arc<Mutex<HashMap<u64, String>>>;

impl LineSource for RemoteFileSource {
    fn spawn(self) -> SourceHandle {
        let (event_tx, events) = async_channel::unbounded();
        let (commands, command_rx) = async_channel::unbounded();
        let hub = self.hub.clone();
        hub.spawn(async move { self.run(event_tx, command_rx).await });
        SourceHandle { events, commands }
    }
}

/// What a read of the file told about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStat {
    ino: Option<u64>,
    size: u64,
}

/// Why serving a session ended.
enum Served {
    Closed,
    /// The connection dropped.
    Lost,
    /// Cannot go on; the user must act.
    Stopped(SourceStatus),
}

/// A Load More the app asked for.
#[derive(Debug, Clone)]
struct LoadMoreRequest {
    generation: u32,
    end_offset: u64,
    lines: usize,
    first_hash: Option<u64>,
}

/// What the app asked for, kept until the source can act on it (it may be
/// disconnected or sleeping).
struct Ctl {
    window: usize,
    paused: bool,
    /// The app dropped what it held: stop reading older lines for good.
    clear: bool,
    load_more: Option<LoadMoreRequest>,
    /// The app asked for the next Rotated File: how many lines fit.
    load_earlier: Option<usize>,
    /// A Load More was dropped before it started: the app is told so.
    cancelled: bool,
}

/// What a command means for the loop that handles it.
enum Flow {
    Close,
    Reconnect,
    Go,
}

impl Ctl {
    fn new(window: usize) -> Self {
        Self { window, paused: false, clear: false, load_more: None, load_earlier: None, cancelled: false }
    }

    fn apply(&mut self, command: SourceCommand) -> Flow {
        match command {
            SourceCommand::Close => return Flow::Close,
            SourceCommand::Reconnect => return Flow::Reconnect,
            SourceCommand::Pause => self.paused = true,
            SourceCommand::Resume => self.paused = false,
            SourceCommand::SetWindow(n) => self.window = n,
            // For a Remote Command only.
            SourceCommand::Restart | SourceCommand::SetRestartOnExit(_) => {}
            SourceCommand::Clear => self.clear = true,
            SourceCommand::LoadEarlier { room } => self.load_earlier = Some(room),
            SourceCommand::CancelLoadMore => {
                if self.load_more.take().is_some() {
                    self.cancelled = true;
                }
            }
            SourceCommand::LoadMore { generation, end_offset, lines, first_hash } => {
                self.load_more = Some(LoadMoreRequest { generation, end_offset, lines, first_hash });
            }
        }
        Flow::Go
    }
}

/// What survives a lost connection.
#[derive(Default)]
struct Memory {
    /// Where reading continues.
    offset: u64,
    ino: Option<u64>,
    opened: bool,
    splitter: LineSplitter,
    backward: Option<BackwardLines>,
    /// Lines held in the app for this file.
    loaded: usize,
    /// Whether `stat` through exec works on this server.
    exec_stat: Option<bool>,
    /// Generation being read, counted like the app counts them.
    generation: u32,
    /// The user cleared the file: older lines are never read on their own.
    no_refill: bool,
    /// Inode of a file at the path that a Live Glob read already: the path
    /// stays Gone while it holds it.
    gone_ino: Option<u64>,
    /// Position in the Rotated Files (newest first) of the next one to load:
    /// the Generations above the file, however they came to be held.
    earlier_next: usize,
    /// Reading up from the current file to the Rotated Files was looked at
    /// once, when the file opened.
    auto_checked: bool,
    /// Lines of the current file are held up to its first byte.
    current_exhausted: bool,
    /// Reading stopped at the Starting Time.
    stop_reached: bool,
    /// The Starting Time, resolved on the clock of the file.
    stop_before: Option<f64>,
    /// The app was told the lines since the Starting Time do not fit.
    overflow_told: bool,
}

fn classify(e: &SftpError) -> Kind {
    match e {
        SftpError::Status(s) => match s.status_code {
            StatusCode::NoSuchFile => Kind::Missing,
            StatusCode::PermissionDenied => Kind::Denied,
            StatusCode::NoConnection | StatusCode::ConnectionLost => Kind::Lost,
            _ => Kind::Other(e.to_string()),
        },
        _ => Kind::Lost,
    }
}

enum Kind {
    Missing,
    Denied,
    Lost,
    Other(String),
}

impl RemoteFileSource {
    async fn run(self, events: Sender<SourceEvent>, commands: Receiver<SourceCommand>) {
        let mut mem = Memory::default();
        let mut ctl = Ctl::new(self.window);
        let mut backoff = Duration::from_secs(1);
        let mut failing_since: Option<Instant> = None;
        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
        loop {
            let store = self.servers.lock().unwrap().clone();
            match self.hub.session(&store, &self.server_id).await {
                Ok(session) => {
                    backoff = Duration::from_secs(1);
                    let was_down = failing_since.take().is_some();
                    match self
                        .serve(&session, &mut mem, &events, &commands, &mut ctl, was_down)
                        .await
                    {
                        Served::Closed => return,
                        Served::Lost => {
                            if mem.opened {
                                let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Disconnected });
                            }
                            let _ = events.try_send(SourceEvent::Status(SourceStatus::Reconnecting));
                            failing_since = Some(Instant::now());
                            self.hub.drop_session(&self.server_id).await;
                        }
                        Served::Stopped(status) => {
                            let _ = events.try_send(SourceEvent::Status(status));
                            if !self.wait_for_reconnect(&commands, &mut ctl).await {
                                return;
                            }
                            let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        }
                    }
                }
                Err(e) => {
                    let status = match &e {
                        ConnectError::NeedsSecret { .. } => Some(SourceStatus::NeedsSecret),
                        ConnectError::AuthFailed { .. } => Some(SourceStatus::AuthFailed),
                        ConnectError::Network(_) | ConnectError::Other(_) => None,
                        other => Some(SourceStatus::Error(other.message())),
                    };
                    if let Some(status) = status {
                        let _ = events.try_send(SourceEvent::Status(status));
                        if !self.wait_for_reconnect(&commands, &mut ctl).await {
                            return;
                        }
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        continue;
                    }
                    let since = *failing_since.get_or_insert_with(Instant::now);
                    if mem.opened {
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Reconnecting));
                    }
                    if since.elapsed() > GIVE_UP_AFTER {
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Disconnected));
                        failing_since = None;
                        backoff = Duration::from_secs(1);
                        if !self.wait_for_reconnect(&commands, &mut ctl).await {
                            return;
                        }
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        continue;
                    }
                }
            }
            // Wait before the next try, still hearing the app.
            if !self.sleep_with_commands(backoff, &commands, &mut ctl).await {
                return;
            }
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    /// Wait until the app says Reconnect. `false` when it closed the source.
    async fn wait_for_reconnect(&self, commands: &Receiver<SourceCommand>, ctl: &mut Ctl) -> bool {
        loop {
            match commands.recv().await {
                Ok(command) => match ctl.apply(command) {
                    Flow::Reconnect => return true,
                    Flow::Close => return false,
                    Flow::Go => {}
                },
                Err(_) => return false,
            }
        }
    }

    /// Sleep, handling commands as they come. `false` when closed; a
    /// Reconnect ends the sleep early.
    async fn sleep_with_commands(
        &self,
        duration: Duration,
        commands: &Receiver<SourceCommand>,
        ctl: &mut Ctl,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return true,
                cmd = commands.recv() => match cmd {
                    Ok(command) => match ctl.apply(command) {
                        Flow::Reconnect => return true,
                        Flow::Close => return false,
                        Flow::Go => {}
                    },
                    Err(_) => return false,
                },
            }
        }
    }

    /// Tell the Live Glob this file's inode has been read.
    fn remember_inode(&self, ino: Option<u64>) {
        if let (Some(known), Some(ino)) = (&self.glob_inodes, ino) {
            known.lock().unwrap().entry(ino).or_insert_with(|| self.path.clone());
        }
    }

    /// Size and inode of the file. `Err(Missing)` when it is not there.
    async fn stat(
        &self,
        session: &RemoteSession,
        sftp: &SftpSession,
        mem: &mut Memory,
    ) -> Result<FileStat, Kind> {
        if mem.exec_stat != Some(false) {
            let command = format!(
                "stat -c '%i %s' {q} 2>/dev/null || stat -f '%i %z' {q} 2>/dev/null",
                q = shell_quote(&self.path)
            );
            if let Ok(out) = session.exec(&command).await {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut parts = text.split_whitespace();
                if let (Some(ino), Some(size)) = (
                    parts.next().and_then(|p| p.parse::<u64>().ok()),
                    parts.next().and_then(|p| p.parse::<u64>().ok()),
                ) {
                    mem.exec_stat = Some(true);
                    return Ok(FileStat { ino: Some(ino), size });
                }
                if out.exit_status == Some(0) {
                    mem.exec_stat = Some(false);
                }
            }
        }
        // Without exec: the size only.
        match sftp.metadata(self.path.clone()).await {
            Ok(meta) => Ok(FileStat { ino: None, size: meta.size.unwrap_or(0) }),
            Err(e) => Err(classify(&e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn serve(
        &self,
        session: &Arc<RemoteSession>,
        mem: &mut Memory,
        events: &Sender<SourceEvent>,
        commands: &Receiver<SourceCommand>,
        ctl: &mut Ctl,
        was_down: bool,
    ) -> Served {
        let sftp = match session.open_sftp().await {
            Ok(sftp) => sftp,
            Err(_) if session.is_closed() => return Served::Lost,
            Err(_) => {
                return Served::Stopped(SourceStatus::Error(format!(
                    "SFTP not available on {}",
                    session.server.name
                )));
            }
        };
        let mut file = None;
        let mut announced_reconnect = !was_down || !mem.opened;
        let mut announced_live = false;
        let mut backfill_ready = mem.backward.is_some();
        // A rotation moved the Rotated Files: the button names another file.
        let mut reannounce = false;
        loop {
            // Commands first.
            loop {
                match commands.try_recv() {
                    Ok(command) => {
                        if let Flow::Close = ctl.apply(command) {
                            return Served::Closed;
                        }
                    }
                    Err(async_channel::TryRecvError::Empty) => break,
                    Err(async_channel::TryRecvError::Closed) => return Served::Closed,
                }
            }
            if events.is_closed() {
                return Served::Closed;
            }
            if ctl.clear {
                // What the app held is gone: no older line is read again
                // unless it asks (Load More).
                ctl.clear = false;
                mem.backward = None;
                mem.no_refill = true;
                mem.loaded = 0;
                backfill_ready = false;
            }
            if ctl.cancelled {
                ctl.cancelled = false;
                let _ = events.try_send(SourceEvent::Older {
                    generation: mem.generation,
                    lines: Vec::new(),
                    bytes: Vec::new(),
                    reached_start: false,
                });
            }
            if let Some(request) = ctl.load_more.take() {
                match file.as_mut() {
                    Some(handle) => match self.load_older(handle, &request, mem, events).await {
                        Ok(()) => {}
                        Err(Kind::Lost) => {
                            // Nothing was read: the app stops waiting and the
                            // lines it holds stay as they are.
                            let _ = events.try_send(SourceEvent::Older {
                                generation: request.generation,
                                lines: Vec::new(),
                                bytes: Vec::new(),
                                reached_start: false,
                            });
                            return Served::Lost;
                        }
                        Err(_) => {
                            let _ = events.try_send(SourceEvent::OlderUnavailable);
                        }
                    },
                    None => {
                        let _ = events.try_send(SourceEvent::OlderUnavailable);
                    }
                }
            }
            if let Some(room) = ctl.load_earlier.take() {
                match self.load_earlier(&sftp, session, mem, events, room, false).await {
                    Ok(()) => {}
                    Err(Kind::Lost) => return Served::Lost,
                    Err(Kind::Denied) => {
                        let _ = events.try_send(SourceEvent::Progress(None));
                        let _ = events.try_send(SourceEvent::EarlierAvailable(None));
                    }
                    Err(Kind::Missing | Kind::Other(_)) => {
                        let _ = events.try_send(SourceEvent::Progress(None));
                        let _ = events.try_send(SourceEvent::Problem("could not read the earlier generation".into()));
                        let _ = events.try_send(SourceEvent::EarlierAvailable(None));
                    }
                }
            }
            if ctl.paused {
                if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, ctl).await {
                    return Served::Closed;
                }
                continue;
            }

            // The file now.
            let stat = match self.stat(session, &sftp, mem).await {
                Ok(stat) => Some(stat),
                Err(Kind::Missing) => None,
                Err(Kind::Denied) => {
                    return Served::Stopped(SourceStatus::Error(format!("Permission denied: {}", self.name)));
                }
                Err(Kind::Lost) => return Served::Lost,
                Err(Kind::Other(e)) => return Served::Stopped(SourceStatus::Error(format!("{}: {e}", self.name))),
            };
            let Some(stat) = stat else {
                // Not there: gone, or not created yet.
                if file.take().is_some() || (mem.opened && mem.ino.is_some()) {
                    let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Deleted });
                    mem.generation += 1;
                    mem.ino = None;
                    mem.offset = 0;
                    mem.splitter.clear();
                    mem.backward = None;
                    mem.loaded = 0;
                }
                announced_live = false;
                let (status, wait) = if self.glob_inodes.is_some() {
                    (SourceStatus::Gone, GONE_POLL_INTERVAL)
                } else {
                    (SourceStatus::WaitingForFile, REMOTE_POLL_INTERVAL)
                };
                let _ = events.try_send(SourceEvent::Status(status));
                if !self.sleep_with_commands(wait, commands, ctl).await {
                    return Served::Closed;
                }
                continue;
            };

            // A file of a Live Glob whose path holds a file the glob read
            // under another name is Gone: it is not read again.
            if let (Some(known), Some(ino)) = (&self.glob_inodes, stat.ino) {
                let others = known.lock().unwrap().get(&ino).is_some_and(|path| *path != self.path);
                if others || mem.gone_ino == Some(ino) {
                    if mem.opened && mem.gone_ino != Some(ino) {
                        // The rest of the old file, then the cut.
                        if let Some(old) = file.as_mut() {
                            if let Err(Kind::Lost) = self.read_new(old, u64::MAX, mem, events).await {
                                return Served::Lost;
                            }
                        }
                        file = None;
                        let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Deleted });
                        mem.generation += 1;
                        mem.ino = None;
                        mem.offset = 0;
                        mem.splitter.clear();
                        mem.backward = None;
                        mem.loaded = 0;
                    }
                    mem.gone_ino = Some(ino);
                    announced_live = false;
                    let _ = events.try_send(SourceEvent::Status(SourceStatus::Gone));
                    if !self.sleep_with_commands(GONE_POLL_INTERVAL, commands, ctl).await {
                        return Served::Closed;
                    }
                    continue;
                }
            }
            mem.gone_ino = None;

            // First sight of the file in this run: show its end at once.
            if !mem.opened {
                match self.open_tail(&sftp, session, stat, mem, events, ctl.window).await {
                    Ok(handle) => file = Some(handle),
                    Err(Kind::Denied) => {
                        return Served::Stopped(SourceStatus::Error(format!("Permission denied: {}", self.name)));
                    }
                    Err(Kind::Lost) => return Served::Lost,
                    Err(Kind::Missing) => continue,
                    Err(Kind::Other(e)) => {
                        return Served::Stopped(SourceStatus::Error(format!("{}: {e}", self.name)));
                    }
                }
                mem.ino = stat.ino;
                self.remember_inode(stat.ino);
                let _ = events.try_send(SourceEvent::Inode(stat.ino));
                mem.opened = true;
                backfill_ready = true;
            } else {
                // Same file, or a new one?
                let rotated = matches!((mem.ino, stat.ino), (Some(a), Some(b)) if a != b);
                let truncated = !rotated && stat.size < mem.offset;
                if rotated || truncated || file.is_none() {
                    if rotated {
                        // The rest of the old file, from the handle still open.
                        if let Some(old) = file.as_mut() {
                            if let Err(Kind::Lost) = self.read_new(old, u64::MAX, mem, events).await {
                                return Served::Lost;
                            }
                        }
                        mem.splitter.clear();
                        let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Rotated });
                        mem.generation += 1;
                        mem.earlier_next += 1;
                        reannounce = true;
                        mem.offset = 0;
                        mem.backward = None;
                        mem.loaded = 0;
                    } else if truncated {
                        mem.splitter.clear();
                        let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Truncated });
                        mem.generation += 1;
                        mem.earlier_next += 1;
                        reannounce = true;
                        mem.offset = 0;
                        mem.backward = None;
                        mem.loaded = 0;
                    } else if mem.ino.is_none() && mem.offset == 0 && mem.loaded == 0 {
                        // The file appeared after waiting: a new Generation.
                        mem.splitter.clear();
                    }
                    match sftp.open(self.path.clone()).await {
                        Ok(handle) => file = Some(handle),
                        Err(e) => match classify(&e) {
                            Kind::Missing => continue,
                            Kind::Denied => {
                                return Served::Stopped(SourceStatus::Error(format!(
                                    "Permission denied: {}",
                                    self.name
                                )));
                            }
                            Kind::Lost => return Served::Lost,
                            Kind::Other(e) => {
                                return Served::Stopped(SourceStatus::Error(format!("{}: {e}", self.name)));
                            }
                        },
                    }
                    mem.ino = stat.ino;
                    self.remember_inode(stat.ino);
                    let _ = events.try_send(SourceEvent::Inode(stat.ino));
                }
            }
            if !announced_reconnect {
                announced_reconnect = true;
                let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Reconnected });
            }
            if !announced_live {
                announced_live = true;
                let _ = events.try_send(SourceEvent::Status(SourceStatus::Live));
            }

            // New bytes.
            if let Some(handle) = file.as_mut() {
                if stat.size > mem.offset {
                    match self.read_new(handle, stat.size, mem, events).await {
                        Ok(()) => {}
                        Err(Kind::Lost) => return Served::Lost,
                        Err(Kind::Denied) => {
                            return Served::Stopped(SourceStatus::Error(format!("Permission denied: {}", self.name)));
                        }
                        Err(_) => {}
                    }
                }
            }

            // Older lines, a batch per turn, until the Live Window is full.
            if backfill_ready {
                if let (Some(handle), Some(backward)) = (file.as_mut(), mem.backward.as_mut()) {
                    if mem.loaded < ctl.window && !backward.is_done() {
                        let want = (ctl.window - mem.loaded).min(BACKFILL_BATCH);
                        match read_backward(handle, backward, want).await {
                            Ok((lines, bytes)) if !lines.is_empty() => {
                                mem.loaded += lines.len();
                                mem.stop_reached |= backward.reached_start_time();
                                mem.current_exhausted |= backward.is_done() && !backward.reached_start_time();
                                let _ = events.try_send(SourceEvent::Backfill { lines, generation: 0, bytes });
                                // Keep going without waiting for the next poll.
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Ok(_) => {
                                mem.stop_reached |= backward.reached_start_time();
                                mem.current_exhausted |= backward.is_done() && !backward.reached_start_time();
                                mem.backward = None;
                            }
                            Err(Kind::Lost) => return Served::Lost,
                            Err(_) => mem.backward = None,
                        }
                    } else if mem.loaded >= ctl.window || backward.is_done() {
                        mem.stop_reached |= backward.reached_start_time();
                        mem.current_exhausted |= backward.is_done() && !backward.reached_start_time();
                        mem.backward = None;
                    }
                }
            }

            // The Live Window filled before the Starting Time was reached.
            if let (Some(time), true) = (mem.stop_before, mem.opened) {
                if !mem.overflow_told && mem.loaded >= ctl.window && !mem.stop_reached && mem.backward.is_none() {
                    mem.overflow_told = true;
                    let lines = self.count_since(&sftp, session, stat.size, time).await;
                    let since = self.start_time.clone().unwrap_or_default();
                    let _ = events.try_send(SourceEvent::StartTimeOverflow { lines, since: since.trim().to_string() });
                }
            }

            // The file is held up to its first byte: offer the Rotated Files,
            // and read into the newest one when it is plain and there is room.
            if mem.opened && mem.current_exhausted && !mem.auto_checked && !mem.no_refill && !mem.stop_reached {
                mem.auto_checked = true;
                let room = ctl.window.saturating_sub(mem.loaded);
                let result = if room > 0 {
                    self.load_earlier(&sftp, session, mem, events, room, true).await
                } else {
                    self.announce_earlier(&sftp, mem, events).await
                };
                match result {
                    Ok(()) => {}
                    Err(Kind::Lost) => return Served::Lost,
                    Err(_) => {
                        let _ = events.try_send(SourceEvent::Progress(None));
                    }
                }
            } else if reannounce && mem.auto_checked {
                reannounce = false;
                if let Err(Kind::Lost) = self.announce_earlier(&sftp, mem, events).await {
                    return Served::Lost;
                }
            }

            if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, ctl).await {
                return Served::Closed;
            }
        }
    }

    /// How many lines the file has since `time`: the byte where it starts is
    /// searched by timestamp, then the lines after it are counted on the
    /// Server. `None` when either fails or takes too long.
    async fn count_since(
        &self,
        sftp: &SftpSession,
        session: &Arc<RemoteSession>,
        size: u64,
        time: f64,
    ) -> Option<usize> {
        let file = sftp.open(self.path.clone()).await.ok()?;
        let handle = tokio::runtime::Handle::current();
        let offset = tokio::task::spawn_blocking(move || {
            let mut src = crate::remote::full_read::SftpReadAt { handle, file };
            crate::remote::full_read::find_start_offset(&mut src, size, time)
        })
        .await
        .ok()?
        .ok()?;
        let command = format!("tail -c +{} -- {} | wc -l", offset + 1, shell_quote(&self.path));
        let out = tokio::time::timeout(WC_TIMEOUT, session.exec(&command)).await.ok()?.ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    /// Tell the app which Rotated File would be loaded next, if any.
    async fn announce_earlier(
        &self,
        sftp: &SftpSession,
        mem: &Memory,
        events: &Sender<SourceEvent>,
    ) -> Result<(), Kind> {
        let list = list_rotated(sftp, &self.path).await.map_err(|_| Kind::Lost)?;
        let next = list.get(mem.earlier_next).map(|f| EarlierFile {
            name: f.name.clone(),
            size: f.size,
            compressed: f.compression.is_compressed(),
        });
        let _ = events.try_send(SourceEvent::EarlierAvailable(next));
        Ok(())
    }

    /// Load the next Rotated File above the lines held, at most `room`
    /// lines. `auto`: it was not asked for, so only a plain file is read.
    async fn load_earlier(
        &self,
        sftp: &SftpSession,
        session: &Arc<RemoteSession>,
        mem: &mut Memory,
        events: &Sender<SourceEvent>,
        room: usize,
        auto: bool,
    ) -> Result<(), Kind> {
        let list = match list_rotated(sftp, &self.path).await {
            Ok(list) => list,
            Err(_) => return Err(Kind::Lost),
        };
        let Some(target) = list.get(mem.earlier_next).cloned() else {
            let _ = events.try_send(SourceEvent::EarlierAvailable(None));
            return Ok(());
        };
        if !target.compression.is_supported() {
            let _ = events.try_send(SourceEvent::Problem(format!("{}: compression not supported", target.name)));
            let _ = events.try_send(SourceEvent::EarlierAvailable(None));
            return Ok(());
        }
        if auto && target.compression.is_compressed() {
            let _ = events.try_send(SourceEvent::EarlierAvailable(Some(EarlierFile {
                name: target.name.clone(),
                size: target.size,
                compressed: true,
            })));
            return Ok(());
        }
        let path = join(&dirname(&self.path), &target.name);
        let room = room.max(1);
        let read = if target.compression.is_compressed() {
            self.read_compressed_earlier(sftp, &path, &target, room, mem.stop_before, events).await?
        } else {
            self.read_plain_earlier(sftp, session, &path, &target, room, mem.stop_before, events).await?
        };
        mem.stop_reached |= read.stopped_at_time;
        if read.lines == 0 {
            let _ = events.try_send(SourceEvent::EarlierAvailable(None));
            return Ok(());
        }
        mem.earlier_next += 1;
        mem.generation += 1;
        mem.loaded += read.lines;
        if read.reached_start && !read.stopped_at_time {
            // Held up to the first line of this file: the next one is offered.
            self.announce_earlier(sftp, mem, events).await?;
        } else {
            let _ = events.try_send(SourceEvent::EarlierAvailable(None));
        }
        Ok(())
    }

    /// A plain Rotated File is read from its end like the current file.
    #[allow(clippy::too_many_arguments)]
    async fn read_plain_earlier(
        &self,
        sftp: &SftpSession,
        session: &Arc<RemoteSession>,
        path: &str,
        target: &RotatedFile,
        room: usize,
        stop_before: Option<f64>,
        events: &Sender<SourceEvent>,
    ) -> Result<EarlierRead, Kind> {
        let mut file = sftp.open(path.to_string()).await.map_err(|e| classify(&e))?;
        // Its real number of lines first, so the numbers are right at once.
        let command = format!("wc -l < {}", shell_quote(path));
        let counted = tokio::time::timeout(WC_TIMEOUT, session.exec(&command)).await;
        let total = counted
            .ok()
            .and_then(|r| r.ok())
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<usize>().ok());
        let mut backward = BackwardLines::new(target.size, BLOCK).with_stop_before(stop_before);
        let mut left = room;
        let mut sent = 0usize;
        let mut start = Some(EarlierStart { name: target.name.clone(), modified: modified_text(target.modified) });
        loop {
            let want = left.min(BACKFILL_BATCH);
            let (lines, bytes) = read_backward(&mut file, &mut backward, want).await?;
            let done = backward.is_done();
            if lines.is_empty() && sent == 0 {
                return Ok(EarlierRead { lines: 0, reached_start: done, stopped_at_time: backward.reached_start_time() });
            }
            if lines.is_empty() {
                break;
            }
            left -= lines.len();
            sent += lines.len();
            let last_line_no = start.as_ref().and(total);
            let _ = events.try_send(SourceEvent::Earlier {
                start: start.take(),
                lines,
                bytes,
                last_line_no,
                reached_start: done,
            });
            if left == 0 || done {
                break;
            }
            tokio::task::yield_now().await;
        }
        Ok(EarlierRead {
            lines: sent,
            reached_start: backward.is_done(),
            stopped_at_time: backward.reached_start_time(),
        })
    }

    /// A compressed Rotated File is decoded from its start on a blocking
    /// thread; only the last `room` lines are kept.
    async fn read_compressed_earlier(
        &self,
        sftp: &SftpSession,
        path: &str,
        target: &RotatedFile,
        room: usize,
        stop_before: Option<f64>,
        events: &Sender<SourceEvent>,
    ) -> Result<EarlierRead, Kind> {
        let file = sftp.open(path.to_string()).await.map_err(|e| classify(&e))?;
        let handle = tokio::runtime::Handle::current();
        let (kind, size, what, progress_events) =
            (target.compression, target.size, target.name.clone(), events.clone());
        let done = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counter = done.clone();
        let job = tokio::task::spawn_blocking(move || -> std::io::Result<(VecDeque<(String, u32)>, usize)> {
            use std::io::BufRead;
            let bridge = CountRead { inner: AsyncBridge::buffered(handle, file), count: counter.clone() };
            let mut reader = std::io::BufReader::with_capacity(256 * 1024, decoder(kind, bridge)?);
            let mut ring: VecDeque<(String, u32)> = VecDeque::new();
            let mut total = 0usize;
            let mut buf = Vec::new();
            let mut last_report = Instant::now() - Duration::from_secs(1);
            loop {
                buf.clear();
                let n = reader.read_until(b'\n', &mut buf)?;
                if n == 0 {
                    break;
                }
                let text = buf.strip_suffix(b"\n").unwrap_or(&buf);
                ring.push_back((super::splitter::decode_line(text), n as u32));
                if ring.len() > room {
                    ring.pop_front();
                }
                total += 1;
                if last_report.elapsed() > Duration::from_millis(250) {
                    last_report = Instant::now();
                    if progress_events.is_closed() {
                        return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "closed"));
                    }
                    let _ = progress_events.try_send(SourceEvent::Progress(Some(ReadProgress {
                        what: what.clone(),
                        done: counter.load(std::sync::atomic::Ordering::Relaxed),
                        total: Some(size),
                    })));
                }
            }
            Ok((ring, total))
        });
        let _ = events.try_send(SourceEvent::Progress(Some(ReadProgress {
            what: target.name.clone(),
            done: 0,
            total: Some(target.size),
        })));
        let outcome = job.await;
        let _ = events.try_send(SourceEvent::Progress(None));
        let (ring, total) = match outcome {
            Ok(Ok(read)) => read,
            Ok(Err(e)) => return Err(Kind::Other(e.to_string())),
            Err(e) => return Err(Kind::Other(e.to_string())),
        };
        let dropped = total - ring.len();
        let (lines, bytes): (Vec<String>, Vec<u32>) = ring.into_iter().unzip();
        let (lines, bytes, stopped) = match stop_before {
            Some(time) => cut_before_start(lines, bytes, time),
            None => (lines, bytes, false),
        };
        let kept = lines.len();
        if kept > 0 {
            let _ = events.try_send(SourceEvent::Earlier {
                start: Some(EarlierStart { name: target.name.clone(), modified: modified_text(target.modified) }),
                lines,
                bytes,
                last_line_no: Some(total),
                reached_start: dropped == 0,
            });
        }
        Ok(EarlierRead { lines: kept, reached_start: dropped == 0, stopped_at_time: stopped })
    }

    /// Open the file and send its last lines; start counting its lines.
    async fn open_tail(
        &self,
        sftp: &SftpSession,
        session: &Arc<RemoteSession>,
        stat: FileStat,
        mem: &mut Memory,
        events: &Sender<SourceEvent>,
        window: usize,
    ) -> Result<russh_sftp::client::fs::File, Kind> {
        let mut file = sftp.open(self.path.clone()).await.map_err(|e| classify(&e))?;
        let mut backward = BackwardLines::new(stat.size, BLOCK);
        let (mut first, mut bytes) = read_backward(&mut file, &mut backward, TAIL_LINES.min(window)).await?;
        if let Some(text) = &self.start_time {
            let newest = newest_timestamp(first.iter().map(String::as_str));
            match resolve_start(text, newest) {
                Ok(Some(time)) => {
                    mem.stop_before = Some(time);
                    (first, bytes) = backward.cut_batch_at(time, first, bytes);
                    mem.stop_reached |= backward.reached_start_time();
                    if first.is_empty() {
                        let _ = events.try_send(SourceEvent::Problem(format!("no lines since {}", text.trim())));
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = events.try_send(SourceEvent::Problem(format!("Starting Time: {e}")));
                }
            }
        }
        mem.offset = backward.follow_offset();
        mem.loaded = first.len();
        mem.splitter.clear();
        let _ = events.try_send(SourceEvent::Tail {
            lines: first,
            generation: 0,
            bytes,
            end_offset: mem.offset,
        });
        // A cleared file never reads older lines on its own.
        mem.backward = if mem.no_refill { None } else { Some(backward) };
        // The real number of lines, in the background.
        let (session, path, follow, events) = (session.clone(), self.path.clone(), mem.offset, events.clone());
        tokio::spawn(async move {
            let command = format!("head -c {follow} -- {} | wc -l", shell_quote(&path));
            let counted = tokio::time::timeout(WC_TIMEOUT, session.exec(&command)).await;
            let total = counted
                .ok()
                .and_then(|r| r.ok())
                .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<usize>().ok());
            let _ = events.try_send(SourceEvent::TotalLines(total));
        });
        Ok(file)
    }

    /// Read from `mem.offset` up to `limit` and send the finished lines.
    async fn read_new(
        &self,
        file: &mut russh_sftp::client::fs::File,
        limit: u64,
        mem: &mut Memory,
        events: &Sender<SourceEvent>,
    ) -> Result<(), Kind> {
        file.seek(std::io::SeekFrom::Start(mem.offset)).await.map_err(|_| Kind::Lost)?;
        let mut lines = Vec::new();
        let mut bytes = Vec::new();
        let mut buf = vec![0u8; READ_CHUNK];
        while mem.offset < limit {
            let want = READ_CHUNK.min((limit - mem.offset).min(READ_CHUNK as u64) as usize);
            let n = file.read(&mut buf[..want]).await.map_err(|_| Kind::Lost)?;
            if n == 0 {
                break;
            }
            mem.offset += n as u64;
            let (new_lines, new_bytes) = mem.splitter.push_with_lens(&buf[..n]);
            lines.extend(new_lines);
            bytes.extend(new_bytes);
        }
        mem.loaded += lines.len();
        if !lines.is_empty() {
            let _ = events.try_send(SourceEvent::Append { lines, bytes, streams: None });
        }
        Ok(())
    }

    /// Load More: the lines above `end_offset`, read backwards. The line at
    /// `end_offset` has to be the one the app holds, or the file changed.
    async fn load_older(
        &self,
        file: &mut russh_sftp::client::fs::File,
        request: &LoadMoreRequest,
        mem: &Memory,
        events: &Sender<SourceEvent>,
    ) -> Result<(), Kind> {
        if request.generation != mem.generation {
            let _ = events.try_send(SourceEvent::OlderUnavailable);
            return Ok(());
        }
        if let Some(expected) = request.first_hash {
            let found = read_line_at(file, request.end_offset).await?;
            if found.map(|l| line_hash(&l)) != Some(expected) {
                let _ = events.try_send(SourceEvent::OlderUnavailable);
                return Ok(());
            }
        }
        let mut backward = BackwardLines::new(request.end_offset, BLOCK);
        let (lines, bytes) = read_backward(file, &mut backward, request.lines).await?;
        let reached_start = backward.is_done();
        let _ = events.try_send(SourceEvent::Older {
            generation: request.generation,
            lines,
            bytes,
            reached_start,
        });
        Ok(())
    }
}

/// What loading a Rotated File gave.
struct EarlierRead {
    lines: usize,
    /// The lines reach the first line of the Rotated File.
    reached_start: bool,
    stopped_at_time: bool,
}

/// `YYYY-MM-DD HH:MM:SS` for a modified time in seconds since the epoch.
fn modified_text(secs: Option<u64>) -> Option<String> {
    let secs = i64::try_from(secs?).ok()?;
    chrono::DateTime::from_timestamp(secs, 0).map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// Counts the bytes that pass through, for the progress of a decode.
struct CountRead<R> {
    inner: R,
    count: Arc<std::sync::atomic::AtomicU64>,
}

impl<R: std::io::Read> std::io::Read for CountRead<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(out)?;
        self.count.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
        Ok(n)
    }
}

/// The line that starts at `offset` (without its line end), when it is whole.
async fn read_line_at(file: &mut russh_sftp::client::fs::File, offset: u64) -> Result<Option<String>, Kind> {
    const CAP: usize = 4 * 1024 * 1024;
    file.seek(std::io::SeekFrom::Start(offset)).await.map_err(|_| Kind::Lost)?;
    let mut data = Vec::new();
    let mut block = vec![0u8; BLOCK];
    while data.len() < CAP {
        let n = file.read(&mut block).await.map_err(|_| Kind::Lost)?;
        if n == 0 {
            return Ok(None);
        }
        data.extend_from_slice(&block[..n]);
        if let Some(end) = data.iter().position(|b| *b == b'\n') {
            return Ok(Some(super::splitter::decode_line(&data[..end])));
        }
    }
    Ok(None)
}

/// `max_lines` lines older than what `backward` gave before.
async fn read_backward(
    file: &mut russh_sftp::client::fs::File,
    backward: &mut BackwardLines,
    max_lines: usize,
) -> Result<(Vec<String>, Vec<u32>), Kind> {
    while let Some((offset, len)) = backward.want_block(max_lines) {
        file.seek(std::io::SeekFrom::Start(offset)).await.map_err(|_| Kind::Lost)?;
        let mut block = vec![0u8; len];
        let mut done = 0;
        while done < len {
            let n = file.read(&mut block[done..]).await.map_err(|_| Kind::Lost)?;
            if n == 0 {
                break;
            }
            done += n;
        }
        block.truncate(done);
        let short = done < len;
        backward.absorb_block(block);
        if short {
            break;
        }
    }
    Ok(backward.take_batch_with_lens(max_lines))
}
