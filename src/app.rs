use crate::filter::FilterEngine;
use crate::marker::MarkerType;
use crate::model::{LogEntry, LogLevel};
use crate::parser::parse_file;
use crate::sample::generate_sample_logs;
use crate::search::SearchState;
use crate::theme::{Density, ZED};
use crate::ui::canvas::{CanvasViewState, LogCanvas};
use crate::ui::command_palette::{CommandPalette, PaletteAction};
use crate::ui::context_view::ContextModal;
use crate::ui::filter_popover::{FilterPopover, QuickLogMenu};
use crate::ui::inspector::{InspectorTab, InspectorView};
use crate::ui::search_overlay::SearchOverlay;
use gpui::{
    div, px, App, AppContext, Context, ElementId, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, ScrollStrategy,
    StatefulInteractiveElement, Styled, UniformListScrollHandle, Window,
    prelude::FluentBuilder as _,
};
use gpui_component::{
    input::{InputEvent, InputState},
    scroll::ScrollableElement as _,
    Icon, IconName,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarState {
    Hidden,
    Compact,  // 48px rail
    Expanded, // 220px drawer
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveRailTab {
    Files,
    Filters,
    Markers,
    Modules,
    Settings,
}

pub struct AppView {
    pub all_entries: Vec<LogEntry>,
    pub visible_indices: Arc<Vec<usize>>,
    pub filter_engine: FilterEngine,
    pub search_state: SearchState,
    pub search_input: Entity<InputState>,
    pub scroll_handle: UniformListScrollHandle,

    pub selected_row_pos: Option<usize>,
    pub inspector_tab: InspectorTab,
    pub is_inspector_open: bool,
    pub is_filter_builder_open: bool,
    pub is_context_open: bool,
    pub is_palette_open: bool,

    pub active_quick_menu: Option<QuickLogMenu>,

    pub file_name: String,
    pub file_format: String,
    pub sidebar_state: SidebarState,
    pub active_rail_tab: ActiveRailTab,
    pub density: Density,
    pub column_layout: crate::column::ColumnLayoutState,
    pub tail_enabled: bool,
    pub focused_tag: Option<String>,

    pub focus_handle: FocusHandle,
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let scroll_handle = UniformListScrollHandle::new();
        let mut all_entries = generate_sample_logs();
        let mut column_layout = crate::column::ColumnLayoutState::new();
        column_layout.update_from_entries(&all_entries, Density::Compact);

        let filter_engine = FilterEngine::new();
        let visible = filter_engine.evaluate(&mut all_entries);
        let visible_indices = Arc::new(visible);

        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Find in log (Aa, .*, ↔)...")
        });

        let mut search_state = SearchState::new();
        search_state.update_matches(&all_entries, &visible_indices);

        cx.subscribe(
            &search_input,
            |this: &mut Self, input, _event: &InputEvent, cx| {
                let text = input.read(cx).text().to_string();
                this.search_state.query = text;
                this.run_search(cx);
            },
        )
        .detach();

        Self {
            all_entries,
            visible_indices,
            filter_engine,
            search_state,
            search_input,
            scroll_handle,
            selected_row_pos: Some(0),
            inspector_tab: InspectorTab::Fields,
            is_inspector_open: false,
            is_filter_builder_open: false,
            is_context_open: false,
            is_palette_open: false,
            active_quick_menu: None,
            file_name: "app.log".to_string(),
            file_format: "Standard Server Log".to_string(),
            sidebar_state: SidebarState::Compact,
            active_rail_tab: ActiveRailTab::Files,
            density: Density::Compact,
            column_layout,
            tail_enabled: false,
            focused_tag: None,
            focus_handle,
        }
    }

    pub fn apply_filters_and_refresh(&mut self, cx: &mut Context<Self>) {
        let visible = self.filter_engine.evaluate(&mut self.all_entries);
        self.visible_indices = Arc::new(visible);
        self.search_state
            .update_matches(&self.all_entries, &self.visible_indices);

        if let Some(pos) = self.selected_row_pos {
            if pos >= self.visible_indices.len() {
                self.selected_row_pos = if self.visible_indices.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
        cx.notify();
    }

    pub fn remove_filter_condition(&mut self, id: usize, cx: &mut Context<Self>) {
        self.filter_engine.remove_condition(id);
        self.apply_filters_and_refresh(cx);
    }

    pub fn toggle_filter_builder(&mut self, cx: &mut Context<Self>) {
        self.is_filter_builder_open = !self.is_filter_builder_open;
        if self.is_filter_builder_open {
            self.search_state.is_open = false;
        }
        cx.notify();
    }

    pub fn toggle_search(&mut self, cx: &mut Context<Self>) {
        self.search_state.is_open = !self.search_state.is_open;
        if self.search_state.is_open {
            self.is_filter_builder_open = false;
        }
        cx.notify();
    }

    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        self.search_state
            .update_matches(&self.all_entries, &self.visible_indices);
        if let Some(target_vis_pos) = self
            .search_state
            .matched_visible_indices
            .get(self.search_state.current_match_idx)
            .copied()
        {
            self.scroll_handle
                .scroll_to_item(target_vis_pos, ScrollStrategy::Center);
            self.selected_row_pos = Some(target_vis_pos);
        }
        cx.notify();
    }

    pub fn jump_next_match(&mut self, cx: &mut Context<Self>) {
        if let Some(vis_pos) = self.search_state.next_match() {
            self.scroll_handle
                .scroll_to_item(vis_pos, ScrollStrategy::Center);
            self.selected_row_pos = Some(vis_pos);
            cx.notify();
        }
    }

    pub fn jump_prev_match(&mut self, cx: &mut Context<Self>) {
        if let Some(vis_pos) = self.search_state.prev_match() {
            self.scroll_handle
                .scroll_to_item(vis_pos, ScrollStrategy::Center);
            self.selected_row_pos = Some(vis_pos);
            cx.notify();
        }
    }

    pub fn jump_to_row(&mut self, vis_pos: usize, cx: &mut Context<Self>) {
        if vis_pos < self.visible_indices.len() {
            self.selected_row_pos = Some(vis_pos);
            self.scroll_handle
                .scroll_to_item(vis_pos, ScrollStrategy::Center);
            cx.notify();
        }
    }

    pub fn trigger_search_query(&mut self, q: &str, cx: &mut Context<Self>) {
        self.search_state.query = q.to_string();
        self.search_state.is_open = true;
        self.run_search(cx);
    }

    pub fn selected_entry(&self) -> Option<&LogEntry> {
        self.selected_row_pos
            .and_then(|pos| self.visible_indices.get(pos))
            .and_then(|&ix| self.all_entries.get(ix))
    }

    pub fn selected_entry_mut(&mut self) -> Option<&mut LogEntry> {
        if let Some(pos) = self.selected_row_pos {
            if let Some(&entry_ix) = self.visible_indices.get(pos) {
                return self.all_entries.get_mut(entry_ix);
            }
        }
        None
    }

    pub fn cycle_selected_marker(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            let next_m = MarkerType::cycle(entry.marker);
            entry.marker = next_m;
            cx.notify();
        }
    }

    pub fn set_selected_marker(&mut self, marker: Option<MarkerType>, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            entry.marker = marker;
            cx.notify();
        }
    }

    pub fn set_selected_note(&mut self, note: Option<String>, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            if note.is_some() && entry.marker.is_none() {
                entry.marker = Some(MarkerType::Note);
            }
            entry.note = note;
            cx.notify();
        }
    }

    pub fn add_quick_filter_level(&mut self, level: LogLevel, cx: &mut Context<Self>) {
        self.filter_engine.add_condition(
            crate::filter::ConditionField::Level,
            crate::filter::ConditionOp::Is,
            level.as_str().to_string(),
        );
        self.apply_filters_and_refresh(cx);
    }

    pub fn add_quick_filter_module(&mut self, module: &str, cx: &mut Context<Self>) {
        self.filter_engine.add_condition(
            crate::filter::ConditionField::Module,
            crate::filter::ConditionOp::Is,
            module.to_string(),
        );
        self.apply_filters_and_refresh(cx);
    }

    pub fn toggle_row_expansion(&mut self, entry_ix: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.all_entries.get_mut(entry_ix) {
            entry.is_expanded = !entry.is_expanded;
            cx.notify();
        }
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_state = match self.sidebar_state {
            SidebarState::Expanded => SidebarState::Compact,
            SidebarState::Compact => SidebarState::Hidden,
            SidebarState::Hidden => SidebarState::Expanded,
        };
        cx.notify();
    }

    pub fn toggle_bookmark_current_line(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.selected_entry_mut() {
            entry.toggle_bookmark();
            cx.notify();
        }
    }

    pub fn jump_to_next_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        for offset in 1..total {
            let next_pos = (current_pos + offset) % total;
            if let Some(&entry_ix) = self.visible_indices.get(next_pos) {
                if let Some(entry) = self.all_entries.get(entry_ix) {
                    if matches!(entry.level, LogLevel::Fatal | LogLevel::Error) {
                        self.jump_to_row(next_pos, cx);
                        break;
                    }
                }
            }
        }
    }

    pub fn jump_to_prev_error(&mut self, cx: &mut Context<Self>) {
        let current_pos = self.selected_row_pos.unwrap_or(0);
        let total = self.visible_indices.len();
        for offset in 1..total {
            let prev_pos = (current_pos + total - offset) % total;
            if let Some(&entry_ix) = self.visible_indices.get(prev_pos) {
                if let Some(entry) = self.all_entries.get(entry_ix) {
                    if matches!(entry.level, LogLevel::Fatal | LogLevel::Error) {
                        self.jump_to_row(prev_pos, cx);
                        break;
                    }
                }
            }
        }
    }

    pub fn execute_palette_action(&mut self, action: PaletteAction, cx: &mut Context<Self>) {
        self.is_palette_open = false;
        match action {
            PaletteAction::ToggleSearch => self.toggle_search(cx),
            PaletteAction::ToggleFilter => self.toggle_filter_builder(cx),
            PaletteAction::ToggleBookmark => self.toggle_bookmark_current_line(cx),
            PaletteAction::ShowContext => {
                self.is_context_open = true;
                cx.notify();
            }
            PaletteAction::GoNextError => self.jump_to_next_error(cx),
            PaletteAction::GoPrevError => self.jump_to_prev_error(cx),
            PaletteAction::SetComfortable => {
                self.density = Density::Comfortable;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            PaletteAction::SetCompact => {
                self.density = Density::Compact;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            PaletteAction::SetUltraCompact => {
                self.density = Density::UltraCompact;
                self.column_layout.update_density(self.density);
                cx.notify();
            }
            PaletteAction::ToggleTail => {
                self.tail_enabled = !self.tail_enabled;
                cx.notify();
            }
            PaletteAction::ToggleInspector => {
                self.is_inspector_open = !self.is_inspector_open;
                cx.notify();
            }
            PaletteAction::LoadDemo => {
                self.all_entries = generate_sample_logs();
                self.column_layout.update_from_entries(&self.all_entries, self.density);
                self.file_name = "app.log".to_string();
                self.file_format = "Standard Server Log".to_string();
                self.apply_filters_and_refresh(cx);
            }
            PaletteAction::OpenFile => {
                self.open_file_dialog(cx);
            }
        }
    }

    pub fn open_file_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(file_path) = rfd::FileDialog::new()
            .add_filter("Log Files (*.log, *.txt, *.json)", &["log", "txt", "json", "jsonl"])
            .pick_file()
        {
            let name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown File".to_string());

            match parse_file(&file_path) {
                Ok(result) => {
                    self.all_entries = result.entries;
                    self.column_layout.max_chars = result.max_chars;
                    self.column_layout.update_density(self.density);
                    self.file_name = name;
                    self.file_format = "Text Log".to_string();
                    self.apply_filters_and_refresh(cx);
                }
                Err(e) => {
                    eprintln!("Failed to parse file: {:?}", e);
                }
            }
        }
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        let shift = event.keystroke.modifiers.shift;
        let alt = event.keystroke.modifiers.alt;

        match (cmd, shift, alt, key) {
            (true, false, false, "f") => {
                self.toggle_search(cx);
            }
            (true, true, false, "F") => {
                self.toggle_filter_builder(cx);
            }
            (true, false, false, "k") => {
                self.is_palette_open = !self.is_palette_open;
                cx.notify();
            }
            (true, false, false, "i") => {
                self.is_inspector_open = !self.is_inspector_open;
                cx.notify();
            }
            (true, false, false, "g") => {
                self.jump_next_match(cx);
            }
            (true, true, false, "G") => {
                self.jump_prev_match(cx);
            }
            (true, false, false, "e") => {
                self.jump_to_next_error(cx);
            }
            (true, true, false, "E") => {
                self.jump_to_prev_error(cx);
            }
            (true, false, false, "b") => {
                self.toggle_sidebar(cx);
            }
            (false, false, false, "b") => {
                self.toggle_bookmark_current_line(cx);
            }
            (false, false, false, "m") | (false, false, false, "M") => {
                self.cycle_selected_marker(cx);
            }
            (false, false, false, "f") => {
                // Focus mode on selected tag
                if let Some(entry) = self.selected_entry() {
                    let tag = entry.tag.clone();
                    if self.focused_tag.as_deref() == Some(&tag) {
                        self.focused_tag = None;
                    } else {
                        self.focused_tag = Some(tag);
                    }
                    cx.notify();
                }
            }
            (false, false, false, " ")
            | (false, false, false, "Enter")
            | (false, false, false, "enter") => {
                self.is_context_open = !self.is_context_open;
                cx.notify();
            }
            (false, false, false, "Escape") => {
                self.search_state.is_open = false;
                self.is_filter_builder_open = false;
                self.is_context_open = false;
                self.is_palette_open = false;
                self.active_quick_menu = None;
                cx.notify();
            }
            (false, false, false, "ArrowDown") => {
                if let Some(pos) = self.selected_row_pos {
                    if pos + 1 < self.visible_indices.len() {
                        self.jump_to_row(pos + 1, cx);
                    }
                }
            }
            (false, false, false, "ArrowUp") => {
                if let Some(pos) = self.selected_row_pos {
                    if pos > 0 {
                        self.jump_to_row(pos - 1, cx);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entries_arc = Arc::new(self.all_entries.clone());
        let visible_arc = self.visible_indices.clone();

        // 1. Top Window Chrome (H: 36px, Integrated Minimalist Header)
        let window_chrome = div()
            .h(px(36.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(ZED.surface_background)
            .border_b_1()
            .border_color(ZED.border_variant)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ZED.text)
                            .child("Zed Logs"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(ZED.element_background)
                            .border_1()
                            .border_color(ZED.border_variant)
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(Icon::new(IconName::FileText).size(px(12.0)))
                            .child(self.file_name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .id("btn_cmd_pal")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(
                                div()
                                    .px_1()
                                    .py_0p5()
                                    .rounded_xs()
                                    .bg(ZED.element_background)
                                    .text_color(ZED.text_muted)
                                    .child("⌘K"),
                            )
                            .child("Palette")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.is_palette_open = !this.is_palette_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("btn_open_file")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(Icon::new(IconName::FolderOpen).size(px(12.0)))
                            .child("Open...")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.open_file_dialog(cx);
                            })),
                    ),
            );

        // 2. Editor Toolbar (H: 34px, Search trigger, Filter Chips, View Controls)
        let mut filter_chips = div().flex().flex_row().items_center().gap_1p5();

        for cond in &self.filter_engine.conditions {
            if !cond.enabled || cond.value.trim().is_empty() {
                continue;
            }
            let cond_id = cond.id;
            let label = cond.to_chip_label();

            let chip = div()
                .id(ElementId::from(format!("chip_{}", cond_id)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(ZED.element_background)
                .border_1()
                .border_color(ZED.border_variant)
                .text_xs()
                .text_color(ZED.text_accent)
                .child(label)
                .child(
                    div()
                        .text_xs()
                        .text_color(ZED.text_muted)
                        .hover(|s| s.text_color(ZED.error))
                        .child("×"),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.remove_filter_condition(cond_id, cx);
                }));

            filter_chips = filter_chips.child(chip);
        }

        let editor_toolbar = div()
            .h(px(34.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(ZED.surface_background)
            .border_b_1()
            .border_color(ZED.border_variant)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    // Search trigger button
                    .child(
                        div()
                            .id("toolbar_search_btn")
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(ZED.element_background)
                            .border_1()
                            .border_color(if self.search_state.is_open {
                                ZED.border_focused
                            } else {
                                ZED.border_variant
                            })
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .text_color(ZED.icon)
                                    .child(Icon::new(IconName::Search).size(px(13.0)))
                                    .when(self.search_state.query.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .text_color(ZED.text_placeholder)
                                                .child("Search in logs..."),
                                        )
                                    }),
                            )
                            .child(if !self.search_state.query.is_empty() {
                                let (c, t) = self.search_state.current_match_display();
                                div()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(ZED.text)
                                    .child(format!("{}  {}/{}", self.search_state.query, c, t))
                                    .into_any_element()
                            } else {
                                div()
                                    .px_1()
                                    .py_0p5()
                                    .rounded_xs()
                                    .bg(ZED.surface_background)
                                    .text_color(ZED.text_muted)
                                    .child("⌘F")
                                    .into_any_element()
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_search(cx);
                            })),
                    )
                    // Active Filter Chips
                    .child(filter_chips)
                    // + Filter button
                    .child(
                        div()
                            .id("toolbar_filter_btn")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(if self.is_filter_builder_open {
                                ZED.ghost_selected
                            } else {
                                ZED.transparent
                            })
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(if self.is_filter_builder_open {
                                ZED.text_accent
                            } else {
                                ZED.text_muted
                            })
                            .child("+ Filter")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_filter_builder(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    // Density toggle
                    .child(
                        div()
                            .id("btn_toggle_density")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(ZED.text_muted)
                            .child(format!("Density: {}", self.density.as_str()))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.density = match this.density {
                                    Density::Comfortable => Density::Compact,
                                    Density::Compact => Density::UltraCompact,
                                    Density::UltraCompact => Density::Comfortable,
                                };
                                this.column_layout.update_density(this.density);
                                cx.notify();
                            })),
                    )
                    // Inspector toggle
                    .child(
                        div()
                            .id("btn_toggle_inspector")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(if self.is_inspector_open {
                                ZED.ghost_selected
                            } else {
                                ZED.transparent
                            })
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .text_color(if self.is_inspector_open {
                                ZED.text_accent
                            } else {
                                ZED.text_muted
                            })
                            .child("Inspector ⌘I")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.is_inspector_open = !this.is_inspector_open;
                                cx.notify();
                            })),
                    ),
            );

        // 3. Activity Rail (W: 40px, Minimalist Icon Bar)
        let rail = if self.sidebar_state != SidebarState::Hidden {
            div()
                .w(px(40.0))
                .h_full()
                .bg(ZED.surface_background)
                .border_r_1()
                .border_color(ZED.border_variant)
                .flex()
                .flex_col()
                .items_center()
                .py_2()
                .gap_2()
                .child(rail_icon(
                    "rail_files",
                    IconName::Folder,
                    self.active_rail_tab == ActiveRailTab::Files,
                    cx.listener(|this, _, _window, cx| {
                        if this.active_rail_tab == ActiveRailTab::Files
                            && this.sidebar_state == SidebarState::Expanded
                        {
                            this.sidebar_state = SidebarState::Compact;
                        } else {
                            this.active_rail_tab = ActiveRailTab::Files;
                            this.sidebar_state = SidebarState::Expanded;
                        }
                        cx.notify();
                    }),
                ))
                .child(rail_icon(
                    "rail_markers",
                    IconName::BookOpen,
                    self.active_rail_tab == ActiveRailTab::Markers,
                    cx.listener(|this, _, _window, cx| {
                        if this.active_rail_tab == ActiveRailTab::Markers
                            && this.sidebar_state == SidebarState::Expanded
                        {
                            this.sidebar_state = SidebarState::Compact;
                        } else {
                            this.active_rail_tab = ActiveRailTab::Markers;
                            this.sidebar_state = SidebarState::Expanded;
                        }
                        cx.notify();
                    }),
                ))
                .child(rail_icon(
                    "rail_modules",
                    IconName::SquareTerminal,
                    self.active_rail_tab == ActiveRailTab::Modules,
                    cx.listener(|this, _, _window, cx| {
                        if this.active_rail_tab == ActiveRailTab::Modules
                            && this.sidebar_state == SidebarState::Expanded
                        {
                            this.sidebar_state = SidebarState::Compact;
                        } else {
                            this.active_rail_tab = ActiveRailTab::Modules;
                            this.sidebar_state = SidebarState::Expanded;
                        }
                        cx.notify();
                    }),
                ))
                .into_any_element()
        } else {
            div().into_any_element()
        };

        // 4. Collapsible Sidebar Drawer (W: 240px when expanded)
        let sidebar_content = if self.sidebar_state == SidebarState::Expanded {
            let mut list = div().flex().flex_col().gap_1().w_full();

            match self.active_rail_tab {
                ActiveRailTab::Markers => {
                    list = list.child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ZED.text_muted)
                            .mb_2()
                            .child("BOOKMARKS & MARKERS"),
                    );

                    for (vis_pos, &entry_ix) in self.visible_indices.iter().enumerate() {
                        if let Some(entry) = self.all_entries.get(entry_ix) {
                            if let Some(marker) = entry.effective_marker() {
                                let marker_sym = marker.symbol();
                                let marker_col = marker.color();
                                let target_pos = vis_pos;

                                let item = div()
                                    .id(ElementId::from(format!("nav_mark_{}", entry_ix)))
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .hover(|s| s.bg(ZED.ghost_hover))
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(marker_col)
                                            .child(marker_sym),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ZED.text_muted)
                                            .child(format!("#{}", entry.index)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_xs()
                                            .text_color(ZED.text)
                                            .truncate()
                                            .child(entry.payload.clone()),
                                    )
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.jump_to_row(target_pos, cx);
                                    }));

                                list = list.child(item);
                            }
                        }
                    }
                }
                ActiveRailTab::Modules => {
                    list = list.child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ZED.text_muted)
                            .mb_2()
                            .child("MODULES"),
                    );

                    let modules = ["API", "DB", "AUTH", "PAYMENT", "CACHE", "WORKER", "GATEWAY"];
                    for mod_name in modules {
                        let m_str = mod_name.to_string();
                        let is_focused = self.focused_tag.as_deref() == Some(mod_name);

                        let item = div()
                            .id(ElementId::from(format!("mod_nav_{}", mod_name)))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(if is_focused {
                                ZED.element_selected
                            } else {
                                ZED.transparent
                            })
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(if is_focused {
                                        ZED.text_accent
                                    } else {
                                        ZED.text
                                    })
                                    .child(mod_name),
                            )
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                if this.focused_tag.as_deref() == Some(&m_str) {
                                    this.focused_tag = None;
                                } else {
                                    this.focused_tag = Some(m_str.clone());
                                }
                                cx.notify();
                            }));

                        list = list.child(item);
                    }
                }
                _ => {
                    list = list.child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ZED.text_muted)
                            .mb_2()
                            .child("WORKSPACE LOGS"),
                    );

                    let files = ["app.log", "api.log", "error.log"];
                    for f in files {
                        let is_curr = self.file_name == f;
                        let item = div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_xs()
                            .font_weight(if is_curr {
                                gpui::FontWeight::SEMIBOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .text_color(if is_curr {
                                ZED.text_accent
                            } else {
                                ZED.text_muted
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(Icon::new(IconName::FileText).size(px(13.0)))
                                    .child(f.to_string()),
                            );
                        list = list.child(item);
                    }
                }
            }

            div()
                .w(px(240.0))
                .h_full()
                .bg(ZED.surface_background)
                .border_r_1()
                .border_color(ZED.border_variant)
                .p_3()
                .overflow_y_scrollbar()
                .child(list)
                .into_any_element()
        } else {
            div().into_any_element()
        };

        // 5. Center Log Canvas (Editor Canvas)
        let selected_entry = self.selected_entry();

        let log_canvas = LogCanvas::render(
            CanvasViewState {
                view: cx.entity().clone(),
                entries: entries_arc.clone(),
                visible_indices: visible_arc.clone(),
                selected_row_pos: self.selected_row_pos,
                density: self.density,
                column_layout: &self.column_layout,
                focused_tag: self.focused_tag.as_deref(),
                search_state: &self.search_state,
                scroll_handle: &self.scroll_handle,
            },
        );

        // 6. Right Inspector View (Slide-out)
        let inspector_view = InspectorView::render(
            selected_entry,
            self.inspector_tab,
            self.is_inspector_open,
            cx,
        );

        // 7. Status Bar (H: 24px, Minimalist Footer)
        let total_count = self.all_entries.len();
        let visible_count = self.visible_indices.len();
        let filter_pct = if total_count > 0 {
            100.0 - (visible_count as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };

        let current_line_num = selected_entry.map(|e| e.index).unwrap_or(0);

        let status_bar = div()
            .h(px(24.0))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(ZED.surface_background)
            .border_t_1()
            .border_color(ZED.border_variant)
            .text_xs()
            .font_family(".AppleSystemUIFontMonospaced")
            .text_color(ZED.text_muted)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(div().child(format!("Ln {}", current_line_num)))
                    .child(div().text_color(ZED.border_variant).child("·"))
                    .child(div().child(format!("{},{:03} events", total_count / 1000, total_count % 1000)))
                    .child(div().text_color(ZED.border_variant).child("·"))
                    .child(if filter_pct > 0.0 {
                        div()
                            .text_color(ZED.text_accent)
                            .child(format!("Filtered {:.1}%", filter_pct))
                            .into_any_element()
                    } else {
                        div().child("All Visible").into_any_element()
                    })
                    .child(div().text_color(ZED.border_variant).child("·"))
                    .child(div().child("UTF-8"))
                    .child(if let Some(ref t) = self.focused_tag {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2p5()
                            .child(div().text_color(ZED.border_variant).child("·"))
                            .child(div().text_color(ZED.text_accent).child(format!("Focus: [{}]", t)))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("btn_status_tail")
                            .cursor_pointer()
                            .px_1p5()
                            .py_0p5()
                            .rounded_xs()
                            .hover(|s| s.bg(ZED.ghost_hover))
                            .text_color(if self.tail_enabled {
                                ZED.success
                            } else {
                                ZED.text_muted
                            })
                            .child(if self.tail_enabled {
                                "Tail: ON ●"
                            } else {
                                "Tail: OFF"
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.tail_enabled = !this.tail_enabled;
                                cx.notify();
                            })),
                    ),
            );

        // 8. Floating Overlays
        let search_overlay = SearchOverlay::render(
            &self.search_state,
            &self.search_input,
            cx,
        );

        let filter_builder_popover = FilterPopover::render_builder(
            &self.filter_engine,
            self.is_filter_builder_open,
            cx,
        );

        let quick_log_menu = FilterPopover::render_quick_log_menu(
            &self.active_quick_menu,
            cx,
        );

        let context_modal = ContextModal::render(
            self.is_context_open,
            self.selected_row_pos
                .and_then(|p| self.visible_indices.get(p).copied()),
            &self.all_entries,
            cx,
        );

        let command_palette = CommandPalette::render(self.is_palette_open, cx);

        // Main Layout Container (Base Window Canvas)
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key_down(event, cx);
            }))
            .size_full()
            .bg(ZED.background)
            .flex()
            .flex_col()
            .relative()
            .child(window_chrome)
            .child(editor_toolbar)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_row()
                    .relative()
                    .child(rail)
                    .child(sidebar_content)
                    .child(log_canvas)
                    .child(inspector_view),
            )
            .child(status_bar)
            // Floating Overlays
            .child(search_overlay)
            .child(filter_builder_popover)
            .child(quick_log_menu)
            .child(context_modal)
            .child(command_palette)
    }
}

fn rail_icon(
    id: &'static str,
    icon: IconName,
    is_active: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .cursor_pointer()
        .w(px(32.0))
        .h(px(32.0))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .bg(if is_active {
            ZED.ghost_selected
        } else {
            ZED.transparent
        })
        .hover(|s| s.bg(ZED.ghost_hover))
        .child(
            Icon::new(icon)
                .size(px(16.0))
                .text_color(if is_active {
                    ZED.icon_accent
                } else {
                    ZED.icon_muted
                }),
        )
        .on_click(on_click)
}
