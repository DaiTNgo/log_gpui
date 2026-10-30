//! The Raw View of read-only files is one virtualized list (openspec change
//! `line-offset-index`, task 8.1): its lines are read through the files'
//! Line Index as they are drawn, nothing keeps a file's whole text, and text
//! is selected with the mouse to copy it or paint a Marker.

mod common;

use common::Harness;
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};
use log_gpui::app::color::ColorTarget;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

fn open_raw(cx: &mut TestAppContext, files: &[(&str, usize)]) -> Harness {
    let mut h = common::open_app(cx);
    let sources: Vec<_> = files
        .iter()
        .map(|(name, n)| {
            let path = h.dir.path().join(name);
            std::fs::write(&path, common::sample_lines(*n).join("\n")).unwrap();
            ImportSource::Disk(path.to_string_lossy().into_owned())
        })
        .collect();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Raw", LogSpaceKind::Static, &sources, cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.toggle_view_mode(cx));
    draw(&mut h);
    h
}

fn draw(h: &mut Harness) {
    for i in 0..2 {
        h.cx.simulate_resize(gpui::size(px(1200.0 + i as f32), px(800.0)));
        h.cx.run_until_parked();
    }
}

/// The window position of byte `byte` of the line on row `row` of the list.
fn at(h: &Harness, row: usize, byte: usize) -> gpui::Point<gpui::Pixels> {
    h.view.read_with(&h.cx, |app, _| {
        let bounds = app.raw.list_bounds.get();
        let row_h = app.text_scale().row_height();
        let font = app.text_scale().font_size();
        let (sections, _) = app.raw_rows();
        let max = sections.iter().map(|s| s.lines.len()).max().unwrap_or(0);
        let x = log_gpui::app::raw_list::gutter_width(max, font)
            + log_gpui::app::raw_list::TEXT_PAD
            + byte as f32 * log_gpui::ui::mono::char_width(font);
        point(bounds.origin.x + px(x), bounds.origin.y + px(row as f32 * row_h + row_h / 2.0))
    })
}

#[gpui::test]
fn read_only_files_are_one_list_that_keeps_no_copy_of_their_text(cx: &mut TestAppContext) {
    let h = open_raw(cx, &[("a.log", 3000)]);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.raw.editors.is_empty(), "no editor holds the text");
        assert!(app.raw_is_list());
        let (sections, rows) = app.raw_rows();
        assert_eq!(rows.len(), 3000, "one file: no header");
        let file = app.parsed_files[&sections[0].file_id].indexed.clone().expect("read through its Line Index");
        assert!(file.cached_entry(2500).is_none(), "only the lines drawn were read");
    });
}

#[gpui::test]
fn a_drag_selects_text_to_paint_a_marker(cx: &mut TestAppContext) {
    let mut h = open_raw(cx, &[("a.log", 50)]);
    // "2026-10-19 10:00:01 INFO line 1": select "INFO" on line 2 through 3.
    let (from, to) = (at(&h, 1, 20), at(&h, 2, 24));
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    let (target, copied) = h.view.read_with(&h.cx, |app, _| {
        (app.color_bar.as_ref().map(|b| b.target.clone()), app.raw_selected_text())
    });
    assert_eq!(copied.as_deref(), Some("INFO line 2\n2026-10-19 10:00:03 INFO"));
    let Some(ColorTarget::RawMark { pieces, .. }) = target else { panic!("the bar to paint it: {target:?}") };
    assert_eq!(pieces.iter().map(|(line, range, _)| (*line, range.clone())).collect::<Vec<_>>(), [(2, 20..31), (3, 0..24)]);

    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(HighlightColor::palette(Hue::Red, Shade::Medium), cx));
    h.view.read_with(&h.cx, |app, _| {
        let file = &app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().files[0];
        assert_eq!(file.markers.len(), 2, "a Marker on each line");
        assert_eq!(app.raw.list_selection, None);
    });
}

#[gpui::test]
fn a_click_makes_the_line_current_and_on_a_marker_offers_to_change_it(cx: &mut TestAppContext) {
    let mut h = open_raw(cx, &[("a.log", 50)]);
    h.view.update(&mut h.cx, |app, cx| {
        let project = app.workspace_state.active_project_mut().unwrap();
        let file = &mut project.log_spaces.last_mut().unwrap().files[0];
        assert!(file.paint_marker(5, "2026-10-19 10:00:05 INFO line 5", 20..24, HighlightColor::palette(Hue::Blue, Shade::Medium)));
        app.reapply_markers();
        cx.notify();
    });
    let p = at(&h, 4, 21);
    h.cx.simulate_click(p, Modifiers::none());
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.current_line().map(|l| l.line), Some(5));
        let target = app.color_bar.as_ref().map(|b| b.target.clone());
        assert!(matches!(target, Some(ColorTarget::RawRecolor { line: 5, .. })), "{target:?}");
    });
}

#[gpui::test]
fn search_finds_and_reveals_a_match_in_a_later_section(cx: &mut TestAppContext) {
    let mut h = open_raw(cx, &[("a.log", 200), ("b.log", 200)]);
    h.view.update(&mut h.cx, |app, cx| app.trigger_search_query("\"line 150\"", cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.raw.matches.len(), 2, "one per section");
        let (_, rows) = app.raw_rows();
        // A header per section: section 0 is rows 0..=200.
        assert_eq!(rows.row(201), Some(log_gpui::app::raw_list::RawRow::Header { section: 1 }));
        assert_eq!(app.current_line().map(|l| l.line), Some(150));
    });
    h.view.update(&mut h.cx, |app, cx| app.jump_next_match(cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        let current = app.current_line().unwrap();
        assert_eq!(current.line, 150);
        let (sections, rows) = app.raw_rows();
        assert_eq!(*current.file_id, *sections[1].file_id, "the Match of the second file");
        let row = rows.index_of_line(1, 149).unwrap();
        let top = -f32::from(app.raw.scroll.0.borrow().base_handle.offset().y) / app.text_scale().row_height();
        assert!((top as usize..top as usize + 40).contains(&row), "row {row} in sight from {top}");
    });
}
