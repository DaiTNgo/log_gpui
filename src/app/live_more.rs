//! Logic: Clear, Load More and Back to Live Window of a Remote File.
//!
//! Clear drops every line a file holds while it keeps being read; Load More
//! reads older lines from the Server and makes the file hold a longer window
//! than the Live Window (`HoldMode::Extended`) until Back to Live Window.

use super::AppView;
use super::live::now_label;
use crate::app::ConfirmAction;
use crate::live::{HoldMode, SourceCommand, SourceStatus};
use crate::workspace::log_space::line_hash;
use gpui::{AppContext as _, Context};
use std::collections::HashSet;

/// Bytes of memory one held line takes, as measured.
pub const BYTES_PER_LINE: usize = 1_320;

/// What the UI says about the lines a Remote File holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldInfo {
    pub lines: usize,
    pub extended: bool,
    /// The file holds up to the first line of its Generation.
    pub at_start: bool,
    /// A Load More is being read.
    pub loading: bool,
    /// Lines the file can still take before the Full Read Limit.
    pub remaining: usize,
    /// Why Load More is off, when it is.
    pub load_more_off: Option<&'static str>,
}

impl HoldInfo {
    /// Estimated memory of the held lines, in MB.
    pub fn approx_mb(&self) -> usize {
        (self.lines * BYTES_PER_LINE).div_ceil(1 << 20)
    }

    /// "10,000", or "10,000, up to 4,000" when the Full Read Limit is near.
    pub fn amount_label(&self, lines: usize) -> String {
        if lines > self.remaining {
            format!("{}, up to {}", group_digits(lines), group_digits(self.remaining))
        } else {
            group_digits(lines)
        }
    }

