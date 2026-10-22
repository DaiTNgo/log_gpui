//! What each cell of a row shows and where it sits: the text of a cell is a
//! span of the stored line, the paint on it is composed from layers, and a
//! pointer position maps back to a byte of the line.

use crate::column::{ColumnLayoutState, PinnableColumn};
use crate::model::{FieldKey, LogEntry};
use crate::text_selection::floor_char_boundary;
use gpui::{HighlightStyle, Hsla};
use std::ops::Range;

/// A cell that shows text from the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCell {
    Timestamp,
    Level,
    Module,
    Message,
    /// The whole stored line, as the Focus Pane shows it.
    Raw,
    /// A Field Column.
    Field(FieldKey),
}

impl TextCell {
    /// Key of the cell's field in `LogEntry::spans`.
    pub fn field(self) -> &'static str {
        match self {
            TextCell::Timestamp => "timestamp",
            TextCell::Level => "level",
            TextCell::Module => "module",
            TextCell::Message => "message",
            TextCell::Raw => "raw",
            TextCell::Field(k) => k.as_str(),
        }
    }

    /// Column key a Highlight Rule uses for this cell.
    pub fn column_key(self) -> &'static str {
        self.field()
    }

    /// Space between the cell's left edge and its text, in pixels.
    pub fn padding(self) -> f32 {
        match self {
            TextCell::Level => 4.0,
            _ => 8.0,
        }
    }

    pub fn from_column(col: PinnableColumn) -> Option<Self> {
        match col {
            PinnableColumn::Timestamp => Some(TextCell::Timestamp),
            PinnableColumn::Level => Some(TextCell::Level),
            PinnableColumn::Module => Some(TextCell::Module),
            PinnableColumn::Field(k) => Some(TextCell::Field(k)),
            PinnableColumn::Source | PinnableColumn::Stream => None,
        }
    }
}

/// The text of a cell and, when it comes from the line, its byte range there.
/// A Continuation Line has text only in the Message cell; its other cells
/// show a dim `_` that is not text.
pub fn cell_text(entry: &LogEntry, cell: TextCell) -> Option<(&str, Option<Range<usize>>)> {
    if cell == TextCell::Raw {
        return Some((entry.raw_line.as_str(), Some(0..entry.raw_line.len())));
    }
    if let Some(range) = entry.span(cell.field()) {
        if let Some(text) = entry.raw_line.get(range.clone()) {
            return Some((text, Some(range.clone())));
        }
    }
    if entry.is_continuation() {
        return None;
    }
    // An entry built without spans shows its fields as they are.
    let text = match cell {
        TextCell::Timestamp => entry.time_str(),
        TextCell::Level => entry.level_text(),
        TextCell::Module => entry.tag(),
        TextCell::Message => entry.payload(),
        TextCell::Raw => entry.raw_line.as_str(),
        TextCell::Field(_) => "",
    };
    (!text.is_empty()).then_some((text, None))
}

/// Whether the cell shows the dim `_` placeholder instead of text.
pub fn shows_placeholder(entry: &LogEntry, cell: TextCell) -> bool {
    entry.is_continuation() && !matches!(cell, TextCell::Message | TextCell::Raw)
}

/// One paint on the text of a cell, as a range of the cell's text.
#[derive(Debug, Clone)]
pub struct Layer {
    pub range: Range<usize>,
    pub background: Option<Hsla>,
    pub color: Option<Hsla>,
}

/// Flatten layers into the non-overlapping highlights `StyledText` takes. A
/// later layer paints over an earlier one.
pub fn compose(text: &str, layers: &[Layer]) -> Vec<(Range<usize>, HighlightStyle)> {
    let len = text.len();
    let mut cuts: Vec<usize> = vec![0, len];
    let clamped: Vec<(Range<usize>, &Layer)> = layers
        .iter()
        .filter_map(|l| {
            let start = floor_char_boundary(text, l.range.start.min(len));
            let end = floor_char_boundary(text, l.range.end.min(len));
            (start < end).then_some((start..end, l))
        })
        .collect();
    for (r, _) in &clamped {
        cuts.push(r.start);
        cuts.push(r.end);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let mut style = HighlightStyle::default();
        let mut painted = false;
        for (r, layer) in &clamped {
            if r.start <= a && b <= r.end {
                if layer.background.is_some() {
                    style.background_color = layer.background;
                    painted = true;
                }
                if layer.color.is_some() {
                    style.color = layer.color;
                    painted = true;
                }
            }
        }
        if painted {
            out.push((a..b, style));
        }
    }
    out
}

