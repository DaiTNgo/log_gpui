//! Logic: selected entry, markers and error navigation.

use super::AppView;
use crate::model::{LogEntry, LogLevel};
use gpui::Context;

impl AppView {
    pub fn selected_entry(&self) -> Option<&LogEntry> {
        self.selected_row_pos
            .and_then(|pos| self.visible_indices.get(pos))
            .and_then(|&ix| self.all_entries.get(ix))
    }

    pub fn selected_entry_mut(&mut self) -> Option<&mut LogEntry> {
        if let Some(pos) = self.selected_row_pos {
            if let Some(&entry_ix) = self.visible_indices.get(pos) {
                return self.all_entries.get_mut(entry_ix);
            }
        }
        None
    }

    /// The text the Inspector shows: the current line as stored, followed by
    /// its Continuation Lines when it is a Parent Line. Works in both views.
    pub fn inspector_lines(&self) -> Option<Vec<String>> {
        use crate::workspace::working_set::ViewMode;
        let current = self.current_line()?;
        match self.view_mode {
            ViewMode::Parsed => {
                let ix = self.entry_of_line_ref(&current)?;
                let first = self.all_entries.get(ix)?;
                let mut lines = vec![first.raw_line.clone()];
                if !first.is_continuation() {
                    lines.extend(
                        self.all_entries[ix + 1..]
                            .iter()
                            .take_while(|e| e.is_continuation() && e.file_id == first.file_id)
                            .map(|e| e.raw_line.clone()),
                    );
                }
                Some(lines)
            }
            ViewMode::Raw => {
                let section = self.raw_sections().into_iter().find(|s| *s.file_id == *current.file_id)?;
                let at = current.line.checked_sub(1)?;
                let text = section.lines.get(at)?.clone();
                let mut lines = vec![text];
                // A file edited since it was parsed has no reliable entries.
                let parsed = self
                    .parsed_files
                    .get(&section.file_id)
                    .filter(|_| !self.raw.edited.contains(&section.file_id));
                if let Some(parsed) = parsed {
                    let entries = &parsed.entries;
                    let start = entries.partition_point(|e| e.index < current.line);
                    let is_parent = entries
                        .get(start)
                        .is_some_and(|e| e.index == current.line && !e.is_continuation());
                    if is_parent {
                        for e in entries[start + 1..].iter().take_while(|e| e.is_continuation()) {
                            if let Some(t) = section.lines.get(e.index - 1) {
                                lines.push(t.clone());
                            }
                        }
                    }
                }
                Some(lines)
            }
        }
    }

    pub fn jump_to_next_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        for offset in 1..total {
            let next_pos = (current_pos + offset) % total;
            if let Some(&entry_ix) = self.visible_indices.get(next_pos) {
                if let Some(entry) = self.all_entries.get(entry_ix) {
                    if matches!(entry.level, LogLevel::Fatal | LogLevel::Error) {
                        self.jump_to_row(next_pos, cx);
                        break;
                    }
                }
            }
        }
    }

    pub fn jump_to_prev_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        for offset in 1..total {
            let prev_pos = (current_pos + total - offset) % total;
            if let Some(&entry_ix) = self.visible_indices.get(prev_pos) {
                if let Some(entry) = self.all_entries.get(entry_ix) {
                    if matches!(entry.level, LogLevel::Fatal | LogLevel::Error) {
                        self.jump_to_row(prev_pos, cx);
                        break;
                    }
                }
            }
        }
    }
}
