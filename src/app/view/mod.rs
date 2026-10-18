//! UI: root render for `AppView`, composed from the sibling view modules.

mod chrome;
mod dialogs;
mod filter_panel;
mod marker_panel;
mod raw_view;
mod status_bar;
mod tab_bar;
mod toolbar;

use super::AppView;
use crate::workspace::working_set::ViewMode;
use gpui_component::ActiveTheme as _;

use crate::ui::canvas::{CanvasColors, CanvasViewState, LogCanvas};
use crate::ui::command_palette::CommandPalette;
use crate::ui::context_view::ContextModal;
use crate::ui::inspector::InspectorView;
use crate::ui::project_switcher::ProjectSwitcher;
use crate::ui::context_sidebar::{ContextSections, ContextSidebar};
use crate::ui::sidebar::SidebarView;
use gpui::{px, 
    div, Context, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, Styled,
    Window,
};
use std::sync::Arc;

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_filter_inputs(window, cx);
        self.sync_raw_editors(window, cx);
        self.sync_new_space_dialog(window, cx);

        let window_chrome = self.render_window_chrome(window, cx);
        let editor_toolbar = self.render_editor_toolbar(cx);
        let tab_bar = self.render_tab_bar(cx);

        // 5. Center: Raw View, Parsed View (the Log Canvas) or an empty state.
        let has_files = self.active_space().is_some_and(|s| !s.files.is_empty());
        let log_canvas = if !self.active_tab_is_log_file() || !has_files {
            self.render_empty_state(cx)
        } else if self.view_mode == ViewMode::Raw {
            self.render_raw_view(cx)
        } else {
            let entries_arc = Arc::new(self.all_entries.clone());
            LogCanvas::render(CanvasViewState {
                view: cx.entity().clone(),
                entries: entries_arc,
                visible_indices: self.visible_indices.clone(),
                selected_row_pos: self.selected_row_pos,
                density: self.density,
                column_layout: &self.column_layout,
                search_state: &self.search_state,
                colors: CanvasColors {
                    background: cx.theme().background,
                    header_bg: cx.theme().table_head,
                    header_fg: cx.theme().table_head_foreground,
                    border: cx.theme().border,
                    selected: cx.theme().list_active,
                    search: cx.theme().warning,
                    muted: cx.theme().muted_foreground,
                    danger: cx.theme().danger,
                    warning: cx.theme().warning,
                    info: cx.theme().info,
                },
                scroll_handle: &self.scroll_handle,
            })
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
        let notice = self.render_notice(cx);
        let quick_settings = self.render_quick_settings(cx);

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

        // Right context sidebar (View / Filter / Marker / Inspector): only for a
        // focused log file tab, built from that file's own state.
        let context_sidebar = self
            .context_sidebar
            .is_shown(self.active_tab_is_log_file())
            .then(|| {
                let sections = ContextSections {
                    filter: self.render_filter_panel_content(cx),
                    marker: self.render_marker_panel_content(cx),
                    inspector: InspectorView::render_body(
                        self.selected_entry(),
                        self.inspector_tab,
                        cx,
                    ),
                    active_filters: self.filter_state.active_count(),
                    marker_count: self
                        .visible_indices
                        .iter()
                        .filter_map(|&ix| self.all_entries.get(ix))
                        .filter(|e| e.effective_marker().is_some())
                        .count(),
                };
                ContextSidebar::render(&self.context_sidebar, window, viewport_height, sections, cx)
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
                            .child(tab_bar)
                            .child(editor_toolbar)
                            .child(log_canvas)
                            .child(notice),
                    )
                    .children(context_sidebar),
            )
            .child(status_bar)
            .child(context_modal)
            .child(command_palette)
            .child(project_switcher)
            .child(quick_settings)
            .child(new_space_dialog)
            .child(confirm)
    }
}
