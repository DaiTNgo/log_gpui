//! Focus Pane model: Focus Tabs, Focus Find and the lines a tab lists.
//!
//! Everything here is plain data and pure functions, so it is tested without
//! a window. The app (`app/focus.rs`) feeds it the lines of the open view.

use crate::model::LogEntry;
use crate::search::SearchState;
use crate::search_expr::{Matcher, SearchError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A line of a file, the same for the Raw View and the Parsed View: the file
/// it is in and its 1-based number there. Unlike a row position, it survives
/// a change of view or a parse.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "LineRefRepr", into = "LineRefRepr")]
pub struct LineRef {
    pub file_id: Arc<str>,
    pub line: usize,
    /// Generation of the file the line was read in (0 for a file that was
    /// never rotated).
    pub generation: u32,
}

impl LineRef {
    pub fn new(file_id: impl Into<Arc<str>>, line: usize) -> Self {
        Self { file_id: file_id.into(), line, generation: 0 }
    }

    pub fn with_generation(mut self, generation: u32) -> Self {
        self.generation = generation;
        self
    }
}

#[derive(Serialize, Deserialize)]
struct LineRefRepr {
    file_id: String,
    line: usize,
    #[serde(default)]
    generation: u32,
}

impl From<LineRefRepr> for LineRef {
    fn from(r: LineRefRepr) -> Self {
        Self { file_id: Arc::from(r.file_id), line: r.line, generation: r.generation }
    }
}

impl From<LineRef> for LineRefRepr {
    fn from(r: LineRef) -> Self {
        Self { file_id: r.file_id.to_string(), line: r.line, generation: r.generation }
    }
}

/// What the Focus Pane says when it lists no lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusPaneNotice {
    /// Nothing narrows the pane yet.
    Hint,
    /// The Search Expression matches none of the lines that pass Filter.
    NoMatches { expression: String, filtering: bool },
    /// Filter alone leaves no line (only in the Parsed View).
    FilterExcludesAll,
    /// Filter leaves no line because a condition is on a column that is no
    /// longer read (its Extraction Rule is off or deleted).
    UnreadColumn {
        /// The column's key.
        key: String,
        /// Its Column Label or name.
        name: String,
        /// Why it is not read ("rule turned off").
        reason: String,
        /// Whether "Turn column back on" can read it again.
        can_turn_on: bool,
    },
}

impl FocusPaneNotice {
    /// The notice for an empty pane. `expression` is the Search Expression
    /// in use, if any; `filtering` whether Filter narrows the pane (Parsed
    /// View with an active Filter); `filter_passes_none` whether no line
    /// passes that Filter.
    pub fn for_empty_pane(expression: Option<&str>, filtering: bool, filter_passes_none: bool) -> Self {
        if filtering && filter_passes_none {
            return Self::FilterExcludesAll;
        }
        match expression {
            Some(expression) => Self::NoMatches { expression: expression.trim().to_string(), filtering },
            None if filtering => Self::FilterExcludesAll,
            None => Self::Hint,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Hint => "Type a Search Expression, or pick a Filter, to list lines here".into(),
            Self::NoMatches { expression, .. } => format!("No lines match \u{201c}{expression}\u{201d}"),
            Self::FilterExcludesAll => "The Filter excludes every line".into(),
            Self::UnreadColumn { name, reason, .. } => {
                format!("No lines: Filter uses {name}, which is no longer read ({reason})")
            }
        }
    }

    /// What to try next, under the title.
    pub fn detail(&self) -> Option<&'static str> {
        match self {
            Self::Hint => None,
            Self::NoMatches { filtering: false, .. } => Some("Check the Search Expression for a typo."),
            Self::NoMatches { filtering: true, .. } => {
                Some("Check the Search Expression for a typo, or loosen the Filter.")
            }
            Self::FilterExcludesAll => Some("Loosen the Filter, or clear it to list every line."),
            Self::UnreadColumn { .. } => None,
        }
    }

    /// Whether the notice offers Clear Filter.
    pub fn offers_clear_filter(&self) -> bool {
        matches!(self, Self::FilterExcludesAll | Self::NoMatches { filtering: true, .. })
    }
}

