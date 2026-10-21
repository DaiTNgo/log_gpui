//! UI glue: builds the Focus Pane's state from the app and renders it.

use super::AppView;
use crate::marker::TextMarker;
use crate::ui::canvas::CanvasColors;
use crate::ui::focus_pane::{FocusPane, PaneMode, PaneSource, PaneState, PaneTab};
use gpui_component::input::InputState;
use crate::workspace::log_space::line_hash;
use crate::workspace::working_set::ViewMode;
use gpui::{
    AppContext as _, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::ActiveTheme as _;
use std::collections::HashMap;
use std::sync::Arc;

impl AppView {
    /// The Markers of each file of the Raw View by line, leaving out those of
    /// a line edited since.
    fn raw_pane_markers(
        &self,
        sections: &[crate::app::raw::RawSection],
    ) -> HashMap<String, HashMap<usize, Vec<TextMarker>>> {
        let mut out = HashMap::new();
        let Some(project) = self.active_project() else { return out };
        for section in sections {
            let Some(file) = project.log_spaces.iter().find_map(|s| s.file(&section.file_id)) else {
                continue;
            };
            let mut by_line: HashMap<usize, Vec<TextMarker>> = HashMap::new();
            for m in &file.markers {
                let fresh = section
                    .lines
                    .get(m.line.wrapping_sub(1))
                    .is_some_and(|t| m.line_hash.is_none_or(|h| h == line_hash(t)));
                if fresh {
                    by_line
                        .entry(m.line)
                        .or_default()
                        .push(TextMarker { range: m.range.clone(), color: m.color.clone() });
                }
            }
            out.insert(section.file_id.clone(), by_line);
        }
        out
    }

    /// What the Focus Pane draws, read from the app. The docked pane and the
    /// Focus Window both build their state here; they differ in the fields
    /// they pass in.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pane_state<'a>(
        &'a self,
        view: gpui::Entity<AppView>,
        mode: PaneMode,
        find_input: &'a gpui::Entity<InputState>,
        search_input: Option<&'a gpui::Entity<InputState>>,
        parsed: bool,
        colors: CanvasColors,
        height: f32,
        expanded: bool,
    ) -> PaneState<'a> {
        let source = match (self.view_mode, parsed) {
            (ViewMode::Parsed, true) => PaneSource::Parsed {
                paints: self.highlight_paints.clone(),
            },
            _ => {
                let sections = self.raw_sections();
                let markers = self.raw_pane_markers(&sections);
                PaneSource::Raw { sections: Arc::new(sections), markers: Arc::new(markers) }
            }
        };
        let notice = self.focus_pane_notice();
        let longest = self.focus_longest;
        PaneState {
            view,
            mode,
            search_input,
            height,
            expanded,
            tabs: self
                .focus
                .tabs
                .iter()
                .enumerate()
                .map(|(i, t)| PaneTab { label: t.label(), active: i == self.focus.active })
                .collect(),
            can_close: self.focus.can_close(),
            find_input,
            find: self.focus.find(),
            search_matcher: self.focus.search().matcher.clone(),
            rows: self.focus_rows.clone(),
            source,
            selected: self.pane_selected_index(),
            show_source: self.pane_shows_source(),
            notice,
            density: self.density,
            colors,
            scroll_handle: &self.focus_scroll,
            selection: self.pane_selection,
            bounds_sink: self.pane_bounds.clone(),
            longest,
            columns: self.pane_layout(),
        }
    }

    /// The Focus Pane, the drag handle above it, or nothing when hidden or
    /// when it lives in the Focus Window.
    pub(super) fn render_focus_pane(
        &mut self,
        parsed: bool,
        colors: CanvasColors,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.focus_window.is_some()
            || !self.active_tab_is_log_file()
            || !self.focus_pane_has_content()
        {
            return None;
        }
        let expanded = self.workspace_state.focus_pane.visible;
        let height = self.focus_pane_height(viewport_height);
        let state = self.pane_state(
            cx.entity(),
            PaneMode::Docked,
            &self.focus_find_input,
            None,
            parsed,
            colors,
            height,
            expanded,
        );
        let pane = FocusPane::render(state, cx);
        // Collapsed, there is nothing to drag.
        if !expanded {
            return Some(div().w_full().flex_shrink_0().child(pane).into_any_element());
        }

        // Drag the border between the view and the pane to resize it.
        let border = cx.theme().border;
        let resizing = self.workspace_state.focus_pane.is_resizing;
        let handle = div()
            .id("focus-pane-resize")
            .w_full()
            .h(px(5.0))
            .flex_shrink_0()
            .cursor_row_resize()
            .bg(if resizing { border } else { gpui::transparent_black() })
            .on_drag(ResizeFocusPane, |drag, _point, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(cx.listener(
                move |this, e: &gpui::DragMoveEvent<ResizeFocusPane>, _window, cx| {
                    this.update_focus_pane_resize(e.event.position.y, viewport_height, cx);
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                    if event.click_count >= 2 {
                        this.reset_focus_pane_height(cx);
                    } else {
                        this.start_resizing_focus_pane(cx);
                    }
                }),
            )
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.finish_resizing_focus_pane(cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| this.finish_resizing_focus_pane(cx)));
        Some(div().w_full().flex_shrink_0().child(handle).child(pane).into_any_element())
    }
}

/// Marker for the drag that resizes the Focus Pane.
#[derive(Clone)]
pub struct ResizeFocusPane;

impl gpui::Render for ResizeFocusPane {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
