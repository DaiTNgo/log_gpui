//! A text selection across rows, counted in bytes of each row's stored line.
//!
//! The same model drives the Parsed View and the Raw View: a position is a row
//! (its position in the rows shown) and a byte of that row's `raw_line`.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextPos {
    /// Position of the row among the rows shown.
    pub row: usize,
    /// Byte offset in the row's line.
    pub byte: usize,
}

impl TextPos {
    pub fn new(row: usize, byte: usize) -> Self {
        Self { row, byte }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSelection {
    /// Where the drag started.
    pub anchor: TextPos,
    /// Where the pointer is now.
    pub head: TextPos,
}

impl TextSelection {
    pub fn new(anchor: TextPos, head: TextPos) -> Self {
        Self { anchor, head }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The two ends in reading order.
    pub fn ordered(&self) -> (TextPos, TextPos) {
        if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) }
    }

    pub fn move_head(&mut self, head: TextPos) {
        self.head = head;
    }

    /// The part of a row's line that is selected: from the start to the end of
    /// the line for the rows between, from the anchor on the first row, up to
    /// the head on the last. `None` when nothing of the row is selected.
    pub fn range_in_row(&self, row: usize, line_len: usize) -> Option<Range<usize>> {
        let (start, end) = self.ordered();
        if row < start.row || row > end.row || self.is_empty() {
            return None;
        }
        let from = if row == start.row { start.byte.min(line_len) } else { 0 };
        let to = if row == end.row { end.byte.min(line_len) } else { line_len };
        (from < to).then_some(from..to)
    }

    /// Whether `row` is inside the selection (its text, or just its line end).
    pub fn covers_row(&self, row: usize) -> bool {
        let (start, end) = self.ordered();
        !self.is_empty() && row >= start.row && row <= end.row
    }

    /// The text to copy: the selected part of each row's stored line, one per
    /// row, joined by newlines. `rows` are the rows currently shown as
    /// `(row position, line)`; a row a Filter hides is simply not in it.
    /// The rows between the ends count even when empty.
    pub fn copy_text<'a>(&self, rows: impl IntoIterator<Item = (usize, &'a str)>) -> String {
        let (start, end) = self.ordered();
        let mut parts: Vec<&str> = Vec::new();
        for (row, line) in rows {
            if row < start.row || row > end.row || self.is_empty() {
                continue;
            }
            let from = if row == start.row { start.byte.min(line.len()) } else { 0 };
            let to = if row == end.row { end.byte.min(line.len()) } else { line.len() };
            if from <= to {
                parts.push(line.get(from..to).unwrap_or(""));
            }
        }
        parts.join("\n")
    }
}

/// The part of `selected` that lies inside `span` (a cell's range in the
/// line), or `None` when they do not meet.
pub fn clip_to_span(selected: &Range<usize>, span: &Range<usize>) -> Option<Range<usize>> {
    let start = selected.start.max(span.start);
    let end = selected.end.min(span.end);
    (start < end).then_some(start..end)
}

/// Move a byte offset to the nearest character boundary at or before it.
pub fn floor_char_boundary(text: &str, mut byte: usize) -> usize {
    byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// The byte offset in `text` of the character whose cell contains `x`, when
/// every character is `char_width` wide. Characters count as wide (two cells)
/// when `is_wide` says so (CJK, emoji), so the hit-test follows the glyphs.
pub fn byte_at_x(text: &str, x: f32, char_width: f32) -> usize {
    if x <= 0.0 {
        return 0;
    }
    let mut cursor = 0.0;
    for (byte, ch) in text.char_indices() {
        let w = char_width * char_cells(ch);
        if x < cursor + w / 2.0 {
            return byte;
        }
        cursor += w;
        if x < cursor {
            return byte + ch.len_utf8();
        }
    }
    text.len()
}

/// Width of a character in monospace cells: wide glyphs take two.
pub fn char_cells(ch: char) -> f32 {
    let c = ch as u32;
    let wide = matches!(c,
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF
        | 0x20000..=0x3FFFD);
    if wide { 2.0 } else { 1.0 }
}
