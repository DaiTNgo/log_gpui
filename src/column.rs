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

#[derive(Debug, Clone)]
pub struct ColumnLayoutState {
    pub pinned_columns: BTreeSet<PinnableColumn>,
    pub widths: HashMap<PinnableColumn, Pixels>,
    pub min_widths: HashMap<PinnableColumn, Pixels>,
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
