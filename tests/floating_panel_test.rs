use log_gpui::ui::floating_panel::{FloatingPanelState, PanelEdge, PanelKind};

#[test]
fn the_filter_panel_starts_closed_with_a_usable_size() {
    let st = FloatingPanelState::default_for(PanelKind::Filter);
    assert!(!st.open);
    assert!(st.w >= 240.0);
    assert!(st.h >= 160.0);
}

#[test]
fn west_edge_resize_only_moves_the_left_edge() {
    let start = FloatingPanelState {
        open: true,
        x: 100.0,
        y: 200.0,
        w: 300.0,
        h: 200.0,
    };
    let (x, y, w, h) = PanelEdge::W.apply(&start, 20.0, 0.0);
    assert_eq!((x, y, h), (120.0, 200.0, 200.0));
    assert_eq!(w, 280.0);
}

#[test]
fn southeast_resize_grows_from_the_top_left() {
    let start = FloatingPanelState {
        open: true,
        x: 100.0,
        y: 200.0,
        w: 300.0,
        h: 200.0,
    };
    let (x, y, w, h) = PanelEdge::SE.apply(&start, 40.0, 25.0);
    assert_eq!((x, y), (100.0, 200.0));
    assert_eq!((w, h), (340.0, 225.0));
}

#[test]
fn northwest_resize_keeps_the_southeast_corner_anchored() {
    let start = FloatingPanelState {
        open: true,
        x: 100.0,
        y: 200.0,
        w: 300.0,
        h: 200.0,
    };
    let (x, y, w, h) = PanelEdge::NW.apply(&start, -40.0, -25.0);
    assert_eq!((x, y), (60.0, 175.0));
    assert_eq!((w, h), (340.0, 225.0));
}

#[test]
fn resize_is_clamped_to_minimum_dimensions() {
    let start = FloatingPanelState {
        open: true,
        x: 0.0,
        y: 0.0,
        w: 300.0,
        h: 200.0,
    };
    let (x, y, w, h) = PanelEdge::NW.apply(&start, 10_000.0, 10_000.0);
    assert_eq!((w, h), (240.0, 160.0));
    assert_eq!((x, y), (60.0, 40.0));
}