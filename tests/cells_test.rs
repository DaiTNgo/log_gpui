use log_gpui::column::{ColumnLayoutState, PinnableColumn};
use log_gpui::parser::text::parse_text_bytes;
use log_gpui::text_selection::byte_at_x;
use log_gpui::ui::cells::{Hit, Layer, TextCell, byte_for_hit, cell_text, compose, hit_cell, shows_placeholder};

fn rows() -> Vec<log_gpui::model::LogEntry> {
    parse_text_bytes(
        b"2026-10-18 10:00:01 [ERROR] [api] boom\n    at A.b(A.java:1)\n",
    )
    .unwrap()
}

#[test]
fn a_cell_shows_its_span_of_the_line() {
    let es = rows();
    let (text, span) = cell_text(&es[0], TextCell::Level).unwrap();
    assert_eq!(text, "ERROR");
    assert_eq!(&es[0].raw_line[span.unwrap()], "ERROR");
    let (module, _) = cell_text(&es[0], TextCell::Module).unwrap();
    assert_eq!(module, "api");
}

#[test]
fn a_continuation_line_has_text_only_in_the_message_cell() {
    let es = rows();
    assert!(shows_placeholder(&es[1], TextCell::Timestamp));
    assert!(shows_placeholder(&es[1], TextCell::Level));
    assert!(shows_placeholder(&es[1], TextCell::Module));
    assert!(!shows_placeholder(&es[1], TextCell::Message));
    assert!(cell_text(&es[1], TextCell::Level).is_none(), "the `_` is not text");
    let (msg, _) = cell_text(&es[1], TextCell::Message).unwrap();
    assert_eq!(msg, "    at A.b(A.java:1)");
}

#[test]
fn layers_paint_over_each_other_in_order() {
    let text = "abcdef";
    let red = gpui::hsla(0.0, 1.0, 0.5, 1.0);
    let blue = gpui::hsla(0.6, 1.0, 0.5, 1.0);
    let out = compose(
        text,
        &[
            Layer { range: 0..6, background: Some(red), color: None },
            Layer { range: 2..4, background: Some(blue), color: None },
        ],
    );
    let spans: Vec<(std::ops::Range<usize>, gpui::Hsla)> =
        out.iter().map(|(r, s)| (r.clone(), s.background_color.unwrap())).collect();
    assert_eq!(spans, vec![(0..2, red), (2..4, blue), (4..6, red)]);
}

#[test]
fn a_pointer_maps_to_the_cell_and_character_under_it() {
    let es = rows();
    let layout = ColumnLayoutState::new();
    let gutter = f32::from(layout.gutter_width());
    let ts = f32::from(layout.width(PinnableColumn::Timestamp));
    // Just inside the LEVEL cell, past its 4px padding and two 7.2px glyphs.
    let x = gutter + ts + 4.0 + 2.0 * 7.2 + 1.0;
    let hit = hit_cell(&layout, x, 0.0);
    match hit {
        Hit::Text { cell: TextCell::Level, x } => assert!(x > 14.0 && x < 16.0, "x = {x}"),
        other => panic!("{other:?}"),
    }
    // A fixed 7.2px advance stands in for the shaped font.
    let byte = byte_for_hit(&es[0], hit, |text, x| byte_at_x(text, x, 7.2));
    assert_eq!(&es[0].raw_line[byte..byte + 3], "ROR", "after `ER`");
    assert_eq!(hit_cell(&layout, 10.0, 0.0), Hit::Gutter);
}

#[test]
fn scrolling_slides_the_cells_under_the_anchored_partition() {
    let layout = ColumnLayoutState::new();
    let gutter = f32::from(layout.gutter_width());
    let ts = f32::from(layout.width(PinnableColumn::Timestamp));
    // Scrolled by one timestamp column, the pointer at the gutter edge is on LEVEL.
    match hit_cell(&layout, gutter + 2.0, ts) {
        Hit::Text { cell: TextCell::Level, .. } => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_pointer_past_the_last_cell_is_in_the_message() {
    let layout = ColumnLayoutState::new();
    match hit_cell(&layout, 5000.0, 0.0) {
        Hit::Text { cell: TextCell::Message, .. } => {}
        other => panic!("{other:?}"),
    }
}
