//! Logic: the Live Window dropped the oldest lines of a file. Their rows
//! leave the open view and everything that holds row positions in place,
//! instead of the view being built again from every file.

use super::AppView;
use crate::filter_index::{remove_positions, take_positions};
use crate::focus::FocusRow;
use crate::live::Trimmed;
use crate::model::LogEntry;
use std::sync::Arc;

/// Remove the numbers at `dropped` (ascending) from `items` (ascending) and
/// move the others down by the number of dropped before each.
fn shift_sorted(items: &mut Vec<usize>, dropped: &[usize]) {
    let mut d = 0;
    items.retain_mut(|p| {
        while d < dropped.len() && dropped[d] < *p {
            d += 1;
        }
        if d < dropped.len() && dropped[d] == *p {
            return false;
        }
        *p -= d;
        true
    });
}

/// Where position `p` goes when the rows at `dropped` leave; `None` when it
/// is one of them.
fn shifted(p: usize, dropped: &[usize]) -> Option<usize> {
    match dropped.binary_search(&p) {
        Ok(_) => None,
        Err(before) => Some(p - before),
    }
}

impl AppView {
    /// The oldest lines of `file_id` left the Live Window: take their rows out
    /// of the open view in place. Returns false, with nothing changed, when
    /// the view does not hold those rows (the caller builds the view again).
    pub(crate) fn trim_open_view(&mut self, file_id: &str, trimmed: &Trimmed) -> bool {
        let (generation, first_kept) = (trimmed.generation, trimmed.first_kept);
        // The rows of the file older than the first line it kept. They are
        // the first rows of the file in the view, wherever the other files'
        // rows are among them.
        let mut dropped: Vec<usize> = Vec::with_capacity(trimmed.removed);
        for (pos, entry) in self.all_entries.iter().enumerate() {
            let older = entry.file_id.as_deref() == Some(file_id)
                && (entry.generation < generation || (entry.generation == generation && entry.index < first_kept));
            if older {
                dropped.push(pos);
                if dropped.len() == trimmed.removed {
                    break;
                }
            }
        }
        if dropped.is_empty() || dropped.len() != trimmed.removed {
            return false;
        }
        // The file's new first line: a Continuation Line whose Parent Line
        // left is sorted by a build of the view in its own way.
        let Some(first) = self.parsed_files.get(file_id).and_then(|p| p.entries.first()) else { return false };
        if first.parent.is_some() {
            return false;
        }
        let break_before = first.break_before.clone();

        let anchor = self.top_row_anchor();
        // A scan of the rows under way is stale: it starts again.
        self.rows_epoch += 1;
        let selected = self.selected_row_pos.and_then(|p| shifted(p, &dropped));

        // What counted these rows stops counting them.
        for rule in self.highlight_rules().to_vec() {
            let lost = crate::highlight::count_matches(&rule, &self.all_entries, &dropped);
            if let Some(n) = self.rule_counts.get_mut(&rule.id) {
                *n = n.saturating_sub(lost);
            }
        }
        let longest_dropped = dropped.binary_search(&self.longest_row).is_ok();
        let longest_row = shifted(self.longest_row, &dropped);

        // Rows the tab would list without the Find Filter that leave.
        let lost_before_filter = self.focus_rows_among(&dropped);

        let removed: Vec<LogEntry> = take_positions(&mut self.all_entries, &dropped);
        let rows = self.all_entries.len();
        self.filter_index.drop_rows(&dropped, &removed, &self.all_entries);
        Arc::make_mut(&mut self.visible_indices).truncate(rows);
        let mut pass = std::mem::take(Arc::make_mut(&mut self.filter_pass));
        shift_sorted(&mut pass, &dropped);
        self.filter_pass = Arc::new(pass);
        if self.highlight_paints.len() == rows + dropped.len() {
            remove_positions(Arc::make_mut(&mut self.highlight_paints), &dropped);
        } else {
            self.recompute_highlight_paints();
        }
        self.longest_row = match longest_row {
            Some(row) if !longest_dropped => row,
            _ => super::filter::longest(self.all_entries.iter().map(|e| e.payload().len())),
        };

        // The Matches and the rows of the Focus Pane, which hold positions.
        {
            let search = self.focus.search_mut();
            let current = search.current_match_idx;
            let lost_before = search.matched_visible_indices[..current.min(search.matched_visible_indices.len())]
                .iter()
                .filter(|p| dropped.binary_search(p).is_ok())
                .count();
            let mut matches = std::mem::take(Arc::make_mut(&mut search.matched_visible_indices));
            shift_sorted(&mut matches, &dropped);
            search.matched_visible_indices = Arc::new(matches);
            search.current_match_idx = current.saturating_sub(lost_before);
            if search.current_match_idx >= search.matched_visible_indices.len() {
                search.current_match_idx = 0;
            }
        }
        if self.view_mode == crate::workspace::working_set::ViewMode::Parsed {
            let mut lost_rows: Vec<usize> = Vec::new();
            {
                let rows = Arc::make_mut(&mut self.focus_rows);
                let mut at = 0;
                rows.retain_mut(|row| {
                    let pos = at;
                    at += 1;
                    match row {
                        FocusRow::Entry(p) => match shifted(*p, &dropped) {
                            Some(q) => {
                                *p = q;
                                true
                            }
                            None => {
                                lost_rows.push(pos);
                                false
                            }
                        },
                        FocusRow::Raw { .. } => true,
                    }
                });
            }
            let find = self.focus.find_mut();
            find.rows_before_filter = find.rows_before_filter.saturating_sub(lost_before_filter);
            let current = find.current;
            let lost_before = find.hits[..current.min(find.hits.len())]
                .iter()
                .filter(|h| lost_rows.binary_search(h).is_ok())
                .count();
            shift_sorted(&mut find.hits, &lost_rows);
            find.current = current.saturating_sub(lost_before);
            if find.current >= find.hits.len() {
                find.current = 0;
            }
            let entries = &self.all_entries;
            self.focus_longest = super::filter::longest(self.focus_rows.iter().map(|row| match *row {
                FocusRow::Entry(ix) => entries.get(ix).map_or(0, |e| e.raw_line.len()),
                FocusRow::Raw { .. } => 0,
            }));
        }

        // The file's first row shows what the Live Window did.
        if let Some(row) = self.all_entries[dropped[dropped.len() - 1] - (dropped.len() - 1)..]
            .iter_mut()
            .find(|e| e.file_id.as_deref() == Some(file_id))
        {
            row.break_before = break_before;
        }

        // The first row left of each run that went has a new row above it.
        for (i, &first) in dropped.iter().enumerate() {
            if i == 0 || dropped[i - 1] + 1 != first {
                let at = first - i;
                if at < rows {
                    refresh_delta_times(&mut self.all_entries, at);
                }
            }
        }

        if let Some((f, line)) = self.raw.selected.as_mut() {
            if f == file_id {
                match line.checked_sub(trimmed.removed) {
                    Some(l) => *line = l,
                    None => self.raw.selected = None,
                }
            }
        }
        self.selected_row_pos = selected;
        self.text_selection = None;
        self.selecting = false;
        self.pane_selection = None;
        self.sync_view_columns();
        self.move_current_lines_off_dropped(file_id, trimmed);
        self.restore_top_anchor(anchor);
        true
    }
}

/// Give the row at `at` its delta time from the row now above it, and the
/// rows that follow up to the next row that is not a Continuation Line.
fn refresh_delta_times(entries: &mut [LogEntry], at: usize) {
    let mut prev: Option<f64> = entries[..at].iter().rev().find(|e| e.parent.is_none()).map(|e| e.timestamp);
    for entry in entries[at..].iter_mut() {
        let parent_row = entry.parent.is_none();
        crate::filter::update_entry_delta_time(entry, &mut prev);
        if parent_row {
            // The rows after it have the same row above them as before.
            break;
        }
    }
}
