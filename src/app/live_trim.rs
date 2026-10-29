//! Logic: the Live Window dropped the oldest lines of a file. Their rows
//! leave the open view and everything that holds row positions in place,
//! instead of the view being built again from every file.

use super::AppView;
use crate::filter_index::{remove_positions, take_positions};
use crate::focus::FocusRow;
use crate::live::Trimmed;
use crate::index::view_rows::{pack, unpack};
use crate::model::LogEntry;
use crate::rows::{RowRef, Rows};
use crate::view_data::{BoundRows, ViewFile};
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

/// The rows of the open view as they were before the oldest lines of one
/// file were trimmed off its entries: that file's rows still count from its
/// old first line, whose entries are `drained`.
struct BeforeTrim<'a> {
    rows: BoundRows<'a>,
    file: usize,
    drained: &'a [LogEntry],
    entries: &'a [LogEntry],
}

impl Rows for BeforeTrim<'_> {
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, row: usize) -> Option<RowRef<'_>> {
        let (file, pos) = self.rows.data.get(row)?;
        if file != self.file {
            return self.rows.get(row);
        }
        match pos.checked_sub(self.drained.len()) {
            None => self.drained.get(pos).map(RowRef::Borrowed),
            Some(kept) => self.entries.get(kept).map(RowRef::Borrowed),
        }
    }

    fn file_id(&self, row: usize) -> Option<&str> {
        self.rows.file_id(row)
    }

    fn source(&self, row: usize) -> Option<&str> {
        self.rows.source(row)
    }

    fn markers(&self, row: usize) -> &[crate::marker::TextMarker] {
        let Some((file, _)) = self.rows.data.get(row) else { return &[] };
        let Some(f) = self.rows.data.files.get(file) else { return &[] };
        match self.get(row) {
            Some(entry) => f.markers_of_line(entry.index, entry.generation),
            None => &[],
        }
    }
}

/// Lines of one file of the view, in order: the rows that left it.
struct FileRows<'a> {
    file: &'a ViewFile,
    entries: &'a [LogEntry],
}

impl Rows for FileRows<'_> {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, row: usize) -> Option<RowRef<'_>> {
        self.entries.get(row).map(RowRef::Borrowed)
    }

    fn file_id(&self, _row: usize) -> Option<&str> {
        Some(&self.file.file_id)
    }

    fn source(&self, _row: usize) -> Option<&str> {
        self.file.source.as_deref()
    }

    fn markers(&self, row: usize) -> &[crate::marker::TextMarker] {
        self.entries.get(row).map_or(&[], |e| self.file.markers_of(e))
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
        let Some(view_file) = self.view.file_index(file_id) else { return false };
        let Some(parsed) = self.parsed_files.get(file_id) else { return false };
        if parsed.trimmed_off.len() != trimmed.removed {
            return false;
        }
        let mut dropped: Vec<usize> = Vec::with_capacity(trimmed.removed);
        let rows = BeforeTrim {
            rows: self.view.bind(&self.parsed_files),
            file: view_file,
            drained: &parsed.trimmed_off,
            entries: &parsed.entries,
        };
        for (pos, entry) in crate::rows::entries(&rows).enumerate() {
            let older = rows.file_id(pos) == Some(file_id)
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
        let Some(first) = parsed.entries.first() else { return false };
        if first.parent.is_some() {
            return false;
        }

        let anchor = self.top_row_anchor();
        // A scan of the rows under way is stale: it starts again.
        self.rows_epoch += 1;
        let selected = self.selected_row_pos.and_then(|p| shifted(p, &dropped));

        // What counted these rows stops counting them.
        for rule in self.highlight_rules().to_vec() {
            let lost = {
                let parsed = &self.parsed_files[file_id];
                let before = BeforeTrim {
                    rows: self.view.bind(&self.parsed_files),
                    file: view_file,
                    drained: &parsed.trimmed_off,
                    entries: &parsed.entries,
                };
                crate::highlight::count_matches(&rule, &before, &dropped)
            };
            if let Some(n) = self.rule_counts.get_mut(&rule.id) {
                *n = n.saturating_sub(lost);
            }
        }
        let longest_dropped = dropped.binary_search(&self.longest_row).is_ok();
        let longest_row = shifted(self.longest_row, &dropped);

        // Rows the tab would list without the Find Filter that leave.
        let lost_before_filter = self.rows_before_find_filter_among(&dropped);

        // The file's rows leave; its other rows now count from its new first
        // line.
        take_positions(self.view.rows.to_mut(), &dropped);
        for row in self.view.rows.to_mut().iter_mut() {
            let (file, pos) = unpack(*row);
            if file == view_file {
                *row = pack(file, pos - trimmed.removed);
            }
        }
        let rows = self.view.len();
        {
            let removed = FileRows { file: &self.view.files[view_file], entries: &self.parsed_files[file_id].trimmed_off };
            self.filter_index.drop_rows(&dropped, &removed, &self.view.bind(&self.parsed_files));
        }
        Arc::make_mut(&mut self.visible_indices).truncate(rows);
        let mut pass = std::mem::take(Arc::make_mut(&mut self.filter_pass));
        shift_sorted(&mut pass, &dropped);
        self.filter_pass = Arc::new(pass);
        if self.highlight_paints.len() == rows + dropped.len() {
            remove_positions(Arc::make_mut(&mut self.highlight_paints).rows_mut(), &dropped);
        } else {
            self.recompute_highlight_paints();
        }
        self.longest_row = match longest_row {
            Some(row) if !longest_dropped => row,
            _ => self.find_longest_row(),
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
            find.rows_before_find_filter = find.rows_before_find_filter.saturating_sub(lost_before_filter);
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
            let entries = &self.view.bind(&self.parsed_files);
            self.focus_longest = super::filter::longest(self.focus_rows.iter().map(|row| match *row {
                FocusRow::Entry(ix) => entries.get(ix).map_or(0, |e| e.raw_line.len()),
                FocusRow::Raw { .. } => 0,
            }));
        }

        // The file's first line shows what the Live Window did (a Break the
        // trim put on it), and the lines that left go.
        if let Some(parsed) = self.parsed_files.get_mut(file_id) {
            parsed.trimmed_off = Vec::new();
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
