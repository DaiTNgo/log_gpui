use crate::model::LogEntry;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct SearchState {
    pub is_open: bool,
    pub query: String,
    pub case_sensitive: bool,
    pub is_regex: bool,
    pub whole_word: bool,
    /// Indices of matching rows in the `visible_indices` list
    pub matched_visible_indices: Vec<usize>,
    /// Current selected match position in `matched_visible_indices` (0-based)
    pub current_match_idx: usize,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            case_sensitive: false,
            is_regex: false,
            whole_word: false,
            matched_visible_indices: Vec::new(),
            current_match_idx: 0,
        }
    }
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

    pub fn toggle_case(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }

    pub fn toggle_regex(&mut self) {
        self.is_regex = !self.is_regex;
    }

    pub fn toggle_whole_word(&mut self) {
        self.whole_word = !self.whole_word;
    }

    /// Build regex pattern based on options
    pub fn build_regex(&self) -> Option<Regex> {
        let q = self.query.trim();
        if q.is_empty() {
            return None;
        }

        let pattern = if self.is_regex {
            if self.whole_word {
                format!(r"\b(?:{})\b", q)
            } else {
                q.to_string()
            }
        } else {
            let escaped = regex::escape(q);
            if self.whole_word {
                format!(r"\b{}\b", escaped)
            } else {
                escaped
            }
        };

        let mut builder = regex::RegexBuilder::new(&pattern);
        builder.case_insensitive(!self.case_sensitive);
        builder.build().ok()
    }

    /// Update matching rows in visible_indices
    pub fn update_matches(&mut self, entries: &[LogEntry], visible_indices: &[usize]) {
        self.matched_visible_indices.clear();
        let q = self.query.trim();
        if q.is_empty() {
            self.current_match_idx = 0;
            return;
        }

        let Some(re) = self.build_regex() else {
            self.current_match_idx = 0;
            return;
        };

        for (vis_pos, &entry_ix) in visible_indices.iter().enumerate() {
            if let Some(entry) = entries.get(entry_ix) {
                if re.is_match(&entry.payload)
                    || re.is_match(&entry.tag)
                    || re.is_match(&entry.raw_line)
                {
                    self.matched_visible_indices.push(vis_pos);
                }
            }
        }

        if self.current_match_idx >= self.matched_visible_indices.len() {
            self.current_match_idx = 0;
        }
    }

    /// Find match byte ranges in a given text for highlight rendering
    pub fn find_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        let Some(re) = self.build_regex() else {
            return Vec::new();
        };

        re.find_iter(text)
            .map(|m| (m.start(), m.end()))
            .collect()
    }
}
