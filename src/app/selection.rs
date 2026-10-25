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

    /// The text the Inspector shows: the current line as stored, followed by
    /// its Continuation Lines when it is a Parent Line. Works in both views.
    pub fn inspector_lines(&self) -> Option<Vec<String>> {
        use crate::workspace::working_set::ViewMode;
        let current = self.current_line()?;
        match self.view_mode {
            ViewMode::Parsed => {
                let ix = self.entry_of_line_ref(&current)?;
                let rows = self.rows();
                let first = rows.get(ix)?;
                let mut lines = vec![first.raw_line.to_string()];
                if !first.is_continuation() {
                    lines.extend(
                        (ix + 1..rows.len())
                            .map_while(|r| {
                                let e = rows.get(r)?;
                                (e.is_continuation() && rows.file_id(r) == rows.file_id(ix)).then_some(e)
                            })
                            .map(|e| e.raw_line.to_string()),
                    );
                }
                Some(lines)
            }
            ViewMode::Raw => {
                let section = self.raw_sections().into_iter().find(|s| *s.file_id == *current.file_id)?;
                let at = current.line.checked_sub(1)?;
                let text = section.lines.get(at)?.to_string();
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
                                lines.push(t.to_string());
                            }
                        }
                    }
                }
                Some(lines)
            }
        }
    }

    /// Select the next row that carries a Marker, wrapping around the view;
    /// with no row selected, the first one. Says so when there is none.
    pub fn jump_to_next_marker(&mut self, cx: &mut Context<Self>) {
        self.jump_to_marker(true, cx);
    }

    /// Select the previous row that carries a Marker, wrapping around.
    pub fn jump_to_prev_marker(&mut self, cx: &mut Context<Self>) {
        self.jump_to_marker(false, cx);
    }

    fn jump_to_marker(&mut self, forward: bool, cx: &mut Context<Self>) {
        let total = self.visible_indices.len();
        let marked = |pos: usize| {
            self.visible_indices
                .get(pos)
                .is_some_and(|&ix| !self.rows().markers(ix).is_empty())
        };
        // With nothing selected the search starts just before the first row
        // (or after the last), so that row counts too.
        let found = (1..=total).find_map(|offset| {
            let pos = match (self.selected_row_pos, forward) {
                (Some(p), true) => (p + offset) % total,
                (Some(p), false) => (p + total - offset % total) % total,
                (None, true) => offset - 1,
                (None, false) => total - offset,
            };
            marked(pos).then_some(pos)
        });
        match found {
            Some(pos) => self.jump_to_row(pos, cx),
            None => {
                self.notice = Some("No Markers in this view".into());
                cx.notify();
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
