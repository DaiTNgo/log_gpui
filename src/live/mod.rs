//! Live reading: a `LineSource` per file sends the lines of a file that is
//! still being written to the app, in the same way for local and remote files.

pub mod backward;
pub mod buffer;
pub mod command;
pub mod health;
pub mod local;
pub mod remote;
pub mod resume;
pub mod source;
pub mod splitter;

pub use backward::BackwardLines;
pub use buffer::{BreakMark, LiveBuffer, PROVISIONAL_BASE, Trimmed, is_real_line_number, line_number_text};
pub use command::RemoteCommandSource;
pub use health::{SourceHealth, connection_label};
pub use local::LocalFileSource;
pub use remote::RemoteFileSource;
pub use source::{
    BreakKind, LineSource, LOCAL_POLL_INTERVAL, RestartKind, SourceCommand, SourceEvent, SourceHandle,
    SourceStatus, StreamMask, clock_text,
};
pub use resume::{ResumePoint, Timestamp, Verdict, parse_timestamp};
pub use splitter::LineSplitter;
