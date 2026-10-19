//! The lines a live file has delivered so far: what `SourceEvent`s do to the
//! parsed cache of one file (`ParsedFile`).
//!
//! Line numbers: a local file is loaded whole, so its numbers are exact from
//! the start. A remote file shows its last lines first and learns the real
//! number of lines later (`TotalLines`), so until then its lines carry
//! provisional numbers from `PROVISIONAL_BASE` up; the Gutter shows none for
//! them.

use super::source::{BreakKind, RestartKind, StreamMask, clock_text};
use crate::parser::LineParser;
use crate::space_view::ParsedFile;
use std::sync::Arc;

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

/// The state a live file keeps next to its `ParsedFile`.
#[derive(Debug, Clone)]
pub struct LiveBuffer {
    /// Name shown in Break labels.
    pub name: String,
    pub generation: u32,
    parser: LineParser,
    /// Line number of the first line loaded in the current Generation.
    first_index: usize,
    /// Line number the next appended line gets.
    next_index: usize,
    /// Whether the line numbers of the current Generation are real.
    pub numbers_known: bool,
    /// A Break waiting for the next line, drawn above it.
    pending: Option<BreakMark>,
    /// When the connection dropped, while it is down.
    disconnected_at: Option<String>,
    /// Lines of the current Generation that came from `Tail` or `Backfill`
    /// (their last number is what `TotalLines` refers to).
    tail_last_index: Option<usize>,
    /// Where a local source starts reading: just after the last newline of
    /// what was loaded, so a line that was still being written completes.
    pub local_offset: u64,
    /// The last loaded line had no newline yet: the first appended line is
    /// that line again, whole, and replaces it.
    replace_last: bool,
}

/// What a trim of the Live Window removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trimmed {
    pub removed: usize,
    pub generation: u32,
    /// Number of the oldest line still kept, in the Generation of that line.
    pub first_kept: usize,
}

impl LiveBuffer {
    /// For a file whose `loaded_lines` lines are already in the cache with
    /// their real numbers.
    pub fn loaded(name: impl Into<String>, loaded_lines: usize) -> Self {
        Self {
            name: name.into(),
            generation: 0,
            parser: LineParser::new(),
            first_index: 1,
            next_index: loaded_lines + 1,
            numbers_known: true,
            pending: None,
            disconnected_at: None,
            tail_last_index: None,
            local_offset: 0,
            replace_last: false,
        }
    }

