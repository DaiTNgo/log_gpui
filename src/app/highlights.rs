//! Logic: Highlight Rules of the active Project. A rule paints every row
//! whose value in one column meets its condition; the paint of each row is
//! worked out again whenever the rules or the rows change.

use super::AppView;
use crate::filter_index::column_value;
use crate::highlight::{
    HighlightRule, MESSAGE_COLUMN, RuleError, TextRule, count_matches_with, move_rule, next_rule_id, paint_rows_split,
    paint_rows_with,
};
use crate::index::file::IndexedFile;
use crate::index::scan::block_order;
use crate::index::view_rows::unpack;
use crate::palette::{HighlightColor, next_rule_color};
use gpui::Context;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Rows around the top row painted at once: more than a screen.
const PAINTED_AT_ONCE: usize = 400;

/// A Scan painting the rows a rule needs the text of (task 7.2); dropping
/// it stops it.
pub(crate) struct PaintScan {
    generation: u64,
    cancel: Arc<AtomicBool>,
}

impl Drop for PaintScan {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// What a Scan read of some rows: the byte of each, and how many of them
/// (not Continuation Lines) each rule that needs the text meets.
struct Painted {
    rows: Vec<(usize, u8)>,
    counts: Vec<(u64, usize)>,
    done: bool,
}

/// Paint Scans started: each one's number.
static PAINT_SCANS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl AppView {
    pub fn highlight_rules(&self) -> &[HighlightRule] {
        self.active_project().map_or(&[], |p| p.highlight_rules.as_slice())
    }

    /// Work out the paint of every row of the open view. Only the Parsed View
    /// shows it.
    ///
    /// A rule that needs the text of the rows' values (a Custom Value on
    /// `message`, a column with too many values) paints at once the rows
    /// held in memory and those around the top row; the rows of files read
    /// from disk are painted by a Scan (`start_paint_scan`).
    pub fn recompute_highlight_paints(&mut self) {
        self.paint_scan = None;
        self.paint_scan_wanted = None;
        let rules = self.highlight_rules().to_vec();
        let rows = self.view.bind(&self.parsed_files);
        let columns = Some(&self.filter_index);
        let top = self.top_row_anchor().map_or(0, |a| a.row);
        let around = top.saturating_sub(PAINTED_AT_ONCE / 4)..top + PAINTED_AT_ONCE;
        let read = |row: usize| around.contains(&row);
        let split = paint_rows_split(&rules, &rows, columns, 0, read);
        let text_ids: Vec<u64> = split.text.iter().map(|t| t.id).collect();
        let read_rows: Vec<usize> = if split.unread.is_empty() {
            Vec::new()
        } else {
            let mut unread = split.unread.iter().map(|&(r, _)| r).peekable();
            (0..rows.len())
                .filter(|&r| {
                    while unread.peek().is_some_and(|&u| u < r) {
                        unread.next();
                    }
                    unread.peek() != Some(&r)
                })
                .collect()
        };
        self.rule_counts = rules
            .iter()
            .map(|r| {
                // A rule that needs the text counts the rows read here; the
                // Scan adds the others.
                let counted = if text_ids.contains(&r.id) && !split.unread.is_empty() { &read_rows } else { &*self.visible_indices };
                (r.id, count_matches_with(r, &rows, columns, counted))
            })
            .collect();
        drop(rows);
        self.highlight_paints = Arc::new(split.paints);
        if !split.unread.is_empty() {
            self.paint_scan_wanted = Some((split.text, split.unread));
        }
    }

    /// Start the Scan painting the rows `recompute_highlight_paints` left
    /// for it, if it left any. The rows around the top row are read first.
    pub(crate) fn start_paint_scan(&mut self, cx: &mut Context<Self>) {
        let Some((text, unread)) = self.paint_scan_wanted.take() else { return };
        let packed: Vec<u64> = unread.iter().map(|&(r, _)| self.view.rows.packed(r).unwrap_or(u64::MAX)).collect();
        let files: Vec<Option<(Arc<IndexedFile>, Option<Arc<str>>)>> = self
            .view
            .files
            .iter()
            .map(|f| self.parsed_files.get(&*f.file_id).and_then(|p| p.indexed.clone()).map(|i| (i, f.source.clone())))
            .collect();
        let top = self.top_row_anchor().map_or(0, |a| a.row);
        let around = unread.partition_point(|&(r, _)| r < top).min(unread.len().saturating_sub(1));
        let cancel = Arc::new(AtomicBool::new(false));
        let generation = PAINT_SCANS.fetch_add(1, Ordering::Relaxed);
        let epoch = self.rows_epoch;
        self.paint_scan = Some(PaintScan { generation, cancel: cancel.clone() });
        let (tx, updates) = async_channel::unbounded::<Result<Painted, String>>();
        cx.background_executor()
            .spawn(async move {
                let order = block_order(0..unread.len(), around);
                let last = order.len().saturating_sub(1);
                for (n, block) in order.into_iter().enumerate() {
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                    let painted = paint_block(&block, &unread, &packed, &files, &text).map(|mut p| {
                        p.done = n == last;
                        p
                    });
                    let failed = painted.is_err();
                    let _ = tx.try_send(painted);
                    if failed {
                        return;
                    }
                }
            })
            .detach();
        cx.spawn(async move |this, cx| {
            while let Ok(painted) = updates.recv().await {
                let go_on = this.update(cx, |this, cx| this.paint_scan_found(generation, epoch, painted, cx)).unwrap_or(false);
                if !go_on {
                    break;
                }
            }
        })
        .detach();
    }

    /// What the paint Scan `generation` read; whether to go on listening.
    fn paint_scan_found(&mut self, generation: u64, epoch: u64, painted: Result<Painted, String>, cx: &mut Context<Self>) -> bool {
        if self.paint_scan.as_ref().is_none_or(|s| s.generation != generation) {
            return false;
        }
        let painted = match painted {
            Ok(painted) if epoch == self.rows_epoch => painted,
            // Rows moved meanwhile, or could not be read: paint again.
            _ => {
                self.recompute_highlight_paints();
                self.start_paint_scan(cx);
                cx.notify();
                return false;
            }
        };
        let paints = Arc::make_mut(&mut self.highlight_paints).rows_mut();
        for (row, byte) in painted.rows {
            if let Some(at) = paints.get_mut(row) {
                *at = byte;
            }
        }
        for (id, n) in painted.counts {
            *self.rule_counts.entry(id).or_insert(0) += n;
        }
        if painted.done {
            self.paint_scan = None;
        }
        cx.notify();
        !painted.done
    }

    /// Whether a Scan is painting rows.
    pub fn paint_is_scanning(&self) -> bool {
        self.paint_scan.is_some()
    }

    /// Paint the rows from `from` on, appended after the rows painted, and
    /// count them in the rules' counts. False when the paints did not cover
    /// the rows before `from` (the caller paints everything again).
    pub(crate) fn extend_highlight_paints(&mut self, from: usize) -> bool {
        if self.highlight_paints.len() != from {
            return false;
        }
        let rules = self.highlight_rules().to_vec();
        let rows = self.view.bind(&self.parsed_files);
        let columns = Some(&self.filter_index);
        let new = paint_rows_with(&rules, &rows, columns, from);
        let counts: Vec<(u64, usize)> =
            rules.iter().map(|r| (r.id, count_matches_with(r, &rows, columns, &self.visible_indices[from..]))).collect();
        drop(rows);
        Arc::make_mut(&mut self.highlight_paints).extend(new);
        for (id, n) in counts {
            *self.rule_counts.entry(id).or_insert(0) += n;
        }
        true
    }

    fn rules_changed(&mut self, cx: &mut Context<Self>) {
        self.recompute_highlight_paints();
        self.start_paint_scan(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Add a rule at the end of the list (lowest priority).
    pub fn add_highlight_rule(
        &mut self,
        column: &str,
        values: &[String],
        custom: &[String],
        color: HighlightColor,
        cx: &mut Context<Self>,
    ) -> Result<(), RuleError> {
        let Some(project) = self.active_project_mut() else { return Ok(()) };
        let id = next_rule_id(&project.highlight_rules);
        let rule = HighlightRule::new(id, column, values, custom, color.clone())?;
        project.highlight_rules.push(rule);
        project.recent_colors.push(color);
        self.rules_changed(cx);
        Ok(())
    }

    /// "Highlight rows where <column> = <value>" from the menu of a cell: a
    /// normal rule with the next color of the palette. On MESSAGE the text is a
    /// Custom Value.
    pub fn highlight_rows_where(&mut self, column: &str, value: &str, cx: &mut Context<Self>) {
        let color = next_rule_color(self.highlight_rules().len());
        let (values, custom) = if column == MESSAGE_COLUMN {
            (Vec::new(), vec![value.to_string()])
        } else {
            (vec![value.to_string()], Vec::new())
        };
        let _ = self.add_highlight_rule(column, &values, &custom, color, cx);
    }

    pub fn remove_highlight_rule(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            project.highlight_rules.retain(|r| r.id != id);
        }
        self.rules_changed(cx);
    }

    pub fn toggle_highlight_rule(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(rule) = self.active_project_mut().and_then(|p| p.highlight_rules.iter_mut().find(|r| r.id == id)) {
            rule.enabled = !rule.enabled;
        }
        self.rules_changed(cx);
    }

    pub fn set_highlight_rule_color(&mut self, id: u64, color: HighlightColor, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            if let Some(rule) = project.highlight_rules.iter_mut().find(|r| r.id == id) {
                rule.color = color.clone();
            }
            project.recent_colors.push(color);
        }
        self.rules_changed(cx);
    }

    /// Move a rule to a new place in the priority order.
    pub fn move_highlight_rule(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            move_rule(&mut project.highlight_rules, from, to);
        }
        self.rules_changed(cx);
    }
}