/// One line a Focus Tab lists. Rows of one list are all `Entry` (Parsed View)
/// or all `Raw` (Raw View), and are kept in view order, which is also the
/// derived ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FocusRow {
    /// Position of the entry among the entries of the Parsed View.
    Entry(usize),
    /// A line of a File Section of the Raw View (`line` is 0-based).
    Raw { section: usize, line: usize },
}

/// Focus Find of a tab: looks for text among the lines the tab lists without
/// narrowing them. It reads like a Search Expression and keeps the last
/// expression that parsed while the typed one is incomplete.
#[derive(Debug, Clone, Default)]
pub struct FocusFind {
    pub query: String,
    matcher: Option<Matcher>,
    pub parse_error: Option<SearchError>,
    /// Positions in the tab's rows of the rows that match, ascending.
    pub hits: Vec<usize>,
    /// Selected hit, an index into `hits`.
    pub current: usize,
    /// Find Filter switched on: the tab lists only the rows Focus Find finds.
    /// Never stored, so it is off whenever the view opens again.
    pub find_filter: bool,
    /// How many rows the tab would list without the Find Filter.
    pub rows_before_find_filter: usize,
}

impl FocusFind {
    pub fn with_query(query: &str) -> Self {
        let mut find = Self { query: query.to_string(), ..Self::default() };
        find.reparse();
        find
    }

    /// Re-parse the typed text. Returns whether the matcher was replaced or
    /// cleared (`false` when the text is invalid and the old one stays).
    pub fn reparse(&mut self) -> bool {
        match crate::search_expr::parse(&self.query) {
            Ok(Some(expr)) => {
                self.matcher = Some(Matcher::compile(&expr));
                self.parse_error = None;
                true
            }
            Ok(None) => {
                self.matcher = None;
                self.parse_error = None;
                true
            }
            Err(err) => {
                self.parse_error = Some(err);
                false
            }
        }
    }

