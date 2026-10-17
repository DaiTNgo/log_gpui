use crate::model::LogEntry;
use crate::theme::Density;
use gpui::{px, Pixels};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

pub const DEFAULT_GUTTER_WIDTH: Pixels = px(88.0);
pub const DEFAULT_TIMESTAMP_WIDTH: Pixels = px(120.0);
pub const DEFAULT_LEVEL_WIDTH: Pixels = px(68.0);
pub const DEFAULT_MODULE_WIDTH: Pixels = px(100.0);

pub const MIN_TIMESTAMP_WIDTH: Pixels = px(48.0);
pub const MIN_LEVEL_WIDTH: Pixels = px(36.0);
pub const MIN_MODULE_WIDTH: Pixels = px(48.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PinnableColumn {
    Timestamp,
    Level,
    Module,
}

impl PinnableColumn {
    pub const ALL: [PinnableColumn; 3] = [
        PinnableColumn::Timestamp,
        PinnableColumn::Level,
        PinnableColumn::Module,
    ];

    #[inline]
    pub fn is_pinnable(name: &str) -> bool {
        matches!(name.to_ascii_uppercase().as_str(), "TIMESTAMP" | "LEVEL" | "MODULE")
    }

    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            PinnableColumn::Timestamp => "TIMESTAMP",
            PinnableColumn::Level => "LEVEL",
            PinnableColumn::Module => "MODULE",
        }
    }

    #[inline]
    pub fn is_compact(self) -> bool {
        matches!(self, PinnableColumn::Level)
    }

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

pub const PUSH_PIN_SIMPLE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M216,168h-9.29L185.54,48H192a8,8,0,0,0,0-16H64a8,8,0,0,0,0,16h6.46L49.29,168H40a8,8,0,0,0,0,16h80v56a8,8,0,0,0,16,0V184h80a8,8,0,0,0,0-16ZM86.71,48h82.58l21.17,120H65.54Z"/></svg>"#;

pub const PUSH_PIN_SIMPLE_SLASH_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="currentColor"><path d="M83.25,40a8,8,0,0,1,8-8H192a8,8,0,0,1,0,16h-6.46l18.75,106.3a8,8,0,0,1-6.48,9.26,7.52,7.52,0,0,1-1.4.13,8,8,0,0,1-7.87-6.61L169.29,48h-78A8,8,0,0,1,83.25,40ZM213.38,221.92a8,8,0,0,1-11.3-.54L168.1,184H136v56a8,8,0,0,1-16,0V184H40a8,8,0,0,1,0-16h9.29L66.24,72,42.08,45.38A8,8,0,1,1,53.92,34.62l160,176A8,8,0,0,1,213.38,221.92ZM153.55,168,79.84,86.92,65.54,168Z"/></svg>"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    PushPinSimple,
    PushPinSimpleSlash,
}

impl IconName {
    #[inline]
    pub fn svg_data(self) -> &'static [u8] {
        match self {
            IconName::PushPinSimple => PUSH_PIN_SIMPLE_SVG,
            IconName::PushPinSimpleSlash => PUSH_PIN_SIMPLE_SLASH_SVG,
        }
    }
}

/// Pin Toggle interaction states defined in CONTEXT.md and Issue 03.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinToggleState {
    /// Icon hidden to keep the minimalist theme clean.
    UnpinnedIdle,
    /// Revealed with muted emphasis (`text_muted`) on Column Header hover.
    UnpinnedHover,
    /// Persistently visible with accent emphasis (`text_accent`).
    PinnedActive,
    /// Displays an unpin release affordance (`PushPinSimpleSlash`).
    PinnedHover,
}

impl PinToggleState {
    #[inline]
    pub fn icon(self) -> Option<IconName> {
        match self {
            PinToggleState::UnpinnedIdle => None,
            PinToggleState::UnpinnedHover => Some(IconName::PushPinSimple),
            PinToggleState::PinnedActive => Some(IconName::PushPinSimple),
            PinToggleState::PinnedHover => Some(IconName::PushPinSimpleSlash),
        }
    }

    #[inline]
    pub fn is_visible(self) -> bool {
        !matches!(self, PinToggleState::UnpinnedIdle)
    }
}

