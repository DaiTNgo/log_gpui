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
}

/// Why the lines of a file stop being continuous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakKind {
    Rotated,
    Truncated,
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
    /// The first line held is not the first line of its Generation.
    StartOfLoaded,
    /// Load More found the file on the Server changed under the held lines.
    Changed,
    /// An earlier Generation, a Rotated File, can be loaded above the oldest
    /// line held. Drawn at the top of the lines of the file, with a button.
    EarlierAvailable,
    /// Like `EarlierAvailable`, but the Live Window is full: only Open in
    /// full can read it.
    EarlierWindowFull,
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
}

impl BreakKind {
    pub fn label(self) -> &'static str {
        match self {
            BreakKind::Rotated => "rotated",
            BreakKind::Truncated => "truncated",
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
            BreakKind::StartOfLoaded => "start of loaded lines",
            BreakKind::Changed => "earlier lines of this generation are no longer on the Server",
            BreakKind::EarlierAvailable | BreakKind::EarlierWindowFull => "earlier generation",
            BreakKind::StartTimeOverflow => "lines since the Starting Time do not fit the Live Window",
            BreakKind::ReadCancelled => "read cancelled",
            BreakKind::ReadLimit => "stopped at Full Read Limit",
            BreakKind::ReadRotated => "file rotated while reading",
            BreakKind::OlderNotRead => "older lines were not read",
        }
    }

    /// Whether the lines after the Break are a new Generation of the file.
    pub fn starts_generation(self) -> bool {
        matches!(
            self,
            BreakKind::Rotated | BreakKind::Truncated | BreakKind::Deleted | BreakKind::Restarted(_)
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
    /// Lines older than what is held, read because the user asked for more
    /// (`LoadMore`), to go above the oldest held line, whose number is
    /// `first_line_no`.
    Older { generation: u32, lines: Vec<String>, bytes: Vec<u32>, reached_start: bool },
    /// Load More could not read the lines above (the file changed, or the
    /// Generation is not the one being read).
    OlderUnavailable,
    /// The inode of the file the lines now come from.
    Inode(Option<u64>),
    /// The real number of complete lines before the first `Append`, once known
    /// (`None` when it could not be counted).
    TotalLines(Option<usize>),
    Break { kind: BreakKind },
    Status(SourceStatus),
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
    /// Read `lines` lines above `end_offset` of `generation`. `first_hash` is
    /// the hash of the line that starts at `end_offset` (`None` when no line
    /// is held there).
    LoadMore { generation: u32, end_offset: u64, lines: usize, first_hash: Option<u64> },
    /// Load the lines of the next Rotated File above the oldest line held:
    /// at most `room` lines (what the Live Window has left).
    LoadEarlier { room: usize },
    /// Drop a Load More that was not started yet (a batch being read is
    /// kept: it arrives as `Older`).
    CancelLoadMore,
    /// Try again now (after Disconnected or an error the user has fixed).
    Reconnect,
    /// Run a Remote Command again from its start, ending the Run in progress.
    Restart,
    /// Turn Restart on exit on or off for a Remote Command.
    SetRestartOnExit(bool),
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
