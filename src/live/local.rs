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
    /// `None` when the identity could not be read: unknown, never equal to
    /// another identity (even another unknown one).
    ino: Option<FileId>,
    offset: u64,
}

/// Identity of a file: its inode on Unix; on Windows its volume serial number
/// and file index (the creation time is no identity there: a file created
/// within ~15 s of another's rename inherits its creation time).
type FileId = u128;

#[cfg(unix)]
fn id_of_open(_file: &File, meta: &std::fs::Metadata) -> Option<FileId> {
    Some(std::os::unix::fs::MetadataExt::ino(meta) as FileId)
}

/// The identity of whatever `path` names now; `None` when that cannot be told.
#[cfg(unix)]
fn id_of_path(_path: &std::path::Path, meta: &std::fs::Metadata) -> Option<FileId> {
    Some(std::os::unix::fs::MetadataExt::ino(meta) as FileId)
}

#[cfg(windows)]
fn id_of_open(file: &File, _meta: &std::fs::Metadata) -> Option<FileId> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    // SAFETY: the handle is valid for the life of `file`; the struct is plain data.
    unsafe {
        let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
        if GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) == 0 {
            return None;
        }
        Some(
            (FileId::from(info.dwVolumeSerialNumber) << 64)
                | (FileId::from(info.nFileIndexHigh) << 32)
                | FileId::from(info.nFileIndexLow),
        )
    }
}

/// std opens files with full sharing on Windows, so this does not block a rename.
#[cfg(windows)]
fn id_of_path(path: &std::path::Path, meta: &std::fs::Metadata) -> Option<FileId> {
    let file = File::open(path).ok()?;
    id_of_open(&file, meta)
}

/// Whether the file at the path is another file than the one open. Only two
/// known, different identities say so: an unreadable identity is unknown,
/// and unknown never equals (or differs from) anything, so no false rotation
/// is reported. The size checks still catch a truncation or a swap that
/// shrinks the file.
fn is_rotated(open: Option<FileId>, at_path: Option<FileId>) -> bool {
    matches!((open, at_path), (Some(a), Some(b)) if a != b)
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
                let ino = id_of_open(&file, &meta);
                open = Some(Open { file, ino, offset: self.start_offset.min(meta.len()) });
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
                    Ok(SourceCommand::LoadMore { lines, first_hash, first_line_no, .. }) => {
                        super::local_older::send_older(&self.path, first_line_no, first_hash, lines, &events);
                    }
                    Ok(SourceCommand::SetWindow(_))
                | Ok(SourceCommand::Reconnect)
                | Ok(SourceCommand::Clear)
                | Ok(SourceCommand::CancelLoadMore)
                | Ok(SourceCommand::LoadEarlier { .. })
                | Ok(SourceCommand::Restart)
                | Ok(SourceCommand::Replace { .. })
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
                    *open = Some(Open { ino: id_of_open(&file, &meta), file, offset: 0 });
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
                if current.ino.is_none() {
                    // Retry: a failed read is not a final answer.
                    current.ino = id_of_open(&current.file, &meta);
                }
                if is_rotated(current.ino, id_of_path(&self.path, &meta)) {
                    // Rotated: read the rest of the old file first.
                    drain(current, splitter, events);
                    splitter.clear();
                    send(SourceEvent::Break { kind: BreakKind::Rotated });
                    if let Ok(file) = File::open(&self.path) {
                        *open = Some(Open { ino: id_of_open(&file, &meta), file, offset: 0 });
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

/// Most bytes one `Append` is read from.
pub const CHUNK_BYTES: usize = 4 << 20;

/// Read everything after `current.offset`, a chunk at a time, and send the
/// completed lines of each chunk: a burst or a rewritten large file never
/// makes one huge batch. Waits while the app is behind.
fn drain(current: &mut Open, splitter: &mut LineSplitter, events: &Sender<SourceEvent>) {
    if current.file.seek(SeekFrom::Start(current.offset)).is_err() {
        return;
    }
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match (&mut current.file).take(CHUNK_BYTES as u64).read_to_end(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        current.offset += buf.len() as u64;
        let lines = splitter.push(&buf);
        if !lines.is_empty() {
            let _ = events.try_send(SourceEvent::Append { lines, bytes: Vec::new(), streams: None });
        }
        while events.len() >= 4 && !events.is_closed() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod identity_tests {
    use super::is_rotated;

    #[test]
    fn two_different_known_identities_are_a_rotation() {
        assert!(is_rotated(Some(1), Some(2)));
        assert!(!is_rotated(Some(7), Some(7)));
    }

    #[test]
    fn an_unknown_identity_is_never_a_rotation_nor_equal_to_another_unknown() {
        assert!(!is_rotated(None, None));
        assert!(!is_rotated(None, Some(3)));
        assert!(!is_rotated(Some(3), None));
    }
}
