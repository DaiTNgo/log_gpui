use crate::model::LogEntry;
use crate::search_expr::{Matcher, SearchError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Search of a Focus Tab: the typed Search Expression and the Matches found
/// on the lines that pass Filter. Only the typed text is stored (with the
/// tab); the matcher and the Matches are computed again.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchState {
    #[serde(skip)]
    pub is_open: bool,
    #[serde(skip)]
    pub query: String,
    /// Indices of matching rows in the `visible_indices` list
    #[serde(skip)]
    pub matched_visible_indices: Vec<usize>,
    /// Current selected match position in `matched_visible_indices` (0-based)
    #[serde(skip)]
    pub current_match_idx: usize,
    /// Same list as `matched_visible_indices`, shared cheaply with the canvas.
    #[serde(skip)]
    pub matched_shared: Arc<Vec<usize>>,
    /// Matcher compiled from the last Search Expression that parsed cleanly.
    /// It stays in place while the typed text is incomplete or invalid.
    #[serde(skip)]
    pub matcher: Option<Matcher>,
    /// Human-readable error for the current `query`, when it does not parse.
    /// `None` when the query is empty or valid.
    #[serde(skip)]
    pub parse_error: Option<SearchError>,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_matches(&self) -> usize {
        self.matched_visible_indices.len()
    }

    pub fn current_match_display(&self) -> (usize, usize) {
        let total = self.total_matches();
        if total == 0 {
            (0, 0)
        } else {
            (self.current_match_idx + 1, total)
        }
    }

    pub fn next_match(&mut self) -> Option<usize> {
        if self.matched_visible_indices.is_empty() {
            return None;
        }
        self.current_match_idx = (self.current_match_idx + 1) % self.matched_visible_indices.len();
        Some(self.matched_visible_indices[self.current_match_idx])
    }

    pub fn prev_match(&mut self) -> Option<usize> {
        if self.matched_visible_indices.is_empty() {
            return None;
        }
        if self.current_match_idx == 0 {
            self.current_match_idx = self.matched_visible_indices.len() - 1;
        } else {
            self.current_match_idx -= 1;
        }
        Some(self.matched_visible_indices[self.current_match_idx])
    }

    /// Re-parse the typed text. A valid expression replaces the matcher, empty
    /// text clears it, and an invalid expression keeps the previous matcher
    /// and records the error. Returns whether the matcher was replaced or
    /// cleared (`false` when the text is invalid).
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

    /// Update the Matches among `filter_pass`, the entries that pass Filter.
    /// A Match is stored as the entry's position, which is also its row in
    /// the view (the view shows every entry). While the typed text is invalid
    /// the Matches of the last valid expression are kept (and the selected
    /// match with them).
    pub fn update_matches(&mut self, entries: &[LogEntry], filter_pass: &[usize]) {
        let replaced = self.reparse();
        self.matched_visible_indices.clear();
        if replaced {
            self.current_match_idx = 0;
        }
        let Some(matcher) = &self.matcher else {
            self.current_match_idx = 0;
            self.matched_shared = Arc::new(Vec::new());
            return;
        };

        for &entry_ix in filter_pass {
            if let Some(entry) = entries.get(entry_ix)
                && matcher.is_match(&entry.raw_line)
            {
                self.matched_visible_indices.push(entry_ix);
            }
        }
        if self.current_match_idx >= self.matched_visible_indices.len() {
            self.current_match_idx = 0;
        }
        self.matched_shared = Arc::new(self.matched_visible_indices.clone());
    }

    /// Position (in the visible list) of the selected match, if any.
    pub fn current_match_pos(&self) -> Option<usize> {
        self.matched_visible_indices.get(self.current_match_idx).copied()
    }

    /// Find match byte ranges in a given text for highlight rendering, based on
    /// the last valid expression. Empty queries highlight nothing.
    pub fn find_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        match &self.matcher {
            Some(m) => m.find_ranges(text),
            None => Vec::new(),
        }
    }
}
