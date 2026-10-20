use crate::model::{FieldKey, LogEntry};
use crate::density::Density;
use gpui::{px, Pixels};
use gpui_kit::assets::IconName;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

pub const DEFAULT_GUTTER_WIDTH: Pixels = px(88.0);
pub const DEFAULT_TIMESTAMP_WIDTH: Pixels = px(120.0);
pub const DEFAULT_LEVEL_WIDTH: Pixels = px(68.0);
pub const DEFAULT_MODULE_WIDTH: Pixels = px(100.0);
pub const DEFAULT_SOURCE_WIDTH: Pixels = px(120.0);
pub const DEFAULT_STREAM_WIDTH: Pixels = px(72.0);
pub const DEFAULT_FIELD_WIDTH: Pixels = px(120.0);

pub const MIN_TIMESTAMP_WIDTH: Pixels = px(48.0);
pub const MIN_LEVEL_WIDTH: Pixels = px(36.0);
pub const MIN_MODULE_WIDTH: Pixels = px(48.0);
pub const MIN_SOURCE_WIDTH: Pixels = px(48.0);
pub const MIN_STREAM_WIDTH: Pixels = px(48.0);
pub const MIN_FIELD_WIDTH: Pixels = px(48.0);

/// A column of the Parsed View other than the Gutter and MESSAGE. Saved as
/// its name (`Level`, `field:user`) so it can key a map in a session file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PinnableColumn {
    Timestamp,
    Level,
    Module,
    /// The Source Column of a Log Space: the name of each row's Source File.
    /// Shown only when the Log Space turns it on.
    Source,
    /// The Stream Column of a Log Space with a Remote Command: `stdout` or
    /// `stderr`. Hidden unless turned on; it is never pinned.
    Stream,
    /// A Field Column: a field read from the line, shown after the fixed
    /// columns in the order of the Columns section.
    Field(FieldKey),
}

impl PinnableColumn {
    /// The column's name in a session file.
    pub fn save_name(self) -> String {
        match self {
            PinnableColumn::Timestamp => "Timestamp".into(),
            PinnableColumn::Level => "Level".into(),
            PinnableColumn::Module => "Module".into(),
            PinnableColumn::Source => "Source".into(),
            PinnableColumn::Stream => "Stream".into(),
            PinnableColumn::Field(k) => format!("field:{k}"),
        }
    }

    fn from_save_name(name: &str) -> Option<Self> {
        Some(match name {
            "Timestamp" => PinnableColumn::Timestamp,
            "Level" => PinnableColumn::Level,
            "Module" => PinnableColumn::Module,
            "Source" => PinnableColumn::Source,
            "Stream" => PinnableColumn::Stream,
            other => PinnableColumn::Field(FieldKey::new(other.strip_prefix("field:")?)),
        })
    }

    /// The column key Filter and Highlight Rules use for this column.
    pub fn key(self) -> &'static str {
        match self {
            PinnableColumn::Timestamp => "timestamp",
            PinnableColumn::Level => "level",
            PinnableColumn::Module => "module",
            PinnableColumn::Source => "source",
            PinnableColumn::Stream => "stream",
            PinnableColumn::Field(k) => k.as_str(),
        }
    }

    /// The column of a Filter or Highlight Rule column key, when it is one
    /// the Parsed View can show.
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "timestamp" => PinnableColumn::Timestamp,
            "level" => PinnableColumn::Level,
            "module" => PinnableColumn::Module,
            "source" => PinnableColumn::Source,
            "stream" => PinnableColumn::Stream,
            "message" | "request_id" | "trace_id" => return None,
            field => PinnableColumn::Field(FieldKey::new(field)),
        })
    }

    pub fn is_field(self) -> bool {
        matches!(self, PinnableColumn::Field(_))
    }

    /// A short id to tell the elements of one column from another's.
    pub fn element_name(self, prefix: &str) -> gpui::SharedString {
        format!("{prefix}-{}", self.save_name()).into()
    }
}

impl Serialize for PinnableColumn {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.save_name())
    }
}

impl<'de> Deserialize<'de> for PinnableColumn {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        PinnableColumn::from_save_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown column {name}")))
    }
}

impl PinnableColumn {
    /// The fixed columns, in their order before the Field Columns.
    pub const ALL: [PinnableColumn; 5] = [
        PinnableColumn::Timestamp,
        PinnableColumn::Level,
        PinnableColumn::Module,
        PinnableColumn::Source,
        PinnableColumn::Stream,
    ];

    /// Whether the user can pin the column (the Stream Column cannot).
    #[inline]
    pub fn can_pin(self) -> bool {
        self != PinnableColumn::Stream
    }