/// A cell of the row under a pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// The Gutter (line number and Level Marker).
    Gutter,
    /// The Source Column.
    Source,
    /// A cell with text; `x` is the distance from the start of its text.
    Text { cell: TextCell, x: f32 },
    /// Past the last cell.
    Beyond,
}

/// Which cell is at `x` (pixels from the left edge of the canvas) with the
/// canvas scrolled `scroll_x` to the left. The anchored partition (Gutter and
/// pinned columns) stays where it is; the other cells slide under it.
pub fn hit_cell(layout: &ColumnLayoutState, x: f32, scroll_x: f32) -> Hit {
    let gutter = f32::from(layout.gutter_width());
    let anchored = f32::from(layout.sticky_partition_width());
    let width = |c: PinnableColumn| f32::from(layout.width(c));
    let cell_hit = |col: PinnableColumn, offset: f32| match TextCell::from_column(col) {
        Some(cell) => Hit::Text { cell, x: offset - cell.padding() },
        None => Hit::Source,
    };

    if x < anchored {
        if x < gutter {
            return Hit::Gutter;
        }
        let mut left = gutter;
        for &col in layout.pinned_columns() {
            let w = width(col);
            if x < left + w {
                return cell_hit(col, x - left);
            }
            left += w;
        }
        return Hit::Beyond;
    }
    let mut left = anchored - scroll_x;
    for col in layout.unpinned_columns() {
        let w = width(col);
        if x < left + w {
            return cell_hit(col, x - left);
        }
        left += w;
    }
    Hit::Text { cell: TextCell::Message, x: x - left - TextCell::Message.padding() }
}

/// The byte of the line a pointer at `hit` stands on in `entry`. A pointer in
/// the Gutter, in the Source Column or over a `_` stands at the start of the
/// line's message.
///
/// `index_for_x(text, x)` is the byte of `text` nearest `x` pixels into it, as
/// the text is drawn.
pub fn byte_for_hit(entry: &LogEntry, hit: Hit, index_for_x: impl Fn(&str, f32) -> usize) -> usize {
    match hit {
        Hit::Text { cell, x } => match cell_text(entry, cell) {
            Some((text, Some(range))) => range.start + index_for_x(text, x),
            _ => message_start(entry),
        },
        Hit::Gutter | Hit::Source => 0,
        Hit::Beyond => entry.raw_line.len(),
    }
}

fn message_start(entry: &LogEntry) -> usize {
    entry.spans.get("message").map_or(0, |r| r.start)
}

/// Where the Matches of a line are when none of them shows: the hidden
/// columns that hold them (none when they are in text no column shows, such
/// as the key names of a JSON line). `None` when a cell that is laid out
/// shows at least one Match, or there is no Match.
pub fn hidden_matches(
    entry: &LogEntry,
    ranges: &[(usize, usize)],
    layout: &ColumnLayoutState,
) -> Option<Vec<PinnableColumn>> {
    hidden_matches_in(entry, ranges, &layout.columns())
}

/// [`hidden_matches`] for the columns `shown`, which the Focus Pane has of
/// its own.
pub fn hidden_matches_in(
    entry: &LogEntry,
    ranges: &[(usize, usize)],
    shown: &[PinnableColumn],
) -> Option<Vec<PinnableColumn>> {
    if ranges.is_empty() {
        return None;
    }
    let overlaps = |span: Range<usize>| ranges.iter().any(|&(s, e)| s < span.end && span.start < e);
    let visible = shown
        .iter()
        .filter_map(|c| TextCell::from_column(*c))
        .chain([TextCell::Message])
        .filter_map(|cell| cell_text(entry, cell).and_then(|(_, span)| span))
        .any(|span| overlaps(span));
    if visible {
        return None;
    }
    let fixed = [PinnableColumn::Timestamp, PinnableColumn::Level, PinnableColumn::Module].into_iter();
    let fields = entry.fields().map(|(k, _)| PinnableColumn::Field(k)).collect::<Vec<_>>();
    let hidden = fixed
        .chain(fields)
        .filter(|c| !shown.contains(c))
        .filter(|c| entry.span(c.key()).is_some_and(overlaps))
        .collect();
    Some(hidden)
}

/// The tooltip of a line whose Matches are all out of sight.
pub fn hidden_matches_tooltip(hidden: &[PinnableColumn], layout: &ColumnLayoutState) -> String {
    if hidden.is_empty() {
        return "Matches only text no column shows".to_string();
    }
    let names: Vec<&str> = hidden.iter().map(|c| layout.header_text(*c)).collect();
    let noun = if names.len() == 1 { "column" } else { "columns" };
    format!("Matches in hidden {noun}: {}", names.join(", "))
}
