//! UI: root render for `AppView`, composed from the sibling view modules.

mod chrome;
mod marker_panel;
mod status_bar;
mod tab_bar;
mod toolbar;
mod view_panel;

use super::AppView;

use crate::ui::canvas::{CanvasViewState, LogCanvas};
use crate::ui::command_palette::CommandPalette;
use crate::ui::context_view::ContextModal;
use crate::ui::filter_popover::FilterPopover;
use crate::ui::floating_panel::{FloatingPanel, PanelKind};
use crate::ui::inspector::InspectorView;
use crate::ui::project_switcher::ProjectSwitcher;
use crate::ui::context_sidebar::ContextSidebar;
use crate::ui::sidebar::SidebarView;
use gpui::{px, 
    div, Context, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, Styled,
    Window,
};
use std::sync::Arc;

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entries_arc = Arc::new(self.all_entries.clone());
        let visible_arc = self.visible_indices.clone();

        let window_chrome = self.render_window_chrome(window, cx);
        let editor_toolbar = self.render_editor_toolbar(cx);
        let tab_bar = self.render_tab_bar(cx);

        // Floating tool panel contents (View / Filter / Marker)
        let marker_panel_content = self.render_marker_panel_content(cx);
        let view_panel_content = self.render_view_panel_content(cx);
        let filter_panel_content = FilterPopover::render_builder_content(&self.filter_engine, cx);

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

        let status_bar = self.render_status_bar(cx);

        // 8. Floating Overlays
        let view_panel_overlay = FloatingPanel::render(
            PanelKind::View,
            self.panel(PanelKind::View),
            view_panel_content,
            cx,
        );
        let marker_panel_overlay = FloatingPanel::render(
            PanelKind::Marker,
            self.panel(PanelKind::Marker),
            marker_panel_content,
            cx,
        );
        let filter_panel_overlay = FloatingPanel::render(
            PanelKind::Filter,
            self.panel(PanelKind::Filter),
            filter_panel_content,
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
            cx,
        );

        // Right context sidebar: only for a focused log file tab.
        let context_sidebar = self
            .context_sidebar
            .is_shown(self.active_tab_is_log_file())
            .then(|| {
                ContextSidebar::render(
                    &self.context_sidebar,
                    &self.workspace_state,
                    &self.all_entries,
                    &self.visible_indices,
                    self.renaming.as_ref(),
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
                            .child(tab_bar)
                            .child(editor_toolbar)
                            .child(log_canvas),
                    )
                    .child(inspector_view)
                    .children(context_sidebar),
            )
            .child(status_bar)
            // Floating Overlays
            .child(view_panel_overlay)
            .child(marker_panel_overlay)
            .child(filter_panel_overlay)
            .child(quick_log_menu)
            .child(context_modal)
            .child(command_palette)
            .child(project_switcher)
    }
}