    #[inline]
    pub fn is_pinnable(name: &str) -> bool {
        matches!(name.to_ascii_uppercase().as_str(), "TIMESTAMP" | "LEVEL" | "MODULE" | "SOURCE")
    }

    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            PinnableColumn::Timestamp => "TIMESTAMP",
            PinnableColumn::Level => "LEVEL",
            PinnableColumn::Module => "MODULE",
            PinnableColumn::Source => "SOURCE",
            PinnableColumn::Stream => "STREAM",
            PinnableColumn::Field(k) => k.as_str(),
        }
    }

    /// Position among `ALL` of a fixed column.
    pub fn fixed_index(self) -> Option<usize> {
        Self::ALL.iter().position(|c| *c == self)
    }

    #[inline]
    pub fn is_compact(self) -> bool {
        matches!(self, PinnableColumn::Level)
    }

    /// Character length of the Column Header label, for estimating its width
    /// before the UI font is measured.
    #[inline]
    pub fn title_chars(&self) -> usize {
        match self {
            PinnableColumn::Timestamp => 9, // "TIMESTAMP"
            PinnableColumn::Level => 5,     // "LEVEL"
            PinnableColumn::Module => 6,    // "MODULE"
            PinnableColumn::Source => 6,    // "SOURCE"
            PinnableColumn::Stream => 6,    // "STREAM", and "stdout" / "stderr"
            PinnableColumn::Field(k) => k.as_str().chars().count(),
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
    /// Displays an unpin release affordance (`IconName::PinOff`).
    PinnedHover,
}

impl PinToggleState {
    #[inline]
    pub fn icon(self) -> Option<IconName> {
        match self {
            PinToggleState::UnpinnedIdle => None,
            PinToggleState::UnpinnedHover => Some(IconName::Pin),
            PinToggleState::PinnedActive => Some(IconName::Pin),
            PinToggleState::PinnedHover => Some(IconName::PinOff),
        }
    }

    #[inline]
    pub fn is_visible(self) -> bool {
        !matches!(self, PinToggleState::UnpinnedIdle)
    }
}

/// Tracks the maximum character lengths per structured field observed across log entries.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FieldMaxChars {
    pub timestamp: usize,
    pub level: usize,
    pub module: usize,
    #[serde(default)]
    pub source: usize,
    #[serde(default)]
    pub stream: usize,
    /// Longest value of each field read from the lines.
    #[serde(default)]
    pub fields: HashMap<FieldKey, usize>,
}

impl FieldMaxChars {
    pub fn new() -> Self {
        Self {
            timestamp: 0,
            level: 0,
            module: 0,
            source: 0,
            stream: 0,
            fields: HashMap::new(),
        }
    }

    /// Longest value seen in `col`.
    pub fn of(&self, col: PinnableColumn) -> usize {
        match col {
            PinnableColumn::Timestamp => self.timestamp,
            PinnableColumn::Level => self.level,
            PinnableColumn::Module => self.module,
            PinnableColumn::Source => self.source,
            PinnableColumn::Stream => self.stream,
            PinnableColumn::Field(k) => self.fields.get(&k).copied().unwrap_or(0),
        }
    }

    /// Take the longer of each length in `other`.
    pub fn merge(&mut self, other: &FieldMaxChars) {
        self.timestamp = self.timestamp.max(other.timestamp);
        self.level = self.level.max(other.level);
        self.module = self.module.max(other.module);
        self.source = self.source.max(other.source);
        self.stream = self.stream.max(other.stream);
        for (k, n) in &other.fields {
            let m = self.fields.entry(*k).or_default();
            *m = (*m).max(*n);
        }
    }

