//! Logic: Raw View. Each file of the open Log Space is a collapsible File
//! Section showing its text line by line. Imported files are read-only; a
//! scratch file is edited in an editor and saved by itself after a pause.
//!
//! Raw View ignores Filter, so Search here looks at every line of every
//! section, only highlights, and never hides anything.

use super::AppView;
use crate::raw_view::{self, RawMatch};
use crate::space_view::{ParsedFile, split_lines};
use crate::palette::{HighlightColor, Tone, text_rgb};
use crate::workspace::log_space::{LogSpaceKind, line_hash};
use crate::workspace::scratch_store;
use crate::workspace::working_set::{TabTarget, ViewMode};
use gpui::{
    AppContext as _, Context, HighlightStyle, ScrollHandle, Subscription,
    UniformListScrollHandle, Window, hsla, point, px,
};
use gpui_component::input::{EditorState, Position, InputEvent, TextDecoration, TextDecorationCollection};
use gpui_component::ActiveTheme as _;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Idle time after typing before a scratch file is written to disk.
pub const SCRATCH_SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// Height of a File Section header and of an imported line, in pixels.
pub const RAW_ROW_HEIGHT: f32 = 22.0;
/// Line height of a scratch editor, in pixels (1.5 x the 12px font).
pub const EDITOR_LINE_HEIGHT: f32 = 18.0;
/// Vertical padding the editor adds around its text.
pub const EDITOR_PADDING: f32 = 16.0;

/// A scratch file's editor.
pub struct ScratchEditor {
    pub state: gpui::Entity<EditorState>,
    pub highlights: Option<TextDecorationCollection>,
    _subscription: Subscription,
    /// Re-renders the view when the editor's selection moves, so the color
    /// bar follows it.
    _observe: Subscription,
}

/// One File Section as the Raw View shows it.
#[derive(Clone)]
pub struct RawSection {
    pub file_id: String,
    pub display_name: String,
    pub folder: Option<String>,
    pub lines: Arc<Vec<String>>,
    /// A scratch file opened as a File View can be edited here.
    pub editable: bool,
}

pub struct RawViewState {
    /// Scroll of the list of imported lines.
    pub scroll: UniformListScrollHandle,
    /// Scroll of the stacked scratch editors.
    pub section_scroll: ScrollHandle,
    pub editors: HashMap<String, ScratchEditor>,
    /// Current text of each edited scratch file.
    pub texts: HashMap<String, String>,
    /// Scratch files edited since they were parsed.
    pub edited: HashSet<String>,
    /// Scratch files edited since they were written to disk.
    pub unsaved: HashSet<String>,
    save_gen: HashMap<String, u64>,
    pub matches: Vec<RawMatch>,
    pub current: usize,
    /// Whether long lines wrap in the editors.
    pub soft_wrap: bool,
    /// A Match to bring into view on the next render: `(file_id, 0-based line)`.
    pending_reveal: Option<(String, usize)>,
    /// The imported line selected for copying: `(file_id, 0-based line)`.
    pub selected: Option<(String, usize)>,
    /// The split lines of each edited scratch file, so rendering does not
    /// split the whole text again every frame.
    /// Selection each editor had when the view last re-rendered for it.
    pub last_selection: HashMap<String, std::ops::Range<usize>>,
    lines_cache: std::cell::RefCell<HashMap<String, Arc<Vec<String>>>>,
}

impl RawViewState {
    pub fn new() -> Self {
        Self {
            scroll: UniformListScrollHandle::new(),
            section_scroll: ScrollHandle::new(),
            editors: HashMap::new(),
            texts: HashMap::new(),
            edited: HashSet::new(),
            unsaved: HashSet::new(),
            save_gen: HashMap::new(),
            matches: Vec::new(),
            current: 0,
            soft_wrap: true,
            pending_reveal: None,
            selected: None,
            last_selection: HashMap::new(),
            lines_cache: Default::default(),
        }
    }

    pub fn clear_search(&mut self) {
        self.matches.clear();
        self.current = 0;
    }