    /// Set the typed text and recompute the hits over `lines` (the text of
    /// each row of the tab, in order).
    pub fn set_query<'a>(&mut self, query: &str, lines: impl Iterator<Item = &'a str>) {
        self.query = query.to_string();
        let replaced = self.reparse();
        self.compute_hits(lines, replaced);
    }

    /// Find the hits among `lines`. The selected hit is kept unless `reset`.
    pub fn compute_hits<'a>(&mut self, lines: impl Iterator<Item = &'a str>, reset: bool) {
        self.hits.clear();
        if let Some(matcher) = &self.matcher {
            for (row, text) in lines.enumerate() {
                if matcher.is_match(text) {
                    self.hits.push(row);
                }
            }
        }
        if reset || self.current >= self.hits.len() {
            self.current = 0;
        }
    }

    /// Add the hits among rows appended to the tab's list: `lines` are the
    /// texts of the new rows, the first being row `first_row`. The selected
    /// hit stays.
    pub fn extend_hits<'a>(&mut self, lines: impl Iterator<Item = &'a str>, first_row: usize) {
        if let Some(matcher) = &self.matcher {
            for (i, text) in lines.enumerate() {
                if matcher.is_match(text) {
                    self.hits.push(first_row + i);
                }
            }
        }
    }

    /// `(selected, total)`, `(0, 0)` without hits.
    pub fn display(&self) -> (usize, usize) {
        if self.hits.is_empty() { (0, 0) } else { (self.current + 1, self.hits.len()) }
    }

    /// Whether an expression is in use (typed and parsed at least once).
    pub fn is_active(&self) -> bool {
        self.matcher.is_some()
    }

    /// Whether the Find Filter actually narrows the tab: switched on and
    /// with an expression to find.
    pub fn find_filter_narrows(&self) -> bool {
        self.find_filter && self.is_active()
    }

    /// Whether `text` satisfies the expression (false without one).
    pub fn matches(&self, text: &str) -> bool {
        self.matcher.as_ref().is_some_and(|m| m.is_match(text))
    }

    /// The counter beside the Focus Find field: "–" with nothing to find,
    /// "No matches" when the expression finds nothing, else "3/12". While
    /// the Find Filter narrows the tab: "N of M lines" (rows kept of the rows
    /// before the Find Filter).
    pub fn label(&self) -> String {
        if self.find_filter_narrows() {
            return format!("{} of {} lines", self.hits.len(), self.rows_before_find_filter);
        }
        match (self.is_active(), self.display()) {
            (false, _) => "–".into(),
            (true, (_, 0)) => "No matches".into(),
            (true, (cur, total)) => format!("{cur}/{total}"),
        }
    }

    /// Row position of the selected hit.
    pub fn current_row(&self) -> Option<usize> {
        self.hits.get(self.current).copied()
    }

    /// Select the next hit (wrapping) and return its row position.
    pub fn next_hit(&mut self) -> Option<usize> {
        if self.hits.is_empty() {
            return None;
        }
        self.current = (self.current + 1) % self.hits.len();
        self.current_row()
    }

    /// Select the previous hit (wrapping) and return its row position.
    pub fn prev_hit(&mut self) -> Option<usize> {
        if self.hits.is_empty() {
            return None;
        }
        self.current = (self.current + self.hits.len() - 1) % self.hits.len();
        self.current_row()
    }

    /// Select the hit at row position `row`, if that row is a hit.
    pub fn select_row(&mut self, row: usize) {
        if let Ok(i) = self.hits.binary_search(&row) {
            self.current = i;
        }
    }

    pub fn is_hit(&self, row: usize) -> bool {
        self.hits.binary_search(&row).is_ok()
    }

    /// Byte ranges to paint in `text`. A line that does not satisfy the
    /// whole expression has none.
    pub fn find_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        match &self.matcher {
            Some(m) if m.is_match(text) => m.find_ranges(text),
            _ => Vec::new(),
        }
    }
}

/// One Focus Tab: a numbered view of the lines that pass Filter and Search.
#[derive(Debug, Clone)]
pub struct FocusTab {
    pub number: u32,
    /// The tab's Search Expression and the Matches it found in the view.
    pub search: SearchState,
    pub find: FocusFind,
    /// The line the view and the pane agree on. It may be a line the tab does
    /// not list (selected in the view).
    pub current: Option<LineRef>,
}

impl FocusTab {
    pub fn new(number: u32) -> Self {
        Self { number, search: SearchState::new(), find: FocusFind::default(), current: None }
    }

    /// Label of the tab: its number and the start of its Search Expression.
    pub fn label(&self) -> String {
        const MAX: usize = 14;
        let query = self.search.query.trim();
        if query.is_empty() {
            return self.number.to_string();
        }
        let head: String = query.chars().take(MAX).collect();
        let ellipsis = if query.chars().count() > MAX { "…" } else { "" };
        format!("{} {head}{ellipsis}", self.number)
    }
}

/// The Focus Tabs of a view (a Log Space or a File View). There is always at
/// least one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "FocusTabsRepr", into = "FocusTabsRepr")]
pub struct FocusTabs {
    pub tabs: Vec<FocusTab>,
    pub active: usize,
}

impl Default for FocusTabs {
    fn default() -> Self {
        Self { tabs: vec![FocusTab::new(1)], active: 0 }
    }
}

