//! Live reading: a `LineSource` per file sends the lines of a file that is
//! still being written to the app, in the same way for local and remote files.

pub mod backward;
pub mod buffer;
pub mod local;
pub mod remote;
pub mod source;
pub mod splitter;

pub use backward::BackwardLines;
pub use buffer::{BreakMark, ClearMark, HeldStart, HoldMode, LiveBuffer, PROVISIONAL_BASE, Trimmed, is_real_line_number, line_number_text};
pub use local::LocalFileSource;
pub use remote::RemoteFileSource;
pub use source::{
    BreakKind, LineSource, SourceCommand, SourceEvent, SourceHandle, SourceStatus,
    LOCAL_POLL_INTERVAL,
};
pub use splitter::LineSplitter;