    /// `(current, total)` of the Raw View's Matches for the navigator.
    pub fn match_display(&self) -> (usize, usize) {
        if self.matches.is_empty() { (0, 0) } else { (self.current + 1, self.matches.len()) }
    }
}

impl Default for RawViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppView {
    // ── View mode ───────────────────────────────────────────────────────────

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if self.view_mode == mode || !self.active_tab_is_log_file() {
            return;
        }
        self.persist_active_session();
        if let Some(tab) = self
            .active_project_mut()
            .and_then(|p| p.working_set.active_tab_mut())
        {
            tab.session.mode = mode;
        }
        // Leaving the Raw View parses the scratch files edited there again.
        self.restore_active_session(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_view_mode(&mut self, cx: &mut Context<Self>) {
        let next = match self.view_mode {
            ViewMode::Raw => ViewMode::Parsed,
            ViewMode::Parsed => ViewMode::Raw,
        };
        self.set_view_mode(next, cx);
    }

    // ── Sections ────────────────────────────────────────────────────────────

    /// The sections of the focused tab: every file of a Log Space in the
    /// space's file order, or the one file of a File View.
    pub fn raw_sections(&self) -> Vec<RawSection> {
        let Some(target) = self.active_target() else { return Vec::new() };
        let Some(space) = target.space_id().and_then(|id| self.active_project()?.log_space(id)) else {
            return Vec::new();
        };
        let only = match &target {
            TabTarget::FileView { file_id, .. } => Some(file_id.as_str()),
            _ => None,
        };
        let empty = Arc::new(Vec::new());
        space
            .ordered_files()
            .into_iter()
            .filter(|f| only.is_none_or(|id| id == f.file_id))
            .map(|f| {
                let scratch = space.kind == LogSpaceKind::Scratch;
                // Only a File View edits; a Log Space's Raw View just shows the files.
                let editable = scratch && only.is_some();
                let lines = match self.raw.texts.get(&f.file_id) {
                    Some(text) if scratch => self
                        .raw
                        .lines_cache
                        .borrow_mut()
                        .entry(f.file_id.clone())
                        .or_insert_with(|| Arc::new(split_lines(text)))
                        .clone(),
                    _ => self
                        .parsed_files
                        .get(&f.file_id)
                        .map_or_else(|| empty.clone(), |p| p.lines.clone()),
                };
                RawSection {
                    file_id: f.file_id.clone(),
                    display_name: f.display_name.clone(),
                    folder: f.folder(),
                    lines,
                    editable,
                }
            })
            .collect()
    }

    pub fn is_section_collapsed(&self, file_id: &str) -> bool {
        self.active_project()
            .and_then(|p| p.working_set.active_tab())
            .is_some_and(|t| t.session.collapsed_sections.contains(file_id))
    }

    pub fn set_section_collapsed(&mut self, file_id: &str, collapsed: bool, cx: &mut Context<Self>) {
        if let Some(tab) = self
            .active_project_mut()
            .and_then(|p| p.working_set.active_tab_mut())
        {
            if collapsed {
                tab.session.collapsed_sections.insert(file_id.to_string());
            } else {
                tab.session.collapsed_sections.remove(file_id);
            }
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn toggle_section(&mut self, file_id: &str, cx: &mut Context<Self>) {
        let collapsed = self.is_section_collapsed(file_id);
        self.set_section_collapsed(file_id, !collapsed, cx);
    }

    // ── Scratch editors ─────────────────────────────────────────────────────

    /// Create the editors of the open scratch space (needs a window, so it is
    /// called while rendering).
    pub fn sync_raw_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view_mode != ViewMode::Raw {
            return;
        }
        let mut created = false;
        for section in self.raw_sections() {
            if self.raw.editors.contains_key(&section.file_id) {
                continue;
            }
            let text = self
                .raw
                .texts
                .get(&section.file_id)
                .cloned()
                .unwrap_or_else(|| section.lines.join("\n"));
            let state = cx.new(|cx| EditorState::new(window, cx).scroll_beyond_last_line(Some(0)));
            state.update(cx, |state, cx| {
                state.set_value(text.clone(), window, cx);
                state.set_soft_wrap(self.raw.soft_wrap, window, cx);
            });
            let file_id = section.file_id.clone();
            let editable = section.editable;
            let subscription = cx.subscribe_in(
                &state,
                window,
                move |this: &mut Self, state, event: &InputEvent, _window, cx| {
                    if editable && matches!(event, InputEvent::Change) {
                        let text = state.read(cx).value().to_string();
                        this.on_scratch_edited(&file_id, text, cx);
                    }
                },
            );
            // A new, empty scratch file: the cursor starts in it.
            if section.editable && text.is_empty() && self.focus.search().query.is_empty() {
                state.update(cx, |state, cx| state.focus(window, cx));
            }
            if section.editable {
                self.raw.texts.entry(section.file_id.clone()).or_insert(text);
            }
            // Re-render only when the selection moves (not on every cursor blink).
            let observed_id = section.file_id.clone();
            let observe = cx.observe(&state, move |this, state, cx| {
                let (range, cursor) = {
                    let st = state.read(cx);
                    (st.selected_range(), st.cursor())
                };
                if this.raw.last_selection.get(&observed_id) != Some(&range) {
                    this.raw.last_selection.insert(observed_id.clone(), range);
                    // The caret's line is the current line.
                    let lines = this
                        .raw_sections()
                        .into_iter()
                        .find(|s| s.file_id == observed_id)
                        .map(|s| s.lines);
                    if let Some((line, _)) =
                        lines.and_then(|l| raw_view::line_and_byte(l.as_slice(), cursor))
                    {
                        let line = crate::focus::LineRef::new(observed_id.as_str(), line);
                        if this.current_line().as_ref() != Some(&line) {
                            this.set_current_line(line, super::focus::Origin::View, cx);
                        }
                    }
                    cx.notify();
                }
            });
            self.raw.editors.insert(
                section.file_id.clone(),
                ScratchEditor { state, highlights: None, _subscription: subscription, _observe: observe },
            );
            created = true;
        }
        if created {
            self.update_raw_highlights(cx);
        }
        if let Some((file_id, line)) = self.raw.pending_reveal.take() {
            if let Some(editor) = self.raw.editors.get(&file_id) {
                // Moving the cursor scrolls the line into view but also takes
                // focus; give it back to the Search box so Enter keeps jumping.
                let search_focused = gpui::Focusable::focus_handle(self.search_input.read(cx), cx).is_focused(window);
                editor.state.update(cx, |state, cx| {
                    state.set_cursor_position(Position::new(line as u32, 0), window, cx);
                });
                if search_focused {
                    self.search_input.update(cx, |state, cx| state.focus(window, cx));
                }
            }
        }
    }

    pub fn set_raw_soft_wrap(&mut self, wrap: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.raw.soft_wrap = wrap;
        for editor in self.raw.editors.values() {
            editor.state.update(cx, |state, cx| state.set_soft_wrap(wrap, window, cx));
        }
        cx.notify();
    }

    fn on_scratch_edited(&mut self, file_id: &str, text: String, cx: &mut Context<Self>) {
        self.raw.texts.insert(file_id.to_string(), text);
        self.raw.lines_cache.borrow_mut().remove(file_id);
        self.raw.edited.insert(file_id.to_string());
        self.raw.unsaved.insert(file_id.to_string());
        self.schedule_scratch_save(file_id, cx);
        // Matches and Markers follow the text.
        if !self.focus.search().query.trim().is_empty() {
            self.run_raw_search_keep_position(cx);
        } else {
            self.update_raw_highlights(cx);
        }
        cx.notify();
    }

    /// Save a scratch file after it was left alone for a moment.
    fn schedule_scratch_save(&mut self, file_id: &str, cx: &mut Context<Self>) {
        let generation = self.raw.save_gen.entry(file_id.to_string()).or_insert(0);
        *generation += 1;
        let generation = *generation;
        let file_id = file_id.to_string();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SCRATCH_SAVE_DEBOUNCE).await;
            this.update(cx, |this, cx| {
                if this.raw.save_gen.get(&file_id) == Some(&generation) {
                    this.flush_scratch_saves();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The path of a file of any Log Space.
    fn file_path_of(&self, file_id: &str) -> Option<String> {
        self.workspace_state
            .projects
            .iter()
            .flat_map(|p| p.log_spaces.iter())
            .find_map(|s| s.file(file_id))
            .and_then(|f| f.path.clone())
    }

    /// Write every edited scratch file to disk. A Marker whose line changed
    /// is dropped: Markers keep the hash of the line they were set on.
    pub fn flush_scratch_saves(&mut self) {
        let unsaved: Vec<String> = self.raw.unsaved.drain().collect();
        for file_id in unsaved {
            let Some(text) = self.raw.texts.get(&file_id).cloned() else { continue };
            let Some(path) = self.file_path_of(&file_id) else { continue };
            if let Err(e) = scratch_store::save_text(Path::new(&path), &text) {
                self.notice = Some(format!("Could not save {path}: {e}"));
                self.raw.unsaved.insert(file_id);
                continue;
            }
            let lines = split_lines(&text);
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            for project in &mut self.workspace_state.projects {
                for space in &mut project.log_spaces {
                    if let Some(file) = space.file_mut(&file_id) {
                        file.prune_markers(&refs);
                    }
                }
            }
        }
    }

    /// ⌘S: write every edited scratch file now.
    pub fn save_now(&mut self, cx: &mut Context<Self>) {
        self.flush_scratch_saves();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Parse again the scratch files edited since they were parsed.
    pub(crate) fn reparse_edited_scratch_files(&mut self) {
        let edited: Vec<String> = self.raw.edited.drain().collect();
        for file_id in edited {
            if let Some(text) = self.raw.texts.get(&file_id) {
                self.parsed_files.insert(file_id, ParsedFile::from_text(text));
            }
        }
    }

    /// Forget everything cached about a file (it was removed).
    pub(crate) fn forget_file(&mut self, file_id: &str) {
        self.parsed_files.remove(file_id);
        self.raw.editors.remove(file_id);
        self.raw.texts.remove(file_id);
        self.raw.lines_cache.borrow_mut().remove(file_id);
        self.raw.edited.remove(file_id);
        self.raw.unsaved.remove(file_id);
        self.raw.save_gen.remove(file_id);
    }

    /// ⌘C in the Raw View: copy the selected imported line.
    pub fn copy_selected_raw_line(&mut self, cx: &mut Context<Self>) {
        let Some((file_id, line)) = self.raw.selected.clone() else { return };
        let text = self
            .raw_sections()
            .into_iter()
            .find(|s| s.file_id == file_id)
            .and_then(|s| s.lines.get(line).cloned());
        if let Some(text) = text {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    pub fn select_raw_line(&mut self, file_id: &str, line: usize, cx: &mut Context<Self>) {
        self.raw.selected = Some((file_id.to_string(), line));
        cx.notify();
    }

    // ── Markers in the Raw View ─────────────────────────────────────────────

    /// Paint Markers on pieces of lines of a file: `(line, bytes, line text)`.
    pub fn mark_raw_pieces(
        &mut self,
        file_id: &str,
        pieces: &[(usize, std::ops::Range<usize>, String)],
        color: HighlightColor,
        cx: &mut Context<Self>,
    ) {
        let mut painted = false;
        for (line, range, text) in pieces {
            let c = color.clone();
            painted |= self
                .with_file_mut(file_id, |file| file.paint_marker(*line, text, range.clone(), c))
                .unwrap_or(false);
        }
        if painted {
            self.note_recent_color(color);
            self.after_raw_markers_changed(cx);
        }
        self.clear_raw_selections(cx);
    }

    /// Give the Marker under a byte of a line another color.
    pub fn recolor_raw_marker(
        &mut self,
        file_id: &str,
        line: usize,
        line_text: &str,
        byte: usize,
        color: HighlightColor,
        cx: &mut Context<Self>,
    ) {
        let c = color.clone();
        let changed = self
            .with_file_mut(file_id, |file| file.recolor_marker_at(line, line_text, byte, c))
            .unwrap_or(false);
        if changed {
            self.note_recent_color(color);
            self.after_raw_markers_changed(cx);
        }
    }

    /// Remove the whole Marker under a byte of a line.
    pub fn remove_raw_marker(&mut self, file_id: &str, line: usize, line_text: &str, byte: usize, cx: &mut Context<Self>) {
        let removed = self
            .with_file_mut(file_id, |file| file.remove_marker_at(line, line_text, byte))
            .unwrap_or(false);
        if removed {
            self.after_raw_markers_changed(cx);
        }
        self.color_bar = None;
        cx.notify();
    }

    fn after_raw_markers_changed(&mut self, cx: &mut Context<Self>) {
        self.update_raw_highlights(cx);
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Collapse the selection of every Raw View editor.
    fn clear_raw_selections(&mut self, cx: &mut Context<Self>) {
        for editor in self.raw.editors.values() {
            editor.state.update(cx, |state, cx| {
                let at = state.cursor();
                state.set_selected_range(at..at, cx);
            });
        }
    }

    /// The Raw View's bar follows the editors' selection: text selected in a
    /// file shows the bar to paint it; the caret inside a Marker shows it to
    /// change or remove that Marker. Called while rendering.
    pub fn sync_raw_color_bar(&mut self, window: &Window, cx: &mut Context<Self>) {
        use crate::app::color::{ColorBar, ColorTarget};
        let raw_open = matches!(
            self.color_bar.as_ref().map(|b| &b.target),
            Some(ColorTarget::RawMark { .. } | ColorTarget::RawRecolor { .. })
        );
        if self.view_mode != ViewMode::Raw {
            if raw_open {
                self.color_bar = None;
            }
            return;
        }
        // A bar opened for something else (a rule's palette) stays as it is.
        if self.color_bar.is_some() && !raw_open {
            return;
        }
        let pos = point(
            (window.viewport_size().width * 0.5 - px(150.0)).max(px(20.0)),
            px(110.0),
        );
        let mut wanted: Option<ColorTarget> = None;
        let sections = self.raw_sections();
        let mut focused_first: Vec<&RawSection> = sections.iter().collect();
        focused_first.sort_by_key(|s| {
            let focused = self.raw.editors.get(&s.file_id).is_some_and(|e| {
                gpui::Focusable::focus_handle(e.state.read(cx), cx).is_focused(window)
            });
            !focused
        });
        for section in focused_first {
            let Some(editor) = self.raw.editors.get(&section.file_id) else { continue };
            let (range, cursor) = {
                let state = editor.state.read(cx);
                (state.selected_range(), state.cursor())
            };
            if range.start != range.end {
                let pieces = raw_view::pieces_from_range(&section.lines, range)
                    .into_iter()
                    .map(|(line, r)| (line, r, section.lines[line - 1].clone()))
                    .collect::<Vec<_>>();
                if !pieces.is_empty() {
                    wanted = Some(ColorTarget::RawMark { file_id: section.file_id.clone(), pieces });
                    break;
                }
            } else if let Some((line, byte)) = raw_view::line_and_byte(&section.lines, cursor) {
                let under = self
                    .active_project()
                    .and_then(|p| p.log_spaces.iter().find_map(|s| s.file(&section.file_id)))
                    .is_some_and(|f| f.markers_on_line(line).iter().any(|m| m.range.start <= byte && byte < m.range.end));
                if under {
                    wanted = Some(ColorTarget::RawRecolor {
                        file_id: section.file_id.clone(),
                        line,
                        line_text: section.lines[line - 1].clone(),
                        byte,
                    });
                    break;
                }
            }
        }
        match (wanted, self.color_bar.as_ref()) {
            (Some(target), Some(bar)) if bar.target == target => {}
            (Some(target), bar) => {
                let palette_open = bar.is_some_and(|b| b.palette_open);
                self.color_bar = Some(ColorBar { pos, target, palette_open });
            }
            (None, Some(_)) if raw_open => self.color_bar = None,
            _ => {}
        }
    }

    // ── Search ──────────────────────────────────────────────────────────────

    /// Find the Matches on every line of every section, ignoring Filter.
    pub fn run_raw_search(&mut self, cx: &mut Context<Self>) {
        let replaced = self.focus.search_mut().reparse();
        self.compute_raw_matches(replaced);
        self.refresh_focus();
        self.update_raw_highlights(cx);
        if replaced {
            self.goto_raw_match(cx);
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub(crate) fn run_raw_search_keep_position(&mut self, cx: &mut Context<Self>) {
        self.focus.search_mut().reparse();
        self.compute_raw_matches(false);
        self.refresh_focus();
        self.update_raw_highlights(cx);
    }

    fn compute_raw_matches(&mut self, reset: bool) {
        let sections = self.raw_sections();
        self.raw.matches = match &self.focus.search().matcher {
            Some(matcher) => {
                let lists: Vec<&[String]> = sections.iter().map(|s| s.lines.as_slice()).collect();
                raw_view::search_sections(matcher, &lists)
                    .into_iter()
                    .map(|m| RawMatch {
                        // Keep file identity through section index.
                        section: m.section,
                        line: m.line,
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        if reset || self.raw.current >= self.raw.matches.len() {
            self.raw.current = 0;
        }
    }

    pub fn raw_jump_next(&mut self, cx: &mut Context<Self>) {
        let n = self.raw.matches.len();
        if n > 0 {
            self.raw.current = (self.raw.current + 1) % n;
            self.goto_raw_match(cx);
            self.update_raw_highlights(cx);
            cx.notify();
        }
    }

    pub fn raw_jump_prev(&mut self, cx: &mut Context<Self>) {
        let n = self.raw.matches.len();
        if n > 0 {
            self.raw.current = (self.raw.current + n - 1) % n;
            self.goto_raw_match(cx);
            self.update_raw_highlights(cx);
            cx.notify();
        }
    }

    /// Make the current Match the current line: the pane and the view agree
    /// on it and both scroll to it.
    fn goto_raw_match(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.raw.matches.get(self.raw.current).copied() else { return };
        let sections = self.raw_sections();
        let Some(section) = sections.get(m.section) else { return };
        let line = crate::focus::LineRef::new(section.file_id.as_str(), m.line + 1);
        self.set_current_line(line, super::focus::Origin::Jump, cx);
    }

    /// Scroll to a line of a section, opening the section when collapsed.
    pub(crate) fn reveal_raw_line(&mut self, section_ix: usize, line: usize, cx: &mut Context<Self>) {
        let sections = self.raw_sections();
        let Some(section) = sections.get(section_ix) else { return };
        if self.is_section_collapsed(&section.file_id) {
            self.set_section_collapsed(&section.file_id, false, cx);
        }
        let single = sections.len() == 1;
        // Stacked editors: scroll the column to the line.
        let mut offset = 0.0;
        for (i, s) in sections.iter().enumerate() {
            if i == section_ix {
                let header = if single { 0.0 } else { RAW_ROW_HEIGHT };
                offset += header + EDITOR_PADDING / 2.0 + line as f32 * EDITOR_LINE_HEIGHT;
                break;
            }
            offset += section_height(s, self.is_section_collapsed(&s.file_id));
        }
        let target = (offset - 80.0).max(0.0);
        self.raw.section_scroll.set_offset(point(px(0.0), px(-target)));
        self.raw.pending_reveal = Some((section.file_id.clone(), line));
    }

    /// Highlight every Match in the scratch editors; the current one stronger.
    pub(crate) fn update_raw_highlights(&mut self, cx: &mut Context<Self>) {
        let sections = self.raw_sections();
        let matcher = self.focus.search().matcher.clone();
        let current = self.raw.matches.get(self.raw.current).copied();
        let all_bg = hsla(0.55, 0.85, 0.55, 0.35);
        let current_bg = hsla(0.55, 0.9, 0.55, 0.7);
        let tone = if cx.theme().is_dark() { Tone::Dark } else { Tone::Light };
        // Markers of each file, leaving out those of a line edited since.
        let stored: HashMap<String, Vec<(usize, std::ops::Range<usize>, HighlightColor)>> = sections
            .iter()
            .filter_map(|section| {
                let file = self
                    .active_project()?
                    .log_spaces
                    .iter()
                    .find_map(|s| s.file(&section.file_id))?;
                let kept = file
                    .markers
                    .iter()
                    .filter(|m| {
                        section.lines.get(m.line - 1).is_some_and(|text| {
                            m.line_hash.is_none_or(|h| h == line_hash(text))
                        })
                    })
                    .map(|m| (m.line, m.range.clone(), m.color.clone()))
                    .collect();
                Some((section.file_id.clone(), kept))
            })
            .collect();
        for (si, section) in sections.iter().enumerate() {
            let Some(editor) = self.raw.editors.get_mut(&section.file_id) else { continue };
            let mut decorations = Vec::new();
            if let Some(matcher) = &matcher {
                let mut offset = 0usize;
                for (li, line) in section.lines.iter().enumerate() {
                    let is_current = current.is_some_and(|m| m.section == si && m.line == li);
                    // Only a line that satisfies the whole expression is a Match;
                    // `a && b` must not paint `a` on a line without `b`.
                    let ranges = if matcher.is_match(line) { matcher.find_ranges(line) } else { Vec::new() };
                    for (s, e) in ranges {
                        decorations.push(TextDecoration::new(
                            offset + s..offset + e,
                            HighlightStyle {
                                background_color: Some(if is_current { current_bg } else { all_bg }),
                                ..Default::default()
                            },
                        ));
                    }
                    offset += line.len() + 1;
                }
            }
            // Markers paint over the Matches.
            if let Some(markers) = stored.get(&section.file_id) {
                let starts = raw_view::line_starts(section.lines.as_slice());
                for (line, range, color) in markers {
                    let Some(&start) = starts.get(line - 1) else { continue };
                    decorations.push(TextDecoration::new(
                        start + range.start..start + range.end,
                        HighlightStyle {
                            background_color: Some(gpui::rgb(color.rgb(tone)).into()),
                            color: Some(gpui::rgb(text_rgb(color.text_tone(tone))).into()),
                            ..Default::default()
                        },
                    ));
                }
            }
            match &editor.highlights {
                Some(collection) => collection.set(decorations, cx),
                None => {
                    let collection = editor
                        .state
                        .update(cx, |state, cx| state.create_decorations_collection(decorations, cx));
                    editor.highlights = Some(collection);
                }
            }
        }
    }
}

/// Height of a File Section in the stacked scratch editors.
pub fn section_height(section: &RawSection, collapsed: bool) -> f32 {
    if collapsed {
        RAW_ROW_HEIGHT
    } else {
        RAW_ROW_HEIGHT + section_editor_height(section.lines.len())
    }
}

/// Tallest a stacked section's editor gets before it scrolls inside itself.
pub const MAX_SECTION_EDITOR_HEIGHT: f32 = 480.0;

/// Height of a section's editor among several stacked ones.
pub fn section_editor_height(lines: usize) -> f32 {
    editor_height(lines).min(MAX_SECTION_EDITOR_HEIGHT)
}

/// Height of a scratch editor showing `lines` lines (at least a few).
pub fn editor_height(lines: usize) -> f32 {
    lines.max(4) as f32 * EDITOR_LINE_HEIGHT + EDITOR_PADDING
}
