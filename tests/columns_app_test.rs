//! The Columns section and Field Columns in the real app.

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::column::PinnableColumn;
use log_gpui::model::FieldKey;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::ui::cells::{TextCell, shows_placeholder};

fn json_lines() -> Vec<String> {
    let mut lines: Vec<String> = (0..99)
        .map(|i| {
            let level = if i % 10 == 0 { "error" } else { "info" };
            let module = if i % 2 == 0 { "DB" } else { "API" };
            format!(
                r#"{{"time":"2026-10-19T10:{:02}:{:02}Z","level":"{level}","module":"{module}","msg":"m{i}","user":"u{}","region":"r{}"}}"#,
                i / 60,
                i % 60,
                i % 3,
                i % 2
            )
        })
        .collect();
    lines.push(r#"{"time":"2026-10-19T10:01:40Z","level":"info","module":"DB","msg":"last","user":"svc-billing"}"#.into());
    lines.push("  at Pool.get(Pool.java:10)".into());
    lines
}

fn field(name: &str) -> PinnableColumn {
    PinnableColumn::Field(FieldKey::new(name))
}

fn columns(h: &mut Harness) -> Vec<PinnableColumn> {
    h.view.read_with(&h.cx, |app, _| app.column_layout.columns())
}

fn show(h: &mut Harness, key: &str, shown: bool) {
    h.view.update(&mut h.cx, |app, cx| app.set_column_shown(key, shown, cx));
    h.cx.run_until_parked();
}

fn visible(h: &mut Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.filter_pass.len())
}

#[gpui::test]
fn turning_a_json_field_on_shows_its_column(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    assert!(!columns(&mut h).contains(&field("region")), "Detected Fields start off");
    let detected = h.view.read_with(&h.cx, |app, _| app.column_groups("").detected);
    let region = detected.iter().find(|r| r.key == "region").expect("region is detected");
    assert!(!region.shown);
    assert_eq!(region.lines, 99);

    show(&mut h, "region", true);
    assert_eq!(columns(&mut h).last(), Some(&field("region")));
    h.cx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(900.0)));
    h.cx.run_until_parked();
}

#[gpui::test]
fn a_hidden_column_still_filters(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    show(&mut h, "level", false);
    assert!(!columns(&mut h).contains(&PinnableColumn::Level));
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    h.cx.run_until_parked();
    assert_eq!(visible(&mut h), 10);
}

#[gpui::test]
fn dragging_reorders_field_columns(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    for key in ["msg", "user", "region"] {
        show(&mut h, key, true);
    }
    h.view.update(&mut h.cx, |app, cx| app.move_field_column(2, 0, cx));
    h.cx.run_until_parked();
    let fields: Vec<PinnableColumn> = columns(&mut h).into_iter().filter(|c| c.is_field()).collect();
    assert_eq!(fields, vec![field("region"), field("msg"), field("user")]);
}

#[gpui::test]
fn field_columns_pin_in_view_order_and_auto_fit(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    show(&mut h, "user", true);
    h.view.update(&mut h.cx, |app, cx| {
        app.toggle_pin_column(field("user"), cx);
        app.toggle_pin_column(PinnableColumn::Level, cx);
    });
    let pinned = h.view.read_with(&h.cx, |app, _| app.column_layout.pinned_columns().to_vec());
    assert_eq!(pinned, vec![PinnableColumn::Level, field("user")]);

    h.view.update(&mut h.cx, |app, cx| {
        app.column_layout.set_width(field("user"), gpui::px(300.0));
        app.auto_fit_column(field("user"), cx);
    });
    let (width, density) = h.view.read_with(&h.cx, |app, _| (app.column_layout.width(field("user")), app.density));
    let fitted = log_gpui::column::compute_column_width(field("user"), "svc-billing".len(), density.font_size(), gpui::px(48.0));
    assert_eq!(width, fitted);
}

#[gpui::test]
fn coverage_shows_99_percent(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    let (covered, total) = h.view.read_with(&h.cx, |app, _| app.filter_index.coverage("region"));
    assert_eq!((covered, total), (99, 100));
    assert_eq!(log_gpui::columns::coverage_label(covered, total).as_deref(), Some("99%"));
}

