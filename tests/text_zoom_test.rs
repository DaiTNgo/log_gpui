//! ⌘+ / ⌘− / ⌘0 make the log text larger, smaller and back.
mod common;

use gpui::{Focusable as _, TestAppContext};
use log_gpui::column::PinnableColumn;

fn press(h: &mut common::Harness, keys: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let handle = app.focus_handle(cx);
        window.focus(&handle, cx);
    });
    h.cx.simulate_keystrokes(keys);
    h.cx.run_until_parked();
}

fn size(h: &common::Harness) -> (f32, f32, f32) {
    h.view.read_with(&h.cx, |app, _| {
        let scale = app.text_scale();
        (scale.font_size(), scale.row_height(), f32::from(app.column_layout.width(PinnableColumn::Timestamp)))
    })
}

#[gpui::test]
fn zoom_keys_resize_text_rows_and_columns(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(20));
    let base = size(&h);
    press(&mut h, "cmd-= cmd-=");
    let larger = size(&h);
    assert_eq!(larger.0, base.0 + 2.0);
    assert!(larger.1 > base.1 && larger.2 > base.2, "{base:?} → {larger:?}");
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.workspace_state.text_zoom), 2, "kept as a setting");

    press(&mut h, "cmd--");
    assert_eq!(size(&h).0, base.0 + 1.0);
    press(&mut h, "cmd-0");
    assert_eq!(size(&h), base);
}

#[gpui::test]
fn zoom_stops_at_its_bounds(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(5));
    for _ in 0..20 {
        h.view.update(&mut h.cx, |app, cx| app.zoom_text(1, cx));
    }
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.text_zoom), log_gpui::density::ZOOM_MAX);
}

#[test]
fn a_layout_saved_at_another_size_scales_its_unmeasured_columns() {
    use log_gpui::column::ColumnLayoutState;
    use log_gpui::density::{Density, TextScale};
    let mut layout = ColumnLayoutState::new();
    let before = f32::from(layout.width(PinnableColumn::Timestamp));
    layout.update_density(TextScale::new(Density::Compact, 3));
    let after = f32::from(layout.width(PinnableColumn::Timestamp));
    assert!((after - before * 15.0 / 12.0).abs() < 0.5, "{before} → {after}");
    assert_eq!(layout.fitted_font, 15.0);
}
