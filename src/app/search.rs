//! Logic: search state, match navigation and row jumping. The Search
//! Expression belongs to the open Focus Tab.

use super::AppView;
use super::focus::Origin;
use crate::workspace::working_set::ViewMode;
use crate::index::scan::{Found, scan_in_steps};
use gpui::{Context, Window};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A Scan for the Matches under way; dropping it stops it.
pub(crate) struct SearchScan {
    cancel: Arc<AtomicBool>,
    /// Go to the current Match once it is done (Search, not a refresh).
    jump: bool,
}

impl Drop for SearchScan {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl AppView {
    /// Find the Matches of the open tab's Search Expression among every row
    /// now, reading the rows' texts as a Scan does (from the files that are
    /// read through their Line Index), on every core.
    pub(crate) fn find_matches_now(&mut self) {
        let texts = self.view.texts(&self.parsed_files);
        let rows = texts.len();
        self.focus.search_mut().update_matches_with(|matcher| {
            let matcher = matcher.clone();
            let find = move |first: usize, lines: &[&str]| -> Vec<u32> {
                lines.iter().enumerate().filter(|(_, l)| matcher.is_match(l)).map(|(i, _)| (first + i) as u32).collect()
            };
            crate::index::scan::scan_all(&texts, 0..rows, &find).lines.into_iter().map(|r| r as usize).collect()
        });
    }

    /// Find the Matches of the open tab's Search Expression again: here and
    /// now when every file of the view is held in memory, else by a Scan
    /// of the lines read from disk (task 6.2), the rows around the one
    /// shown first. `jump`: go to the current Match once they are found.
    pub(crate) fn find_matches(&mut self, jump: bool, cx: &mut Context<Self>) {
        let on_disk = self.view.files.iter().any(|f| self.parsed_files.get(&*f.file_id).is_some_and(|p| p.indexed.is_some()));
        let search = self.focus.search_mut();
        if search.reparse() {
            search.current_match_idx = 0;
        }
        search.pending = false;
        if !on_disk || search.matcher.is_none() {
            self.find_matches_now();
            if jump && let Some(entry_ix) = self.focus.search().current_match_pos() {
                self.goto_entry(entry_ix, Origin::Jump, cx);
            }
            return;
        }
        self.supersede_search_scan();
        let generation = self.search_generation;
        self.start_search_scan(generation, jump, cx);
    }

    /// Focus the search field and make sure the active query is surfaced
    /// (match navigator, ruler marks, highlight).
    pub fn activate_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.search_input.read(cx).value().to_string();
        let search = self.focus.search_mut();
        search.query = text;
        search.is_open = true;
        self.search_input.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    /// Show the open tab's query in the search field. Called from `render`.
    pub fn sync_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input_dirty {
            return;
        }
        self.search_input_dirty = false;
        let query = self.focus.search().query.clone();
        self.search_input.update(cx, |state, cx| {
            state.set_value(gpui::SharedString::from(query), window, cx);
        });
    }

    /// A scan under way is not needed any more: it stops, and what it would
    /// find is dropped.
    pub fn supersede_search_scan(&mut self) {
        self.search_generation += 1;
        self.search_scan = None;
        self.focus.search_mut().scanning = None;
    }

    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        // Found here and now: a scan under way is not needed.
        self.supersede_search_scan();
        if self.view_mode == ViewMode::Raw {
            // Raw View ignores Filter: look at every line of every section.
            self.run_raw_search(cx);
            return;
        }
        self.find_matches(true, cx);
        self.refresh_focus();
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn jump_next_match(&mut self, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Raw {
            self.raw_jump_next(cx);
            return;
        }
        if let Some(entry_ix) = self.focus.search_mut().next_match() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
    }

    pub fn jump_prev_match(&mut self, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Raw {
            self.raw_jump_prev(cx);
            return;
        }
        if let Some(entry_ix) = self.focus.search_mut().prev_match() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
    }

    /// Jump to a specific search match by absolute entry index (used by the
    /// overview ruler click). Updates the current match so the navigator
    /// counter reflects the new position.
    pub fn jump_to_match_by_entry(&mut self, entry_ix: usize, cx: &mut Context<Self>) {
        let search = self.focus.search_mut();
        if let Ok(match_idx) = search.matched_visible_indices.binary_search(&entry_ix) {
            search.current_match_idx = match_idx;
        }
        self.jump_to_row(entry_ix, cx);
    }