#[gpui::test]
fn renaming_a_column_keeps_filter_and_highlight_rules(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_toggle_value("module", "DB", cx);
        app.add_highlight_rule("module", &["DB".into()], &[], HighlightColor::palette(Hue::Red, Shade::Medium), cx)
            .unwrap();
    });
    h.cx.run_until_parked();
    let before = visible(&mut h);

    h.view.update_in(&mut h.cx, |app, window, cx| app.begin_column_rename("module", false, window, cx));
    h.cx.run_until_parked();
    h.cx.simulate_input("Service");
    h.cx.simulate_keystrokes("enter");
    h.cx.run_until_parked();

    let (name, header, editing) = h.view.read_with(&h.cx, |app, _| {
        (app.column_display_name("module"), app.column_layout.header_text(PinnableColumn::Module).to_string(), app.label_edit.is_some())
    });
    assert_eq!(name, "Service");
    assert_eq!(header, "Service");
    assert!(!editing);
    assert_eq!(visible(&mut h), before);
    let painted = h.view.read_with(&h.cx, |app, _| app.highlight_paints.iter().filter(|p| p.is_some()).count());
    assert!(painted > 0);
}

#[gpui::test]
fn a_label_taken_by_another_column_is_refused_until_changed(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    h.view.update_in(&mut h.cx, |app, window, cx| app.begin_column_rename("region", false, window, cx));
    h.cx.run_until_parked();
    h.cx.simulate_input("user");
    let error_while_typing = h.view.read_with(&h.cx, |app, _| app.label_edit.as_ref().and_then(|e| e.error.clone()));
    assert!(error_while_typing.is_none(), "no error while typing");
    h.cx.simulate_keystrokes("enter");
    h.cx.run_until_parked();
    let error = h.view.read_with(&h.cx, |app, _| app.label_edit.as_ref().and_then(|e| e.error.clone()));
    assert!(error.is_some_and(|e| e.contains("user")));
    h.cx.simulate_input("2");
    h.cx.run_until_parked();
    let error = h.view.read_with(&h.cx, |app, _| app.label_edit.as_ref().and_then(|e| e.error.clone()));
    assert!(error.is_none(), "typing hides the error");
    h.cx.simulate_keystrokes("enter");
    h.cx.run_until_parked();
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.column_display_name("region")), "user2");
}

#[gpui::test]
fn a_continuation_line_shows_the_placeholder_in_field_columns(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    show(&mut h, "user", true);
    let placeholder = h.view.read_with(&h.cx, |app, _| {
        let last = app.all_entries.last().unwrap();
        last.is_continuation() && shows_placeholder(last, TextCell::Field(FieldKey::new("user")))
    });
    assert!(placeholder);
}

#[gpui::test]
fn the_source_switch_is_the_settings_switch(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    show(&mut h, "source", true);
    assert!(h.view.read_with(&h.cx, |app, _| app.source_column_shown()));
    show(&mut h, "source", false);
    assert!(!h.view.read_with(&h.cx, |app, _| app.source_column_shown()));
}

#[gpui::test]
fn manage_columns_opens_the_section_on_the_column(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    h.view.update(&mut h.cx, |app, cx| app.manage_columns(Some("user"), cx));
    let (collapsed, picked) = h.view.read_with(&h.cx, |app, _| {
        (
            app.context_sidebar.is_collapsed(log_gpui::workspace::state::ContextSection::Columns),
            app.columns_highlighted.clone(),
        )
    });
    assert!(!collapsed);
    assert_eq!(picked.as_deref(), Some("user"));
    h.cx.run_until_parked();
}

#[gpui::test]
fn column_settings_survive_a_save(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    show(&mut h, "user", true);
    show(&mut h, "level", false);
    let manifest = h.view.read_with(&h.cx, |app, _| {
        log_gpui::workspace::session::SessionManifest::from_workspace(&app.workspace_state)
    });
    let back = log_gpui::workspace::session::SessionManifest::from_json(&manifest.to_json().unwrap()).unwrap();
    let ws = back.into_workspace();
    let columns = &ws.projects[0].columns;
    assert_eq!(columns.fields, vec!["user"]);
    assert!(!columns.is_shown("level"));
}

#[gpui::test]
fn the_focus_pane_shows_the_columns_of_the_view(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &json_lines());
    show(&mut h, "user", true);
    show(&mut h, "level", false);
    h.view.update(&mut h.cx, |app, cx| app.toggle_pin_column(field("user"), cx));
    let pane = h.view.read_with(&h.cx, |app, _| app.pane_columns()).expect("Parsed View");
    assert_eq!(pane.columns(), columns(&mut h), "same columns, same order");
    assert!(!pane.columns().contains(&PinnableColumn::Level));
    assert!(pane.pinned_columns().is_empty(), "the pane scrolls as one");
    h.view.update(&mut h.cx, |app, _| app.view_mode = log_gpui::workspace::working_set::ViewMode::Raw);
    assert!(h.view.read_with(&h.cx, |app, _| app.pane_columns()).is_none(), "the Raw View lists raw lines");
}
