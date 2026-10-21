//! Raw View: every file's text line by line, one collapsible File Section per
//! file. It ignores Filter, so Search here looks at every line of every
//! section.

use crate::search_expr::Matcher;

/// A line of a File Section that matches the Search Expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawMatch {
    pub section: usize,
    /// 0-based line within the section.
    pub line: usize,
}

/// Find the Matches in every line of every section, in section order.
pub fn search_sections<S: AsRef<str>>(matcher: &Matcher, sections: &[&[S]]) -> Vec<RawMatch> {
    let mut matches = Vec::new();
    for (section, lines) in sections.iter().enumerate() {
        for (line, text) in lines.iter().enumerate() {
            if matcher.is_match(text.as_ref()) {
                matches.push(RawMatch { section, line });
            }
        }
    }
    matches
}

/// Rows of the flat list that shows the sections: one header per section,
/// then its lines unless it is collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawRow {
    Header { section: usize },
    Line { section: usize, line: usize },
}

/// The row at `index` of the flat list. `sections` gives each section's line
/// count and whether it is collapsed.
pub fn row_at(sections: &[(usize, bool)], mut index: usize) -> Option<RawRow> {
    for (section, &(lines, collapsed)) in sections.iter().enumerate() {
        if index == 0 {
            return Some(RawRow::Header { section });
        }
        index -= 1;
        if collapsed {
            continue;
        }
        if index < lines {
            return Some(RawRow::Line { section, line: index });
        }
        index -= lines;
    }
    None
}

/// Total number of rows of the flat list.
pub fn row_count(sections: &[(usize, bool)]) -> usize {
    sections
        .iter()
        .map(|&(lines, collapsed)| 1 + if collapsed { 0 } else { lines })
        .sum()
}

/// Index in the flat list of line `line` of `section`, expanding is the
/// caller's job: `None` when the section is collapsed.
pub fn index_of_line(sections: &[(usize, bool)], section: usize, line: usize) -> Option<usize> {
    let mut index = 0;
    for (i, &(lines, collapsed)) in sections.iter().enumerate() {
        if i == section {
            return if collapsed || line >= lines { None } else { Some(index + 1 + line) };
        }
        index += 1 + if collapsed { 0 } else { lines };
    }
    None
}

/// The pieces of a text range of an editor holding `lines` joined by `\n`:
/// each line the range touches, with its 1-based number and the selected
/// bytes of that line. Line breaks themselves are not part of any piece.
pub fn pieces_from_range<S: AsRef<str>>(
    lines: &[S],
    range: std::ops::Range<usize>,
) -> Vec<(usize, std::ops::Range<usize>)> {
    let mut pieces = Vec::new();
    let mut offset = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let len = line.as_ref().len();
        let start = range.start.max(offset);
        let end = range.end.min(offset + len);
        if start < end {
            pieces.push((i + 1, start - offset..end - offset));
        }
        offset += len + 1;
        if offset > range.end {
            break;
        }
    }
    pieces
}

/// The line (1-based) and the byte of it that an editor offset is at.
pub fn line_and_byte<S: AsRef<str>>(lines: &[S], cursor: usize) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let len = line.as_ref().len();
        if cursor <= offset + len {
            return Some((i + 1, cursor - offset));
        }
        offset += len + 1;
    }
    None
}

/// `line_and_byte` read from the editor's own text instead of walking every
/// line: `lines` is how many lines the editor holds. The editor's rope finds
/// the line in time that grows with the log of the text, so a caret at the
/// end of a million lines costs no more than one at the top.
pub fn line_and_byte_in_rope(rope: &gpui_component::input::Rope, cursor: usize, lines: usize) -> Option<(usize, usize)> {
    use gpui_component::input::RopeExt as _;
    if cursor > rope.len() {
        return None;
    }
    let row = rope.offset_to_position(cursor).line as usize;
    (row < lines).then(|| (row + 1, cursor - rope.line_start_offset(row)))
}

/// Byte offset in the editor text of the start of each line.
pub fn line_starts<S: AsRef<str>>(lines: &[S]) -> Vec<usize> {
    let mut offset = 0usize;
    lines
        .iter()
        .map(|l| {
            let start = offset;
            offset += l.as_ref().len() + 1;
            start
        })
        .collect()
}
