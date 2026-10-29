//! Logic: Filter actions (values, custom values, Hide Mode, Time Range) and the
//! state of the Filter section's inputs.

use super::AppView;
use crate::filter::{TextCondition, parse_time_bound};
use crate::filter_index::column_value;
use crate::index::file::IndexedFile;
use crate::index::scan::{Found, scan_blocks};
use crate::index::view_rows::unpack;
use crate::view_data::FileEntries;
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use gpui_component::input::{InputEvent, InputState};

/// UI-only state of the Filter section in the context sidebar.
pub struct FilterUi {
    /// The "Add filter" column list is open.
    pub picker_open: bool,
    /// Column whose values are being edited (at most one at a time).
    pub expanded: Option<String>,
    /// "Add value" input of the expanded column.
    pub value_input: Entity<InputState>,
    /// Narrows the value list of the expanded column.
    pub search_input: Entity<InputState>,
    /// Column `search_input` was last reset for; a different column clears it.
    pub search_scope: Option<String>,
    pub from_input: Entity<InputState>,
    pub to_input: Entity<InputState>,
    /// The Time Range inputs must be rewritten from the Filter (set when the
    /// Filter is replaced, e.g. after switching file); done at render time.
    pub inputs_dirty: bool,
    _subscriptions: Vec<Subscription>,
}

impl FilterUi {
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Add value…"));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search values…"));
        let from_input = cx.new(|cx| InputState::new(window, cx).placeholder("From"));
        let to_input = cx.new(|cx| InputState::new(window, cx).placeholder("To"));

        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &value_input,
            window,
            |this: &mut AppView, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().to_string();
                    if this.filter_add_expanded_custom(&text, cx) {
                        input.update(cx, |state, cx| {
                            state.set_value(SharedString::from(""), window, cx)
                        });
                    }
                }
            },
        ));
        subscriptions.push(cx.subscribe(&search_input, |_this: &mut AppView, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        }));
        for input in [&from_input, &to_input] {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this: &mut AppView, _, event: &InputEvent, _window, cx| {
                    match event {
                        InputEvent::Change => this.on_time_inputs_typed(cx),
                        InputEvent::PressEnter { .. } => this.on_time_inputs_changed(cx),
                        _ => {}
                    }
                },
            ));
        }

        Self {
            picker_open: false,
            expanded: None,
            value_input,
            search_input,
            search_scope: None,
            from_input,
            to_input,
            inputs_dirty: true,
            _subscriptions: subscriptions,
        }
    }
}