/// Tracks the maximum character lengths per structured field observed across log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnLayoutState {
    pub pinned_columns: BTreeSet<PinnableColumn>,
    pub ordered_pinned: Vec<PinnableColumn>,
    pub widths: HashMap<PinnableColumn, Pixels>,
    pub min_widths: HashMap<PinnableColumn, Pixels>,
    pub max_chars: FieldMaxChars,
    pub gutter_width: Pixels,
    pub is_resizing: Option<(PinnableColumn, Pixels)>,
    pub resize_start_x: Option<Pixels>,
    pub hovered_header: Option<PinnableColumn>,
    pub hovered_toggle: Option<PinnableColumn>,
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
            ordered_pinned: Vec::new(),
            widths,
            min_widths,
            max_chars: FieldMaxChars::new(),
            gutter_width: DEFAULT_GUTTER_WIDTH,
            is_resizing: None,
            resize_start_x: None,
            hovered_header: None,
            hovered_toggle: None,
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
    pub fn is_pinned(&self, col: PinnableColumn) -> bool {
        self.pinned_columns.contains(&col)
    }

    pub fn pin(&mut self, col: PinnableColumn) -> bool {
        let inserted = self.pinned_columns.insert(col);
        if inserted {
            self.sync_ordered_pinned();
        }
        inserted
    }

    pub fn unpin(&mut self, col: PinnableColumn) -> bool {
        let removed = self.pinned_columns.remove(&col);
        if removed {
            self.sync_ordered_pinned();
        }
        removed
    }

    pub fn toggle_pin(&mut self, col: PinnableColumn) -> bool {
        if self.pinned_columns.contains(&col) {
            self.pinned_columns.remove(&col);
            self.sync_ordered_pinned();
            false
        } else {
            self.pinned_columns.insert(col);
            self.sync_ordered_pinned();
            true
        }
    }

    pub fn unpin_all(&mut self) {
        self.pinned_columns.clear();
        self.ordered_pinned.clear();
    }

    fn sync_ordered_pinned(&mut self) {
        self.ordered_pinned = self.pinned_columns.iter().copied().collect();
    }

    /// Returns pinned columns in natural schema order (`TIMESTAMP` -> `LEVEL` -> `MODULE`).
    #[inline]
    pub fn pinned_columns(&self) -> &[PinnableColumn] {
        &self.ordered_pinned
    }

    /// Read-only access to all column widths keyed by column.
    #[inline]
    pub fn widths(&self) -> &HashMap<PinnableColumn, Pixels> {
        &self.widths
    }

    /// Reset all column widths back to their defaults, discarding manual
    /// adjustments and dynamic sizing (used when restoring a saved view).
    pub fn reset_widths_to_defaults(&mut self) {
        self.widths.clear();
        self.widths.insert(PinnableColumn::Timestamp, DEFAULT_TIMESTAMP_WIDTH);
        self.widths.insert(PinnableColumn::Level, DEFAULT_LEVEL_WIDTH);
        self.widths.insert(PinnableColumn::Module, DEFAULT_MODULE_WIDTH);
    }

    /// Returns unpinned columns in natural schema order.
    pub fn unpinned_columns(&self) -> Vec<PinnableColumn> {
        PinnableColumn::ALL
            .iter()
            .copied()
            .filter(|col| !self.is_pinned(*col))
            .collect()
    }

    /// Sum of the widths of all currently pinned columns.
    #[inline]
    pub fn pinned_width(&self) -> Pixels {
        self.pinned_columns()
            .iter()
            .map(|col| self.width(*col))
            .sum()
    }

    /// Total width of the anchored section (Gutter + all pinned columns).
    #[inline]
    pub fn sticky_partition_width(&self) -> Pixels {
        self.gutter_width + self.pinned_width()
    }
    /// Whether the partition divider's elevation shadow is active (scroll_x > 0).
    #[inline]
    pub fn is_partition_divider_active(scroll_x: Pixels) -> bool {
        scroll_x > px(0.0)
    }

    /// Position of the vertical Partition Divider at the right boundary of the sticky partition.
    #[inline]
    pub fn partition_divider_offset(&self) -> Pixels {
        self.sticky_partition_width()
    }

    /// Natural schema position of a column at scroll_x = 0.
    #[inline]
    pub fn natural_column_left(&self, col: PinnableColumn) -> Pixels {
        match col {
            PinnableColumn::Timestamp => self.gutter_width,
            PinnableColumn::Level => self.gutter_width + self.width(PinnableColumn::Timestamp),
            PinnableColumn::Module => {
                self.gutter_width
                    + self.width(PinnableColumn::Timestamp)
                    + self.width(PinnableColumn::Level)
            }
        }
    }

    /// Natural schema position of the MESSAGE column at scroll_x = 0.
    #[inline]
    pub fn natural_message_left(&self) -> Pixels {
        self.natural_column_left(PinnableColumn::Module) + self.width(PinnableColumn::Module)
    }

    /// Target anchored position of a column when sticky. Returns None if not pinned.
    pub fn sticky_column_left(&self, col: PinnableColumn) -> Option<Pixels> {
        if !self.is_pinned(col) {
            return None;
        }
        let mut offset = self.gutter_width;
        for pinned in self.pinned_columns() {
            if *pinned == col {
                return Some(offset);
            }
            offset += self.width(*pinned);
        }
        None
    }

    /// Computes the net screen position for a column, implementing true sticky positioning:
    /// (natural_left - scroll_x).max(sticky_target).
    #[inline]
    pub fn column_screen_position(&self, col: PinnableColumn, scroll_x: Pixels) -> Pixels {
        let natural = self.natural_column_left(col);
        if let Some(sticky_target) = self.sticky_column_left(col) {
            (natural - scroll_x).max(sticky_target)
        } else {
            natural - scroll_x
        }
    }

    /// Computes the net screen position for the MESSAGE column given horizontal scroll offset.
    #[inline]
    pub fn message_screen_position(&self, scroll_x: Pixels) -> Pixels {
        self.natural_message_left() - scroll_x
    }

    pub fn set_header_hovered(&mut self, col: PinnableColumn, hovered: bool) {
        if hovered {
            self.hovered_header = Some(col);
        } else if self.hovered_header == Some(col) {
            self.hovered_header = None;
        }
    }

    pub fn set_toggle_hovered(&mut self, col: PinnableColumn, hovered: bool) {
        if hovered {
            self.hovered_toggle = Some(col);
        } else if self.hovered_toggle == Some(col) {
            self.hovered_toggle = None;
        }
    }

    pub fn pin_toggle_state(&self, col: PinnableColumn) -> PinToggleState {
        let is_header_hovered = self.hovered_header == Some(col);
        let is_toggle_hovered = self.hovered_toggle == Some(col);
        self.pin_toggle_state_with(col, is_header_hovered, is_toggle_hovered)
    }

    pub fn pin_toggle_state_with(
        &self,
        col: PinnableColumn,
        header_hovered: bool,
        toggle_hovered: bool,
    ) -> PinToggleState {
        if self.is_pinned(col) {
            if toggle_hovered {
                PinToggleState::PinnedHover
            } else {
                PinToggleState::PinnedActive
            }
        } else {
            if header_hovered || toggle_hovered {
                PinToggleState::UnpinnedHover
            } else {
                PinToggleState::UnpinnedIdle
            }
        }
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

    #[inline]
    pub fn is_resizing_column(&self, col: PinnableColumn) -> bool {
        matches!(self.is_resizing, Some((c, _)) if c == col)
    }

    pub fn start_resizing(&mut self, col: PinnableColumn, start_x: Pixels) {
        let initial_width = self.width(col);
        self.is_resizing = Some((col, initial_width));
        self.resize_start_x = Some(start_x);
    }

    pub fn update_resizing(&mut self, current_x: Pixels) {
        if let (Some((col, initial_width)), Some(start_x)) = (self.is_resizing, self.resize_start_x) {
            let delta = current_x - start_x;
            let new_width = initial_width + delta;
            self.set_width(col, new_width);
        }
    }

    pub fn finish_resizing(&mut self) {
        self.is_resizing = None;
        self.resize_start_x = None;
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
