//! Logic: Clear of a Remote File, and what the UI says about the lines it
//! holds. A Remote File holds at most its Live Window (decision 10 of
//! `remote-local-cache`): Open in full reads more.

use super::AppView;
use super::live::now_label;
use crate::app::ConfirmAction;
use crate::live::SourceCommand;
use gpui::Context;
use std::collections::HashSet;

/// What the UI says about the lines a Remote File holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldInfo {
    pub lines: usize,
    /// A Remote Command.
    pub command: bool,
}

impl HoldInfo {
    /// "120,000 lines held".
    pub fn tooltip(&self) -> String {
        format!("{} lines held", group_digits(self.lines))
    }
}

/// The row to go to when row `row` goes with the files in `gone`: the first
/// row after it of another file, or the last one before it when none
/// follows. `file_of` tells the file of a row.
pub fn next_surviving_row<'a>(
    len: usize,
    file_of: impl Fn(usize) -> Option<&'a str>,
    row: usize,
    gone: &HashSet<String>,
) -> Option<usize> {
    let survives = |ix: &usize| file_of(*ix).is_none_or(|f| !gone.contains(f));
    let after = (row.saturating_add(1)..len).find(survives);
    after.or_else(|| (0..row.min(len)).rev().find(survives))
}

pub use crate::count::format_count as group_digits;

impl AppView {
    /// Whether a file of the open Log Space is a Remote File with a source.
    pub fn is_live_remote_file(&self, file_id: &str) -> bool {
        self.live.sources.get(file_id).is_some_and(|s| s.remote)
    }

    /// The Remote Files of a Log Space.
    fn remote_files_of_space(&self, space_id: &str) -> Vec<String> {
        let Some(space) = self.active_project().and_then(|p| p.log_space(space_id)) else {
            return Vec::new();
        };
        space
            .files
            .iter()
            .filter(|f| self.is_live_remote_file(&f.file_id))
            .map(|f| f.file_id.clone())
            .collect()
    }

    /// The file of the selected line, or of the only Remote File of the open
    /// Log Space.
    pub fn current_remote_file(&self) -> Option<String> {
        let selected = match self.view_mode {
            crate::workspace::working_set::ViewMode::Parsed => self
                .selected_row_pos
                .and_then(|ix| self.rows().file_id(ix).map(str::to_string)),
            crate::workspace::working_set::ViewMode::Raw => self.raw.selected.as_ref().map(|(f, _)| f.clone()),
        };
        if let Some(file_id) = selected.filter(|f| self.is_live_remote_file(f)) {
            return Some(file_id);
        }
        let space_id = self.active_space()?.id.clone();
        let mut files = self.remote_files_of_space(&space_id);
        (files.len() == 1).then(|| files.remove(0))
    }

    /// What the file holds, for tooltips and the buttons.
    pub fn hold_info(&self, file_id: &str) -> Option<HoldInfo> {
        // A local file is read whole from disk: it holds no Live Window.
        if !self.is_live_remote_file(file_id) {
            return None;
        }
        // Read through its Remote Cache.
        let cached = self.live.cached.get(file_id)?;
        let command = self.live.sources.get(file_id).is_some_and(|s| s.command);
        Some(HoldInfo { lines: cached.len(), command })
    }

    // ── Clear ───────────────────────────────────────────────────────────────

    /// How many Markers of a file would go with a Clear.
    fn markers_in_file(&self, file_id: &str) -> usize {
        self.active_project()
            .map(|p| {
                p.log_spaces
                    .iter()
                    .filter_map(|s| s.file(file_id))
                    .map(|f| f.markers.len())
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    /// Clear one Remote File; asks first when Markers would go.
    pub fn request_clear_file(&mut self, file_id: &str, cx: &mut Context<Self>) {
        self.request_clear_files(vec![file_id.to_string()], cx);
    }

    /// Clear every Remote File of a Log Space at once.
    pub fn request_clear_all_files(&mut self, space_id: &str, cx: &mut Context<Self>) {
        let files = self.remote_files_of_space(space_id);
        self.request_clear_files(files, cx);
    }

    /// Clear the file of the selected line, or the only one.
    pub fn request_clear_current_file(&mut self, cx: &mut Context<Self>) {
        if let Some(file_id) = self.current_remote_file() {
            self.request_clear_file(&file_id, cx);
        }
    }

    fn request_clear_files(&mut self, file_ids: Vec<String>, cx: &mut Context<Self>) {
        let file_ids: Vec<String> = file_ids.into_iter().filter(|f| self.is_live_remote_file(f)).collect();
        if file_ids.is_empty() {
            return;
        }
        let markers: usize = file_ids.iter().map(|f| self.markers_in_file(f)).sum();
        if markers > 0 {
            self.confirm = Some(ConfirmAction::ClearFiles { file_ids, markers });
            cx.notify();
        } else {
            self.clear_files_now(&file_ids, cx);
        }
    }

    /// Drop every line the files hold, in one update of the view.
    pub(crate) fn clear_files_now(&mut self, file_ids: &[String], cx: &mut Context<Self>) {
        // What the sources already sent is applied first: it came before the
        // Clear.
        self.pump_live(cx);
        let time = now_label();
        let mut cleared: HashSet<String> = HashSet::new();
        for file_id in file_ids {
            if let Some(cached) = self.live.cached.get_mut(file_id) {
                match cached.clear(&time) {
                    Ok(trimmed) => self.drop_markers_for(file_id, &trimmed),
                    Err(e) => self.notice = Some(format!("Could not clear {file_id}: {e}")),
                }
                self.refresh_cached_parsed(file_id);
                if let Some(source) = self.live.sources.get(file_id) {
                    source.handle.send(SourceCommand::Clear);
                }
                if self.raw.selected.as_ref().is_some_and(|(f, _)| f == file_id) {
                    self.raw.selected = None;
                }
                cleared.insert(file_id.clone());
            }
        }
        if cleared.is_empty() {
            return;
        }
        self.rebuild_open_view_without(&cleared, cx);
        super::live::release_freed_memory();
        if self.live.follow {
            self.live.unseen = 0;
            self.scroll_to_newest();
        }
        cx.notify();
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn set_full_read_limit(&mut self, lines: usize, cx: &mut Context<Self>) {
        self.live.full_read_limit = lines.clamp(10_000, 20_000_000);
        self.workspace_state.full_read_limit_lines = self.live.full_read_limit;
        self.schedule_session_save(cx);
        cx.notify();
    }
}
