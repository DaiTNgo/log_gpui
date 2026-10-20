//! The lines a live file has delivered so far: what `SourceEvent`s do to the
//! parsed cache of one file (`ParsedFile`).
//!
//! Line numbers: a local file is loaded whole, so its numbers are exact from
//! the start. A remote file shows its last lines first and learns the real
//! number of lines later (`TotalLines`), so until then its lines carry
//! provisional numbers from `PROVISIONAL_BASE` up; the Gutter shows none for
//! them.

use super::source::{BreakKind, RestartKind, SourceEvent, StreamMask, clock_text};
use crate::parser::LineParser;
use crate::space_view::ParsedFile;
use std::collections::VecDeque;
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
    /// Lines read ahead of the app applying them (`PrepareJob`), in the order
    /// they will be applied. An entry that does not fit what is applied is
    /// dropped and the lines are read again.
    prepared: VecDeque<PreparedLines>,
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
    /// Bytes the removed lines took in the Raw View's text: each line and
    /// its newline.
    pub text_bytes: usize,
}

/// How a run of lines is read: one after another with the Parent Line carried
/// on from the lines before (the lines that follow what is held), or on their
/// own (lines inserted above).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrepKind {
    Sequential,
    Fresh,
}

/// The entries of lines that were read before the app applied them.
#[derive(Debug, Clone)]
pub struct PreparedLines {
    kind: PrepKind,
    /// The number the first line got.
    first: usize,
    parent_before: Option<usize>,
    parent_after: Option<usize>,
    reading: LineParser,
    entries: Vec<crate::model::LogEntry>,
}

/// What a worker needs to read the lines of a batch of events before the app
/// applies them: the state of the buffer's parser and numbering when it was
/// taken. Reading ahead is a guess about the state the events will be applied
/// in; `LiveBuffer` uses it only when the guess was right.
#[derive(Debug, Clone)]
pub struct PrepareJob {
    parser: LineParser,
    next_index: usize,
    first_index: usize,
    /// Whether the oldest line held is of the current Generation (what
    /// `Older` lines go above).
    older_goes_above_current: bool,
    replace_last: bool,
}

impl PrepareJob {
    /// Read the lines of `events` in the order they will be applied. Stops
    /// at the first event whose effect on the numbering is not known here.
    pub fn run(mut self, events: &[SourceEvent]) -> VecDeque<PreparedLines> {
        let mut out = VecDeque::new();
        for event in events {
            match event {
                SourceEvent::Append { lines, .. } => {
                    if self.replace_last {
                        break;
                    }
                    out.push_back(self.sequential(lines));
                }
                SourceEvent::Tail { lines, .. } => {
                    let start = self.next_index;
                    out.push_back(self.sequential(lines));
                    self.first_index = start;
                }
                SourceEvent::Backfill { lines, .. } => out.push_back(self.fresh(lines)),
                SourceEvent::Older { lines, .. } => {
                    if !self.older_goes_above_current {
                        break;
                    }
                    out.push_back(self.fresh(lines));
                }
                SourceEvent::Status(_)
                | SourceEvent::Progress(_)
                | SourceEvent::Inode(_)
                | SourceEvent::Problem(_)
                | SourceEvent::Run { .. } => {}
                _ => break,
            }
        }
        out
    }

    fn sequential(&mut self, lines: &[String]) -> PreparedLines {
        let first = self.next_index;
        let parent_before = self.parser.parent();
        let reading = self.parser.clone();
        let entries: Vec<_> = lines.iter().enumerate().map(|(i, line)| self.parser.parse(first + i, line)).collect();
        self.next_index += lines.len();
        PreparedLines {
            kind: PrepKind::Sequential,
            first,
            parent_before,
            parent_after: self.parser.parent(),
            reading,
            entries,
        }
    }

