//! Logic: floating panel visibility and drag/resize gestures.

use super::AppView;
use crate::ui::floating_panel::{FloatingPanelState, PanelEdge, PanelGesture, PanelKind};
use gpui::{
    Context, Pixels,
};

impl AppView {
    pub fn toggle_panel(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        let panel = self.floating_panels.entry(kind).or_insert_with(|| {
            FloatingPanelState::default_for(kind)
        });
        panel.open = !panel.open;
        self.panel_gesture = None;
        if panel.open && kind == PanelKind::Filter {
            self.search_state.is_open = false;
        }
        cx.notify();
    }

    pub fn panel(&self, kind: PanelKind) -> FloatingPanelState {
        *self
            .floating_panels
            .get(&kind)
            .unwrap_or(&FloatingPanelState::default_for(kind))
    }

    pub fn panel_mut(&mut self, kind: PanelKind) -> &mut FloatingPanelState {
        self.floating_panels
            .entry(kind)
            .or_insert_with(|| FloatingPanelState::default_for(kind))
    }

    pub fn toggle_filter_builder(&mut self, cx: &mut Context<Self>) {
        self.toggle_panel(PanelKind::Filter, cx);
    }

    pub fn panel_begin_move(
        &mut self,
        kind: PanelKind,
        pos: gpui::Point<Pixels>,
        _cx: &mut Context<Self>,
    ) {
        let st = self.panel(kind);
        self.panel_gesture = Some(PanelGesture {
            kind,
            edge: None,
            start: st,
            mouse_x: pos.x.as_f32(),
            mouse_y: pos.y.as_f32(),
        });
    }

    pub fn panel_begin_resize(
        &mut self,
        kind: PanelKind,
        edge: PanelEdge,
        pos: gpui::Point<Pixels>,
        _cx: &mut Context<Self>,
    ) {
        let st = self.panel(kind);
        self.panel_gesture = Some(PanelGesture {
            kind,
            edge: Some(edge),
            start: st,
            mouse_x: pos.x.as_f32(),
            mouse_y: pos.y.as_f32(),
        });
    }

    pub fn panel_gesture_move(
        &mut self,
        kind: PanelKind,
        pos: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(gesture) = self.panel_gesture else {
            return;
        };
        if gesture.kind != kind {
            return;
        }
        let dx = pos.x.as_f32() - gesture.mouse_x;
        let dy = pos.y.as_f32() - gesture.mouse_y;
        let st = self.panel_mut(kind);
        match gesture.edge {
            None => {
                st.x = gesture.start.x + dx;
                st.y = gesture.start.y + dy;
            }
            Some(edge) => {
                let (x, y, w, h) = edge.apply(&gesture.start, dx, dy);
                st.x = x;
                st.y = y;
                st.w = w;
                st.h = h;
            }
        }
        cx.notify();
    }

    pub fn panel_gesture_end(&mut self, _cx: &mut Context<Self>) {
        self.panel_gesture = None;
    }

    pub fn close_all_panels(&mut self) {
        for panel in self.floating_panels.values_mut() {
            panel.open = false;
        }
        self.panel_gesture = None;
    }
}
