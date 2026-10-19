use super::source::{
    BreakKind, LineSource, SourceCommand, SourceEvent, SourceHandle, SourceStatus,
    LOCAL_POLL_INTERVAL,
};
use super::splitter::LineSplitter;
use async_channel::{Receiver, Sender};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

/// Follows a local file: checks its metadata every 250 ms and reads from the
/// byte where it stopped. Detects rotation (another inode), truncation (a
/// smaller size) and deletion (the file is gone, then waits for it).
pub struct LocalFileSource {
    path: PathBuf,
    /// Where reading starts: the end of what the app has already loaded.
    start_offset: u64,
    poll: Duration,
}

impl LocalFileSource {
    pub fn new(path: impl Into<PathBuf>, start_offset: u64) -> Self {
        Self { path: path.into(), start_offset, poll: LOCAL_POLL_INTERVAL }
    }

    pub fn with_poll_interval(mut self, poll: Duration) -> Self {
        self.poll = poll;
        self
    }
}

impl LineSource for LocalFileSource {
    fn spawn(self) -> SourceHandle {
        let (event_tx, events) = async_channel::unbounded();
        let (commands, command_rx) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("local-file-source".into())
            .spawn(move || self.run(event_tx, command_rx))
            .expect("spawn local file source");
        SourceHandle { events, commands }
    }
}

struct Open {
    file: File,
    ino: u64,
    offset: u64,
}

/// Identity of the file behind a path: its inode on Unix. Windows has no
/// stable file index in std, so there the creation time stands in for it.
#[cfg(unix)]
fn file_id(meta: &std::fs::Metadata) -> u64 {
    std::os::unix::fs::MetadataExt::ino(meta)
}

#[cfg(not(unix))]
fn file_id(meta: &std::fs::Metadata) -> u64 {
    meta.created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

impl LocalFileSource {
    fn run(self, events: Sender<SourceEvent>, commands: Receiver<SourceCommand>) {
        let mut splitter = LineSplitter::new();
        let mut open: Option<Open> = None;
        let mut waiting = false;
        let mut paused = false;
        let mut first = true;

        // The file as the app loaded it.
        match File::open(&self.path).and_then(|f| f.metadata().map(|m| (f, m))) {
            Ok((file, meta)) => {
                open = Some(Open { file, ino: file_id(&meta), offset: self.start_offset.min(meta.len()) });
                let _ = events.try_send(SourceEvent::Status(SourceStatus::Live));
            }
            Err(_) => {
                waiting = true;
                let _ = events.try_send(SourceEvent::Status(SourceStatus::WaitingForFile));
            }
        }

        loop {
            loop {
                match commands.try_recv() {
                    Ok(SourceCommand::Close) => return,
                    Ok(SourceCommand::Pause) => paused = true,
                    Ok(SourceCommand::Resume) => paused = false,
                    Ok(SourceCommand::SetWindow(_))
                | Ok(SourceCommand::Reconnect)
                | Ok(SourceCommand::Clear)
                | Ok(SourceCommand::CancelLoadMore)
                | Ok(SourceCommand::LoadMore { .. })
                | Ok(SourceCommand::Restart)
                | Ok(SourceCommand::SetRestartOnExit(_)) => {}
                    Err(async_channel::TryRecvError::Empty) => break,
                    Err(async_channel::TryRecvError::Closed) => return,
                }
            }
            if events.is_closed() {
                return;
            }
            if !paused {
                self.poll_once(&events, &mut open, &mut splitter, &mut waiting, &mut first);
            }
            std::thread::sleep(self.poll);
        }
    }

    fn poll_once(
        &self,
        events: &Sender<SourceEvent>,
        open: &mut Option<Open>,
        splitter: &mut LineSplitter,
        waiting: &mut bool,
        _first: &mut bool,
    ) {
        let send = |event| {
            let _ = events.try_send(event);
        };
        let meta = std::fs::metadata(&self.path).ok();
        match (open.as_mut(), meta) {
            (Some(current), None) => {
                // Deleted: take what the open handle still has, then wait.
                drain(current, splitter, events);
                splitter.clear();
                *open = None;
                *waiting = true;
                send(SourceEvent::Break { kind: BreakKind::Deleted });
                send(SourceEvent::Status(SourceStatus::WaitingForFile));
            }
            (None, None) => {}
            (None, Some(meta)) => {
                // The file appeared (or came back): a new Generation.
                if let Ok(file) = File::open(&self.path) {
                    *open = Some(Open { file, ino: file_id(&meta), offset: 0 });
                    splitter.clear();
                    if !*waiting {
                        send(SourceEvent::Break { kind: BreakKind::Rotated });
                    }
                    *waiting = false;
                    send(SourceEvent::Status(SourceStatus::Live));
                    if let Some(current) = open.as_mut() {
                        drain(current, splitter, events);
                    }
                }
            }
            (Some(current), Some(meta)) => {
                if file_id(&meta) != current.ino {
                    // Rotated: read the rest of the old file first.
                    drain(current, splitter, events);
                    splitter.clear();
                    send(SourceEvent::Break { kind: BreakKind::Rotated });
                    if let Ok(file) = File::open(&self.path) {
                        *open = Some(Open { file, ino: file_id(&meta), offset: 0 });
                        if let Some(current) = open.as_mut() {
                            drain(current, splitter, events);
                        }
                    } else {
                        *open = None;
                        *waiting = true;
                        send(SourceEvent::Status(SourceStatus::WaitingForFile));
                    }
                } else if meta.len() < current.offset {
                    splitter.clear();
                    current.offset = 0;
                    send(SourceEvent::Break { kind: BreakKind::Truncated });
                    drain(current, splitter, events);
                } else if meta.len() > current.offset {
                    drain(current, splitter, events);
                }
            }
        }
    }
}

/// Read everything after `current.offset` and send the completed lines.
fn drain(current: &mut Open, splitter: &mut LineSplitter, events: &Sender<SourceEvent>) {
    if current.file.seek(SeekFrom::Start(current.offset)).is_err() {
        return;
    }
    let mut buf = Vec::new();
    if current.file.read_to_end(&mut buf).is_err() {
        return;
    }
    current.offset += buf.len() as u64;
    let lines = splitter.push(&buf);
    if !lines.is_empty() {
        let _ = events.try_send(SourceEvent::Append { lines, bytes: Vec::new(), streams: None });
    }
}
