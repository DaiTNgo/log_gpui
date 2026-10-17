use crate::theme::ZED;
use gpui::{
    div, px, AppContext, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{Icon, IconName};

/// Which floating tool panel a given overlay represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    View,
    Filter,
    Marker,
}

impl PanelKind {
    pub fn id(self) -> &'static str {
        match self {
            PanelKind::View => "panel_view",
            PanelKind::Filter => "panel_filter",
            PanelKind::Marker => "panel_marker",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PanelKind::View => "View",
            PanelKind::Filter => "Filter",
            PanelKind::Marker => "Marker",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            PanelKind::View => IconName::SquareTerminal,
            PanelKind::Filter => IconName::FileText,
            PanelKind::Marker => IconName::BookOpen,
        }
    }
}

/// Geometry + visibility of a floating panel (all pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingPanelState {
    pub open: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl FloatingPanelState {
    pub fn default_for(kind: PanelKind) -> Self {
        match kind {
            // Staggered defaults so opened panels don't fully overlap.
            PanelKind::View => Self {
                open: false,
                x: 24.0,
                y: 88.0,
                w: 300.0,
                h: 380.0,
            },
            PanelKind::Filter => Self {
                open: false,
                x: 348.0,
                y: 88.0,
                w: 380.0,
                h: 428.0,
            },
            PanelKind::Marker => Self {
                open: false,
                x: 672.0,
                y: 120.0,
                w: 320.0,
                h: 360.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelEdge {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl PanelEdge {
    /// Apply a drag delta to a starting rect, returning the new (x, y, w, h).
    pub fn apply(&self, start: &FloatingPanelState, dx: f32, dy: f32) -> (f32, f32, f32, f32) {
        let (mut x, mut y, mut w, mut h) = (start.x, start.y, start.w, start.h);
        let west = matches!(self, PanelEdge::W | PanelEdge::NW | PanelEdge::SW);
        let east = matches!(self, PanelEdge::E | PanelEdge::NE | PanelEdge::SE);
        let north = matches!(self, PanelEdge::N | PanelEdge::NW | PanelEdge::NE);
        let south = matches!(self, PanelEdge::S | PanelEdge::SW | PanelEdge::SE);

        if east {
            w = (start.w + dx).max(MIN_W);
        }
        if west {
            let nw = (start.w - dx).max(MIN_W);
            x = start.x + (start.w - nw);
            w = nw;
        }
        if south {
            h = (start.h + dy).max(MIN_H);
        }
        if north {
            let nh = (start.h - dy).max(MIN_H);
            y = start.y + (start.h - nh);
            h = nh;
        }
        (x, y, w, h)
    }
}

const MIN_W: f32 = 240.0;
const MIN_H: f32 = 160.0;

/// Gesture in flight: either moving a panel (edge = None) or resizing it.
#[derive(Debug, Clone, Copy)]
pub struct PanelGesture {
    pub kind: PanelKind,
    pub edge: Option<PanelEdge>,
    pub start: FloatingPanelState,
    pub mouse_x: f32,
    pub mouse_y: f32,
}

/// Drag payload used to initiate a panel move (see canvas `ResizeColumn`).
#[derive(Debug, Clone, Copy)]
pub struct PanelDrag {
    pub kind: PanelKind,
}

impl Render for PanelDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Drag payload used to initiate a panel resize.
#[derive(Debug, Clone, Copy)]
pub struct PanelResizeDrag {
    pub kind: PanelKind,
    pub edge: PanelEdge,
}

impl Render for PanelResizeDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct FloatingPanel;

impl FloatingPanel {
    /// Frame a panel with a draggable title bar, close button, fully
    /// resizable edges/corners, and the given scrollable body content.
    pub fn render(
        kind: PanelKind,
        state: FloatingPanelState,
        content: gpui::AnyElement,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !state.open {
            return div().into_any_element();
        }

        let view = cx.entity().clone();
        let header_drag = view.clone();

        let header = div()
            .id(ElementId::from(format!("{}_title", kind.id())))
            .cursor_pointer()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_2()
            .border_b_1()
            .border_color(ZED.border_variant)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_color(ZED.icon_muted)
                            .child(Icon::new(kind.icon()).size(px(13.0))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ZED.text)
                            .child(kind.title()),
                    ),
            )
            .child(
                div()
                    .id(ElementId::from(format!("{}_close", kind.id())))
                    .cursor_pointer()
                    .p_1()
                    .rounded_md()
                    .text_color(ZED.icon_muted)
                    .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.icon_hover))
                    .child(Icon::new(IconName::Close).size(px(13.0)))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.toggle_panel(kind, cx);
                    })),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                    if event.button == MouseButton::Left {
                        this.panel_begin_move(kind, event.position, cx);
                    }
                }),
            )
            .on_drag(PanelDrag { kind }, move |drag, _point, _window, cx| {
                cx.stop_propagation();
                cx.new(|_| *drag)
            })
            .on_drag_move(move |e: &gpui::DragMoveEvent<PanelDrag>, _window, cx| {
                let kind = e.drag(cx).kind;
                header_drag.update(cx, |this, cx| {
                    this.panel_gesture_move(kind, e.event.position, cx);
                });
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.panel_gesture_end(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.panel_gesture_end(cx)),
            );

let mut handles = Vec::new();
        for edge in [
            PanelEdge::NW,
            PanelEdge::N,
            PanelEdge::NE,
            PanelEdge::E,
            PanelEdge::SE,
            PanelEdge::S,
            PanelEdge::SW,
            PanelEdge::W,
        ] {
            handles.push(resize_handle(kind, edge, view.clone()));
        }

        div()
            .id(ElementId::from(kind.id()))
            .absolute()
            .left(px(state.x))
            .top(px(state.y))
            .w(px(state.w))
            .h(px(state.h))
            .bg(ZED.elevated_surface)
            .border_1()
            .border_color(ZED.border)
            .rounded_lg()
            .shadow_2xl()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .p_2p5()
                    .child(content),
            )
            .children(handles)
            .into_any_element()
    }
}

