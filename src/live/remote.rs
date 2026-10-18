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
        let mut window = self.window;
        let mut paused = false;
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
                        .serve(&session, &mut mem, &events, &commands, &mut window, &mut paused, was_down)
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
                            if !self.wait_for_reconnect(&commands, &mut window, &mut paused).await {
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
                        if !self.wait_for_reconnect(&commands, &mut window, &mut paused).await {
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
                        if !self.wait_for_reconnect(&commands, &mut window, &mut paused).await {
                            return;
                        }
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        continue;
                    }
                }
            }
            // Wait before the next try, still hearing the app.
            if !self.sleep_with_commands(backoff, &commands, &mut window, &mut paused).await {
                return;
            }
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    /// Wait until the app says Reconnect. `false` when it closed the source.
    async fn wait_for_reconnect(
        &self,
        commands: &Receiver<SourceCommand>,
        window: &mut usize,
        paused: &mut bool,
    ) -> bool {
        loop {
            match commands.recv().await {
                Ok(SourceCommand::Reconnect) => return true,
                Ok(SourceCommand::Close) | Err(_) => return false,
                Ok(SourceCommand::Pause) => *paused = true,
                Ok(SourceCommand::Resume) => *paused = false,
                Ok(SourceCommand::SetWindow(n)) => *window = n,
            }
        }
    }

    /// Sleep, handling commands as they come. `false` when closed; a
    /// Reconnect ends the sleep early.
    async fn sleep_with_commands(
        &self,
        duration: Duration,
        commands: &Receiver<SourceCommand>,
        window: &mut usize,
        paused: &mut bool,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return true,
                cmd = commands.recv() => match cmd {
                    Ok(SourceCommand::Close) | Err(_) => return false,
                    Ok(SourceCommand::Reconnect) => return true,
                    Ok(SourceCommand::Pause) => *paused = true,
                    Ok(SourceCommand::Resume) => *paused = false,
                    Ok(SourceCommand::SetWindow(n)) => *window = n,
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
        window: &mut usize,
        paused: &mut bool,
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
                    Ok(SourceCommand::Close) => return Served::Closed,
                    Ok(SourceCommand::Pause) => *paused = true,
                    Ok(SourceCommand::Resume) => *paused = false,
                    Ok(SourceCommand::SetWindow(n)) => *window = n,
                    Ok(SourceCommand::Reconnect) => {}
                    Err(async_channel::TryRecvError::Empty) => break,
                    Err(async_channel::TryRecvError::Closed) => return Served::Closed,
                }
            }
            if events.is_closed() {
                return Served::Closed;
            }
            if *paused {
                if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, window, paused).await {
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
                    mem.ino = None;
                    mem.offset = 0;
                    mem.splitter.clear();
                    mem.backward = None;
                    mem.loaded = 0;
                }
                announced_live = false;
                let _ = events.try_send(SourceEvent::Status(SourceStatus::WaitingForFile));
                if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, window, paused).await {
                    return Served::Closed;
                }
                continue;
            };

            // First sight of the file in this run: show its end at once.
            if !mem.opened {
                match self.open_tail(&sftp, session, stat, mem, events, *window).await {
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
                        mem.offset = 0;
                        mem.backward = None;
                        mem.loaded = 0;
                    } else if truncated {
                        mem.splitter.clear();
                        let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Truncated });
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
                    if mem.loaded < *window && !backward.is_done() {
                        let want = (*window - mem.loaded).min(BACKFILL_BATCH);
                        match read_backward(handle, backward, want).await {
                            Ok(lines) if !lines.is_empty() => {
                                mem.loaded += lines.len();
                                let _ = events.try_send(SourceEvent::Backfill { lines, generation: 0 });
                                // Keep going without waiting for the next poll.
                                tokio::task::yield_now().await;
                                continue;
                            }
                            Ok(_) => mem.backward = None,
                            Err(Kind::Lost) => return Served::Lost,
                            Err(_) => mem.backward = None,
                        }
                    } else if mem.loaded >= *window || backward.is_done() {
                        mem.backward = None;
                    }
                }
            }

            if !self.sleep_with_commands(REMOTE_POLL_INTERVAL, commands, window, paused).await {
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
        let first = read_backward(&mut file, &mut backward, TAIL_LINES.min(window)).await?;
        mem.offset = backward.follow_offset();
        mem.loaded = first.len();
        mem.splitter.clear();
        let _ = events.try_send(SourceEvent::Tail { lines: first, generation: 0 });
        mem.backward = Some(backward);
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
        let mut buf = vec![0u8; READ_CHUNK];
        while mem.offset < limit {
            let want = READ_CHUNK.min((limit - mem.offset).min(READ_CHUNK as u64) as usize);
            let n = file.read(&mut buf[..want]).await.map_err(|_| Kind::Lost)?;
            if n == 0 {
                break;
            }
            mem.offset += n as u64;
            lines.extend(mem.splitter.push(&buf[..n]));
        }
        mem.loaded += lines.len();
        if !lines.is_empty() {
            let _ = events.try_send(SourceEvent::Append { lines });
        }
        Ok(())
    }
}

/// `max_lines` lines older than what `backward` gave before.
async fn read_backward(
    file: &mut russh_sftp::client::fs::File,
    backward: &mut BackwardLines,
    max_lines: usize,
) -> Result<Vec<String>, Kind> {
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
    Ok(backward.take_batch(max_lines))
}