    /// For a local file loaded from `bytes` (its lines are in the cache).
    pub fn loaded_from_bytes(name: impl Into<String>, bytes: &[u8], loaded_lines: usize) -> Self {
        let mut buf = Self::loaded(name, loaded_lines);
        buf.local_offset = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |p| p as u64 + 1);
        buf.replace_last = !bytes.is_empty() && bytes.last() != Some(&b'\n');
        buf
    }

    /// For a Remote Command: its lines are numbered from 1 in the order the
    /// command printed them, so the numbers are real from the start.
    pub fn command(name: impl Into<String>) -> Self {
        Self::loaded(name, 0)
    }

    /// For a remote file that has nothing yet.
    pub fn remote(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            generation: 0,
            parser: LineParser::new(),
            first_index: PROVISIONAL_BASE,
            next_index: PROVISIONAL_BASE,
            numbers_known: false,
            pending: None,
            disconnected_at: None,
            tail_last_index: None,
            local_offset: 0,
            replace_last: false,
        }
    }

    /// Whether the next appended line replaces the last loaded one.
    pub fn replaces_last_line(&self) -> bool {
        self.replace_last
    }

    pub fn first_index(&self) -> usize {
        self.first_index
    }

    pub fn next_index(&self) -> usize {
        self.next_index
    }

    /// The last lines of the file, at the end of the cache.
    pub fn apply_tail(&mut self, parsed: &mut ParsedFile, lines: &[String], generation: u32) {
        let _ = generation;
        let start = self.next_index;
        self.push_lines(parsed, lines, None);
        self.first_index = start;
        if !lines.is_empty() {
            self.tail_last_index = Some(self.next_index - 1);
        }
    }

    /// Older lines, inserted above everything in the current Generation.
    pub fn apply_backfill(&mut self, parsed: &mut ParsedFile, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let count = lines.len();
        let first = self.first_index.saturating_sub(count);
        // The Generation's first loaded line is where the new lines go.
        let at = parsed
            .entries
            .iter()
            .position(|e| e.generation == self.generation)
            .unwrap_or(parsed.entries.len());
        let mut parser = LineParser::new();
        let mut entries: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let mut entry = parser.parse(first + i, line);
                entry.generation = self.generation;
                entry
            })
            .collect();
        // Lines at the top of what was loaded that continued a line not yet
        // known now continue the last line with a time of the new lines.
        let last_parent = entries.iter().rev().find(|e| e.has_time).map(|e| e.index);
        for entry in parsed.entries[at..].iter_mut() {
            if entry.generation != self.generation || entry.has_time || entry.parent.is_some() {
                break;
            }
            match last_parent {
                Some(p) => {
                    *entry = continuation_of(entry, p);
                }
                None => break,
            }
        }
        for entry in &entries {
            parsed.max_chars.record_entry(entry);
        }
        parsed.lines_mut().splice(at..at, lines.iter().cloned());
        parsed.entries.splice(at..at, entries.drain(..));
        parsed.shift_breaks_from(at, count as isize);
        self.first_index = first;
    }

    /// New lines at the end. Returns the positions of the new entries.
    /// `streams` says which output of a Remote Command each line is from.
    pub fn apply_append(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        streams: Option<&StreamMask>,
    ) -> std::ops::Range<usize> {
        if self.replace_last && !lines.is_empty() {
            self.replace_last = false;
            if parsed.entries.pop().is_some() {
                parsed.lines_mut().pop();
                parsed.raw_breaks.remove(&parsed.entries.len());
                self.next_index -= 1;
            }
        }
        self.push_lines(parsed, lines, streams)
    }

    fn push_lines(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        streams: Option<&StreamMask>,
    ) -> std::ops::Range<usize> {
        let from = parsed.entries.len();
        for (i, line) in lines.iter().enumerate() {
            let mut entry = self.parser.parse(self.next_index, line);
            entry.generation = self.generation;
            // The Stream is the line's own: a Continuation Line keeps its own.
            entry.stream = streams.and_then(|s| s.get(i));
            if let Some(mark) = self.pending.take() {
                let mark = Arc::new(mark);
                entry.break_before = Some(mark.clone());
                parsed.raw_breaks.insert(parsed.entries.len(), mark);
            }
            self.next_index += 1;
            parsed.max_chars.record_entry(&entry);
            parsed.entries.push(entry);
        }
        parsed.lines_mut().extend(lines.iter().cloned());
        from..parsed.entries.len()
    }

    /// The file is not continuous here. The Break is drawn above the next
    /// line that arrives; a Generation starts after a rotation, truncation or
    /// deletion.
    pub fn apply_break(&mut self, kind: BreakKind, time: &str) {
        match kind {
            BreakKind::Disconnected => {
                self.disconnected_at = Some(time.to_string());
                return;
            }
            BreakKind::Reconnected => {
                let since = self.disconnected_at.take().unwrap_or_else(|| time.to_string());
                self.add_pending(BreakMark {
                    kind,
                    label: format!("{} disconnected {since} – reconnected {time}", self.name),
                });
                return;
            }
            BreakKind::Resumed(seconds) => {
                let since = self.disconnected_at.take().unwrap_or_else(|| time.to_string());
                self.add_pending(BreakMark {
                    kind,
                    label: format!(
                        "{} disconnected {since} – reconnected {time}, resumed from {}",
                        self.name,
                        clock_text(seconds)
                    ),
                });
                return;
            }
            BreakKind::Exited(code) => {
                let how = match code {
                    Some(code) => format!("exited ({code})"),
                    None => "exited (killed)".to_string(),
                };
                self.add_pending(BreakMark { kind, label: format!("{} {how} {time}", self.name) });
                return;
            }
            BreakKind::Skipped(count) => {
                // The numbers of the dropped lines are not given again.
                self.next_index += count as usize;
                self.add_pending(BreakMark {
                    kind,
                    label: format!("{} {} {time}", self.name, kind.label()),
                });
                return;
            }
            BreakKind::ClockWentBack => {
                self.add_pending(BreakMark {
                    kind,
                    label: format!("{} {} {time}", self.name, kind.label()),
                });
                return;
            }
            BreakKind::Restarted(why) => {
                let label = match why {
                    RestartKind::Manual | RestartKind::OnExit => {
                        format!("{} restarted {time}", self.name)
                    }
                    RestartKind::AfterDisconnect => {
                        let since = self.disconnected_at.take().unwrap_or_else(|| time.to_string());
                        format!(
                            "{} disconnected {since} – reconnected {time}, command restarted, lines may be missing or repeated",
                            self.name
                        )
                    }
                    RestartKind::ResumeFailed => format!(
                        "{} restarted {time}, resume not supported, lines may be missing or repeated",
                        self.name
                    ),
                };
                self.begin_generation_state();
                self.add_pending(BreakMark { kind, label });
                return;
            }
            _ => {}
        }
        if kind.starts_generation() {
            self.begin_generation_state();
        }
        self.add_pending(BreakMark { kind, label: format!("{} {} {time}", self.name, kind.label()) });
    }

    fn begin_generation_state(&mut self) {
        self.generation += 1;
        self.parser.reset();
        self.first_index = 1;
        self.next_index = 1;
        self.numbers_known = true;
        self.tail_last_index = None;
    }

    fn add_pending(&mut self, mark: BreakMark) {
        self.pending = Some(match self.pending.take() {
            Some(old) => BreakMark { kind: mark.kind, label: format!("{}; {}", old.label, mark.label) },
            None => mark,
        });
    }

    /// The real number of lines up to the end of the tail is known: number
    /// the current Generation's lines for real. Returns the amount every
    /// number of the Generation moved by (negative).
    pub fn apply_total(&mut self, parsed: &mut ParsedFile, total: Option<usize>) -> Option<isize> {
        let total = total?;
        let last = self.tail_last_index?;
        if self.numbers_known {
            return None;
        }
        let delta = total as isize - last as isize;
        let generation = self.generation;
        for entry in parsed.entries.iter_mut().filter(|e| e.generation == generation) {
            entry.index = (entry.index as isize + delta) as usize;
            entry.parent = entry.parent.map(|p| (p as isize + delta) as usize);
        }
        self.first_index = (self.first_index as isize + delta) as usize;
        self.next_index = (self.next_index as isize + delta) as usize;
        self.tail_last_index = Some(total);
        self.numbers_known = true;
        Some(delta)
    }

    /// Keep at most `window` lines: the oldest ones leave, and the first line
    /// kept gets a Break saying so. Nothing happens within 10% over the
    /// window.
    pub fn trim_to_window(&mut self, parsed: &mut ParsedFile, window: usize) -> Option<Trimmed> {
        let len = parsed.entries.len();
        if window == 0 || len <= window + window / 10 {
            return None;
        }
        let removed = len - window;
        parsed.entries.drain(..removed);
        parsed.lines_mut().drain(..removed);
        parsed.shift_breaks_from(0, -(removed as isize));
        let first = parsed.entries.first_mut()?;
        let (generation, first_kept) = (first.generation, first.index);
        let mark = Arc::new(BreakMark {
            kind: BreakKind::LeftWindow,
            label: format!("{} older lines left the window", self.name),
        });
        first.break_before = Some(mark.clone());
        parsed.raw_breaks.insert(0, mark);
        if generation == self.generation {
            self.first_index = first_kept;
        }
        Some(Trimmed { removed, generation, first_kept })
    }
}

fn continuation_of(entry: &crate::model::LogEntry, parent: usize) -> crate::model::LogEntry {
    let mut e = entry.clone();
    e.parent = Some(parent);
    e.time_str = Default::default();
    e.has_time = false;
    e
}
