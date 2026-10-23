//! Recent Search Expressions of a Project, newest first.

use serde::{Deserialize, Serialize};

/// How many Search Expressions a Project remembers.
pub const LIMIT: usize = 20;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHistory {
    /// Newest first, no two the same.
    pub items: Vec<String>,
}

impl SearchHistory {
    /// Remember `expression` as the newest. With `replace_newest` (the user
    /// is still typing the expression recorded last) it takes the place of
    /// the newest instead, so one typing does not leave every pause behind.
    pub fn record(&mut self, expression: &str, replace_newest: bool) {
        let expression = expression.trim();
        if expression.is_empty() {
            return;
        }
        if replace_newest && !self.items.is_empty() {
            self.items.remove(0);
        }
        self.items.retain(|e| e != expression);
        self.items.insert(0, expression.to_string());
        self.items.truncate(LIMIT);
    }

    pub fn remove(&mut self, expression: &str) {
        self.items.retain(|e| e != expression);
    }

    /// The expression ↑ (`older`) or ↓ shows after `at` (None: the text the
    /// user typed). Returns the new place, None when back at the typed text.
    pub fn step(&self, at: Option<usize>, older: bool) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        match (at, older) {
            (None, true) => Some(0),
            (None, false) => None,
            (Some(i), true) => Some((i + 1).min(self.items.len() - 1)),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_repeats_and_capped() {
        let mut h = SearchHistory::default();
        h.record("error", false);
        h.record("timeout", false);
        h.record(" error ", false);
        assert_eq!(h.items, ["error", "timeout"]);
        for i in 0..30 {
            h.record(&format!("q{i}"), false);
        }
        assert_eq!(h.items.len(), LIMIT);
        assert_eq!(h.items[0], "q29");
    }

    #[test]
    fn a_pause_while_typing_replaces_the_newest() {
        let mut h = SearchHistory::default();
        h.record("db", false);
        h.record("err", false);
        h.record("error", true);
        assert_eq!(h.items, ["error", "db"]);
    }

    #[test]
    fn up_goes_older_down_comes_back_to_the_typed_text() {
        let mut h = SearchHistory::default();
        h.record("a", false);
        h.record("b", false);
        assert_eq!(h.step(None, true), Some(0));
        assert_eq!(h.step(Some(0), true), Some(1));
        assert_eq!(h.step(Some(1), true), Some(1), "stops at the oldest");
        assert_eq!(h.step(Some(1), false), Some(0));
        assert_eq!(h.step(Some(0), false), None);
        assert_eq!(SearchHistory::default().step(None, true), None);
    }
}
