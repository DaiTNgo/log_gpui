use log_gpui::ui::floating_panel::{FloatingPanelState, PanelEdge, PanelKind};

#[test]
fn default_panels_start_closed_with_staggered_geometry() {
    let parse = |kind| {
        let st = FloatingPanelState::default_for(kind);
        (st.open, st.w, st.h)
    };

    let (v_open, v_w, v_h) = parse(PanelKind::View);
    let (f_open, f_w, f_h) = parse(PanelKind::Filter);
    let (m_open, m_w, m_h) = parse(PanelKind::Marker);

    assert!(!v_open && !f_open && !m_open);
    assert!(v_w >= 240.0 && f_w >= 240.0 && m_w >= 240.0);
    assert!(v_h >= 160.0 && f_h >= 160.0 && m_h >= 160.0);

    // Panels are placed so they don't fully overlap: X origins are distinct.
    let x = |kind| FloatingPanelState::default_for(kind).x;
    assert_ne!(x(PanelKind::View), x(PanelKind::Filter));
    assert_ne!(x(PanelKind::Filter), x(PanelKind::Marker));
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