impl FocusTabs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_tab(&self) -> &FocusTab {
        &self.tabs[self.active.min(self.tabs.len() - 1)]
    }

    pub fn active_tab_mut(&mut self) -> &mut FocusTab {
        let i = self.active.min(self.tabs.len() - 1);
        &mut self.tabs[i]
    }

    pub fn search(&self) -> &SearchState {
        &self.active_tab().search
    }

    pub fn search_mut(&mut self) -> &mut SearchState {
        &mut self.active_tab_mut().search
    }

    pub fn find(&self) -> &FocusFind {
        &self.active_tab().find
    }

    pub fn find_mut(&mut self) -> &mut FocusFind {
        &mut self.active_tab_mut().find
    }

    /// Add an empty tab numbered one more than the largest number, open it,
    /// and return its index.
    pub fn add(&mut self) -> usize {
        let number = self.tabs.iter().map(|t| t.number).max().unwrap_or(0) + 1;
        self.tabs.push(FocusTab::new(number));
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Close a tab. The last tab cannot be closed (returns `false`). When the
    /// open tab closes, the one that took its place (or the new last) opens.
    pub fn close(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1).min(self.tabs.len() - 1);
        }
        true
    }

    /// Open a tab. Returns `false` when `index` is out of range.
    pub fn set_active(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        self.active = index;
        true
    }

    pub fn can_close(&self) -> bool {
        self.tabs.len() > 1
    }

    /// Switch the Find Filter off on every tab (the view opens again).
    pub fn clear_find_filters(&mut self) {
        for tab in &mut self.tabs {
            tab.find.find_filter = false;
        }
    }
}

// What is stored of the tabs: the numbers, the typed expressions and the
// current lines. Matchers and results are computed again when a view opens.

#[derive(Serialize, Deserialize)]
struct FocusTabRepr {
    number: u32,
    #[serde(default)]
    query: String,
    #[serde(default)]
    find: String,
    #[serde(default)]
    current: Option<LineRef>,
}

#[derive(Serialize, Deserialize)]
struct FocusTabsRepr {
    #[serde(default)]
    tabs: Vec<FocusTabRepr>,
    #[serde(default)]
    active: usize,
}

impl From<FocusTabsRepr> for FocusTabs {
    fn from(repr: FocusTabsRepr) -> Self {
        if repr.tabs.is_empty() {
            return Self::default();
        }
        let tabs: Vec<FocusTab> = repr
            .tabs
            .into_iter()
            .map(|t| {
                let mut tab = FocusTab::new(t.number);
                tab.search.query = t.query;
                tab.search.reparse();
                tab.find = FocusFind::with_query(&t.find);
                tab.current = t.current;
                tab
            })
            .collect();
        let active = repr.active.min(tabs.len() - 1);
        Self { tabs, active }
    }
}

impl From<FocusTabs> for FocusTabsRepr {
    fn from(tabs: FocusTabs) -> Self {
        Self {
            active: tabs.active,
            tabs: tabs
                .tabs
                .into_iter()
                .map(|t| FocusTabRepr {
                    number: t.number,
                    query: t.search.query,
                    find: t.find.query,
                    current: t.current,
                })
                .collect(),
        }
    }
}

/// The lines a Focus Tab lists in the Parsed View: those that pass Filter
/// (`filter_pass`, ascending entry positions) and match the Search matcher.
/// No Filter means every line passes, no matcher means every line matches;
/// with neither the tab is empty. A Continuation Line is listed only when it
/// matches on its own.
pub fn compute_focus_rows(
    entries: &[LogEntry],
    filter_pass: &[usize],
    filter_active: bool,
    matcher: Option<&Matcher>,
) -> Vec<FocusRow> {
    if matcher.is_none() && !filter_active {
        return Vec::new();
    }
    let keep = |ix: usize| match (matcher, entries.get(ix)) {
        (Some(m), Some(entry)) => m.is_match(&entry.raw_line),
        (None, Some(_)) => true,
        (_, None) => false,
    };
    if filter_active {
        filter_pass.iter().copied().filter(|&ix| keep(ix)).map(FocusRow::Entry).collect()
    } else {
        (0..entries.len()).filter(|&ix| keep(ix)).map(FocusRow::Entry).collect()
    }
}

/// The index in `rows` of the last row at or before `target` in view order:
/// where the pane scrolls when the view selects a line the tab does not list.
/// `None` when every row comes after it.
pub fn nearest_row_before(rows: &[FocusRow], target: FocusRow) -> Option<usize> {
    rows.partition_point(|r| *r <= target).checked_sub(1)
}
