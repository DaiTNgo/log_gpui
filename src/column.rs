use crate::model::LogEntry;
use crate::theme::Density;
use gpui::{px, Pixels};
use std::collections::{BTreeSet, HashMap};

pub const DEFAULT_GUTTER_WIDTH: Pixels = px(88.0);
pub const DEFAULT_TIMESTAMP_WIDTH: Pixels = px(120.0);
pub const DEFAULT_LEVEL_WIDTH: Pixels = px(68.0);
pub const DEFAULT_MODULE_WIDTH: Pixels = px(100.0);

pub const MIN_TIMESTAMP_WIDTH: Pixels = px(48.0);
pub const MIN_LEVEL_WIDTH: Pixels = px(36.0);
pub const MIN_MODULE_WIDTH: Pixels = px(48.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PinnableColumn {
    Timestamp,
    Level,
    Module,
}

impl PinnableColumn {
    /// Returns the character length of the column header title to prevent title truncation.
    #[inline]
    pub const fn title_chars(&self) -> usize {
        match self {
            PinnableColumn::Timestamp => 9, // "TIMESTAMP"
            PinnableColumn::Level => 5,     // "LEVEL"
            PinnableColumn::Module => 6,    // "MODULE"
        }
    }
}

/// Tracks the maximum character lengths per structured field observed across log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FieldMaxChars {
    pub timestamp: usize,
    pub level: usize,
    pub module: usize,
}

impl FieldMaxChars {
    pub const fn new() -> Self {
        Self {
            timestamp: 0,
            level: 0,
            module: 0,
        }
    }

    /// Updates running maximums with a single log entry in O(1).
    #[inline]
    pub fn record_entry(&mut self, entry: &LogEntry) {
        self.timestamp = self.timestamp.max(entry.time_str.chars().count());
        self.level = self.level.max(entry.level.short_code().chars().count());
        self.module = self.module.max(entry.tag.chars().count());
    }

    /// Scans a slice of log entries in a single O(N) pass, O(1) per entry.
    pub fn scan_entries(entries: &[LogEntry]) -> Self {
        let mut max_chars = Self::new();
        for entry in entries {
            max_chars.record_entry(entry);
        }
        max_chars
    }
}

/// Computes dynamic column width based on ADR 0003:
/// Width = max(min_width, (max_chars * W_char) + padding)
/// where W_char = 0.6 * font_size.
/// Also ensures effective character length is at least the column title length
/// so column headers never truncate and widths scale consistently with font size.
#[inline]
pub fn compute_column_width(
    col: PinnableColumn,
    max_chars: usize,
    font_size: f32,
    min_width: Pixels,
) -> Pixels {
    let char_width = 0.6 * font_size;
    let effective_chars = max_chars.max(col.title_chars());
    let padding = match col {
        PinnableColumn::Timestamp => 16.0,
        PinnableColumn::Level => 24.0,
        PinnableColumn::Module => 16.0,
    };
    let content_width = (effective_chars as f32) * char_width + padding;
    px(content_width).max(min_width)
}

#[derive(Debug, Clone)]
pub struct ColumnLayoutState {
    pub pinned_columns: BTreeSet<PinnableColumn>,
    pub widths: HashMap<PinnableColumn, Pixels>,
    pub min_widths: HashMap<PinnableColumn, Pixels>,
    pub max_chars: FieldMaxChars,
    pub gutter_width: Pixels,
    pub is_resizing: Option<(PinnableColumn, Pixels)>,
}

impl Default for ColumnLayoutState {
    fn default() -> Self {
        Self::new()
    }
}