    fn fresh(&mut self, lines: &[String]) -> PreparedLines {
        let first = self.first_index.saturating_sub(lines.len());
        let mut parser = LineParser::new();
        parser.set_format(self.parser.format());
        let reading = self.parser.clone();
        let entries: Vec<_> = lines.iter().enumerate().map(|(i, line)| parser.parse(first + i, line)).collect();
        self.first_index = first;
        PreparedLines { kind: PrepKind::Fresh, first, parent_before: None, parent_after: None, reading, entries }
    }
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
            prepared: VecDeque::new(),
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
            byte_lens: Vec::new(),
            prepared: VecDeque::new(),
            next_offset: 0,
            hold: HoldMode::Window,
            clear_mark: None,
            inodes: Vec::new(),
        }
    }

    /// Whether the next appended line replaces the last loaded one.
    /// Read new lines with the Log Format `format` (`None`: Auto-detect).
    pub fn set_format(&mut self, format: Option<std::sync::Arc<crate::log_format::CompiledFormat>>) {
        self.parser.set_format(format);
    }

    pub fn replaces_last_line(&self) -> bool {
        self.replace_last
    }

    pub fn first_index(&self) -> usize {
        self.first_index
    }

    /// What a worker needs to read the lines of events ahead of their being
    /// applied. `top_generation` is the Generation of the oldest line held.
    pub fn prepare_job(&self, top_generation: Option<u32>) -> PrepareJob {
        PrepareJob {
            parser: self.parser.clone(),
            next_index: self.next_index,
            first_index: self.first_index,
            older_goes_above_current: top_generation.is_none_or(|g| g == self.generation),
            replace_last: self.replace_last,
        }
    }

    /// Lines read ahead, to be used by the `apply_*` call that matches them.
    pub fn set_prepared(&mut self, prepared: VecDeque<PreparedLines>) {
        self.prepared = prepared;
    }

    /// The first prepared lines when they are the ones being applied: read in
    /// the same way, with the same numbers (and, one after another, the same
    /// Parent Line). Anything else prepared is not used.
    fn take_prepared(&mut self, kind: PrepKind, first: usize, count: usize) -> Option<PreparedLines> {
        let front = self.prepared.pop_front()?;
        let fits = front.kind == kind
            && front.first == first
            && front.entries.len() == count
            && front.reading.reads_like(&self.parser)
            && (kind == PrepKind::Fresh || front.parent_before == self.parser.parent());
        if fits {
            Some(front)
        } else {
            self.prepared.clear();
            None
        }
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
        self.push_lines(parsed, lines, bytes, None);
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
        let first = self.first_index.saturating_sub(lines.len());
        // The Generation's first loaded line is where the new lines go.
        let at = parsed
            .entries
            .iter()
            .position(|e| e.generation == self.generation)
            .unwrap_or(parsed.entries.len());
        self.insert_above(parsed, self.generation, at, first, lines, bytes);
        self.first_index = first;
    }

    /// Insert `lines` of `generation` at `at`, numbered from `first`. Lines at
    /// the top of what was loaded in that Generation that continued a line not
    /// yet known now continue the last line with a time of the new lines.
    fn insert_above(
        &mut self,
        parsed: &mut ParsedFile,
        generation: u32,
        at: usize,
        first: usize,
        lines: &[String],
        bytes: &[u32],
    ) {
        let count = lines.len();
        let mut entries: Vec<_> = match self.take_prepared(PrepKind::Fresh, first, count) {
            Some(p) => p.entries,
            None => {
                let mut parser = LineParser::new();
                parser.set_format(self.parser.format());
                lines.iter().enumerate().map(|(i, line)| parser.parse(first + i, line)).collect()
            }
        };
        for entry in entries.iter_mut() {
            entry.generation = generation;
        }
        let last_parent = entries.iter().rev().find(|e| e.has_time).map(|e| e.index);
        for entry in parsed.entries[at..].iter_mut() {
            if entry.generation != generation || entry.has_time || entry.parent.is_some() {
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
    }

    /// Lines of a Rotated File, an earlier Generation, above everything held.
    /// With `start` they begin a new Generation: every Generation held moves
    /// one up and a Break "rotated" goes between. Without it they are more
    /// lines at the top of the Generation loaded last. Returns whether the
    /// Generations moved (the app moves the file's own count with them).
    pub fn apply_earlier(
        &mut self,
        parsed: &mut ParsedFile,
        start: Option<&super::source::EarlierStart>,
        lines: &[String],
        bytes: &[u32],
        last_line_no: Option<usize>,
    ) -> bool {
        if lines.is_empty() && start.is_none() {
            return false;
        }
        let count = lines.len();
        let moved = start.is_some();
        let (generation, first) = match start {
            Some(_) => {
                for entry in parsed.entries.iter_mut() {
                    entry.generation += 1;
                }
                self.generation += 1;
                for (g, _) in self.inodes.iter_mut() {
                    *g += 1;
                }
                if let Some(mark) = self.clear_mark.as_mut() {
                    mark.generation += 1;
                }
                // The first line after the Break loses what said it was the
                // top of the lines.
                let first = match last_line_no {
                    Some(last) => (last + 1).saturating_sub(count),
                    None => PROVISIONAL_BASE,
                };
                (0, first)
            }
            None => match parsed.entries.first() {
                Some(top) => (top.generation, top.index.saturating_sub(count)),
                None => return false,
            },
        };
        if let Some(top) = parsed.entries.first_mut() {
            if top.break_before.as_ref().is_some_and(|m| {
                matches!(
                    m.kind,
                    BreakKind::LeftWindow
                        | BreakKind::StartOfLoaded
                        | BreakKind::EarlierAvailable
                        | BreakKind::EarlierWindowFull
                        | BreakKind::StartTimeOverflow
                )
            }) {
                top.break_before = None;
                parsed.raw_breaks.remove(&0);
            }
        }
        if count > 0 {
            self.insert_above(parsed, generation, 0, first, lines, bytes);
        }
        if let Some(start) = start {
            // The Break sits above the first line of the newer Generation.
            if let Some(newer) = parsed.entries.get_mut(count) {
                let when = start.modified.as_deref().map(|m| format!(" {m}")).unwrap_or_default();
                let mark = Arc::new(BreakMark {
                    kind: BreakKind::Rotated,
                    label: format!("{} rotated{when}", self.name),
                });
                newer.break_before = Some(mark.clone());
                parsed.raw_breaks.insert(count, mark);
            }
        }
        moved
    }

    /// The Live Window filled before the Starting Time: a Break on top of the
    /// lines says how many lines there are since it.
    pub fn set_start_overflow(&mut self, parsed: &mut ParsedFile, lines: Option<usize>, since: &str) {
        let Some(top) = parsed.entries.first_mut() else { return };
        let count = lines.map(|n| format!("{} lines", group(n))).unwrap_or_else(|| "Lines".to_string());
        let mark = Arc::new(BreakMark {
            kind: BreakKind::StartTimeOverflow,
            label: format!("{count} since {since} do not fit the Live Window"),
        });
        top.break_before = Some(mark.clone());
        parsed.raw_breaks.insert(0, mark);
    }

    // ── Remote Copy ─────────────────────────────────────────────────────────

    /// A Generation of a Remote Copy begins: the first one just numbers its
    /// lines; a later one is a new Generation with a Break `label` above its
    /// first line. `first_line_no` is the number of its first line in its
    /// file, or `None` while that is being counted (the lines carry
    /// provisional numbers until `set_first_line_no`).
    pub fn begin_copy_generation(&mut self, first_line_no: Option<usize>, first: bool, label: &str) {
        if !first {
            self.begin_generation_state();
            self.add_pending(BreakMark { kind: BreakKind::Rotated, label: label.to_string() });
        }
        let base = first_line_no.unwrap_or(PROVISIONAL_BASE);
        self.first_index = base;
        self.next_index = base;
        self.numbers_known = first_line_no.is_some();
        self.tail_last_index = None;
    }

    /// The number of the first line of the Generation is known now. Returns
    /// how far every number of the Generation moved.
    pub fn set_first_line_no(&mut self, parsed: &mut ParsedFile, first_line_no: usize) -> Option<isize> {
        if self.numbers_known {
            return None;
        }
        let delta = first_line_no as isize - self.first_index as isize;
        let generation = self.generation;
        for entry in parsed.entries.iter_mut().filter(|e| e.generation == generation) {
            entry.index = (entry.index as isize + delta) as usize;
            entry.parent = entry.parent.map(|p| (p as isize + delta) as usize);
        }
        self.first_index = (self.first_index as isize + delta) as usize;
        self.next_index = (self.next_index as isize + delta) as usize;
        self.numbers_known = true;
        Some(delta)
    }

    /// The lines of the Remote Copy end here, short of the end of the file:
    /// a Break is drawn below the last line.
    pub fn mark_read_end(&mut self, parsed: &mut ParsedFile, kind: BreakKind, label: &str) {
        if let Some(last) = parsed.entries.last_mut() {
            last.break_after = Some(Arc::new(BreakMark { kind, label: label.to_string() }));
        }
    }

    /// Older lines were not read: a Break above the first line says so.
    pub fn mark_older_not_read(&mut self, parsed: &mut ParsedFile, label: &str) {
        if let Some(top) = parsed.entries.first_mut() {
            let mark = Arc::new(BreakMark { kind: BreakKind::OlderNotRead, label: label.to_string() });
            top.break_before = Some(mark.clone());
            parsed.raw_breaks.insert(0, mark);
        }
    }

    /// Draw (or take away) the Break at the top of the lines that offers the
    /// next Rotated File. It only exists while the top line is the first line
    /// of its Generation.
    pub fn set_earlier_available(
        &mut self,
        parsed: &mut ParsedFile,
        file: Option<&super::source::EarlierFile>,
        window_full: bool,
    ) {
        let Some(top) = parsed.entries.first_mut() else { return };
        let is_ours =
            |m: &Arc<BreakMark>| matches!(m.kind, BreakKind::EarlierAvailable | BreakKind::EarlierWindowFull);
        match file {
            Some(file) => {
                if top.break_before.is_none() {
                    let size = size_text(file.size);
                    let (kind, label) = if window_full {
                        (BreakKind::EarlierWindowFull, format!("Live Window is full; earlier generation: {}, {size}", file.name))
                    } else {
                        (BreakKind::EarlierAvailable, format!("Load earlier generation ({}, {size})", file.name))
                    };
                    let mark = Arc::new(BreakMark { kind, label });
                    top.break_before = Some(mark.clone());
                    parsed.raw_breaks.insert(0, mark);
                }
            }
            None => {
                if top.break_before.as_ref().is_some_and(is_ours) {
                    top.break_before = None;
                    parsed.raw_breaks.remove(&0);
                }
            }
        }
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
        self.apply_append_streams(parsed, lines, bytes, None)
    }

    /// `apply_append_with` for the lines of a Remote Command: `streams` says
    /// which output each line is from.
    pub fn apply_append_streams(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        bytes: &[u32],
        streams: Option<&StreamMask>,
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
        self.push_lines(parsed, lines, bytes, streams)
    }

    /// Keep `byte_lens` as long as the entries were before an insertion of
    /// lines (they are padded when a source gave no lengths).
    fn sync_byte_lens(&mut self, len: usize) {
        self.byte_lens.resize(len, 0);
    }

    fn push_lines(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        bytes: &[u32],
        streams: Option<&StreamMask>,
    ) -> std::ops::Range<usize> {
        let from = parsed.entries.len();
        self.sync_byte_lens(from);
        for i in 0..lines.len() {
            let len = bytes.get(i).copied().unwrap_or(0);
            self.byte_lens.push(len);
            self.next_offset += u64::from(len);
        }
        let mut prepared = self.take_prepared(PrepKind::Sequential, self.next_index, lines.len()).map(|p| {
            self.parser.set_parent(p.parent_after);
            p.entries.into_iter()
        });
        for (i, line) in lines.iter().enumerate() {
            let mut entry = match prepared.as_mut().and_then(|it| it.next()) {
                Some(entry) => entry,
                None => self.parser.parse(self.next_index, line),
            };
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

    /// The Remote Command was edited: the Break names both commands, and the
    /// lines after it are a new Generation.
    pub fn apply_command_changed(&mut self, from: &str, to: &str, time: &str) {
        self.begin_generation_state();
        self.name = to.to_string();
        self.disconnected_at = None;
        self.add_pending(BreakMark {
            kind: BreakKind::CommandChanged,
            label: format!("command changed {time}: {from} → {to}"),
        });
    }

    fn begin_generation_state(&mut self) {
        self.generation += 1;
        self.parser.reset();
        self.first_index = 1;
        self.next_index = 1;
        self.next_offset = 0;
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
        let text_bytes = parsed.lines.iter().take(removed).map(|l| l.len() + 1).sum();
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
        Some(Trimmed { removed, generation, first_kept, text_bytes })
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
        Trimmed { removed, generation: self.generation, first_kept: usize::MAX, text_bytes: 0 }
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
        self.apply_older_with(parsed, lines, bytes, reached_start, false)
    }

    /// `apply_older`. The lines may belong to a Rotated File (an older
    /// Generation than the one being read) when that is what the oldest held
    /// line is of. `before_start_time`: they are from before the Starting
    /// Time the file was opened with, which a Break says.
    pub fn apply_older_with(
        &mut self,
        parsed: &mut ParsedFile,
        lines: &[String],
        bytes: &[u32],
        reached_start: bool,
        before_start_time: bool,
    ) -> bool {
        let top_generation = parsed.entries.first().map(|e| e.generation);
        // The lines are continuous now: what said older lines were left out
        // or offered above the oldest one goes.
        if let Some(first) = parsed.entries.first_mut() {
            if !lines.is_empty()
                && first.break_before.as_ref().is_some_and(|m| {
                    matches!(
                        m.kind,
                        BreakKind::LeftWindow
                            | BreakKind::StartOfLoaded
                            | BreakKind::EarlierAvailable
                            | BreakKind::EarlierWindowFull
                            | BreakKind::StartTimeOverflow
                    )
                })
            {
                first.break_before = None;
                parsed.raw_breaks.remove(&0);
            }
        }
        match top_generation {
            Some(g) if g != self.generation => {
                let first = parsed.entries[0].index.saturating_sub(lines.len());
                if !lines.is_empty() {
                    self.insert_above(parsed, g, 0, first, lines, bytes);
                }
            }
            _ => self.apply_backfill_with(parsed, lines, bytes),
        }
        if !lines.is_empty() {
            self.hold = HoldMode::Extended;
        }
        if before_start_time && !lines.is_empty() {
            if let Some(joined) = parsed.entries.get_mut(lines.len()) {
                if joined.break_before.is_none() {
                    let mark = Arc::new(BreakMark {
                        kind: BreakKind::BeforeStartTime,
                        label: format!(
                            "{} {} lines before the Starting Time loaded",
                            self.name,
                            group(lines.len())
                        ),
                    });
                    joined.break_before = Some(mark.clone());
                    parsed.raw_breaks.insert(lines.len(), mark);
                }
            }
        }
        if !reached_start && !lines.is_empty() {
            if let Some(first) = parsed.entries.get_mut(0) {
                if first.break_before.is_none() {
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

    /// Bytes in the file of the lines held of the Generation of the oldest
    /// one (they are the first lines held).
    pub fn top_generation_bytes(&self, parsed: &ParsedFile) -> u64 {
        let Some(top) = parsed.entries.first() else { return 0 };
        let n = parsed.entries.iter().take_while(|e| e.generation == top.generation).count();
        self.byte_lens.iter().take(n).map(|b| u64::from(*b)).sum()
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

fn group(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
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

fn continuation_of(entry: &crate::model::LogEntry, parent: usize) -> crate::model::LogEntry {
    let mut e = entry.clone();
    e.parent = Some(parent);
    e.time_str = Default::default();
    e.has_time = false;
    e
}
