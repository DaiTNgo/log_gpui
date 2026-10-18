//! Markers: colors the user paints on a stretch of text in one line, and the
//! Level Marker the Gutter shows for error and warning lines.

use crate::model::LogLevel;
use crate::palette::HighlightColor;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// A color painted on `range` (bytes of the line's stored text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextMarker {
    pub range: Range<usize>,
    pub color: HighlightColor,
}

/// Paint `color` on `range` of one line's Markers. Markers never overlap: an
/// older Marker under the new one is cut, and the part under it takes the new
/// color. Returns false (and changes nothing) for an empty range.
pub fn add_marker(markers: &mut Vec<TextMarker>, range: Range<usize>, color: HighlightColor) -> bool {
    if range.start >= range.end {
        return false;
    }
    let mut next = Vec::with_capacity(markers.len() + 2);
    for m in markers.drain(..) {
        if m.range.end <= range.start || m.range.start >= range.end {
            next.push(m);
            continue;
        }
        if m.range.start < range.start {
            next.push(TextMarker { range: m.range.start..range.start, color: m.color.clone() });
        }
        if m.range.end > range.end {
            next.push(TextMarker { range: range.end..m.range.end, color: m.color });
        }
    }
    next.push(TextMarker { range, color });
    next.sort_by_key(|m| m.range.start);
    *markers = next;
    true
}

/// The Marker containing `byte`, if any.
pub fn marker_at(markers: &[TextMarker], byte: usize) -> Option<&TextMarker> {
    markers.iter().find(|m| m.range.start <= byte && byte < m.range.end)
}

/// Remove the whole Marker containing `byte`. Returns whether one was there.
pub fn remove_marker_at(markers: &mut Vec<TextMarker>, byte: usize) -> bool {
    match markers.iter().position(|m| m.range.start <= byte && byte < m.range.end) {
        Some(pos) => {
            markers.remove(pos);
            true
        }
        None => false,
    }
}

/// Give the Marker containing `byte` another color.
pub fn recolor_marker_at(markers: &mut [TextMarker], byte: usize, color: HighlightColor) -> bool {
    match markers.iter_mut().find(|m| m.range.start <= byte && byte < m.range.end) {
        Some(m) => {
            m.color = color;
            true
        }
        None => false,
    }
}

/// What the Gutter shows for a line by the meaning of its level. Not a Marker:
/// the user cannot add or remove it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelMarker {
    Error,
    Warning,
}

impl LevelMarker {
    pub fn symbol(self) -> &'static str {
        match self {
            LevelMarker::Error => "●",
            LevelMarker::Warning => "▲",
        }
    }
}

pub fn level_marker(level: LogLevel) -> Option<LevelMarker> {
    match level {
        LogLevel::Fatal | LogLevel::Error => Some(LevelMarker::Error),
        LogLevel::Warn => Some(LevelMarker::Warning),
        _ => None,
    }
}

/// An entry in the overview ruler (minimap indicator)
#[derive(Debug, Clone)]
pub struct RulerMark {
    /// Normalized position in file: 0.0 ..= 1.0
    pub position: f32,
    /// Absolute row index in the all_entries
    pub entry_index: usize,
    /// Position among the rows shown.
    pub row_pos: usize,
    /// Color of the first Marker on the line, when this is a Marker.
    pub color: Option<HighlightColor>,
    /// Whether this is a search result match
    pub is_search: bool,
    /// Whether this is the currently selected search match
    pub is_current_match: bool,
}
