//! Logic: selected entry, markers and error navigation.

use super::AppView;
use crate::model::LogLevel;
use gpui::Context;

impl AppView {
    pub fn selected_entry(&self) -> Option<crate::rows::RowRef<'_>> {
        self.selected_row_pos
            .and_then(|pos| self.visible_indices.get(pos))
            .and_then(|&ix| self.rows().get(ix))
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
                    let file = parsed.file_entries();
                    let number = |pos: usize| file.line_number(pos).map(|(n, _)| n);
                    let start = {
                        let (mut lo, mut hi) = (0, file.len());
                        while lo < hi {
                            let mid = lo + (hi - lo) / 2;
                            if number(mid).is_some_and(|n| n < current.line) {
                                lo = mid + 1;
                            } else {
                                hi = mid;
                            }
                        }
                        lo
                    };
                    if number(start) == Some(current.line) && !file.continues(start) {
                        for pos in (start + 1..file.len()).take_while(|&p| file.continues(p)) {
                            if let Some(t) = number(pos).and_then(|n| section.lines.get(n - 1)) {
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
        let rows = self.rows();
        let marked = |pos: usize| self.visible_indices.get(pos).is_some_and(|&ix| !rows.markers(ix).is_empty());
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

    /// Whether the row shown at `pos` is an Error or Fatal line.
    fn is_error_row(&self, pos: usize) -> bool {
        let rows = self.rows();
        self.visible_indices
            .get(pos)
            .and_then(|&ix| rows.level(ix))
            .is_some_and(|level| matches!(level, LogLevel::Fatal | LogLevel::Error))
    }

    pub fn jump_to_next_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        let found = (1..total).map(|offset| (current_pos + offset) % total).find(|&pos| self.is_error_row(pos));
        if let Some(pos) = found {
            self.jump_to_row(pos, cx);
        }
    }

    pub fn jump_to_prev_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        let found = (1..total).map(|offset| (current_pos + total - offset) % total).find(|&pos| self.is_error_row(pos));
        if let Some(pos) = found {
            self.jump_to_row(pos, cx);
        }
    }
}
