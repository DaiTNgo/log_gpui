//! `RemoteFileSource`: a file on a Server read over SFTP by byte offset.
//!
//! Opening shows the last ~1,000 lines at once, then older lines are read in
//! the background up to the Live Window. New lines come from polling every
//! 500 ms and reading from the remembered offset, so a lost connection
//! carries on exactly where it stopped. Rotation is told by the inode (read
//! with `stat` through exec); a smaller size is a truncation.

use super::backward::BackwardLines;
use super::source::{
    BreakKind, LineSource, SourceCommand, SourceEvent, SourceHandle, SourceStatus,
};
use crate::workspace::log_space::line_hash;
use super::splitter::LineSplitter;
use crate::remote::{ConnectError, RemoteHub, RemoteSession, ServerStore};
use async_channel::{Receiver, Sender};
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::StatusCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// How often the file is checked for new bytes.
pub const REMOTE_POLL_INTERVAL: Duration = Duration::from_millis(500);
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
}

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
        Self { window, paused: false, clear: false, load_more: None, cancelled: false }
    }

    fn apply(&mut self, command: SourceCommand) -> Flow {
        match command {
            SourceCommand::Close => return Flow::Close,
            SourceCommand::Reconnect => return Flow::Reconnect,
            SourceCommand::Pause => self.paused = true,
            SourceCommand::Resume => self.paused = false,
            SourceCommand::SetWindow(n) => self.window = n,
            SourceCommand::Clear => self.clear = true,
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

fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
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
                let _ = events.try_send(SourceEvent::Status(SourceStatus::WaitingForFile));
                if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, ctl).await {
                    return Served::Closed;
                }
                continue;
            };

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
                        mem.offset = 0;
                        mem.backward = None;
                        mem.loaded = 0;
                    } else if truncated {
                        mem.splitter.clear();
                        let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Truncated });
                        mem.generation += 1;
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
                                let _ = events.try_send(SourceEvent::Backfill { lines, generation: 0, bytes });
                                // Keep going without waiting for the next poll.
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Ok(_) => mem.backward = None,
                            Err(Kind::Lost) => return Served::Lost,
                            Err(_) => mem.backward = None,
                        }
                    } else if mem.loaded >= ctl.window || backward.is_done() {
                        mem.backward = None;
                    }
                }
            }

            if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, ctl).await {
                return Served::Closed;
            }
        }
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
        let (first, bytes) = read_backward(&mut file, &mut backward, TAIL_LINES.min(window)).await?;
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
            let _ = events.try_send(SourceEvent::Append { lines, bytes });
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