    /// Updates running maximums with a single log entry in O(1).
    #[inline]
    pub fn record_entry(&mut self, entry: &LogEntry) {
        self.timestamp = self.timestamp.max(entry.time_str.chars().count());
        self.level = self.level.max(entry.level_text.chars().count());
        self.module = self.module.max(entry.tag.chars().count());
        if let Some(source) = &entry.source {
            self.source = self.source.max(source.chars().count());
        }
        if let Some(stream) = entry.stream {
            self.stream = self.stream.max(stream.as_str().len());
        }
        if entry.parent.is_none() {
            for (key, text) in entry.fields() {
                let n = text.chars().count();
                match self.fields.get_mut(&key) {
                    Some(m) => *m = (*m).max(n),
                    None => {
                        self.fields.insert(key, n);
                    }
                }
            }
        }
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
/// Width = max(min_width, (max_chars * W_char) + padding, header width)
/// where W_char is the measured advance of the monospace font (0.6 * font_size
/// until measured, see `ui::mono`). The header width is the Column Header
/// label in the UI font plus the header's own padding, so headers never
/// truncate, even over a column with no values.
#[inline]
pub fn compute_column_width(
    col: PinnableColumn,
    max_chars: usize,
    font_size: f32,
    min_width: Pixels,
) -> Pixels {
    let char_width = crate::ui::mono::char_width(font_size);
    let padding = match col {
        PinnableColumn::Timestamp => 16.0,
        PinnableColumn::Level => 24.0,
        PinnableColumn::Module => 16.0,
        PinnableColumn::Source => 16.0,
        PinnableColumn::Stream => 16.0,
        PinnableColumn::Field(_) => 16.0,
    };
    let content_width = (max_chars as f32) * char_width + padding;
    px(content_width.max(header_width(col))).max(min_width)
}

/// The narrowest a column shows its whole Column Header label: the label,
/// the header's side padding (`px_1` compact, else `px_2`) and a little room
/// for rounding.
#[inline]
pub fn header_width(col: PinnableColumn) -> f32 {
    let side_padding = if col.is_compact() { 8.0 } else { 16.0 };
    crate::ui::mono::header_label_width(col) + side_padding + 2.0 + coverage_room(col)
}

/// Room a Field Column's header keeps for its Coverage (`99%`) and the Pin
/// Toggle beside it.
fn coverage_room(col: PinnableColumn) -> f32 {
    if col.is_field() { crate::ui::mono::header_text_width("99%") + 22.0 } else { 0.0 }
}

/// The narrowest a column shows a Column Label in full.
pub fn label_header_width(col: PinnableColumn, label: &str) -> f32 {
    let side_padding = if col.is_compact() { 8.0 } else { 16.0 };
    crate::ui::mono::header_text_width(label) + side_padding + 2.0 + coverage_room(col)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnLayoutState {
    pub pinned_columns: BTreeSet<PinnableColumn>,
    pub ordered_pinned: Vec<PinnableColumn>,
    pub widths: HashMap<PinnableColumn, Pixels>,
    pub min_widths: HashMap<PinnableColumn, Pixels>,
    pub max_chars: FieldMaxChars,
    pub gutter_width: Pixels,
    /// Whether the Source Column is shown (a pinned Source Column that is
    /// hidden keeps its pin and is simply not laid out).
    #[serde(default)]
    pub source_visible: bool,
    /// Whether the Stream Column is shown.
    #[serde(default)]
    pub stream_visible: bool,
    /// Fixed columns the user hid (TIMESTAMP, LEVEL, MODULE). Set from the
    /// Project's Columns section.
    #[serde(default)]
    pub hidden: BTreeSet<PinnableColumn>,
    /// The Field Columns laid out, in the order the Columns section gives:
    /// the ones turned on that have a value in the view. Set from the
    /// Project and the view's rows.
    #[serde(default)]
    pub fields: Vec<FieldKey>,
    /// Column Labels by column key, from the Project.
    #[serde(default)]
    pub labels: HashMap<String, String>,
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
        widths.insert(PinnableColumn::Source, DEFAULT_SOURCE_WIDTH);
        widths.insert(PinnableColumn::Stream, DEFAULT_STREAM_WIDTH);

        let mut min_widths = HashMap::new();
        min_widths.insert(PinnableColumn::Timestamp, MIN_TIMESTAMP_WIDTH);
        min_widths.insert(PinnableColumn::Level, MIN_LEVEL_WIDTH);
        min_widths.insert(PinnableColumn::Module, MIN_MODULE_WIDTH);
        min_widths.insert(PinnableColumn::Source, MIN_SOURCE_WIDTH);
        min_widths.insert(PinnableColumn::Stream, MIN_STREAM_WIDTH);

        Self {
            pinned_columns: BTreeSet::new(),
            ordered_pinned: Vec::new(),
            widths,
            min_widths,
            max_chars: FieldMaxChars::new(),
            gutter_width: DEFAULT_GUTTER_WIDTH,
            source_visible: false,
            stream_visible: false,
            hidden: BTreeSet::new(),
            fields: Vec::new(),
            labels: HashMap::new(),
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
        if !col.can_pin() {
            return false;
        }
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
        if !col.can_pin() {
            return false;
        }
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

    /// Pinned columns keep the view's column order, whatever order they
    /// were pinned in.
    fn sync_ordered_pinned(&mut self) {
        self.ordered_pinned = self
            .columns()
            .into_iter()
            .filter(|c| self.pinned_columns.contains(c))
            .collect();
    }

    /// Whether the column takes part in the layout: a fixed column the user
    /// has not hidden, a Source or Stream Column turned on, a Field Column
    /// of the view.
    #[inline]
    pub fn is_laid_out(&self, col: PinnableColumn) -> bool {
        match col {
            PinnableColumn::Source => self.source_visible,
            PinnableColumn::Stream => self.stream_visible,
            PinnableColumn::Field(k) => self.fields.contains(&k),
            fixed => !self.hidden.contains(&fixed),
        }
    }

    /// The columns laid out, in the view's order: TIMESTAMP, LEVEL, MODULE,
    /// Source, Stream, then the Field Columns.
    pub fn columns(&self) -> Vec<PinnableColumn> {
        PinnableColumn::ALL
            .iter()
            .copied()
            .chain(self.fields.iter().map(|k| PinnableColumn::Field(*k)))
            .filter(|c| self.is_laid_out(*c))
            .collect()
    }

    /// Set the fixed columns the user hid and the Field Columns to lay out,
    /// in order. A new Field Column is fitted to its values.
    pub fn set_view_columns(
        &mut self,
        hidden: BTreeSet<PinnableColumn>,
        fields: Vec<FieldKey>,
        density: Density,
    ) {
        for key in &fields {
            if !self.widths.contains_key(&PinnableColumn::Field(*key)) {
                self.auto_fit(PinnableColumn::Field(*key), density);
            }
        }
        self.hidden = hidden;
        self.fields = fields;
        self.sync_ordered_pinned();
    }

    /// The text of a column's Column Header: its Column Label, or its name.
    pub fn header_text(&self, col: PinnableColumn) -> &str {
        self.labels.get(col.key()).map(String::as_str).unwrap_or(col.label())
    }

    /// Show or hide the Stream Column.
    pub fn set_stream_visible(&mut self, visible: bool) {
        self.stream_visible = visible;
        self.sync_ordered_pinned();
    }

    /// Show or hide the Source Column.
    pub fn set_source_visible(&mut self, visible: bool) {
        self.source_visible = visible;
        self.sync_ordered_pinned();
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
        self.widths.retain(|c, _| c.is_field());
        self.widths.insert(PinnableColumn::Timestamp, DEFAULT_TIMESTAMP_WIDTH);
        self.widths.insert(PinnableColumn::Level, DEFAULT_LEVEL_WIDTH);
        self.widths.insert(PinnableColumn::Module, DEFAULT_MODULE_WIDTH);
        self.widths.insert(PinnableColumn::Source, DEFAULT_SOURCE_WIDTH);
        self.widths.insert(PinnableColumn::Stream, DEFAULT_STREAM_WIDTH);
    }

    /// Returns unpinned columns in natural schema order.
    pub fn unpinned_columns(&self) -> Vec<PinnableColumn> {
        self.columns().into_iter().filter(|col| !self.is_pinned(*col)).collect()
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
        let mut left = self.gutter_width;
        for earlier in self.columns() {
            if earlier == col {
                break;
            }
            left += self.width(earlier);
        }
        left
    }

    /// Natural schema position of the MESSAGE column at scroll_x = 0.
    #[inline]
    pub fn natural_message_left(&self) -> Pixels {
        self.columns().into_iter().map(|c| self.width(c)).sum::<Pixels>() + self.gutter_width
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
        if !col.can_pin() {
            return PinToggleState::UnpinnedIdle;
        }
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
            PinnableColumn::Source => DEFAULT_SOURCE_WIDTH,
            PinnableColumn::Stream => DEFAULT_STREAM_WIDTH,
            PinnableColumn::Field(_) => DEFAULT_FIELD_WIDTH,
        })
    }

    #[inline]
    pub fn min_width(&self, col: PinnableColumn) -> Pixels {
        self.min_widths.get(&col).copied().unwrap_or(match col {
            PinnableColumn::Timestamp => MIN_TIMESTAMP_WIDTH,
            PinnableColumn::Level => MIN_LEVEL_WIDTH,
            PinnableColumn::Module => MIN_MODULE_WIDTH,
            PinnableColumn::Source => MIN_SOURCE_WIDTH,
            PinnableColumn::Stream => MIN_STREAM_WIDTH,
            PinnableColumn::Field(_) => MIN_FIELD_WIDTH,
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
        let fields = self.fields.iter().map(|k| PinnableColumn::Field(*k));
        for col in PinnableColumn::ALL.into_iter().chain(fields.collect::<Vec<_>>()) {
            self.auto_fit(col, density);
        }
    }

    /// Auto-fits a specific column back to its optimal dynamic width for the given density.
    pub fn auto_fit(&mut self, col: PinnableColumn, density: Density) {
        let count = self.max_chars.of(col);
        let min = self.min_width(col);
        let mut calculated = compute_column_width(col, count, density.font_size(), min);
        if let Some(label) = self.labels.get(col.key()) {
            calculated = calculated.max(px(label_header_width(col, label)));
        }
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
