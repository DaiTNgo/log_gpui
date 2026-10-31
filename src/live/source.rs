use crate::model::Stream;
use async_channel::{Receiver, Sender};
use std::time::Duration;

/// How often a local file is checked for new bytes.
pub const LOCAL_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Why a Remote Command runs again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartKind {
    /// The user pressed Restart.
    Manual,
    /// The command exited and Restart on exit is on.
    OnExit,
    /// The connection came back and the command is run again as typed.
    AfterDisconnect,
    /// Resuming by time failed: the command runs as it was written.
    ResumeFailed,
    /// The tab was chosen again or the app opened: the command runs again
    /// below the output its Remote Cache kept, resumed from the time of its
    /// last line (seconds into the day) or as written.
    Reopened(Option<u32>),
}

/// Why the lines of a file stop being continuous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakKind {
    /// Another file is at the path; the old one is still on disk, renamed.
    Rotated,
    /// Another file is at the path; the old one was deleted (`mv other.log
    /// app.log`).
    Replaced,
    Truncated,
    /// The same file, written over in place: the lines read are gone.
    Rewritten,
    Deleted,
    Disconnected,
    Reconnected,
    /// Older lines left the Live Window.
    LeftWindow,
    /// A Remote Command exited with this code (`None`: killed by a signal).
    Exited(Option<u32>),
    /// A Remote Command runs again from its start: a new Generation.
    Restarted(RestartKind),
    /// The connection came back and the Remote Command went on from the time
    /// of its last line (seconds into the day of the Server's own clock).
    Resumed(u32),
    /// The Server's clock went back while resuming: lines may repeat.
    ClockWentBack,
    /// This many lines of a Remote Command, output while Paused, did not fit
    /// the Live Window and were dropped; their numbers are not reused.
    Skipped(u32),
    /// The user cleared the file.
    Cleared,
    /// An earlier Generation, a Rotated File, is on the Server above the
    /// oldest line held: Open in full reads it. Drawn at the top of the
    /// lines of the file, with a button.
    EarlierAvailable,
    /// The lines since the Starting Time do not fit the Live Window.
    StartTimeOverflow,
    /// The read of a Remote Copy was cancelled; the lines end here.
    ReadCancelled,
    /// The read of a Remote Copy stopped at the Full Read Limit.
    ReadLimit,
    /// The file was rotated while a Remote Copy read it.
    ReadRotated,
    /// Older lines of a Remote Copy were left out to fit the Full Read Limit.
    OlderNotRead,
    /// The Remote Command was edited: the lines after the Break are the new
    /// command's, a new Generation. The Break names both commands.
    CommandChanged,
    /// Above the output of an earlier Run of a Remote Command, kept in its
    /// Remote Cache.
    PreviousRun,
}

impl BreakKind {
    pub fn label(self) -> &'static str {
        match self {
            BreakKind::Rotated => "rotated",
            BreakKind::Replaced => "replaced",
            BreakKind::Truncated => "truncated",
            BreakKind::Rewritten => "rewritten",
            BreakKind::Deleted => "deleted",
            BreakKind::Disconnected => "disconnected",
            BreakKind::Reconnected => "reconnected",
            BreakKind::LeftWindow => "older lines left the window",
            BreakKind::Exited(_) => "exited",
            BreakKind::Restarted(_) => "restarted",
            BreakKind::Resumed(_) => "reconnected",
            BreakKind::ClockWentBack => "server clock went back, lines may be repeated",
            BreakKind::Skipped(_) => "older lines left the window",
            BreakKind::Cleared => "cleared",
            BreakKind::EarlierAvailable => "earlier generation",
            BreakKind::StartTimeOverflow => "lines since the Starting Time do not fit the Live Window",
            BreakKind::ReadCancelled => "read cancelled",
            BreakKind::ReadLimit => "stopped at Full Read Limit",
            BreakKind::ReadRotated => "file rotated while reading",
            BreakKind::OlderNotRead => "older lines were not read",
            BreakKind::CommandChanged => "command changed",
            BreakKind::PreviousRun => "previous run",
        }
    }

    /// Whether the lines after the Break are a new Generation of the file.
    pub fn starts_generation(self) -> bool {
        matches!(
            self,
            BreakKind::Rotated
                | BreakKind::Replaced
                | BreakKind::Truncated
                | BreakKind::Rewritten
                | BreakKind::Deleted
                | BreakKind::Restarted(_)
                | BreakKind::CommandChanged
        )
    }
}

