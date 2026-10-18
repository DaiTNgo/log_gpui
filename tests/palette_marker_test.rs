use log_gpui::marker::{
    LevelMarker, TextMarker, add_marker, level_marker, marker_at, recolor_marker_at, remove_marker_at,
};
use log_gpui::model::LogLevel;
use log_gpui::palette::{
    HighlightColor, Hue, MAX_RECENT, RecentColors, Shade, Tone, contrast_ratio, palette_cells, palette_rgb,
    text_rgb, text_tone_on,
};
use log_gpui::workspace::log_space::LogFile;

fn red() -> HighlightColor {
    HighlightColor::palette(Hue::Red, Shade::Medium)
}
fn blue() -> HighlightColor {
    HighlightColor::palette(Hue::Blue, Shade::Medium)
}

// ─── Palette ─────────────────────────────────────────────────────────────────

#[test]
fn the_palette_has_thirty_cells() {
    assert_eq!(palette_cells().len(), 30);
}

#[test]
fn text_is_readable_on_every_cell_in_both_themes() {
    for color in palette_cells() {
        for tone in [Tone::Light, Tone::Dark] {
            let bg = color.rgb(tone);
            let fg = text_rgb(color.text_tone(tone));
            assert!(contrast_ratio(bg, fg) >= 4.5, "{color:?} {tone:?} contrast {}", contrast_ratio(bg, fg));
        }
    }
}

#[test]
fn light_and_dark_themes_have_their_own_values() {
    let l = palette_rgb(Hue::Green, Shade::Medium, Tone::Light);
    let d = palette_rgb(Hue::Green, Shade::Medium, Tone::Dark);
    assert_ne!(l, d);
}

#[test]
fn a_dark_custom_color_gets_light_text() {
    let c = HighlightColor::custom("#1A1A40").unwrap();
    assert_eq!(c.text_tone(Tone::Light), Tone::Light);
    assert_eq!(text_tone_on(0xFFF2CC), Tone::Dark);
    assert!(HighlightColor::custom("not a color").is_none());
}

#[test]
fn recent_colors_are_newest_first_without_repeats_and_capped() {
    let mut recent = RecentColors::new();
    recent.push(red());
    recent.push(blue());
    recent.push(red());
    assert_eq!(recent.colors(), &[red(), blue()]);
    for i in 0..20 {
        recent.push(HighlightColor::custom(&format!("#0000{:02X}", i)).unwrap());
    }
    assert_eq!(recent.colors().len(), MAX_RECENT);
    assert_eq!(recent.colors()[0], HighlightColor::custom("#000013").unwrap());
}

// ─── Markers ─────────────────────────────────────────────────────────────────

fn spans(markers: &[TextMarker]) -> Vec<(usize, usize, HighlightColor)> {
    markers.iter().map(|m| (m.range.start, m.range.end, m.color.clone())).collect()
}

#[test]
fn painting_over_an_old_marker_cuts_it() {
    // `abcdef` red, then `cd` blue: `ab` and `ef` stay red.
    let mut m = Vec::new();
    assert!(add_marker(&mut m, 0..6, red()));
    assert!(add_marker(&mut m, 2..4, blue()));
    assert_eq!(spans(&m), vec![(0, 2, red()), (2, 4, blue()), (4, 6, red())]);
}

#[test]
fn an_empty_range_paints_nothing() {
    let mut m = Vec::new();
    assert!(!add_marker(&mut m, 3..3, red()));
    assert!(m.is_empty());
}

#[test]
fn removing_takes_the_whole_marker_under_the_position() {
    let mut m = Vec::new();
    add_marker(&mut m, 0..4, red());
    add_marker(&mut m, 10..14, blue());
    assert!(remove_marker_at(&mut m, 2));
    assert_eq!(spans(&m), vec![(10, 14, blue())], "the other marker stays");
    assert!(!remove_marker_at(&mut m, 5));
}

#[test]
fn recolouring_changes_one_marker() {
    let mut m = Vec::new();
    add_marker(&mut m, 0..4, red());
    add_marker(&mut m, 10..14, red());
    assert!(recolor_marker_at(&mut m, 11, blue()));
    assert_eq!(marker_at(&m, 12).unwrap().color, blue());
    assert_eq!(marker_at(&m, 1).unwrap().color, red());
}

#[test]
fn level_marker_follows_the_meaning_of_the_level() {
    assert_eq!(level_marker(LogLevel::Fatal), Some(LevelMarker::Error));
    assert_eq!(level_marker(LogLevel::Error), Some(LevelMarker::Error));
    assert_eq!(level_marker(LogLevel::from_token("E")), Some(LevelMarker::Error));
    assert_eq!(level_marker(LogLevel::Warn), Some(LevelMarker::Warning));
    assert_eq!(level_marker(LogLevel::Info), None);
    assert_eq!(level_marker(LogLevel::Unknown), None);
}

// ─── Markers stored on a file ────────────────────────────────────────────────

#[test]
fn markers_on_several_lines_are_kept_per_line() {
    let mut file = LogFile::new("a.log".into(), "a.log".into());
    // A drag from the middle of line 3 to the middle of line 5.
    assert!(file.paint_marker(3, "line three", 5..10, red()));
    assert!(file.paint_marker(4, "line four", 0..9, red()));
    assert!(file.paint_marker(5, "line five", 0..4, red()));
    assert_eq!(file.markers.len(), 3);
    assert_eq!(file.markers_on_line(4)[0].range, 0..9);
}

#[test]
fn removing_a_marker_keeps_the_others_on_the_line() {
    let mut file = LogFile::new("a.log".into(), "a.log".into());
    file.paint_marker(1, "abcdefghij", 0..3, red());
    file.paint_marker(1, "abcdefghij", 5..8, blue());
    assert!(file.remove_marker_at(1, "abcdefghij", 1));
    assert_eq!(file.markers_on_line(1).len(), 1);
    assert_eq!(file.markers_on_line(1)[0].color, blue());
}

#[test]
fn editing_a_line_drops_its_markers_only() {
    let mut file = LogFile::new("a.log".into(), "a.log".into());
    file.paint_marker(1, "first line", 0..5, red());
    file.paint_marker(2, "second line", 0..6, red());
    // Line 2 was edited; line 1 was not.
    let dropped = file.prune_markers(&["first line", "second line EDITED"]);
    assert_eq!(dropped, 1);
    assert_eq!(file.markers.len(), 1);
    assert_eq!(file.markers[0].line, 1);
}

#[test]
fn markers_survive_a_save_and_load() {
    let mut file = LogFile::new("a.log".into(), "a.log".into());
    file.paint_marker(42, "payment_id=123 failed", 0..14, red());
    let json = serde_json::to_string(&file).unwrap();
    let back: LogFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.markers, file.markers);
}

#[test]
fn old_whole_line_markers_are_dropped_on_read() {
    let json = r#"{"file_id":"a","display_name":"a","path":null,"added_seq":1,
        "markers":[{"line":3,"marker":"Bookmark","note":"x","content_hash":5},
                   {"line":4,"range":{"start":0,"end":3},"color":{"Palette":{"hue":"Red","shade":"Medium"}},"line_hash":7}]}"#;
    let file: LogFile = serde_json::from_str(json).unwrap();
    assert_eq!(file.markers.len(), 1, "the old marker is not converted");
    assert_eq!(file.markers[0].line, 4);
}