/// The "+ Add rule" form while it is open.
#[derive(Debug, Clone)]
pub struct RuleForm {
    /// Column key chosen, if any.
    pub column: Option<String>,
    /// Ticked Filter Values, as shown (case folded when the rule is made).
    pub values: Vec<String>,
    pub custom: Vec<String>,
    pub color: HighlightColor,
    pub error: Option<&'static str>,
}

impl AppView {
    /// The columns a rule can use, in the order of the Filter: every
    /// Filterable Column (the Source Column included) and MESSAGE. Never
    /// TIMESTAMP.
    pub fn rule_columns(&self) -> Vec<(String, String)> {
        let mut columns: Vec<(String, String)> = self
            .filter_index
            .columns_in_use()
            .map(|c| (c.key.clone(), c.label.clone()))
            .collect();
        columns.push((MESSAGE_COLUMN.to_string(), "Message".to_string()));
        columns
    }

    /// "+" on the Highlights header: the New rule form. One already open
    /// keeps what was entered.
    pub fn open_rule_form(&mut self, cx: &mut Context<Self>) {
        if self.rule_form.is_some() {
            return;
        }
        let color = next_rule_color(self.highlight_rules().len());
        self.rule_form = Some(RuleForm { column: None, values: Vec::new(), custom: Vec::new(), color, error: None });
        cx.notify();
    }

