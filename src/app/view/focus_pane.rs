//! UI glue: builds the Focus Pane's state from the app and renders it.

use super::AppView;
use crate::marker::TextMarker;
use crate::ui::canvas::CanvasColors;
use crate::ui::focus_pane::{FocusPane, PaneSource, PaneState, PaneTab};
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

    /// The Focus Pane, the drag handle above it, or nothing when hidden.
    pub(super) fn render_focus_pane(
        &mut self,
        entries: Option<Arc<Vec<crate::model::LogEntry>>>,
        colors: CanvasColors,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.active_tab_is_log_file() || !self.focus_pane_has_content() {
            return None;
        }
        let expanded = self.workspace_state.focus_pane.visible;
        let height = self.focus_pane_height(viewport_height);
        let source = match (self.view_mode, entries) {
            (ViewMode::Parsed, Some(entries)) => PaneSource::Parsed {
                entries,
                paints: self.highlight_paints.clone(),
            },
            _ => {
                let sections = self.raw_sections();
                let markers = self.raw_pane_markers(&sections);
                PaneSource::Raw { sections: Arc::new(sections), markers: Arc::new(markers) }
            }
        };
        let filtering = self.view_mode == ViewMode::Parsed && self.filter_state.active_count() > 0;
        let hint = (self.focus.search().matcher.is_none() && !filtering).then_some(
            "Type a Search Expression, or pick a Filter, to list lines here",
        );
        let state = PaneState {
            view: cx.entity(),
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
            find_input: &self.focus_find_input,
            find: self.focus.find().clone(),
            search_matcher: self.focus.search().matcher.clone(),
            rows: self.focus_rows.clone(),
            source,
            selected: self.pane_selected_index(),
            show_source: self.pane_shows_source(),
            hint,
            density: self.density,
            colors,
            scroll_handle: &self.focus_scroll,
            selection: self.pane_selection,
            bounds_sink: self.pane_bounds.clone(),
        };
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
