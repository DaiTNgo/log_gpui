use crate::model::LogEntry;
use crate::query::ast::QueryExpression;
use crate::query::parser::ParseError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchState {
    pub is_open: bool,
    pub query: String,
    /// Indices of matching rows in the `visible_indices` list
    pub matched_visible_indices: Vec<usize>,
    /// Current selected match position in `matched_visible_indices` (0-based)
    pub current_match_idx: usize,
    /// Parsed query-language expression for `query` when it parses cleanly.
    #[serde(skip)]
    pub parsed_query: Option<QueryExpression>,
    /// Human-readable parse error for the current `query`, when it is invalid
    /// query language. `None` when the query is empty or valid.
    #[serde(skip)]
    pub parse_error: Option<ParseError>,
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

    /// Re-parse the current query text into a `QueryExpression`, recording a
    /// human-readable `parse_error` when the text is not valid query language.
    pub fn reparse(&mut self) {
        let text = self.query.trim();
        if text.is_empty() {
            self.parsed_query = None;
            self.parse_error = None;
            return;
        }
        match crate::query::parser::parse(text) {
            Ok(expr) => {
                self.parsed_query = Some(expr);
                self.parse_error = None;
            }
            Err(err) => {
                self.parsed_query = None;
                self.parse_error = Some(err);
            }
        }
    }

    /// Update matching rows in visible_indices using the parsed query language.
    /// Invalid query text produces no matches.
    pub fn update_matches(&mut self, entries: &[LogEntry], visible_indices: &[usize]) {
        self.reparse();
        self.matched_visible_indices.clear();
        self.current_match_idx = 0;
        let Some(expr) = &self.parsed_query else {
            return;
        };

        for (vis_pos, &entry_ix) in visible_indices.iter().enumerate() {
            if let Some(entry) = entries.get(entry_ix) {
                if crate::query::engine::QueryEngine::evaluate_entry(entry, expr) {
                    self.matched_visible_indices.push(vis_pos);
                }
            }
        }
    }

    /// Find match byte ranges in a given text for highlight rendering, based on
    /// the parsed query language. Invalid/empty queries highlight nothing.
    pub fn find_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        let Some(expr) = &self.parsed_query else {
            return Vec::new();
        };
        crate::query::engine::QueryEngine::find_ranges_in_text(text, expr)
    }
}