    pub fn cancel_rule_form(&mut self, cx: &mut Context<Self>) {
        self.rule_form = None;
        cx.notify();
    }

    pub fn rule_form_pick_column(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            form.column = Some(key.to_string());
            form.values.clear();
            form.custom.clear();
            form.error = None;
        }
        cx.notify();
    }

    pub fn rule_form_toggle_value(&mut self, value: &str, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            match form.values.iter().position(|v| v == value) {
                Some(pos) => {
                    form.values.remove(pos);
                }
                None => form.values.push(value.to_string()),
            }
            form.error = None;
        }
        cx.notify();
    }

    /// Add a Custom Value to the form. Blank or repeated text is ignored.
    pub fn rule_form_add_custom(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let text = text.trim();
        let Some(form) = &mut self.rule_form else { return false };
        if text.is_empty() || form.custom.iter().any(|c| c == text) {
            return false;
        }
        form.custom.push(text.to_string());
        form.error = None;
        cx.notify();
        true
    }

    pub fn rule_form_remove_custom(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            form.custom.retain(|c| c != text);
        }
        cx.notify();
    }

    pub fn rule_form_set_color(&mut self, color: HighlightColor, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.rule_form {
            form.color = color;
        }
        cx.notify();
    }

    /// Make the rule: it goes at the end of the list.
    pub fn rule_form_submit(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.rule_form.clone() else { return };
        let Some(column) = form.column.clone() else {
            if let Some(f) = &mut self.rule_form {
                f.error = Some("Choose a column");
            }
            cx.notify();
            return;
        };
        match self.add_highlight_rule(&column, &form.values, &form.custom, form.color.clone(), cx) {
            Ok(()) => self.rule_form = None,
            Err(e) => {
                if let Some(f) = &mut self.rule_form {
                    f.error = Some(match e {
                        RuleError::TimestampColumn => "Timestamp cannot be highlighted",
                        RuleError::MessageNeedsCustomValue => "Message takes added values only",
                        RuleError::NoCondition => "Tick a value or add one",
                    });
                }
            }
        }
        cx.notify();
    }
}

