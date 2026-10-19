//! The lines a live file has delivered so far: what `SourceEvent`s do to the
//! parsed cache of one file (`ParsedFile`).
//!
//! Line numbers: a local file is loaded whole, so its numbers are exact from
//! the start. A remote file shows its last lines first and learns the real
//! number of lines later (`TotalLines`), so until then its lines carry
//! provisional numbers from `PROVISIONAL_BASE` up; the Gutter shows none for
//! them.

use super::source::BreakKind;
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
    /// Length in the file of each held line, line end included, parallel to
    /// the entries of the cache (0 when not known: local files).
    byte_lens: Vec<u32>,
    /// Offset just after the last line of the current Generation held or
    /// cleared.
    next_offset: u64,
    pub hold: HoldMode,
    /// Where the file was last cleared.
    pub clear_mark: Option<ClearMark>,
    /// Inode of each Generation, as the source reported it.
    inodes: Vec<(u32, Option<u64>)>,
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
            byte_lens: Vec::new(),
            next_offset: 0,
            hold: HoldMode::Window,
            clear_mark: None,
            inodes: Vec::new(),
        }
    }

    /// For a local file loaded from `bytes` (its lines are in the cache).
    pub fn loaded_from_bytes(name: impl Into<String>, bytes: &[u8], loaded_lines: usize) -> Self {
        let mut buf = Self::loaded(name, loaded_lines);
        buf.local_offset = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |p| p as u64 + 1);
        buf.replace_last = !bytes.is_empty() && bytes.last() != Some(&b'\n');
        buf
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
            byte_lens: Vec::new(),
            next_offset: 0,
            hold: HoldMode::Window,
            clear_mark: None,
            inodes: Vec::new(),
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
        self.apply_tail_with(parsed, lines, generation, &[], 0);
    }

    /// `apply_tail` with the length of each line in the file and the offset
    /// just after the last of them.
    pub fn apply_tail_with(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        generation: u32,
        bytes: &[u32],
        end_offset: u64,
    ) {
        let _ = generation;
        let start = self.next_index;
        self.push_lines(parsed, lines, bytes);
        self.first_index = start;
        self.next_offset = end_offset;
        if !lines.is_empty() {
            self.tail_last_index = Some(self.next_index - 1);
        }
    }

    /// Older lines, inserted above everything in the current Generation.
    pub fn apply_backfill(&mut self, parsed: &mut ParsedFile, lines: &[String]) {
        self.apply_backfill_with(parsed, lines, &[]);
    }

    /// `apply_backfill` with the length of each line in the file.
    pub fn apply_backfill_with(&mut self, parsed: &mut ParsedFile, lines: &[String], bytes: &[u32]) {
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
        self.sync_byte_lens(parsed.entries.len() - count);
        self.byte_lens.splice(at..at, (0..count).map(|i| bytes.get(i).copied().unwrap_or(0)));
        parsed.shift_breaks_from(at, count as isize);
        self.first_index = first;
    }

    /// New lines at the end. Returns the positions of the new entries.
    pub fn apply_append(&mut self, parsed: &mut ParsedFile, lines: &[String]) -> std::ops::Range<usize> {
        self.apply_append_with(parsed, lines, &[])
    }

    /// `apply_append` with the length of each line in the file.
    pub fn apply_append_with(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        bytes: &[u32],
    ) -> std::ops::Range<usize> {
        if self.replace_last && !lines.is_empty() {
            self.replace_last = false;
            if parsed.entries.pop().is_some() {
                parsed.lines_mut().pop();
                parsed.raw_breaks.remove(&parsed.entries.len());
                self.byte_lens.truncate(parsed.entries.len());
                self.next_index -= 1;
            }
        }
        self.push_lines(parsed, lines, bytes)
    }

    /// Keep `byte_lens` as long as the entries were before an insertion of
    /// lines (they are padded when a source gave no lengths).
    fn sync_byte_lens(&mut self, len: usize) {
        self.byte_lens.resize(len, 0);
    }

    fn push_lines(&mut self, parsed: &mut ParsedFile, lines: &[String], bytes: &[u32]) -> std::ops::Range<usize> {
        let from = parsed.entries.len();
        self.sync_byte_lens(from);
        for i in 0..lines.len() {
            let len = bytes.get(i).copied().unwrap_or(0);
            self.byte_lens.push(len);
            self.next_offset += u64::from(len);
        }
        for line in lines {
            let mut entry = self.parser.parse(self.next_index, line);
            entry.generation = self.generation;
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
            _ => {}
        }
        if kind.starts_generation() {
            self.generation += 1;
            self.parser.reset();
            self.first_index = 1;
            self.next_index = 1;
            self.next_offset = 0;
            self.numbers_known = true;
            self.tail_last_index = None;
        }
        self.add_pending(BreakMark { kind, label: format!("{} {} {time}", self.name, kind.label()) });
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

    /// How many lines the file may hold now: the Live Window, or the Full
    /// Read Limit once Load More made it `Extended`.
    pub fn limit(&self, window: usize, full_read_limit: usize) -> usize {
        match self.hold {
            HoldMode::Window => window,
            HoldMode::Extended => full_read_limit,
        }
    }

    /// Keep at most `window` lines: the oldest ones leave, and the first line
    /// kept gets a Break saying so. Nothing happens within 10% over the
    /// window.
    pub fn trim_to_window(&mut self, parsed: &mut ParsedFile, window: usize) -> Option<Trimmed> {
        let len = parsed.entries.len();
        if window == 0 || len <= window + window / 10 {
            return None;
        }
        self.trim_to(parsed, window)
    }

    /// Back to the Live Window at once: only `window` lines stay and the
    /// memory of the rest is given back.
    pub fn back_to_window(&mut self, parsed: &mut ParsedFile, window: usize) -> Option<Trimmed> {
        self.hold = HoldMode::Window;
        let trimmed = if window > 0 && parsed.entries.len() > window {
            self.trim_to(parsed, window)
        } else {
            None
        };
        self.shrink(parsed);
        trimmed
    }

    fn trim_to(&mut self, parsed: &mut ParsedFile, keep: usize) -> Option<Trimmed> {
        let removed = parsed.entries.len() - keep;
        parsed.entries.drain(..removed);
        parsed.lines_mut().drain(..removed);
        self.sync_byte_lens(removed + keep);
        self.byte_lens.drain(..removed);
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

    /// Give back the memory the cache holds beyond its lines.
    pub fn shrink(&mut self, parsed: &mut ParsedFile) {
        parsed.entries.shrink_to_fit();
        parsed.lines_mut().shrink_to_fit();
        self.byte_lens.shrink_to_fit();
    }

    /// Drop every line held and draw a Break "cleared" above the next line.
    /// What is dropped is given back; the file goes back to `Window`.
    pub fn clear(&mut self, parsed: &mut ParsedFile, time: &str) -> Trimmed {
        let removed = parsed.entries.len();
        // A new vector, not `clear`: the old one's memory is freed.
        parsed.entries = Vec::new();
        parsed.lines = Arc::new(Vec::new());
        parsed.raw_breaks.clear();
        parsed.max_chars = crate::column::FieldMaxChars::new();
        self.byte_lens = Vec::new();
        self.parser.reset();
        self.replace_last = false;
        self.hold = HoldMode::Window;
        self.first_index = self.next_index;
        self.clear_mark = Some(ClearMark {
            generation: self.generation,
            line_no: self.next_index,
            offset: self.next_offset,
            time: time.to_string(),
        });
        self.add_pending(BreakMark {
            kind: BreakKind::Cleared,
            label: format!("{} {} {time}", self.name, BreakKind::Cleared.label()),
        });
        Trimmed { removed, generation: self.generation, first_kept: usize::MAX }
    }

    /// Where the oldest held line starts in the file on the Server.
    pub fn held_start(&self, parsed: &ParsedFile) -> HeldStart {
        let Some(first) = parsed.entries.first() else {
            return HeldStart {
                generation: self.generation,
                line_no: self.next_index,
                offset: Some(self.next_offset),
            };
        };
        if first.generation != self.generation {
            return HeldStart { generation: first.generation, line_no: first.index, offset: None };
        }
        // The lines of the current Generation are all the lines held, then
        // the offset is the end of the last minus their lengths.
        let held: u64 = self.byte_lens.iter().map(|b| u64::from(*b)).sum();
        HeldStart {
            generation: first.generation,
            line_no: first.index,
            offset: self.next_offset.checked_sub(held),
        }
    }

    /// Older lines read by Load More, inserted above the oldest held line.
    /// The file keeps them: it is `Extended` from now on. Returns false when
    /// they do not belong above the lines held (another Generation).
    pub fn apply_older(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        bytes: &[u32],
        reached_start: bool,
    ) -> bool {
        if parsed.entries.first().is_some_and(|e| e.generation != self.generation) {
            return false;
        }
        // The lines are continuous now: the Break that said older lines were
        // left out above the oldest one goes.
        if let Some(first) = parsed.entries.first_mut() {
            if first.break_before.as_ref().is_some_and(|m| {
                matches!(m.kind, BreakKind::LeftWindow | BreakKind::StartOfLoaded)
            }) {
                first.break_before = None;
                parsed.raw_breaks.remove(&0);
            }
        }
        self.apply_backfill_with(parsed, lines, bytes);
        if !lines.is_empty() {
            self.hold = HoldMode::Extended;
        }
        if !reached_start && !lines.is_empty() {
            if let Some(first) = parsed.entries.get_mut(0) {
                if first.break_before.is_none() && first.generation == self.generation {
                    let mark = Arc::new(BreakMark {
                        kind: BreakKind::StartOfLoaded,
                        label: format!("{} {}", self.name, BreakKind::StartOfLoaded.label()),
                    });
                    first.break_before = Some(mark.clone());
                    parsed.raw_breaks.insert(0, mark);
                }
            }
        }
        true
    }

    /// Load More found the file changed under the lines held: a Break above
    /// the oldest one says so.
    pub fn mark_changed(&mut self, parsed: &mut ParsedFile) {
        let mark = Arc::new(BreakMark {
            kind: BreakKind::Changed,
            label: format!("{} {}", self.name, BreakKind::Changed.label()),
        });
        match parsed.entries.first_mut() {
            Some(first) => {
                first.break_before = Some(mark.clone());
                parsed.raw_breaks.insert(0, mark);
            }
            None => self.add_pending(BreakMark { kind: BreakKind::Changed, label: mark.label.clone() }),
        }
    }

    /// The source reported the inode of the file read now.
    pub fn set_inode(&mut self, inode: Option<u64>) {
        let generation = self.generation;
        match self.inodes.iter_mut().find(|(g, _)| *g == generation) {
            Some(slot) => slot.1 = inode,
            None => self.inodes.push((generation, inode)),
        }
    }

    /// The inode of a Generation, when the source told it.
    pub fn inode_of(&self, generation: u32) -> Option<u64> {
        self.inodes.iter().find(|(g, _)| *g == generation).and_then(|(_, i)| *i)
    }

    /// Whether the file holds up to the first line of its Generation.
    pub fn at_generation_start(&self, parsed: &ParsedFile) -> bool {
        self.numbers_known && self.held_start(parsed).line_no <= 1
    }
}

fn continuation_of(entry: &crate::model::LogEntry, parent: usize) -> crate::model::LogEntry {
    let mut e = entry.clone();
    e.parent = Some(parent);
    e.time_str = Default::default();
    e.has_time = false;
    e
}