impl AppView {
    /// The rows changed (loaded, appended, rebuilt): evaluate the Filter on
    /// them, then refresh everything that reads the rows: highlights, Search
    /// and the Focus Pane. The view keeps every line: Filter only narrows the
    /// pane.
    pub fn apply_filters_and_refresh(&mut self, cx: &mut Context<Self>) {
        self.refreshes.full += 1;
        // The Matches are found again below: a scan under way is stale.
        self.rows_epoch += 1;
        self.supersede_search_scan();
        self.evaluate_filter_pass(cx);
        if self.visible_indices.len() != self.view.len() {
            // Always every row in order: grow or cut the list where it is, not
            // a new one of the size of the file for every batch of live lines.
            let rows = self.view.len();
            let visible = std::sync::Arc::make_mut(&mut self.visible_indices);
            let have = visible.len();
            if have < rows {
                visible.extend(have..rows);
            } else {
                visible.truncate(rows);
            }
        }
        self.longest_row = self.find_longest_row();
        // A Field Column shows once a row has a value in it.
        self.sync_view_columns();
        // Rows may be new: a selection of them no longer means the same.
        self.text_selection = None;
        self.selecting = false;
        self.recompute_highlight_paints();
        self.find_matches(false, cx);
        self.refresh_focus();
        self.clamp_selected_row();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Rows from `from` on were appended after the last row, and the rows
    /// before them did not change: the cost is that of the new rows. Filter,
    /// the longest row, highlight paints and counts, Matches and
    /// the Focus Pane all continue where the last pass stopped, and give what
    /// `apply_filters_and_refresh` would. Call it after `filter_index` took
    /// the new rows in.
    pub fn apply_rows_appended(&mut self, from: usize, cx: &mut Context<Self>) {
        let rows = self.view.len();
        if from > rows || self.visible_indices.len() != from || self.filter_pass.len() > from {
            return self.apply_filters_and_refresh(cx);
        }
        self.refreshes.appended += 1;
        let pass_before = self.filter_pass.len();
        let passing = self.filter_state.evaluate_from(&self.filter_index, &self.view.bind(&self.parsed_files), from);
        std::sync::Arc::make_mut(&mut self.filter_pass).extend(passing);
        std::sync::Arc::make_mut(&mut self.visible_indices).extend(from..rows);

        let mut longest_len = self.rows().get(self.longest_row).map_or(0, |e| e.payload().len());
        for (i, entry) in crate::rows::entries_from(&self.view.bind(&self.parsed_files), from).enumerate() {
            if entry.payload().len() > longest_len {
                longest_len = entry.payload().len();
                self.longest_row = from + i;
            }
        }
        // A Field Column shows once a row has a value in it.
        self.sync_view_columns();
        self.text_selection = None;
        self.selecting = false;
        if !self.extend_highlight_paints(from) {
            self.recompute_highlight_paints();
        }
        // A scan under way takes the new rows in when it is done.
        if !self.focus.search().pending {
            let matches_before = self.focus.search().total_matches();
            self.focus.search_mut().extend_matches(&self.view.bind(&self.parsed_files), from);
            self.extend_focus(matches_before, pass_before);
        }
        self.clamp_selected_row();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// The Filter changed (a Value, Hide Mode, a Custom Value, the Time
    /// Range) and the rows did not: only the Focus Pane follows. Matches,
    /// highlight paints and rule counts read every row of the view (Search
    /// matches the whole view, ADR 0012), so they stay as they are.
    pub fn apply_filter_change(&mut self, cx: &mut Context<Self>) {
        self.refreshes.filter += 1;
        self.evaluate_filter_pass(cx);
        self.text_selection = None;
        self.selecting = false;
        self.refresh_focus();
        self.clamp_selected_row();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Evaluate the Filter on the rows. The conditions read from the value
    /// ids apply at once; one that needs the text of a row's value (task
    /// 5.3) is checked here on the rows of files held in memory, and by a
    /// Scan on those of files read from disk: their rows join the Focus
    /// Pane as it finds them.
    fn evaluate_filter_pass(&mut self, cx: &mut Context<Self>) {
        // A Scan of the Filter before is stale.
        self.filter_scan = None;
        self.filter_scanning = None;
        let rows = self.view.bind(&self.parsed_files);
        let (pass, text) = self.filter_state.evaluate_split(&self.filter_index, rows.len(), 0);
        if text.is_empty() {
            self.filter_pass = Arc::new(pass);
            return;
        }
        let mut held = Vec::new();
        let mut on_disk = Vec::new();
        for row in pass {
            let file = self.view.rows.packed(row).map(|p| unpack(p).0).and_then(|f| rows.file(f));
            if matches!(file, Some(FileEntries::Indexed(_))) {
                on_disk.push(row);
            } else if text.iter().all(|c| c.passes_row(&rows, row)) {
                held.push(row);
            }
        }
        self.filter_pass = Arc::new(held.clone());
        if !on_disk.is_empty() {
            self.start_filter_scan(held, on_disk, text, cx);
        }
    }

    /// Check `text` on the rows `rows` (of files read from disk) with a
    /// Scan, the rows around the one shown first. `held` are the rows that
    /// passed already.
    fn start_filter_scan(&mut self, held: Vec<usize>, rows: Vec<usize>, text: Vec<TextCondition>, cx: &mut Context<Self>) {
        let packed: Vec<u64> = rows.iter().map(|&r| self.view.rows.packed(r).unwrap_or(u64::MAX)).collect();
        let files: Vec<Option<(Arc<IndexedFile>, Option<Arc<str>>)>> = self
            .view
            .files
            .iter()
            .map(|f| {
                let indexed = self.parsed_files.get(&*f.file_id).and_then(|p| p.indexed.clone());
                indexed.map(|i| (i, f.source.clone()))
            })
            .collect();
        let top = self.top_row_anchor().map_or(0, |a| a.row);
        let around = rows.partition_point(|&r| r < top).min(rows.len().saturating_sub(1));
        let cancel = Arc::new(AtomicBool::new(false));
        let generation = SCANS.fetch_add(1, Ordering::Relaxed);
        let scan = FilterScan { generation, cancel: cancel.clone(), held, rows: Arc::new(rows), scanned_rows: self.view.len() };
        let total = scan.rows.len();
        self.filter_scan = Some(scan);
        self.filter_scanning = Some((0, total));
        let (tx, updates) = async_channel::unbounded::<Found>();
        cx.background_executor()
            .spawn(async move {
                let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
                let look = |block: &std::ops::Range<usize>| passing_in(block, &packed, &files, &text);
                let end = scan_blocks(0..total, around, workers, &cancel, look, |f| {
                    let _ = tx.try_send(f);
                });
                if !cancel.load(Ordering::Acquire) {
                    let _ = tx.try_send(end);
                }
            })
            .detach();
        cx.spawn(async move |this, cx| {
            while let Ok(found) = updates.recv().await {
                let done = found.done || found.error.is_some();
                let go_on = this.update(cx, |this, cx| this.filter_scan_found(generation, found, cx)).unwrap_or(false);
                if done || !go_on {
                    break;
                }
            }
        })
        .detach();
    }

    /// What the Filter's Scan `generation` found so far; whether to go on
    /// listening.
    fn filter_scan_found(&mut self, generation: u64, found: Found, cx: &mut Context<Self>) -> bool {
        let Some(scan) = self.filter_scan.as_ref().filter(|s| s.generation == generation) else {
            return false;
        };
        if let Some(_error) = &found.error {
            // The lines could not be read: the Filter reads them here.
            self.filter_scan = None;
            self.filter_scanning = None;
            self.filter_pass = Arc::new(self.filter_state.evaluate(&self.filter_index, &self.view.bind(&self.parsed_files)));
            self.refresh_focus();
            self.clamp_selected_row();
            cx.notify();
            return false;
        }
        let found_rows: Vec<usize> = found.lines.iter().map(|&i| scan.rows[i as usize]).collect();
        let mut pass = merge_sorted(&scan.held, &found_rows);
        // Rows appended since passed the Filter as they came.
        let after = scan.scanned_rows;
        pass.extend(self.filter_pass.iter().copied().filter(|&r| r >= after));
        self.filter_pass = Arc::new(pass);
        if found.done {
            self.filter_scan = None;
            self.filter_scanning = None;
        } else {
            self.filter_scanning = Some((found.scanned, found.total));
        }
        self.refresh_focus();
        self.clamp_selected_row();
        cx.notify();
        !found.done
    }

    /// Whether a Scan is checking the Filter.
    pub fn filter_is_scanning(&self) -> bool {
        self.filter_scan.is_some()
    }

    fn clamp_selected_row(&mut self) {
        if let Some(pos) = self.selected_row_pos {
            if pos >= self.visible_indices.len() {
                self.selected_row_pos = if self.visible_indices.is_empty() { None } else { Some(0) };
            }
        }
    }

    /// Detect the columns and values of the loaded file. Call whenever
    /// `all_entries` is replaced.
    pub fn rebuild_filter_index(&mut self) {
        self.filter_index = crate::filter_index::FilterIndex::build(&self.view.bind(&self.parsed_files));
        self.filter_ui.picker_open = false;
        self.filter_ui.expanded = None;
        self.filter_ui.inputs_dirty = true;
    }

    pub fn toggle_filter_picker(&mut self, cx: &mut Context<Self>) {
        self.filter_ui.picker_open = !self.filter_ui.picker_open;
        cx.notify();
    }

    /// Open a column for editing (or close it when it is already open).
    pub fn toggle_filter_column(&mut self, key: &str, cx: &mut Context<Self>) {
        let open = self.filter_ui.expanded.as_deref() == Some(key);
        self.filter_ui.expanded = if open { None } else { Some(key.to_string()) };
        self.filter_ui.picker_open = false;
        cx.notify();
    }

    /// Choose a column from the "Add filter" list. Columns above the distinct
    /// value limit are not available. Message takes only Custom Values.
    pub fn pick_filter_column(&mut self, key: &str, cx: &mut Context<Self>) {
        let available = key == crate::columns::MESSAGE_KEY
            || self.filter_index.column(key).is_some_and(|c| c.is_filterable());
        if available {
            self.filter_ui.expanded = Some(key.to_string());
            self.filter_ui.picker_open = false;
            cx.notify();
        }
    }

    pub fn filter_toggle_value(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        self.filter_state.toggle_value(key, value);
        self.apply_filter_change(cx);
    }

    /// Add the typed text as a Custom Value of the expanded column.
    pub fn filter_add_expanded_custom(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.filter_ui.expanded.clone() else {
            return false;
        };
        let added = self.filter_state.add_custom(&key, text);
        if added {
            self.apply_filter_change(cx);
        }
        added
    }

    pub fn filter_remove_custom(&mut self, key: &str, text: &str, cx: &mut Context<Self>) {
        self.filter_state.remove_custom(key, text);
        self.apply_filter_change(cx);
    }

    pub fn filter_set_hide(&mut self, key: &str, hide: bool, cx: &mut Context<Self>) {
        self.filter_state.set_hide(key, hide);
        self.apply_filter_change(cx);
    }

    pub fn filter_clear_column(&mut self, key: &str, cx: &mut Context<Self>) {
        self.filter_state.clear_column(key);
        if self.filter_ui.expanded.as_deref() == Some(key) {
            self.filter_ui.expanded = None;
        }
        self.apply_filter_change(cx);
    }

    pub fn filter_clear_all(&mut self, cx: &mut Context<Self>) {
        self.filter_state.clear();
        self.filter_ui.expanded = None;
        self.filter_ui.inputs_dirty = true;
        self.apply_filter_change(cx);
    }

    /// The From/To inputs changed: store the typed bounds and re-filter.
    pub fn on_time_inputs_changed(&mut self, cx: &mut Context<Self>) {
        self.time_range_generation += 1;
        self.apply_time_inputs(cx);
    }

    /// The From/To inputs changed as the user types: the Filter follows once
    /// the typing pauses, and only when the bounds typed can be read and are
    /// not the bounds already in use. Half-typed text waits.
    pub fn on_time_inputs_typed(&mut self, cx: &mut Context<Self>) {
        let (from, to) = self.typed_time_range(cx);
        let anchor = self.filter_index.anchor_day();
        let (Ok(new_from), Ok(new_to)) = (parse_time_bound(&from, anchor), parse_time_bound(&to, anchor)) else {
            return;
        };
        if (new_from, new_to) == self.filter_state.time_range.bounds(anchor) {
            // The same bounds, maybe written another way: remember the text.
            self.filter_state.set_time_range(&from, &to);
            return;
        }
        self.time_range_generation += 1;
        let generation = self.time_range_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(super::search::SEARCH_DEBOUNCE).await;
            this.update(cx, |this, cx| {
                if this.time_range_generation == generation {
                    this.apply_time_inputs(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn typed_time_range(&self, cx: &Context<Self>) -> (String, String) {
        let from = self.filter_ui.from_input.read(cx).value().to_string();
        let to = self.filter_ui.to_input.read(cx).value().to_string();
        (from.trim().to_string(), to.trim().to_string())
    }

    fn apply_time_inputs(&mut self, cx: &mut Context<Self>) {
        let (from, to) = self.typed_time_range(cx);
        if from == self.filter_state.time_range.from && to == self.filter_state.time_range.to {
            return;
        }
        self.filter_state.set_time_range(&from, &to);
        self.apply_filter_change(cx);
    }

    /// Whether a typed time bound can be read; empty text is fine.
    pub fn time_bound_is_valid(&self, text: &str) -> bool {
        parse_time_bound(text, self.filter_index.anchor_day()).is_ok()
    }

    /// Rewrite the Time Range inputs from the Filter and refresh their hints.
    /// Called from `render`, where a window is available.
    /// A value-search box starts empty for each column it is shown for.
    pub fn sync_value_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = self.filter_ui.expanded.clone();
        if scope != self.filter_ui.search_scope {
            self.filter_ui.search_scope = scope;
            self.filter_ui
                .search_input
                .update(cx, |state, cx| state.set_value(SharedString::from(""), window, cx));
        }
        let scope = self.rule_form.as_ref().and_then(|f| f.column.clone());
        if scope != self.rule_search_scope {
            self.rule_search_scope = scope;
            self.rule_search
                .update(cx, |state, cx| state.set_value(SharedString::from(""), window, cx));
        }
    }

    pub fn sync_filter_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.filter_ui.inputs_dirty {
            return;
        }
        self.filter_ui.inputs_dirty = false;
        let (from, to) = (
            self.filter_state.time_range.from.clone(),
            self.filter_state.time_range.to.clone(),
        );
        let (from_hint, to_hint) = match self.filter_index.time_hint() {
            Some((a, b)) => (a, b),
            None => ("From".to_string(), "To".to_string()),
        };
        self.filter_ui.from_input.update(cx, |state, cx| {
            state.set_placeholder(from_hint, window, cx);
            state.set_value(SharedString::from(from), window, cx);
        });
        self.filter_ui.to_input.update(cx, |state, cx| {
            state.set_placeholder(to_hint, window, cx);
            state.set_value(SharedString::from(to), window, cx);
        });
    }
}

/// Position of the largest of `lens` (the first one on a tie), 0 when empty.
impl AppView {
    /// The row with the longest text, which sets how wide the list is. A
    /// file read through its Line Index gives its longest line from the
    /// index; only held files are walked.
    pub(crate) fn find_longest_row(&self) -> usize {
        let rows = self.rows();
        let mut best: Option<(usize, usize, usize)> = None; // (length, file, position)
        for file in 0..self.view.files.len() {
            let found = match rows.file(file) {
                Some(crate::view_data::FileEntries::Indexed(indexed)) => {
                    indexed.longest_line().map(|(pos, len)| (len as usize, pos))
                }
                Some(crate::view_data::FileEntries::Held(entries)) => {
                    let pos = longest(entries.iter().map(|e| e.payload().len()));
                    entries.get(pos).map(|e| (e.payload().len(), pos))
                }
                None => None,
            };
            if let Some((len, pos)) = found {
                if best.is_none_or(|(b, _, _)| len > b) {
                    best = Some((len, file, pos));
                }
            }
        }
        best.and_then(|(_, file, pos)| self.view.rows.row_of(file, pos)).unwrap_or(0)
    }
}

pub(crate) fn longest(lens: impl Iterator<Item = usize>) -> usize {
    let mut best = (0, 0);
    for (pos, len) in lens.enumerate() {
        if len > best.1 {
            best = (pos, len);
        }
    }
    best.0
}

/// Filter Scans started: each one's number.
static SCANS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A Scan checking the Filter conditions that need the rows' text; dropping
/// it stops the Scan.
pub(crate) struct FilterScan {
    generation: u64,
    cancel: Arc<AtomicBool>,
    /// The rows that passed without it (of files held in memory).
    held: Vec<usize>,
    /// The rows it checks, ascending.
    rows: Arc<Vec<usize>>,
    /// Rows the view had when it started.
    scanned_rows: usize,
}

impl Drop for FilterScan {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// Which of the rows `block` (positions in `packed`, the rows checked)
/// pass every condition of `text`, their lines read from their files.
fn passing_in(
    block: &std::ops::Range<usize>,
    packed: &[u64],
    files: &[Option<(Arc<IndexedFile>, Option<Arc<str>>)>],
    text: &[TextCondition],
) -> Result<Vec<u32>, String> {
    let mut by_file: std::collections::BTreeMap<usize, Vec<(usize, u32)>> = Default::default();
    for i in block.clone() {
        let (file, pos) = unpack(packed[i]);
        by_file.entry(file).or_default().push((pos, i as u32));
    }
    let mut passing = Vec::new();
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
                let passes = text.iter().all(|c| {
                    let value = match c.column.as_str() {
                        "source" => source.as_deref().filter(|s| !s.is_empty()),
                        "stream" => column_value(entry, "stream"),
                        key => column_value(parent, key),
                    };
                    c.passes(value)
                });
                if passes {
                    passing.push(lines[next].1);
                }
            })
            .map_err(|e| e.to_string())?;
    }
    passing.sort_unstable();
    Ok(passing)
}

/// The rows of two ascending lists, ascending.
fn merge_sorted(a: &[usize], b: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] <= b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            out.push(b[j]);
            j += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}
