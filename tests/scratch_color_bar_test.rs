//! The color bar in the Raw View (openspec change
//! `scratch-color-bar-on-demand`): selecting text in a scratch file's editor
//! does not open the bar, since text is selected there to edit it; Color
//! Selection opens it for the selection in any view, and the caret on a
//! Marker still opens it to change that Marker.

mod common;

use common::Harness;
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};
use log_gpui::app::color::ColorTarget;
use log_gpui::keymap::{Command, Shortcut};
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use log_gpui::workspace::working_set::ViewMode;

const TEXT: &str = "error at db\ntimeout here\nok";

fn draw(h: &mut Harness) {
    for i in 0..2 {
        h.cx.simulate_resize(gpui::size(px(900.0 + i as f32), px(700.0)));
        h.cx.run_until_parked();
    }
}

/// A Scratch Log Space with one file holding [`TEXT`], in its Raw View.
fn open_scratch(cx: &mut TestAppContext) -> (Harness, String) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Notes", LogSpaceKind::Scratch, &[], cx).unwrap();
        app.set_view_mode(ViewMode::Raw, cx);
    });
    draw(&mut h);
    let id = h.view.read_with(&h.cx, |app, _| app.raw.editors.keys().next().expect("the file is edited here").clone());
    // Typed, as the user would: the new file's editor has the keyboard.
    for (i, line) in TEXT.split('\n').enumerate() {
        if i > 0 {
            h.cx.simulate_keystrokes("enter");
        }
        h.cx.simulate_input(line);
    }
    draw(&mut h);
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.raw_sections()[0].lines.len()), 3);
    (h, id)
}

fn select(h: &mut Harness, id: &str, range: std::ops::Range<usize>) {
    let state = h.view.read_with(&h.cx, |app, _| app.raw.editors[id].state.clone());
    h.cx.update(|_, cx| state.update(cx, |s, cx| s.set_selected_range(range, cx)));
    draw(h);
}

fn run(h: &mut Harness, command: Command) {
    let view = h.view.clone();
    h.cx.update(|window, cx| view.update(cx, |app, cx| app.execute_command(command, window, cx)));
    draw(h);
}

fn bar(h: &Harness) -> Option<ColorTarget> {
    h.view.read_with(&h.cx, |app, _| app.color_bar.as_ref().map(|b| b.target.clone()))
}

#[gpui::test]
fn color_selection_is_a_command_with_a_default_shortcut() {
    assert_eq!(Command::from_id("marker.color_selection"), Some(Command::ColorSelection));
    assert_eq!(Command::ColorSelection.default_shortcuts(), ["cmd-shift-h"]);
    assert!(Shortcut::parse("cmd-shift-h").is_some());
}

#[gpui::test]
fn selecting_in_a_scratch_editor_does_not_open_the_bar(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    // Across lines 1 and 2, then everything.
    select(&mut h, &id, 6..19);
    assert_eq!(bar(&h), None);
    select(&mut h, &id, 0..TEXT.len());
    assert_eq!(bar(&h), None);
}

#[gpui::test]
fn color_selection_paints_the_text_selected_in_a_scratch_editor(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    // "timeout" on line 2.
    select(&mut h, &id, 12..19);
    run(&mut h, Command::ColorSelection);
    let target = bar(&h);
    let Some(ColorTarget::RawMark { pieces, .. }) = &target else { panic!("the bar to paint it: {target:?}") };
    assert_eq!(pieces.iter().map(|(line, range, _)| (*line, range.clone())).collect::<Vec<_>>(), [(2, 0..7)]);

    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(HighlightColor::palette(Hue::Red, Shade::Medium), false, cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        let file = &app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().files[0];
        let marks = file.markers_on_line(2);
        assert_eq!(marks.iter().map(|m| m.range.clone()).collect::<Vec<_>>(), [0..7]);
    });
}

#[gpui::test]
fn the_bar_color_selection_opened_closes_when_the_selection_changes(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    select(&mut h, &id, 12..19);
    run(&mut h, Command::ColorSelection);
    assert!(matches!(bar(&h), Some(ColorTarget::RawMark { .. })), "stays while the text stays selected");
    select(&mut h, &id, 0..5);
    assert_eq!(bar(&h), None);
}

#[gpui::test]
fn the_caret_on_a_marker_in_a_scratch_editor_still_opens_the_bar(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    select(&mut h, &id, 12..19);
    run(&mut h, Command::ColorSelection);
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(HighlightColor::palette(Hue::Blue, Shade::Medium), false, cx));
    draw(&mut h);
    select(&mut h, &id, 14..14);
    let target = bar(&h);
    assert!(matches!(target, Some(ColorTarget::RawRecolor { line: 2, byte: 2, .. })), "{target:?}");
}

#[gpui::test]
fn color_selection_without_a_selection_opens_nothing(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    select(&mut h, &id, 3..3);
    run(&mut h, Command::ColorSelection);
    assert_eq!(bar(&h), None);
}

#[gpui::test]
fn color_selection_opens_the_bar_for_a_selection_in_the_read_only_list(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let path = h.dir.path().join("a.log");
    std::fs::write(&path, common::sample_lines(20).join("\n")).unwrap();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Raw", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.toggle_view_mode(cx));
    draw(&mut h);
    // Drag across line 1, then close the bar it opened.
    let (from, to) = h.view.read_with(&h.cx, |app, _| {
        let b = app.raw.list_bounds.get();
        let row_h = app.text_scale().row_height();
        (point(b.origin.x + px(80.0), b.origin.y + px(row_h / 2.0)), point(b.origin.x + px(200.0), b.origin.y + px(row_h / 2.0)))
    });
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    assert!(matches!(bar(&h), Some(ColorTarget::RawMark { .. })), "a drag still opens it here");
    h.view.update(&mut h.cx, |app, cx| {
        app.color_bar = None;
        cx.notify();
    });
    run(&mut h, Command::ColorSelection);
    assert!(matches!(bar(&h), Some(ColorTarget::RawMark { .. })));
}


#[gpui::test]
fn the_shortcut_reaches_the_app_while_the_editor_has_the_keyboard(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    select(&mut h, &id, 12..19);
    h.cx.simulate_keystrokes("cmd-shift-h");
    draw(&mut h);
    assert!(matches!(bar(&h), Some(ColorTarget::RawMark { .. })), "{:?}", bar(&h));
    let line = h.view.read_with(&h.cx, |app, _| app.raw_sections()[0].lines.get(1).map(|l| l.to_string()));
    assert_eq!(line.as_deref(), Some("timeout here"), "the editor typed nothing over the selection");
}

#[gpui::test]
fn color_selection_everywhere_makes_a_text_highlight_from_a_scratch_editor(cx: &mut TestAppContext) {
    let (mut h, id) = open_scratch(cx);
    select(&mut h, &id, 12..19);
    run(&mut h, Command::ColorSelection);
    // Alt+click on a color is "Everywhere".
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(HighlightColor::palette(Hue::Green, Shade::Medium), true, cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.text_highlights().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["timeout"]);
    });
}
