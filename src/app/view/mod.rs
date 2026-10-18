//! UI: root render for `AppView`, composed from the sibling view modules.

mod chrome;
mod color_bar;
mod dialogs;
mod filter_panel;
mod focus_pane;
mod highlights_panel;
mod raw_view;
mod remote_dialogs;
mod server_views;
mod status_bar;
mod tab_bar;
mod toolbar;

use super::AppView;
use crate::workspace::working_set::ViewMode;
use gpui_component::ActiveTheme as _;

use crate::ui::canvas::{CanvasColors, CanvasViewState, LogCanvas};
use crate::ui::command_palette::CommandPalette;
use crate::ui::context_sidebar::{ContextSections, ContextSidebar};
use crate::ui::context_view::ContextModal;
use crate::ui::inspector::InspectorView;
use crate::ui::project_switcher::ProjectSwitcher;
use crate::ui::sidebar::SidebarView;
use gpui::{
    Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Window,
    div, px,
};
use std::sync::Arc;

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        sync_editor_colors(cx);
        self.sync_focus_pane_visibility(cx);
        self.sync_filter_inputs(window, cx);
        self.sync_search_input(window, cx);
        self.sync_focus_find_input(window, cx);
        self.sync_raw_editors(window, cx);
        self.sync_raw_color_bar(window, cx);
        self.sync_new_space_dialog(window, cx);
        self.sync_secret_prompt(window, cx);
        self.sync_server_dialog(window, cx);
        self.sync_remote_picker(window, cx);

        let window_chrome = self.render_window_chrome(window, cx);
        let editor_toolbar = self.render_editor_toolbar(cx);
        let tab_bar = self.render_tab_bar(cx);

        // 5. Center: Raw View, Parsed View (the Log Canvas) or an empty state.
        let has_files = self.active_space().is_some_and(|s| !s.files.is_empty());
        let colors = self.canvas_colors(cx);
        let parsed = self.active_tab_is_log_file() && has_files && self.view_mode == ViewMode::Parsed;
        let log_canvas = if !self.active_tab_is_log_file() || !has_files {
            self.render_empty_state(cx)
        } else if self.view_mode == ViewMode::Raw {
            self.render_raw_view(cx)
        } else {
            LogCanvas::render(CanvasViewState {
                view: cx.entity().clone(),
                entries: if parsed { &self.all_entries } else { &[] },
                visible_indices: self.visible_indices.clone(),
                selected_row_pos: self.selected_row_pos,
                density: self.density,
                show_minimap: self.show_minimap,
                column_layout: &self.column_layout,
                search_state: self.focus.search(),
                bounds_sink: self.canvas_bounds.clone(),
                paints: self.highlight_paints.clone(),
                selection: self.text_selection,
                colors,
                scroll_handle: &self.scroll_handle,
            })
        };

        let viewport_height = f32::from(window.viewport_size().height);
        let focus_pane =
            self.render_focus_pane(parsed, colors, viewport_height, cx);
        // The sidebars end where the pane begins.
        let pane_taken = if focus_pane.is_some() {
            self.focus_pane_height(viewport_height)
        } else {
            0.0
        };

        let status_bar = self.render_status_bar(cx);

        let context_modal = ContextModal::render(
            self.is_context_open,
            self.selected_row_pos
                .and_then(|p| self.visible_indices.get(p).copied()),
            &self.all_entries,
            cx,
        );

        let command_palette = CommandPalette::render(self.is_palette_open, cx);
        let new_space_dialog = self.render_new_space_dialog(cx);
        let confirm = self.render_confirm(cx);
        let host_key_dialog = self.render_host_key_dialog(cx);
        let secret_dialog = self.render_secret_dialog(cx);
        let servers_dialog = self.render_servers_dialog(cx);
        let remote_picker = self.render_remote_picker(cx);
        let download_panel = self.render_download(cx);
        let color_bar = self.render_color_bar(cx);
        let notice = self.render_notice(cx);

        let project_switcher = ProjectSwitcher::render(
            self.project_switcher_open,
            &self.workspace_state.projects,
            self.workspace_state.active_project_id.as_deref(),
            self.project_switcher_index,
            self.project_renaming.as_deref(),
            &self.project_rename_buffer,
            cx,
        );

        // Sidebar
        let sidebar_view = SidebarView::render(
            &self.sidebar_state,
            &self.workspace_state,
            self.renaming.as_ref(),
            &self.expanded_spaces,
            &self.no_timestamp_files,
            cx,
        );

        let viewport_height = f32::from(window.viewport_size().height);

        // Right context sidebar (Filter / Highlights / Inspector): only for a
        // focused log file tab, built from that file's own state.
        let context_sidebar = self
            .context_sidebar
            // The Raw View keeps the Inspector alone; Filter and Highlights are for parsed rows.
            .is_shown(self.active_tab_is_log_file())
            .then(|| {
                let sections = ContextSections {
                    filter: self.render_filter_panel_content(cx),
                    highlights: self.render_highlights_panel_content(cx),
                    inspector: InspectorView::render_body(self.inspector_lines()),
                    active_filters: self.filter_state.active_count(),
                    rule_count: self.highlight_rules().len(),
                    parsed: self.view_mode == ViewMode::Parsed,
                };
                ContextSidebar::render(
                    &self.context_sidebar,
                    window,
                    viewport_height - pane_taken,
                    sections,
                    cx,
                )
            });

        // Main Layout Container (Base Window Canvas)
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .child(window_chrome)
            // Three columns: left sidebar | main view | right sidebar(s).
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .flex()
                    .flex_row()
                    .relative()
                    .child(sidebar_view)
                    // Main view: tabs, search, then the log canvas.
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .bg(crate::ui::main_view::background(cx))
                            .child(tab_bar)
                            .child(editor_toolbar)
                            .child(log_canvas)
                            .child(notice),
                    )
                    .children(context_sidebar),
            )
            .children(focus_pane)
            .child(status_bar)
            .child(context_modal)
            .child(command_palette)
            .child(project_switcher)
            .child(new_space_dialog)
            .child(confirm)
            .child(servers_dialog)
            .child(remote_picker)
            .child(download_panel)
            .child(secret_dialog)
            .child(host_key_dialog)
            .children(color_bar)
    }
}

