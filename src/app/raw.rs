//! Logic: Raw View. Each file of the open Log Space is a collapsible File
//! Section showing its text line by line. Imported files are read-only; a
//! scratch file is edited in an editor and saved by itself after a pause.
//!
//! Raw View ignores Filter, so Search here looks at every line of every
//! section, only highlights, and never hides anything.

use super::AppView;
use crate::marker::MarkerType;
use crate::raw_view::{self, RawMatch};
use crate::space_view::{ParsedFile, split_lines};
use crate::workspace::log_space::{LogSpaceKind, StoredMarker, line_hash};
use crate::workspace::scratch_store;
use crate::workspace::working_set::{TabTarget, ViewMode};
use gpui::{
    AppContext as _, Context, HighlightStyle, ScrollHandle, Subscription,
    UniformListScrollHandle, Window, hsla, point, px,
};
use gpui_component::input::{EditorState, InputEvent, TextDecoration, TextDecorationCollection};
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
    /// The imported line selected for copying: `(file_id, 0-based line)`.
    pub selected: Option<(String, usize)>,
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
            selected: None,
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
                    Some(text) if scratch => Arc::new(split_lines(text)),
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
            state.update(cx, |state, cx| state.set_value(text.clone(), window, cx));
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
            if section.editable && text.is_empty() && self.search_state.query.is_empty() {
                state.update(cx, |state, cx| state.focus(window, cx));
            }
            if section.editable {
                self.raw.texts.entry(section.file_id.clone()).or_insert(text);
            }
            self.raw.editors.insert(
                section.file_id.clone(),
                ScratchEditor { state, highlights: None, _subscription: subscription },
            );
        }
    }

    fn on_scratch_edited(&mut self, file_id: &str, text: String, cx: &mut Context<Self>) {
        self.raw.texts.insert(file_id.to_string(), text);
        self.raw.edited.insert(file_id.to_string());
        self.raw.unsaved.insert(file_id.to_string());
        self.schedule_scratch_save(file_id, cx);
        // Matches and highlights follow the text.
        if !self.search_state.query.trim().is_empty() {
            self.run_raw_search_keep_position(cx);
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

    /// The marker symbol of a line, if the user set one.
    pub fn raw_marker_symbol(&self, file_id: &str, line_no: usize) -> Option<&'static str> {
        let file = self
            .workspace_state
            .projects
            .iter()
            .flat_map(|p| p.log_spaces.iter())
            .find_map(|s| s.file(file_id))?;
        let stored = file.markers.iter().find(|m| m.line == line_no)?;
        stored
            .marker
            .or(stored.note.as_ref().map(|_| MarkerType::Note))
            .map(|m| m.symbol())
    }

    /// Set or remove a bookmark on a line (1-based) of a file.
    pub fn toggle_raw_marker(
        &mut self,
        file_id: &str,
        line_no: usize,
        line_text: &str,
        cx: &mut Context<Self>,
    ) {
        for project in &mut self.workspace_state.projects {
            for space in &mut project.log_spaces {
                let Some(file) = space.file_mut(file_id) else { continue };
                if let Some(pos) = file.markers.iter().position(|m| m.line == line_no) {
                    file.markers.remove(pos);
                } else {
                    file.markers.push(StoredMarker {
                        line: line_no,
                        marker: Some(MarkerType::Bookmark),
                        note: None,
                        content_hash: Some(line_hash(line_text)),
                    });
                }
            }
        }
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// ⌘D: set or remove a bookmark on the line of the focused scratch editor.
    pub fn toggle_marker_at_cursor(&mut self, window: &Window, cx: &mut Context<Self>) {
        let focused = self
            .raw
            .editors
            .iter()
            .find(|(_, e)| gpui::Focusable::focus_handle(e.state.read(cx), cx).is_focused(window))
            .map(|(id, e)| (id.clone(), e.state.read(cx).cursor_position().line as usize));
        let Some((file_id, line)) = focused else { return };
        let text = self
            .raw
            .texts
            .get(&file_id)
            .and_then(|t| t.lines().nth(line).map(str::to_string))
            .unwrap_or_default();
        self.toggle_raw_marker(&file_id, line + 1, &text, cx);
    }

    // ── Search ──────────────────────────────────────────────────────────────

    /// Find the Matches on every line of every section, ignoring Filter.
    pub fn run_raw_search(&mut self, cx: &mut Context<Self>) {
        let replaced = self.search_state.reparse();
        self.compute_raw_matches(replaced);
        self.update_raw_highlights(cx);
        if replaced {
            self.reveal_raw_match(cx);
        }
        cx.notify();
    }

    fn run_raw_search_keep_position(&mut self, cx: &mut Context<Self>) {
        self.search_state.reparse();
        self.compute_raw_matches(false);
        self.update_raw_highlights(cx);
    }

    fn compute_raw_matches(&mut self, reset: bool) {
        let sections = self.raw_sections();
        self.raw.matches = match &self.search_state.matcher {
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
            self.reveal_raw_match(cx);
            self.update_raw_highlights(cx);
            cx.notify();
        }
    }

    pub fn raw_jump_prev(&mut self, cx: &mut Context<Self>) {
        let n = self.raw.matches.len();
        if n > 0 {
            self.raw.current = (self.raw.current + n - 1) % n;
            self.reveal_raw_match(cx);
            self.update_raw_highlights(cx);
            cx.notify();
        }
    }

    /// Scroll to the current Match, opening its section when collapsed.
    fn reveal_raw_match(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.raw.matches.get(self.raw.current).copied() else { return };
        let sections = self.raw_sections();
        let Some(section) = sections.get(m.section) else { return };
        if self.is_section_collapsed(&section.file_id) {
            self.set_section_collapsed(&section.file_id, false, cx);
        }
        let single = sections.len() == 1;
        // Stacked editors: scroll the column to the matching line.
        let mut offset = 0.0;
        for (i, s) in sections.iter().enumerate() {
            if i == m.section {
                let header = if single { 0.0 } else { RAW_ROW_HEIGHT };
                offset += header + EDITOR_PADDING / 2.0 + m.line as f32 * EDITOR_LINE_HEIGHT;
                break;
            }
            offset += section_height(s, self.is_section_collapsed(&s.file_id));
        }
        let target = (offset - 80.0).max(0.0);
        self.raw.section_scroll.set_offset(point(px(0.0), px(-target)));
    }

    /// Highlight every Match in the scratch editors; the current one stronger.
    fn update_raw_highlights(&mut self, cx: &mut Context<Self>) {
        let sections = self.raw_sections();
        let matcher = self.search_state.matcher.clone();
        let current = self.raw.matches.get(self.raw.current).copied();
        let all_bg = hsla(0.12, 0.9, 0.55, 0.35);
        let current_bg = hsla(0.12, 0.95, 0.55, 0.7);
        for (si, section) in sections.iter().enumerate() {
            let Some(editor) = self.raw.editors.get_mut(&section.file_id) else { continue };
            let mut decorations = Vec::new();
            if let Some(matcher) = &matcher {
                let mut offset = 0usize;
                for (li, line) in section.lines.iter().enumerate() {
                    let is_current = current.is_some_and(|m| m.section == si && m.line == li);
                    for (s, e) in matcher.find_ranges(line) {
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
