//! What each line keeps besides where it is (decision 3): 16 bytes, enough
//! to merge, filter by time and level, and keep a Continuation Line with its
//! Parent Line without reading the text again. The rest of a `LogEntry` is
//! made only when the line is read again to be shown.

use crate::model::{LogEntry, LogLevel};

/// The effective timestamp of a file's lines before its first timestamp,
/// while the file has none yet.
pub const NO_TIME: i64 = i64::MIN;

pub mod flags {
    /// The line has a time of its own.
    pub const HAS_TIME: u8 = 1;
    /// The line's bytes are not UTF-8; it shows with replacement characters.
    pub const NOT_UTF8: u8 = 2;
    /// `ts_us` is the line's own timestamp, not one it took.
    pub const OWN_TS: u8 = 4;
}

/// One line's metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineMeta {
    /// Effective timestamp, µs: the line's own, else the Parent Line's.
    /// Lines before the first timestamp take that first timestamp.
    pub ts_us: i64,
    /// Lines up to the Parent Line; 0 when not a Continuation Line.
    pub parent_dist: u32,
    /// `LogLevel as u8`.
    pub level: u8,
    pub flags: u8,
}

const LEVELS: [LogLevel; 7] = [
    LogLevel::Verbose,
    LogLevel::Debug,
    LogLevel::Info,
    LogLevel::Warn,
    LogLevel::Error,
    LogLevel::Fatal,
    LogLevel::Unknown,
];

impl LineMeta {
    pub fn has_time(&self) -> bool {
        self.flags & flags::HAS_TIME != 0
    }

    pub fn level(&self) -> LogLevel {
        LEVELS.get(self.level as usize).copied().unwrap_or(LogLevel::Unknown)
    }

    /// Line `n`'s Parent Line (0-based), when it is a Continuation Line.
    pub fn parent_of(&self, n: usize) -> Option<usize> {
        (self.parent_dist > 0).then(|| n - self.parent_dist as usize)
    }
}

/// A line's own time and level, before it is stitched to the lines above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawMeta {
    /// The line has a time of its own, so it is not a Continuation Line.
    pub has_time: bool,
    /// That time, µs, when it is a usable timestamp (finite and positive).
    pub ts_us: Option<i64>,
    pub level: LogLevel,
    pub not_utf8: bool,
}

/// µs of a timestamp in seconds.
pub fn micros(seconds: f64) -> i64 {
    (seconds * 1e6).round() as i64
}

impl RawMeta {
    /// What `entry` (a parsed line) says of itself.
    pub fn of_entry(entry: &LogEntry) -> Self {
        Self {
            has_time: entry.has_time,
            ts_us: crate::filter_index::has_timestamp(entry).then(|| micros(entry.timestamp)),
            level: entry.level,
            not_utf8: false,
        }
    }
}

/// Runs of a value over line numbers: the Generation or the Stream of each
/// line, stored where it changes instead of on every line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runs<T> {
    /// First line of each run, and its value; first lines increase.
    runs: Vec<(usize, T)>,
}

impl<T> Default for Runs<T> {
    fn default() -> Self {
        Self { runs: Vec::new() }
    }
}

impl<T: Copy + PartialEq> Runs<T> {
    /// Lines from `line` on have `value` (until the next `set`). `line` is
    /// at or after the first line of the last run.
    pub fn set(&mut self, line: usize, value: T) {
        if let Some(&(start, last)) = self.runs.last() {
            debug_assert!(line >= start);
            if last == value {
                return;
            }
            if start == line {
                self.runs.pop();
                if self.runs.last().is_some_and(|&(_, v)| v == value) {
                    return;
                }
            }
        }
        self.runs.push((line, value));
    }

    /// The value of `line`, `None` before the first run.
    pub fn get(&self, line: usize) -> Option<T> {
        let at = self.runs.partition_point(|&(start, _)| start <= line);
        at.checked_sub(1).map(|i| self.runs[i].1)
    }

    /// Forget the lines from `line` on.
    pub fn truncate(&mut self, line: usize) {
        self.runs.retain(|&(start, _)| start < line);
    }

    /// The runs, first line and value.
    pub fn runs(&self) -> &[(usize, T)] {
        &self.runs
    }
}

/// The metadata of a file's lines, stitched as lines arrive in order.
#[derive(Debug, Clone, Default)]
pub struct MetaTable {
    meta: Vec<LineMeta>,
    /// The latest line with a time of its own.
    last_timed: Option<usize>,
    /// The latest timestamp.
    cur_ts: Option<i64>,
    /// Generation of each line.
    pub generations: Runs<u32>,
    /// Stream of each line of a Remote Command (`None` for files).
    pub streams: Runs<Option<crate::model::Stream>>,
}

impl MetaTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.meta.len()
    }

    pub fn is_empty(&self) -> bool {
        self.meta.is_empty()
    }

    pub fn get(&self, n: usize) -> Option<&LineMeta> {
        self.meta.get(n)
    }

    pub fn as_slice(&self) -> &[LineMeta] {
        &self.meta
    }

    /// Whether a line has a time of its own: the next line without one
    /// continues it.
    pub fn continues(&self) -> bool {
        self.last_timed.is_some()
    }

    /// Whether any line has a timestamp.
    pub fn has_any_time(&self) -> bool {
        self.cur_ts.is_some()
    }

    /// Add the next line. A line without a time of its own continues the
    /// latest line with one. A line without a timestamp takes the latest
    /// timestamp; lines before the first timestamp take it when it comes.
    pub fn push(&mut self, raw: RawMeta) {
        let n = self.meta.len();
        let mut flags = if raw.not_utf8 { flags::NOT_UTF8 } else { 0 };
        let parent_dist = if raw.has_time {
            flags |= flags::HAS_TIME;
            self.last_timed = Some(n);
            0
        } else {
            self.last_timed.map_or(0, |p| (n - p) as u32)
        };
        if let Some(ts) = raw.ts_us {
            flags |= flags::OWN_TS;
            if self.cur_ts.is_none() {
                for head in &mut self.meta {
                    head.ts_us = ts;
                }
            }
            self.cur_ts = Some(ts);
        }
        let ts_us = self.cur_ts.unwrap_or(NO_TIME);
        // A Continuation Line is all message: it has no level of its own.
        let level = if parent_dist > 0 { LogLevel::Unknown } else { raw.level };
        self.meta.push(LineMeta { ts_us, parent_dist, level: level as u8, flags });
    }

    /// Forget the lines from line `n` on.
    pub fn truncate(&mut self, n: usize) {
        if n >= self.meta.len() {
            return;
        }
        self.meta.truncate(n);
        self.last_timed = self.meta.iter().rposition(LineMeta::has_time);
        // Lines keep the timestamp they took; with no timestamp left, the
        // head is untimed again.
        let any_ts = self.meta.iter().any(|m| m.flags & flags::OWN_TS != 0);
        self.cur_ts = any_ts.then(|| self.meta.last().map(|m| m.ts_us)).flatten();
        if self.cur_ts.is_none() {
            for head in &mut self.meta {
                head.ts_us = NO_TIME;
            }
        }
        self.generations.truncate(n);
        self.streams.truncate(n);
    }

    pub fn shrink_to_fit(&mut self) {
        self.meta.shrink_to_fit();
    }

    pub fn heap_bytes(&self) -> usize {
        self.meta.capacity() * std::mem::size_of::<LineMeta>()
    }
}
