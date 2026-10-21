//! Log Formats in the real app: choosing one for a file, its fields as
//! columns, reading files again, and the editor.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::column::PinnableColumn;
use log_gpui::log_format::{BUILT_IN, FormatChoice};
use log_gpui::model::FieldKey;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::text_selection::{TextPos, TextSelection};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::time::Duration;

const ACCESS: &str = r#"10.0.4.17 - - [20/Oct/2026:10:00:01 +0000] "GET /health HTTP/1.1" 200 2 "-" "kube-probe/1.29"
203.0.113.9 - alice [20/Oct/2026:10:00:03 +0000] "POST /api/orders HTTP/1.1" 201 512 "https://shop.example/cart" "Mozilla/5.0"
198.51.100.23 - - [20/Oct/2026:10:00:05 +0000] "GET /static/app.js HTTP/2.0" 304 0 "https://shop.example/" "Mozilla/5.0"
"#;
const APP: &str = "2026-10-20 10:00:02 [INFO] [api] order received\n2026-10-20 10:00:04 [WARN] [api] slow gateway\n";

fn open(cx: &mut TestAppContext) -> Harness {
    let mut h = common::open_app(cx);
    let access = h.dir.path().join("access.log");
    let app = h.dir.path().join("app.log");
    std::fs::write(&access, ACCESS).unwrap();
    std::fs::write(&app, APP).unwrap();
    let sources = [
        ImportSource::Disk(access.to_string_lossy().into_owned()),
        ImportSource::Disk(app.to_string_lossy().into_owned()),
    ];
    h.view.update(&mut h.cx, |a, cx| a.create_log_space("Web", LogSpaceKind::Imported, &sources, cx));
    h.cx.run_until_parked();
    h
}

fn access_id(h: &mut Harness) -> String {
    h.view.read_with(&h.cx, |a, _| {
        a.workspace_state.active_project().unwrap().log_spaces[0].files.iter().find(|f| f.display_name.contains("access")).unwrap().file_id.clone()
    })
}

fn add_nginx(h: &mut Harness) -> String {
    let id = h.view.update(&mut h.cx, |a, cx| a.save_log_format(None, "nginx", BUILT_IN[0].1, vec![], cx));
    h.cx.run_until_parked();
    id
}

fn choose_for_access(h: &mut Harness, format: &str) {
    let file = access_id(h);
    h.view.update(&mut h.cx, |a, cx| a.set_file_format(&file, Some(FormatChoice::Format(format.into())), cx));
    h.cx.run_until_parked();
}

fn field(name: &str) -> PinnableColumn {
    PinnableColumn::Field(FieldKey::new(name))
}

fn columns(h: &mut Harness) -> Vec<PinnableColumn> {
    h.view.read_with(&h.cx, |a, _| a.column_layout.columns())
}

#[gpui::test]
fn the_nginx_template_gives_columns_and_interleaves_by_time(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let id = add_nginx(&mut h);
    choose_for_access(&mut h, &id);
    let cols = columns(&mut h);
    assert!(cols.contains(&field("client")) && cols.contains(&field("status")), "{cols:?}");
    let order: Vec<String> = h.view.read_with(&h.cx, |a, _| {
        a.all_entries.iter().map(|e| e.raw_line.chars().take(10).collect()).collect()
    });
    assert_eq!(order, vec!["10.0.4.17 ", "2026-10-20", "203.0.113.", "2026-10-20", "198.51.100"]);
    let groups = h.view.read_with(&h.cx, |a, _| a.column_groups(""));
    let names: Vec<&str> = groups.from_format.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(names, vec!["client", "user", "request", "status", "bytes", "referrer", "agent"]);

    h.view.update(&mut h.cx, |a, cx| a.set_column_shown("referrer", false, cx));
    h.cx.run_until_parked();
    let pane = h.view.read_with(&h.cx, |a, _| a.pane_columns().unwrap().columns());
    assert!(!pane.contains(&field("referrer")));
    assert!(pane.contains(&field("agent")), "Log Format fields are on in the Focus Pane by default");
    assert!(columns(&mut h).contains(&field("referrer")), "the Canvas is not changed");
}

