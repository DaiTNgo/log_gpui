//! What the live files of the app share about their lines, which are read
//! through a Line Index (a local file where it is, a Remote File or Remote
//! Command through its Remote Cache, a Remote Copy through its folder): the
//! Breaks drawn between them, how many a Remote File holds, and how its
//! numbers read before they are known.
//!
//! Line numbers: a Remote File shows its last lines first and learns the
//! real number of lines later (`TotalLines`), so until then its lines carry
//! provisional numbers from `PROVISIONAL_BASE` up; the Gutter shows none for
//! them.

use super::source::{BreakKind, clock_text};

/// Provisional line numbers start here; a number at or above half of it is
/// not a real one.
pub const PROVISIONAL_BASE: usize = 1 << 40;

/// Whether `index` is a real line number.
pub fn is_real_line_number(index: usize) -> bool {
    index < PROVISIONAL_BASE / 2
}

/// The number the Gutter shows for a line: nothing while the real number of
/// the line is not known.
pub fn line_number_text(index: usize) -> String {
    if is_real_line_number(index) { index.to_string() } else { String::new() }
}

/// A mark drawn between two rows where the lines of a file are not
/// continuous. Not a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakMark {
    pub kind: BreakKind,
    /// Reason, file name and time, e.g. `app.log rotated 10:02:11`.
    pub label: String,
}

/// How many lines a Remote File keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldMode {
    /// The newest lines, as many as the Live Window.
    #[default]
    Window,
    /// Everything from the oldest line loaded by Load More, up to the Full
    /// Read Limit.
    Extended,
}

/// Where the oldest held line is in the file on the Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldStart {
    pub generation: u32,
    /// Number of the oldest held line (or of the next line when none is held).
    pub line_no: usize,
    /// Byte offset of its start in the file; `None` when it is in an older
    /// Generation than the one being read.
    pub offset: Option<u64>,
}

/// Where a Clear cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearMark {
    pub generation: u32,
    pub line_no: usize,
    pub offset: u64,
    pub time: String,
}

/// What a trim of the Live Window removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trimmed {
    pub removed: usize,
    pub generation: u32,
    /// Number of the oldest line still kept, in the Generation of that line.
    pub first_kept: usize,
    /// Bytes the removed lines took in the Raw View's text: each line and
    /// its newline.
    pub text_bytes: usize,
}

/// `38 MB`, `1.4 GB`, `812 B`.
pub fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 10.0 { format!("{} {}", value.round() as u64, UNITS[unit]) } else { format!("{value:.1} {}", UNITS[unit]) }
}

/// The Break above the output of a Remote Command run again below what its
/// Remote Cache kept.
pub fn reopened_label(name: &str, time: &str, resumed_from: Option<u32>) -> String {
    match resumed_from {
        Some(seconds) => format!("{name} run again {time}, resumed from {}", clock_text(seconds)),
        None => format!("{name} run again {time}, lines may be missing or repeated"),
    }
}