/// `10:02:03` for seconds into the day.
pub fn clock_text(seconds_into_day: u32) -> String {
    let s = seconds_into_day % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

/// What a source says about its file, worst first by `severity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Connecting,
    Live,
    Reconnecting,
    Disconnected,
    WaitingForFile,
    /// A Remote Command exited (its exit code, `None` when it was killed by
    /// a signal) and has not been run again.
    Exited(Option<u32>),
    /// A file of a Live Glob that no longer exists on its Server.
    Gone,
    /// A password or passphrase is needed: waits for the user to Connect.
    NeedsSecret,
    /// The Server refused the password or passphrase; waits for the user to
    /// type it again.
    AuthFailed,
    /// The Server asks Sign-in Prompts (names it): waits for the user to
    /// press Connect or Reconnect.
    SignInRequired(String),
    Error(String),
}

/// How a connection state reads at a glance in the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    /// Reading: nothing to do.
    Good,
    /// On its way: wait.
    Busy,
    /// Not reading, but the app or the user can bring it back.
    Warning,
    /// Failed.
    Bad,
}

impl SourceStatus {
    /// Higher is worse; the status bar shows the worst status of the files.
    pub fn severity(&self) -> u8 {
        match self {
            SourceStatus::Live => 0,
            SourceStatus::Gone => 0,
            SourceStatus::Connecting => 1,
            SourceStatus::WaitingForFile => 2,
            SourceStatus::Exited(_) => 3,
            SourceStatus::NeedsSecret => 4,
            SourceStatus::SignInRequired(_) => 4,
            SourceStatus::Reconnecting => 4,
            SourceStatus::AuthFailed => 5,
            SourceStatus::Disconnected => 5,
            SourceStatus::Error(_) => 6,
        }
    }

    /// How the status bar colours this state.
    pub fn tone(&self) -> StatusTone {
        match self {
            SourceStatus::Live | SourceStatus::Gone | SourceStatus::Exited(_) => StatusTone::Good,
            SourceStatus::Connecting | SourceStatus::WaitingForFile => StatusTone::Busy,
            SourceStatus::Reconnecting | SourceStatus::NeedsSecret | SourceStatus::SignInRequired(_) => {
                StatusTone::Warning
            }
            SourceStatus::Disconnected | SourceStatus::AuthFailed | SourceStatus::Error(_) => StatusTone::Bad,
        }
    }

    /// The word the status bar shows (no exit code: "Exited (1 of 3)" counts
    /// the members, so the code stays in the lists).
    pub fn bar_label(&self) -> String {
        match self {
            SourceStatus::Exited(_) => "Exited".into(),
            other => other.label(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            SourceStatus::Connecting => "Connecting".into(),
            SourceStatus::Live => "Live".into(),
            SourceStatus::Reconnecting => "Reconnecting".into(),
            SourceStatus::Disconnected => "Disconnected".into(),
            SourceStatus::WaitingForFile => "Waiting for file".into(),
            SourceStatus::Exited(Some(code)) => format!("Exited (code {code})"),
            SourceStatus::Exited(None) => "Exited".into(),
            SourceStatus::Gone => "Gone".into(),
            SourceStatus::NeedsSecret => "Waiting to connect".into(),
            SourceStatus::AuthFailed => "Authentication failed".into(),
            SourceStatus::SignInRequired(server) => format!("Sign-in required: {server}"),
            SourceStatus::Error(message) => message.clone(),
        }
    }
}

/// Which output each line of an `Append` came from: one bit per line, set
/// for stderr. Only Remote Commands have one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamMask {
    bits: Vec<u64>,
    len: usize,
}

impl StreamMask {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, stream: Stream) {
        if self.len % 64 == 0 {
            self.bits.push(0);
        }
        if stream == Stream::Stderr {
            self.bits[self.len / 64] |= 1 << (self.len % 64);
        }
        self.len += 1;
    }

    pub fn get(&self, index: usize) -> Option<Stream> {
        (index < self.len).then(|| {
            if self.bits[index / 64] >> (index % 64) & 1 == 1 { Stream::Stderr } else { Stream::Stdout }
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Cut the mask at `at`: it keeps the first `at` streams and the rest
    /// are returned.
    pub fn split_off(&mut self, at: usize) -> StreamMask {
        let at = at.min(self.len);
        let rest: StreamMask = (at..self.len).filter_map(|i| self.get(i)).collect();
        let keep: StreamMask = (0..at).filter_map(|i| self.get(i)).collect();
        *self = keep;
        rest
    }
}

impl FromIterator<Stream> for StreamMask {
    fn from_iter<I: IntoIterator<Item = Stream>>(iter: I) -> Self {
        let mut mask = Self::new();
        for stream in iter {
            mask.push(stream);
        }
        mask
    }
}

/// A Rotated File that can be loaded above the lines held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlierFile {
    pub name: String,
    pub size: u64,
    pub compressed: bool,
}

/// A Rotated File whose lines arrive as an earlier Generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlierStart {
    pub name: String,
    /// When the file was last written, as `YYYY-MM-DD HH:MM:SS` (on the Server's
    /// clock as it reports it), for the Break label.
    pub modified: Option<String>,
}