#[gpui::test]
fn a_glob_applies_the_format_and_auto_detect_turns_it_off(cx: &mut TestAppContext) {
    let mut h = open(cx);
    h.view.update(&mut h.cx, |a, cx| {
        a.save_log_format(None, "nginx", BUILT_IN[0].1, vec!["access*.log".into()], cx);
    });
    h.cx.run_until_parked();
    assert!(columns(&mut h).contains(&field("client")));
    let space = h.view.read_with(&h.cx, |a, _| a.workspace_state.active_project().unwrap().log_spaces[0].id.clone());
    h.view.update(&mut h.cx, |a, cx| a.set_space_format(&space, Some(FormatChoice::AutoDetect), cx));
    h.cx.run_until_parked();
    assert!(!columns(&mut h).contains(&field("client")));
}

#[gpui::test]
fn changing_the_format_keeps_markers_and_filter_conditions(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let id = add_nginx(&mut h);
    choose_for_access(&mut h, &id);
    h.view.update(&mut h.cx, |a, cx| {
        let row = a.all_entries.iter().position(|e| e.raw_line.starts_with("203.0.113.9")).unwrap();
        a.text_selection = Some(TextSelection::new(TextPos::new(row, 0), TextPos::new(row, 11)));
        a.mark_selection(HighlightColor::palette(Hue::Orange, Shade::Medium), cx);
        a.filter_toggle_value("client", "203.0.113.9", cx);
    });
    h.cx.run_until_parked();
    let file = access_id(&mut h);
    h.view.update(&mut h.cx, |a, cx| a.set_file_format(&file, Some(FormatChoice::AutoDetect), cx));
    h.cx.run_until_parked();
    let (marked, kept, passing) = h.view.read_with(&h.cx, |a, _| {
        let e = a.all_entries.iter().find(|e| e.raw_line.starts_with("203.0.113.9")).unwrap();
        (e.markers.len(), a.filter_state.column("client").is_some_and(|c| c.is_active()), a.filter_pass.len())
    });
    assert_eq!(marked, 1, "the Marker stays on its line");
    assert!(kept, "the condition stays");
    assert_eq!(passing, 0, "but no line has the field any more");
}

#[gpui::test]
fn the_editor_keeps_the_last_preview_while_a_token_is_half_typed(cx: &mut TestAppContext) {
    let mut h = open(cx);
    choose_for_access(&mut h, "none");
    h.view.update_in(&mut h.cx, |a, window, cx| {
        a.open_format_editor(None, None, window, cx);
        a.use_built_in_format(0, window, cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |a, _| {
        let e = a.format_editor.as_ref().unwrap();
        let p = e.preview.as_ref().unwrap();
        assert!(e.can_save());
        assert_eq!(p.lines.len(), 5);
        assert_eq!(p.matched(), 3, "{}", p.summary());
        assert!(p.first_unmatched.is_some());
    });
    h.view.update_in(&mut h.cx, |a, window, cx| {
        let input = a.format_editor.as_ref().unwrap().template_input.clone();
        input.update(cx, |s, cx| s.focus(window, cx));
    });
    h.cx.simulate_keystrokes("cmd-right");
    h.cx.simulate_input(" %{IP");
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |a, _| {
        let e = a.format_editor.as_ref().unwrap();
        assert!(e.stale && !e.can_save());
        assert!(e.template.visible_error().is_none(), "no error while typing");
    });
    h.cx.executor().advance_clock(Duration::from_millis(600));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |a, cx| {
        let e = a.format_editor.as_ref().unwrap();
        let err = e.template.visible_error().expect("the error shows after a pause");
        let text = e.template_input.read(cx).value().to_string();
        assert_eq!(&text[err.span.clone()], "%{IP");
        assert!(err.message.contains("not closed"));
    });
}

#[gpui::test]
fn deleting_a_format_in_use_asks_and_goes_back_to_auto_detect(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let id = add_nginx(&mut h);
    choose_for_access(&mut h, &id);
    h.view.update(&mut h.cx, |a, cx| a.request_delete_log_format(&id, cx));
    let uses = h.view.read_with(&h.cx, |a, _| a.log_format_uses(&id));
    assert_eq!(uses.len(), 1);
    assert!(uses[0].place.contains("access.log"));
    assert!(h.view.read_with(&h.cx, |a, _| a.confirm.is_some()));
    h.view.update(&mut h.cx, |a, cx| a.accept_confirm(cx));
    h.cx.run_until_parked();
    assert!(h.view.read_with(&h.cx, |a, _| a.log_formats().is_empty()));
    assert!(!columns(&mut h).contains(&field("client")));
}

fn space_id(h: &mut Harness) -> String {
    h.view.read_with(&h.cx, |a, _| a.workspace_state.active_project().unwrap().log_spaces[0].id.clone())
}