/// Read the rows `block` (positions in `unread` and `packed`) from their
/// files and paint them: the first rule of `text` a row's value meets,
/// unless the rule told by value ids comes first.
fn paint_block(
    block: &std::ops::Range<usize>,
    unread: &[(usize, u8)],
    packed: &[u64],
    files: &[Option<(Arc<IndexedFile>, Option<Arc<str>>)>],
    text: &[TextRule],
) -> Result<Painted, String> {
    let mut by_file: std::collections::BTreeMap<usize, Vec<(usize, usize)>> = Default::default();
    for i in block.clone() {
        let (file, pos) = unpack(packed[i]);
        by_file.entry(file).or_default().push((pos, i));
    }
    let mut rows = Vec::with_capacity(block.len());
    let mut counts = vec![0usize; text.len()];
    for (file, mut lines) in by_file {
        let Some((indexed, source)) = files.get(file).and_then(|f| f.as_ref()) else {
            return Err("the file is no longer read".into());
        };
        lines.sort_unstable();
        let positions: Vec<usize> = lines.iter().map(|&(pos, _)| pos).collect();
        let mut next = 0;
        indexed
            .each_entry_at(&positions, |line, entry, parent| {
                while lines[next].0 != line {
                    next += 1;
                }
                let (row, by_ids) = unread[lines[next].1];
                let value = |key: &str| match key {
                    "source" => source.as_deref().filter(|s| !s.is_empty()),
                    "stream" => column_value(entry, "stream"),
                    key => column_value(parent, key),
                };
                let mut byte = by_ids;
                let mut won = false;
                for (i, rule) in text.iter().enumerate() {
                    if !rule.matches(value(&rule.column)) {
                        continue;
                    }
                    if !entry.is_continuation() {
                        counts[i] += 1;
                    }
                    if !won && (by_ids == 0 || rule.at < by_ids) {
                        byte = rule.at;
                        won = true;
                    }
                }
                rows.push((row, byte));
            })
            .map_err(|e| e.to_string())?;
    }
    let counts = text.iter().zip(counts).map(|(rule, n)| (rule.id, n)).collect();
    Ok(Painted { rows, counts, done: false })
}