/// How far a long read has come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadProgress {
    /// What is read, e.g. `web1:/app/app.log.2.gz`.
    pub what: String,
    pub done: u64,
    pub total: Option<u64>,
}

/// What a source sends to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceEvent {
    /// Lines of a Rotated File, an earlier Generation of the file, inserted
    /// above everything held. `start` is given with the first batch of a
    /// Rotated File: it begins the Generation and draws the Break between it
    /// and the one below; later batches of the same file only add lines at
    /// its top. `last_line_no` is the number in the Rotated File of the last
    /// line of the batch when `start` is given (`None`: not known).
    Earlier {
        start: Option<EarlierStart>,
        lines: Vec<String>,
        bytes: Vec<u32>,
        last_line_no: Option<usize>,
        reached_start: bool,
    },
    /// The next Rotated File that could be loaded, or none (told whenever the
    /// lines held reach the start of the oldest Generation held).
    EarlierAvailable(Option<EarlierFile>),
    /// The Live Window filled before the Starting Time was reached: how many
    /// lines there are since it (`None`: not counted), and the time as typed.
    StartTimeOverflow { lines: Option<usize>, since: String },
    /// A long read is under way (`None`: it ended).
    Progress(Option<ReadProgress>),
    /// The last lines of the file, shown at once. `bytes` is the length of
    /// each line in the file, line end included (empty when not known), and
    /// `end_offset` the offset just after the last of them.
    Tail { lines: Vec<String>, generation: u32, bytes: Vec<u32>, end_offset: u64 },
    /// Lines older than everything received so far, inserted above.
    Backfill { lines: Vec<String>, generation: u32, bytes: Vec<u32> },
    /// New lines, each ended by a newline in the file. A Remote Command also
    /// says which output each line came from (`streams` is as long as
    /// `lines`; a file has none) and has no `bytes`.
    Append { lines: Vec<String>, bytes: Vec<u32>, streams: Option<StreamMask> },
    /// The inode of the file the lines now come from.
    Inode(Option<u64>),
    /// Where the file being read stands on the Server, to tell next time
    /// whether it is the same file (openspec change `remote-local-cache`,
    /// decision 5): `server_end` is the offset after its last complete line,
    /// `head_hash` the FNV-1a of its first `min(4096, server_end)` bytes and
    /// `tail_hash` that of the `min(4096, server_end)` bytes before
    /// `server_end`, all as they are on the Server.
    Checkpoint { generation: u32, inode: Option<u64>, server_end: u64, head_hash: u64, tail_hash: u64 },
    /// The real number of complete lines before the first `Append`, once known
    /// (`None` when it could not be counted).
    TotalLines(Option<usize>),
    Break { kind: BreakKind },
    Status(SourceStatus),
    /// The Remote Command was edited: its lines stay and a new Run of the new
    /// command starts (a new Generation). `from` and `to` are the names
    /// `<server>$ <command>`, for the Break.
    CommandChanged { from: String, to: String },
    /// A Remote Command started a Run: the command line as sent to the Server
    /// (the `--since` form when resumed).
    Run { command: String },
    /// Something went wrong that the status does not say (kept as the last
    /// error of the source).
    Problem(String),
}

/// What the app tells a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommand {
    Pause,
    Resume,
    SetWindow(usize),
    /// Stop reading older lines and never refill: the app dropped what it held.
    Clear,
    /// Try again now (after Disconnected or an error the user has fixed).
    Reconnect,
    /// Run a Remote Command again from its start, ending the Run in progress.
    Restart,
    /// Turn Restart on exit on or off for a Remote Command.
    SetRestartOnExit(bool),
    /// Run another command (or the same one on another Server) in place of a
    /// Remote Command: the Run in progress ends, the lines stay, and the new
    /// command starts a new Run. `name` is `<server>$ <command>`.
    Replace { server_id: String, target: crate::workspace::log_space::RemoteTarget, name: String },
    Close,
}

/// The app's end of a running source.
#[derive(Debug, Clone)]
pub struct SourceHandle {
    pub events: Receiver<SourceEvent>,
    pub commands: Sender<SourceCommand>,
}

impl SourceHandle {
    pub fn send(&self, command: SourceCommand) {
        let _ = self.commands.try_send(command);
    }

    pub fn close(&self) {
        self.send(SourceCommand::Close);
    }
}

/// A file read as it is written. The app does not know where the file is.
pub trait LineSource: Send + 'static {
    /// Start reading in the background.
    fn spawn(self) -> SourceHandle;
}
