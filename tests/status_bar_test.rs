mod common;

use gpui::TestAppContext;
use log_gpui::app::format_count;

#[test]
fn format_count_groups_thousands() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1,000");
    assert_eq!(format_count(12_345), "12,345");
    assert_eq!(format_count(1_200_000), "1,200,000");
}

#[gpui::test]
fn status_bar_shows_line_count(cx: &mut TestAppContext) {
    let h = common::open_app_with_lines(cx, &common::sample_lines(400));
    let labels = h.view.read_with(&h.cx, |app, _| app.status_bar_labels());
    assert!(labels.contains(&"400 lines".to_string()), "{labels:?}");
}