    /// Select a row of the view and scroll to it. The view shows every entry,
    /// so a row position is the entry's position.
    pub fn jump_to_row(&mut self, vis_pos: usize, cx: &mut Context<Self>) {
        if vis_pos < self.visible_indices.len() {
            self.goto_entry(vis_pos, Origin::Jump, cx);
        }
    }

    /// A row of the view was clicked: select it; the pane follows.
    pub fn select_view_row(&mut self, vis_pos: usize, cx: &mut Context<Self>) {
        if vis_pos < self.visible_indices.len() {
            self.goto_entry(vis_pos, Origin::View, cx);
        }
    }

    pub fn trigger_search_query(&mut self, q: &str, cx: &mut Context<Self>) {
        let search = self.focus.search_mut();
        search.query = q.to_string();
        search.is_open = true;
        self.run_search(cx);
    }

    /// The text of the Search field changed, as the user types. The Matches
    /// are found on a worker thread once the typing pauses (`SEARCH_DEBOUNCE`),
    /// and only for the last text: until then the Matches are those of the
    /// text before it, and the typed expression already highlights.
    pub fn search_text_changed(&mut self, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Raw {
            return self.run_search(cx);
        }
        let search = self.focus.search_mut();
        if !search.reparse() && search.parse_error.is_some() {
            // Half typed: the expression before it stays in use.
            cx.notify();
            return;
        }
        if search.matcher.is_none() {
            // Nothing to scan for: the Matches go at once.
            return self.run_search(cx);
        }
        search.pending = true;
        self.supersede_search_scan();
        let generation = self.search_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            this.update(cx, |this, cx| {
                // A pause in the typing: what was typed counts as searched.
                if generation == this.search_generation {
                    this.record_search_history(cx);
                }
                this.start_search_scan(generation, true, cx)
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Type `text` in the Search field, as the user would.
    pub fn type_search_query(&mut self, text: &str, cx: &mut Context<Self>) {
        let search = self.focus.search_mut();
        search.query = text.to_string();
        search.is_open = true;
        self.search_text_changed(cx);
    }

    /// Look for the Matches among every row with a Scan: on worker threads,
    /// the rows around the one shown first. The Matches found so far show as
    /// they come, with how far the Scan is; Next and Previous go through
    /// them. Live lines can go on arriving meanwhile.
    fn start_search_scan(&mut self, generation: u64, jump: bool, cx: &mut Context<Self>) {
        if generation != self.search_generation {
            return;
        }
        let Some(matcher) = self.focus.search().matcher.clone() else { return };
        let texts = Arc::new(self.view.texts(&self.parsed_files));
        let rows = texts.len();
        let around = self.top_row_anchor().map_or(0, |a| a.row);
        let epoch = self.rows_epoch;
        let matcher: Arc<crate::index::scan::Matcher> = Arc::new(move |first: usize, lines: &[&str]| {
            lines.iter().enumerate().filter(|(_, l)| matcher.is_match(l)).map(|(i, _)| (first + i) as u32).collect()
        });
        let cancel = Arc::new(AtomicBool::new(false));
        self.search_scan = Some(SearchScan { cancel: cancel.clone(), jump });
        self.focus.search_mut().scanning = Some((0, rows));
        self.refreshes.scans += 1;
        let (tx, updates) = async_channel::unbounded::<Found>();
        cx.background_executor()
            .spawn(async move {
            let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
            let end = scan_in_steps(&*texts, 0..rows, around, &*matcher, workers, &cancel, |f| {
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
                let go_on = this
                    .update(cx, |this, cx| this.search_scan_found(generation, epoch, rows, found, cx))
                    .unwrap_or(false);
                if done || !go_on {
                    break;
                }
            }
        })
        .detach();
    }

    /// Whether a Scan is looking for the Matches.
    pub fn search_scanning(&self) -> bool {
        self.search_scan.is_some()
    }

    /// What the Scan of `generation` found so far; whether to go on
    /// listening.
    fn search_scan_found(&mut self, generation: u64, epoch: u64, rows: usize, found: Found, cx: &mut Context<Self>) -> bool {
        if generation != self.search_generation {
            return false;
        }
        if epoch != self.rows_epoch {
            // Rows left or moved while it ran: look again.
            let jump = self.search_scan.take().is_some_and(|s| s.jump);
            self.start_search_scan(generation, jump, cx);
            return false;
        }
        let matches: Vec<usize> = found.lines.iter().map(|&r| r as usize).collect();
        if !found.done {
            let search = self.focus.search_mut();
            search.scanning = Some((found.scanned, found.total));
            if found.error.is_none() {
                search.matched_visible_indices = Arc::new(matches);
                if search.current_match_idx >= search.matched_visible_indices.len() {
                    search.current_match_idx = 0;
                }
                cx.notify();
                return true;
            }
            // The rows could not all be read: they are searched here.
            let jump = self.search_scan.take().is_some_and(|s| s.jump);
            self.focus.search_mut().scanning = None;
            self.find_matches_now();
            if jump && let Some(entry_ix) = self.focus.search().current_match_pos() {
                self.goto_entry(entry_ix, Origin::Jump, cx);
            }
            self.refresh_focus();
            cx.notify();
            return false;
        }
        let jump = self.search_scan.take().is_some_and(|s| s.jump);
        self.focus.search_mut().scanning = None;
        // Rows that arrived after the Scan started are searched here.
        self.focus.search_mut().set_scanned_matches(matches, &self.view.bind(&self.parsed_files), rows);
        self.refresh_focus();
        if jump && let Some(entry_ix) = self.focus.search().current_match_pos() {
            self.goto_entry(entry_ix, Origin::Jump, cx);
        }
        self.schedule_session_save(cx);
        cx.notify();
        false
    }
}

/// How long the Search field stays still before the Matches are looked for.
pub const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(100);

// ── Recent Search Expressions ───────────────────────────────────────────────

impl AppView {
    /// The Recent Search Expressions of the active Project, newest first.
    pub fn search_history(&self) -> Vec<String> {
        self.active_project().map(|p| p.search_history.items.clone()).unwrap_or_default()
    }

    /// Remember the expression in the Search field when it parses; while the
    /// user goes on typing it, it replaces the one recorded at the last pause.
    pub fn record_search_history(&mut self, cx: &mut Context<Self>) {
        let search = self.focus.search();
        if search.matcher.is_none() || search.parse_error.is_some() {
            return;
        }
        let query = search.query.clone();
        let streak = self.search_history_streak;
        if let Some(project) = self.active_project_mut() {
            project.search_history.record(&query, streak);
        }
        self.search_history_streak = true;
        self.schedule_session_save(cx);
    }

    /// ↑ (`older`) or ↓ in the Search field: step through the Recent Search
    /// Expressions while the field is empty or already showing one. Returns
    /// whether the key was taken.
    pub fn step_search_history(&mut self, older: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let stepping = self.search_history_at.is_some();
        if !stepping && !self.focus.search().query.is_empty() {
            return false;
        }
        let history = self.active_project().map(|p| p.search_history.clone()).unwrap_or_default();
        let at = self.search_history_at.as_ref().map(|(i, _)| *i);
        let typed = self.search_history_at.as_ref().map_or_else(|| self.focus.search().query.clone(), |(_, t)| t.clone());
        let next = history.step(at, older);
        let text = match next {
            Some(i) => history.items[i].clone(),
            None if stepping => typed.clone(),
            None => return false,
        };
        self.search_history_at = next.map(|i| (i, typed));
        self.show_search_text(&text, window, cx);
        true
    }

    /// Escape while stepping through Recent Search Expressions: back to what
    /// was typed. Returns whether it was stepping.
    pub fn leave_search_history(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some((_, typed)) = self.search_history_at.take() else { return false };
        self.show_search_text(&typed, window, cx);
        true
    }

    /// Search for a Recent Search Expression picked from the list.
    pub fn pick_search_history(&mut self, expression: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search_history_at = None;
        self.show_search_text(expression, window, cx);
        self.search_input.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn remove_search_history(&mut self, expression: &str, cx: &mut Context<Self>) {
        if let Some(project) = self.active_project_mut() {
            project.search_history.remove(expression);
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Put `text` in the Search field and search for it, as typing would.
    fn show_search_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |input, cx| input.set_value(gpui::SharedString::from(text.to_string()), window, cx));
        // set_value emits no Change: the query follows here.
        let search = self.active_search_mut();
        search.query = text.to_string();
        search.is_open = !text.trim().is_empty();
        self.search_text_changed(cx);
    }
}