impl AppView {
    /// The colors the Log Canvas and the Focus Pane paint with.
    pub(crate) fn canvas_colors(&self, cx: &gpui::App) -> CanvasColors {
        CanvasColors {
            background: crate::ui::main_view::background(cx),
            header_bg: crate::ui::main_view::background(cx),
            header_fg: cx.theme().table_head_foreground,
            border: cx.theme().border,
            selected: cx.theme().list_active,
            search: self.workspace_state.match_colors.search.map_or(
                gpui::hsla(0.55, 0.85, 0.55, 1.0),
                hue_color,
            ),
            find: self.workspace_state.match_colors.find.map_or(cx.theme().warning, hue_color),
            muted: cx.theme().muted_foreground,
            danger: cx.theme().danger,
            warning: cx.theme().warning,
            info: cx.theme().info,
            text_selection: cx.theme().selection,
            dark: cx.theme().is_dark(),
        }
    }
}

/// Raw View editors paint their own background and line-number gutter from the
/// highlight theme; keep both on the main view's background.
fn sync_editor_colors(cx: &mut gpui::App) {
    let main_bg = crate::ui::main_view::background(cx);
    let style = &cx.theme().highlight_theme.style;
    if style.editor_background == Some(main_bg) && style.editor_gutter_background == Some(main_bg) {
        return;
    }
    let theme = gpui_component::Theme::global_mut(cx);
    let highlight = Arc::make_mut(&mut theme.highlight_theme);
    highlight.style.editor_background = Some(main_bg);
    highlight.style.editor_gutter_background = Some(main_bg);
}

/// The bright paint a hue gives to text matches.
fn hue_color(hue: crate::palette::Hue) -> gpui::Hsla {
    let (h, s, l) = hue.vivid_hsl();
    gpui::hsla(h, s, l, 1.0)
}
