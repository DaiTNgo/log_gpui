use async_channel::{Receiver, Sender};
use std::time::Duration;

/// How often a local file is checked for new bytes.
pub const LOCAL_POLL_INTERVAL: Duration = Duration::from_millis(250);

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
        }
    }

    /// Whether the lines after the Break are a new Generation of the file.
    pub fn starts_generation(self) -> bool {
        matches!(self, BreakKind::Rotated | BreakKind::Truncated | BreakKind::Deleted)
    }
}

/// What a source says about its file, worst first by `severity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Connecting,
    Live,
    Reconnecting,
    Disconnected,
    WaitingForFile,
    /// A password or passphrase is needed: waits for the user to Connect.
    NeedsSecret,
    Error(String),
}

impl SourceStatus {
    /// Higher is worse; the status bar shows the worst status of the files.
    pub fn severity(&self) -> u8 {
        match self {
            SourceStatus::Live => 0,
            SourceStatus::Connecting => 1,
            SourceStatus::WaitingForFile => 2,
            SourceStatus::NeedsSecret => 3,
            SourceStatus::Reconnecting => 3,
            SourceStatus::Disconnected => 4,
            SourceStatus::Error(_) => 5,
        }
    }

    pub fn label(&self) -> String {
        match self {
            SourceStatus::Connecting => "Connecting".into(),
            SourceStatus::Live => "Live".into(),
            SourceStatus::Reconnecting => "Reconnecting".into(),
            SourceStatus::Disconnected => "Disconnected".into(),
            SourceStatus::WaitingForFile => "Waiting for file".into(),
            SourceStatus::NeedsSecret => "Waiting to connect".into(),
            SourceStatus::Error(message) => message.clone(),
        }
    }
}

/// What a source sends to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceEvent {
    /// The last lines of the file, shown at once.
    Tail { lines: Vec<String>, generation: u32 },
    /// Lines older than everything received so far, inserted above.
    Backfill { lines: Vec<String>, generation: u32 },
    /// New lines, each ended by a newline in the file.
    Append { lines: Vec<String> },
    /// The real number of complete lines before the first `Append`, once known
    /// (`None` when it could not be counted).
    TotalLines(Option<usize>),
    Break { kind: BreakKind },
    Status(SourceStatus),
}

/// What the app tells a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommand {
    Pause,
    Resume,
    SetWindow(usize),
    /// Try again now (after Disconnected or an error the user has fixed).
    Reconnect,
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