impl ColumnLayoutState {
    pub fn new() -> Self {
        let mut widths = HashMap::new();
        widths.insert(PinnableColumn::Timestamp, DEFAULT_TIMESTAMP_WIDTH);
        widths.insert(PinnableColumn::Level, DEFAULT_LEVEL_WIDTH);
        widths.insert(PinnableColumn::Module, DEFAULT_MODULE_WIDTH);

        let mut min_widths = HashMap::new();
        min_widths.insert(PinnableColumn::Timestamp, MIN_TIMESTAMP_WIDTH);
        min_widths.insert(PinnableColumn::Level, MIN_LEVEL_WIDTH);
        min_widths.insert(PinnableColumn::Module, MIN_MODULE_WIDTH);

        Self {
            pinned_columns: BTreeSet::new(),
            widths,
            min_widths,
            max_chars: FieldMaxChars::new(),
            gutter_width: DEFAULT_GUTTER_WIDTH,
            is_resizing: None,
        }
    }

    /// The Gutter is permanently anchored by domain rule.
    #[inline]
    pub fn is_gutter_anchored(&self) -> bool {
        true
    }

    #[inline]
    pub fn gutter_width(&self) -> Pixels {
        self.gutter_width
    }

    #[inline]
    pub fn width(&self, col: PinnableColumn) -> Pixels {
        self.widths.get(&col).copied().unwrap_or(match col {
            PinnableColumn::Timestamp => DEFAULT_TIMESTAMP_WIDTH,
            PinnableColumn::Level => DEFAULT_LEVEL_WIDTH,
            PinnableColumn::Module => DEFAULT_MODULE_WIDTH,
        })
    }

    #[inline]
    pub fn min_width(&self, col: PinnableColumn) -> Pixels {
        self.min_widths.get(&col).copied().unwrap_or(match col {
            PinnableColumn::Timestamp => MIN_TIMESTAMP_WIDTH,
            PinnableColumn::Level => MIN_LEVEL_WIDTH,
            PinnableColumn::Module => MIN_MODULE_WIDTH,
        })
    }

    /// Sets manual column width with safety floor clamping.
    #[inline]
    pub fn set_width(&mut self, col: PinnableColumn, width: Pixels) {
        let floor = self.min_width(col);
        self.widths.insert(col, width.max(floor));
    }

    /// Updates dynamic column widths based on maximum characters observed in ingested entries.
    pub fn update_from_entries(&mut self, entries: &[LogEntry], density: Density) {
        self.max_chars = FieldMaxChars::scan_entries(entries);
        self.recompute_widths(density);
    }

    /// Recomputes dynamic column widths when visual density (and thus monospace font size) changes.
    #[inline]
    pub fn update_density(&mut self, density: Density) {
        self.recompute_widths(density);
    }

    /// Recalculates dynamic widths from recorded max_chars for a given density.
    pub fn recompute_widths(&mut self, density: Density) {
        for col in [
            PinnableColumn::Timestamp,
            PinnableColumn::Level,
            PinnableColumn::Module,
        ] {
            self.auto_fit(col, density);
        }
    }

    /// Auto-fits a specific column back to its optimal dynamic width for the given density.
    pub fn auto_fit(&mut self, col: PinnableColumn, density: Density) {
        let count = match col {
            PinnableColumn::Timestamp => self.max_chars.timestamp,
            PinnableColumn::Level => self.max_chars.level,
            PinnableColumn::Module => self.max_chars.module,
        };
        let min = self.min_width(col);
        let calculated = compute_column_width(col, count, density.font_size(), min);
        self.widths.insert(col, calculated);
    }

    /// Computes the sticky offset needed for the Gutter inside a horizontally scrolled row
    /// so that the Gutter stays stationary at screen coordinate x = 0.
    #[inline]
    pub fn gutter_sticky_offset(&self, scroll_x: Pixels) -> Pixels {
        scroll_x
    }

    /// Computes the net screen position of the Gutter given horizontal scroll offset.
    /// Invariant: (-scroll_x) + gutter_sticky_offset(scroll_x) == 0.0
    #[inline]
    pub fn screen_gutter_offset(&self, scroll_x: Pixels) -> Pixels {
        -scroll_x + self.gutter_sticky_offset(scroll_x)
    }

    /// Computes the horizontal shift applied to unpinned columns and payload content.
    #[inline]
    pub fn columns_scroll_offset(&self, scroll_x: Pixels) -> Pixels {
        -scroll_x
    }
}