fn file_id_of(h: &mut Harness, name: &str) -> String {
    h.view.read_with(&h.cx, |a, _| {
        a.workspace_state.active_project().unwrap().log_spaces[0].files.iter().find(|f| f.display_name.contains(name)).unwrap().file_id.clone()
    })
}

#[gpui::test]
fn log_format_from_a_file_opens_the_format_that_applies_and_saving_changes_the_columns(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let id = add_nginx(&mut h);
    choose_for_access(&mut h, &id);
    let (space, file) = (space_id(&mut h), access_id(&mut h));
    h.view.update_in(&mut h.cx, |a, window, cx| a.edit_log_format(&space, Some(&file), window, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |a, cx| {
        let e = a.format_editor.as_ref().unwrap();
        assert_eq!(e.id.as_deref(), Some(id.as_str()));
        assert_eq!(e.template_input.read(cx).value().to_string(), BUILT_IN[0].1);
        assert!(e.preview.is_some());
    });
    h.view.update_in(&mut h.cx, |a, window, cx| {
        let input = a.format_editor.as_ref().unwrap().template_input.clone();
        input.update(cx, |s, cx| s.set_value(BUILT_IN[0].1.replace("client", "peer"), window, cx));
    });
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |a, cx| a.save_format_editor(cx));
    h.cx.run_until_parked();
    let cols = columns(&mut h);
    assert!(cols.contains(&field("peer")) && !cols.contains(&field("client")), "{cols:?}");
}

#[gpui::test]
fn log_format_from_a_space_opens_the_format_chosen_for_it(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let id = add_nginx(&mut h);
    let space = space_id(&mut h);
    h.view.update(&mut h.cx, |a, cx| a.set_space_format(&space, Some(FormatChoice::Format(id.clone())), cx));
    h.cx.run_until_parked();
    h.view.update_in(&mut h.cx, |a, window, cx| a.edit_log_format(&space, None, window, cx));
    h.view.read_with(&h.cx, |a, _| assert_eq!(a.format_editor.as_ref().unwrap().id.as_deref(), Some(id.as_str())));
}

#[gpui::test]
fn a_files_own_format_wins_over_its_log_spaces(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let nginx = add_nginx(&mut h);
    let other = h.view.update(&mut h.cx, |a, cx| a.save_log_format(None, "app", "%{TIMESTAMP:timestamp} %{GREEDY:message}", vec![], cx));
    let space = space_id(&mut h);
    h.view.update(&mut h.cx, |a, cx| a.set_space_format(&space, Some(FormatChoice::Format(other.clone())), cx));
    choose_for_access(&mut h, &nginx);
    let (access, app) = (access_id(&mut h), file_id_of(&mut h, "app.log"));
    h.view.update_in(&mut h.cx, |a, window, cx| a.edit_log_format(&space, Some(&access), window, cx));
    h.view.read_with(&h.cx, |a, _| assert_eq!(a.format_editor.as_ref().unwrap().id.as_deref(), Some(nginx.as_str())));
    h.view.update_in(&mut h.cx, |a, window, cx| a.edit_log_format(&space, Some(&app), window, cx));
    h.view.read_with(&h.cx, |a, _| assert_eq!(a.format_editor.as_ref().unwrap().id.as_deref(), Some(other.as_str())));
}

#[gpui::test]
fn with_no_format_applied_the_editor_opens_new_with_a_sample_line_and_applies_to_the_file(cx: &mut TestAppContext) {
    let mut h = open(cx);
    let (space, app) = (space_id(&mut h), file_id_of(&mut h, "app.log"));
    h.view.update_in(&mut h.cx, |a, window, cx| a.edit_log_format(&space, Some(&app), window, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |a, cx| {
        let e = a.format_editor.as_ref().unwrap();
        assert_eq!(e.id, None);
        assert_eq!(e.template_input.read(cx).value().to_string(), "2026-10-20 10:00:02 [INFO] [api] order received");
        assert_eq!(e.preview.as_ref().unwrap().matched(), 1);
    });
    h.view.update_in(&mut h.cx, |a, window, cx| {
        let name = a.format_editor.as_ref().unwrap().name_input.clone();
        name.update(cx, |s, cx| s.focus(window, cx));
    });
    h.cx.simulate_input("mine");
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |a, cx| a.save_format_editor(cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |a, _| {
        assert!(matches!(a.file_format_choice(&app), Some(FormatChoice::Format(_))), "applied to the file");
        assert_eq!(a.space_format_choice(&space), None, "not to the whole Log Space");
    });
}
