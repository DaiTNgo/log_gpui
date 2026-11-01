//! Not a gate: prints what Syntax Color costs in a scratch editor and in the
//! read-only Raw list on big files. Run with
//! `cargo test --release --test syntax_perf_test -- --ignored --nocapture`.

mod common;

use common::{Harness, open_app};
use gpui::{TestAppContext, px};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use log_gpui::workspace::working_set::ViewMode;
use std::time::Instant;

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

fn big_text(lines: usize) -> String {
    (0..lines)
        .map(|i| format!("2026-10-20 10:00:01.{:03} INFO [worker-{}] com.acme.Gateway: charged user=alice order={i} took={}ms id=3f2b8c1e-6a4d-4e0b-9a52-1c7d0e5f8a90\n", i % 1000, i % 8, i % 90))
        .collect()
}

fn draw(h: &mut Harness) {
    for i in 0..3 {
        h.cx.simulate_resize(gpui::size(px(900.0 + i as f32), px(700.0)));
        h.cx.run_until_parked();
    }
}

fn leave(h: &mut Harness) {
    let path = h.dir.path().join("other.log");
    std::fs::write(&path, "2026-10-20 10:00:01 INFO other\n").unwrap();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Other", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();
}

fn measure(cx: &mut TestAppContext, lines: usize, kind: LogSpaceKind) {
    measure_with(cx, lines, kind, true)
}

fn measure_with(cx: &mut TestAppContext, lines: usize, kind: LogSpaceKind, syntax_at_open: bool) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let name = format!("Perf {lines} {kind:?}");
    let sources = Vec::new();
    let space = h.view.update(&mut h.cx, |app, cx| app.create_log_space(&name, kind, &sources, cx).unwrap());
    h.cx.run_until_parked();
    let path = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_space(&space).unwrap().files[0].path.clone().unwrap()
    });
    std::fs::write(&path, big_text(lines)).unwrap();
    leave(&mut h);
    let tab = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.tabs().iter().position(|t| t.target.space_id() == Some(space.as_str())).unwrap()
    });
    h.view.update(&mut h.cx, |app, cx| app.set_show_syntax_color(syntax_at_open, cx));
    let before = rss_mb();
    h.view.update(&mut h.cx, |app, cx| {
        app.switch_to_tab(tab, cx);
        app.set_view_mode(ViewMode::Raw, cx);
    });
    h.cx.run_until_parked();
    draw(&mut h);
    let after_open = rss_mb();
    println!("[{lines} lines, {kind:?}, syntax {syntax_at_open}] RSS {before:.0} MB -> {after_open:.0} MB after opening Raw");
    for syntax in [false, true] {
        let start = Instant::now();
        let runs = 5;
        for _ in 0..runs {
            h.view.update(&mut h.cx, |app, cx| app.set_show_syntax_color(syntax, cx));
        }
        println!("[{lines} lines, {kind:?}] one repaint (set_show_syntax_color), syntax {syntax}: {:?} per call", start.elapsed() / runs);
    }
    draw(&mut h);
    println!("[{lines} lines, {kind:?}] RSS at the end: {:.0} MB", rss_mb());
}

#[gpui::test]
#[ignore]
fn scratch_editor_cost_100k(cx: &mut TestAppContext) {
    measure(cx, 100_000, LogSpaceKind::Scratch);
}

#[gpui::test]
#[ignore]
fn scratch_editor_cost_500k(cx: &mut TestAppContext) {
    measure(cx, 500_000, LogSpaceKind::Scratch);
}

#[gpui::test]
#[ignore]
fn control_scratch_100k_syntax_off(cx: &mut TestAppContext) {
    measure_with(cx, 100_000, LogSpaceKind::Scratch, false);
}

#[gpui::test]
#[ignore]
fn control_static_100k(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    common::no_warm_tabs(&mut h);
    let path = h.dir.path().join("big.log");
    std::fs::write(&path, big_text(100_000)).unwrap();
    let before = rss_mb();
    let source = ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Big Static", LogSpaceKind::Static, &[source], cx);
        app.set_view_mode(ViewMode::Raw, cx);
    });
    h.cx.run_until_parked();
    draw(&mut h);
    println!("[100000 lines, Static list] RSS {before:.0} MB -> {:.0} MB after opening Raw", rss_mb());
}
