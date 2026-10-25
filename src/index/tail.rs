//! The last lines of a file before Indexing gets there (task 3.3): a live
//! file opens at its newest line (L3), and Indexing a big file without the
//! lock takes seconds, so its last lines are read on their own first and
//! shown until Indexing reaches them. Their line numbers are known only then.

use super::LineSource;
use super::line_index::{line_text, split_lines};
use crate::model::LogEntry;
use crate::parser::text::LineParser;
use std::io;

/// Bytes read from the end: more than a screen of lines of any usual width.
pub const TAIL_BYTES: usize = 64 << 10;

/// The last lines of a source, read before their line numbers are known.
#[derive(Debug)]
pub struct Tail {
    /// Where the first of `lines` starts in the source.
    pub start: u64,
    /// Where the lines end: the size of the source when read.
    pub end: u64,
    /// The lines, oldest first, parsed with no line above them.
    pub lines: Vec<LogEntry>,
}

/// The whole lines in the last `max_bytes` of `source`, parsed with
/// `parser`. A line cut by the start of the read is left out, unless the
/// read starts at the top of the file.
pub fn read_tail(source: &dyn LineSource, max_bytes: usize, parser: &mut LineParser) -> io::Result<Tail> {
    let end = source.len()?;
    let from = end.saturating_sub(max_bytes as u64);
    let data = source.read_range(from, (end - from) as usize)?;
    let (skip, start) = if from == 0 {
        (0, 0)
    } else {
        match memchr::memchr(b'\n', &data) {
            Some(at) => (at + 1, from + at as u64 + 1),
            None => (data.len(), end),
        }
    };
    parser.reset();
    let lines = split_lines(&data[skip..])
        .map(|bytes| String::from_utf8_lossy(line_text(bytes)).into_owned())
        .map(|text| parser.parse(0, &text))
        .collect();
    Ok(Tail { start, end, lines })
}