    /// "120,000 lines held (≈160 MB) · Extended".
    pub fn tooltip(&self) -> String {
        let mut text = format!("{} lines held (≈{} MB)", group_digits(self.lines), self.approx_mb());
        if self.extended {
            text.push_str(" · Extended");
        }
        text
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

/// The "Load more" window the user is typing a number in.
pub struct CustomLoad {
    pub file_id: String,
    pub input: gpui::Entity<gpui_component::input::InputState>,
}

/// `1234567` as "1,234,567".
pub fn group_digits(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

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
                .and_then(|ix| self.all_entries.get(ix))
                .and_then(|e| e.file_id.as_deref().map(str::to_string)),
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
        if !self.is_live_remote_file(file_id) {
            return None;
        }
        let parsed = self.parsed_files.get(file_id)?;
        let buffer = parsed.live.as_ref()?;
        let at_start = buffer.at_generation_start(parsed);
        let status = self.live.sources.get(file_id).map(|s| &s.status);
        let load_more_off = if at_start {
            Some("Already at the first line")
        } else if !matches!(status, Some(SourceStatus::Live)) {
            Some("Disconnected")
        } else if parsed.entries.len() >= self.live.full_read_limit {
            Some("At the Full Read Limit")
        } else if buffer.held_start(parsed).offset.is_none() {
            Some("The oldest line is in an older generation")
        } else {
            None
        };
        Some(HoldInfo {
            lines: parsed.entries.len(),
            extended: buffer.hold == HoldMode::Extended,
            at_start,
            loading: self.live.loading_more.contains(file_id),
            remaining: self.live.full_read_limit.saturating_sub(parsed.entries.len()),
            load_more_off,
        })
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
            let Some(mut parsed) = self.parsed_files.remove(file_id) else { continue };
            let Some(mut buffer) = parsed.live.take() else {
                self.parsed_files.insert(file_id.clone(), parsed);
                continue;
            };
            let trimmed = buffer.clear(&mut parsed, &time);
            parsed.live = Some(buffer);
            self.parsed_files.insert(file_id.clone(), parsed);
            self.drop_markers_for(file_id, &trimmed);
            if let Some(source) = self.live.sources.get(file_id) {
                source.handle.send(SourceCommand::Clear);
            }
            self.live.loading_more.remove(file_id);
            if self.raw.editors.contains_key(file_id) {
                self.raw.stale.insert(file_id.clone());
            }
            if self.raw.selected.as_ref().is_some_and(|(f, _)| f == file_id) {
                self.raw.selected = None;
            }
            cleared.insert(file_id.clone());
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

    // ── Load More ───────────────────────────────────────────────────────────

    /// Read `lines` older lines (the Load More Step when `None`) above the
    /// oldest line the file holds.
    pub fn load_more(&mut self, file_id: &str, lines: Option<usize>, cx: &mut Context<Self>) {
        let Some(info) = self.hold_info(file_id) else { return };
        if info.loading {
            return;
        }
        if let Some(reason) = info.load_more_off {
            self.notice = Some(format!("Load more: {reason}"));
            cx.notify();
            return;
        }
        let Some(parsed) = self.parsed_files.get(file_id) else { return };
        let Some(buffer) = parsed.live.as_ref() else { return };
        let start = buffer.held_start(parsed);
        let Some(end_offset) = start.offset else { return };
        let remaining = self.live.full_read_limit.saturating_sub(parsed.entries.len());
        let wanted = lines.unwrap_or(self.live.load_more_step).max(1);
        let n = wanted.min(remaining).max(1);
        // The line the batch has to end just above: a different one means the
        // file changed.
        let first_hash = match (parsed.entries.first(), parsed.lines.first()) {
            (Some(entry), Some(text)) if entry.generation == start.generation => Some(line_hash(text)),
            _ => None,
        };
        let Some(source) = self.live.sources.get(file_id) else { return };
        source.handle.send(SourceCommand::LoadMore {
            generation: start.generation,
            end_offset,
            lines: n,
            first_hash,
        });
        self.live.loading_more.insert(file_id.to_string());
        cx.notify();
    }

    /// Load the next Rotated File above the lines the file holds, as many
    /// lines as the Live Window has room for.
    pub fn load_earlier(&mut self, file_id: &str, cx: &mut Context<Self>) {
        if self.live.loading_earlier.contains(file_id) {
            return;
        }
        let Some(parsed) = self.parsed_files.get(file_id) else { return };
        let room = self.live.window.saturating_sub(parsed.entries.len());
        if room == 0 {
            self.request_open_in_full(file_id, cx);
            return;
        }
        let Some(source) = self.live.sources.get(file_id) else { return };
        source.handle.send(SourceCommand::LoadEarlier { room });
        self.live.loading_earlier.insert(file_id.to_string());
        cx.notify();
    }

    /// Drop a Load More that has not started reading.
    pub fn cancel_load_more(&mut self, file_id: &str) {
        if let Some(source) = self.live.sources.get(file_id) {
            source.handle.send(SourceCommand::CancelLoadMore);
        }
    }

    /// Ask for a number of lines to load.
    pub fn open_custom_load(&mut self, file_id: &str, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let remaining = self.hold_info(file_id).map_or(self.live.full_read_limit, |i| i.remaining);
        let input = cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .placeholder(format!("1 – {}", group_digits(remaining)))
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        self.custom_load = Some(CustomLoad { file_id: file_id.to_string(), input });
        cx.notify();
    }

    pub fn cancel_custom_load(&mut self, cx: &mut Context<Self>) {
        self.custom_load = None;
        cx.notify();
    }

    /// Load the number of lines typed (commas and spaces allowed).
    pub fn accept_custom_load(&mut self, cx: &mut Context<Self>) {
        let Some(custom) = self.custom_load.as_ref() else { return };
        let text = custom.input.read(cx).value().to_string();
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
        let file_id = custom.file_id.clone();
        match digits.parse::<usize>() {
            Ok(n) if n >= 1 => {
                self.custom_load = None;
                self.load_more(&file_id, Some(n.min(self.live.full_read_limit)), cx);
            }
            _ => {
                self.notice = Some("Type a number of lines, 1 or more".to_string());
                cx.notify();
            }
        }
    }

    /// Load More of the file of the selected line.
    pub fn load_more_current_file(&mut self, cx: &mut Context<Self>) {
        if let Some(file_id) = self.current_remote_file() {
            self.load_more(&file_id, None, cx);
        }
    }

    // ── Back to Live Window ─────────────────────────────────────────────────

    /// Cut the file down to the Live Window at once and give the memory back.
    pub fn back_to_live_window(&mut self, file_id: &str, cx: &mut Context<Self>) {
        let window = self.live.window;
        let Some(mut parsed) = self.parsed_files.remove(file_id) else { return };
        let Some(mut buffer) = parsed.live.take() else {
            self.parsed_files.insert(file_id.to_string(), parsed);
            return;
        };
        let trimmed = buffer.back_to_window(&mut parsed, window);
        parsed.live = Some(buffer);
        self.parsed_files.insert(file_id.to_string(), parsed);
        if let Some(trimmed) = trimmed {
            self.drop_markers_for(file_id, &trimmed);
            if self.raw.editors.contains_key(file_id) {
                self.raw.stale.insert(file_id.to_string());
            }
            self.rebuild_open_view_keeping_position(cx);
            self.move_current_lines_off_dropped(file_id, &trimmed);
            super::live::release_freed_memory();
        }
        cx.notify();
    }

    /// Back to Live Window for the file of the selected line.
    pub fn back_to_live_window_current_file(&mut self, cx: &mut Context<Self>) {
        if let Some(file_id) = self.current_remote_file() {
            self.back_to_live_window(&file_id, cx);
        }
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn set_full_read_limit(&mut self, lines: usize, cx: &mut Context<Self>) {
        self.live.full_read_limit = lines.clamp(10_000, 20_000_000);
        self.workspace_state.full_read_limit_lines = self.live.full_read_limit;
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn set_load_more_step(&mut self, step: usize, cx: &mut Context<Self>) {
        self.live.load_more_step = step.clamp(1_000, 1_000_000);
        self.workspace_state.load_more_step = self.live.load_more_step;
        self.schedule_session_save(cx);
        cx.notify();
    }
}