/// A single 8-way resize handle strip/corner.
fn resize_handle(
    kind: PanelKind,
    edge: PanelEdge,
    view: gpui::Entity<crate::app::AppView>,
) -> gpui::AnyElement {
    let size = match edge {
        PanelEdge::NW | PanelEdge::NE | PanelEdge::SW | PanelEdge::SE => 14.0,
        _ => 6.0,
    };
    let is_west = matches!(edge, PanelEdge::W | PanelEdge::NW | PanelEdge::SW);
    let is_east = matches!(edge, PanelEdge::E | PanelEdge::NE | PanelEdge::SE);
    let is_north = matches!(edge, PanelEdge::N | PanelEdge::NW | PanelEdge::NE);
    let is_south = matches!(edge, PanelEdge::S | PanelEdge::SW | PanelEdge::SE);

    let view_drag = view.clone();
    let view_down = view.clone();
    let view_up = view.clone();
    let view_up_out = view.clone();

    let mut handle = div()
        .id(ElementId::from(format!("{}_resize_{:?}", kind.id(), edge)))
        .cursor_pointer()
        .absolute()
        .flex_1()
        .on_drag(PanelResizeDrag { kind, edge }, move |drag, _point, _window, cx| {
            cx.stop_propagation();
            cx.new(|_| *drag)
        })
        .on_drag_move(move |e: &gpui::DragMoveEvent<PanelResizeDrag>, _window, cx| {
            let d = *e.drag(cx);
            view_drag.update(cx, |this, cx| {
                this.panel_gesture_move(d.kind, e.event.position, cx);
            });
        })
        .on_mouse_down(
            MouseButton::Left,
            move |event: &gpui::MouseDownEvent, _window, cx| {
                if event.button == MouseButton::Left {
                    view_down.update(cx, |this, cx| {
                        this.panel_begin_resize(kind, edge, event.position, cx);
                    });
                }
            },
        )
        .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
            view_up.update(cx, |this, cx| this.panel_gesture_end(cx));
        })
        .on_mouse_up_out(MouseButton::Left, move |_event, _window, cx| {
            view_up_out.update(cx, |this, cx| this.panel_gesture_end(cx));
        });

    if is_west {
        handle = handle.left_0().top(px(size)).bottom(px(size)).w(px(size));
    } else if is_east {
        handle = handle.right_0().top(px(size)).bottom(px(size)).w(px(size));
    }
    if is_north {
        handle = handle.top_0().left(px(size / 2.0)).right(px(size / 2.0)).h(px(size));
    } else if is_south {
        handle = handle.bottom_0().left(px(size / 2.0)).right(px(size / 2.0)).h(px(size));
    }

    // Corner overlays take priority over edge strips.
    match edge {
        PanelEdge::NW => handle = handle.left_0().top_0().w(px(size)).h(px(size)),
        PanelEdge::NE => handle = handle.right_0().top_0().w(px(size)).h(px(size)),
        PanelEdge::SW => handle = handle.left_0().bottom_0().w(px(size)).h(px(size)),
        PanelEdge::SE => handle = handle.right_0().bottom_0().w(px(size)).h(px(size)),
        _ => {}
    }

    handle.into_any_element()
}