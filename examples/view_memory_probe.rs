//! The memory an open view holds per line (lean-view-memory, spec
//! `view-memory`): `view_memory_probe [lines]` (3,000,000 by default). Run
//! it in release: `cargo run --release --example view_memory_probe`.
//!
//! Three views, each in a process of its own: a File View, a Log Space of
//! one file, and a Log Space of three files of `lines / 3` lines with their
//! timestamps interleaved. For each it prints what malloc hands out more
//! once the view is open, then frees the view's parts one at a time and
//! prints what each held. "OVER" marks a view past its limit (50 B/line
//! for one file, 60 B/line for several). The system allocator is used, so
//! `malloc_zone_statistics` counts the bytes in use, not what an allocator
//! keeps.
//!
//! `--view file|space1|space3` runs one view in this process.

#[path = "../tests/common/mod.rs"]
mod common;

use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::io::Write;
use std::time::Instant;

/// Bytes malloc hands out now, in MB.
#[cfg(target_os = "macos")]
fn in_use_mb() -> f64 {
    #[repr(C)]
    #[derive(Default)]
    struct Stats {
        blocks_in_use: u32,
        size_in_use: usize,
        max_size_in_use: usize,
        size_allocated: usize,
    }
    unsafe extern "C" {
        fn malloc_zone_statistics(zone: *mut std::ffi::c_void, stats: *mut Stats);
    }
    let mut stats = Stats::default();
    // SAFETY: a null zone sums every zone into `stats`.
    unsafe { malloc_zone_statistics(std::ptr::null_mut(), &mut stats) };
    stats.size_in_use as f64 / 1e6
}

#[cfg(not(target_os = "macos"))]
fn in_use_mb() -> f64 {
    0.0
}

fn line(i: usize) -> String {
    let levels = ["INFO", "DEBUG", "WARN", "ERROR"];
    let s = i % 86_400;
    format!(
        "2026-09-05 {:02}:{:02}:{:02}.{:03} [{}] [api] request {} handled user={} took {}ms\n",
        s / 3600,
        (s / 60) % 60,
        s % 60,
        i % 1000,
        levels[i % 4],
        i,
        i % 977,
        i % 300
    )
}

/// `n` lines, line `k` of the file being line `k * step + offset` of one
/// timeline, so files written with the same step interleave.
fn write_log(path: &std::path::Path, n: usize, step: usize, offset: usize) {
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    for k in 0..n {
        out.write_all(line(k * step + offset).as_bytes()).unwrap();
    }
}

fn run(view: &str, n: usize) {
    let mut cx = gpui::TestAppContext::single();
    let mut h = common::open_app(&mut cx);
    let files = if view == "space3" { 3 } else { 1 };
    let sources: Vec<ImportSource> = (0..files)
        .map(|f| {
            let path = h.dir.path().join(format!("f{f}.log"));
            write_log(&path, n / files, files, f);
            ImportSource::Disk(path.to_string_lossy().into_owned())
        })
        .collect();
    let lines = n / files * files;
    let base = in_use_mb();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("probe", LogSpaceKind::Static, &sources, cx));
    h.cx.run_until_parked();
    if view == "file" {
        let (space, file) = h.view.read_with(&h.cx, |app, _| {
            let project = app.workspace_state.active_project().unwrap();
            let space = project.working_set.active_tab().unwrap().target.space_id().unwrap().to_string();
            let file = project.log_space(&space).unwrap().files[0].file_id.clone();
            (space, file)
        });
        h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space, &file, cx));
        h.cx.run_until_parked();
    }
    let open = in_use_mb() - base;
    let per_line = open * 1e6 / lines as f64;
    let limit = if files == 1 { 50.0 } else { 60.0 };
    println!(
        "== {view}: {lines} lines, open +{open:.1} MB, {per_line:.1} B/line (limit {limit:.0}){}",
        if per_line > limit { "  OVER" } else { "" }
    );

    if view == "space3" {
        // The time a Filter on Level takes over every row (the mark for
        // "Filter slower" in design.md).
        h.view.update(&mut h.cx, |app, _| app.filter_state.toggle_value("level", "error"));
        let (took, passing) = h.view.read_with(&h.cx, |app, _| {
            let t = Instant::now();
            let pass = app.filter_state.evaluate(&app.index(), &app.rows());
            (t.elapsed(), pass.len())
        });
        println!("   Filter Level = ERROR over every row: {took:.2?} ({passing} rows pass)");
        h.view.update(&mut h.cx, |app, _| app.filter_state.toggle_value("level", "error"));
    }

    println!("   freed by taking each part out:");
    macro_rules! part {
        ($name:literal, $take:expr) => {{
            let before = in_use_mb();
            let gone = h.view.update(&mut h.cx, $take);
            drop(gone);
            let freed = before - in_use_mb();
            println!("     {:<18} {:>7.1} MB  {:>5.1} B/line", $name, freed, freed * 1e6 / lines as f64);
        }};
    }
    part!("filter_pass", |app, _| std::mem::take(&mut app.filter_pass));
    part!("focus_rows", |app, _| std::mem::take(&mut app.focus_rows));
    part!("highlight_paints", |app, _| std::mem::take(&mut app.highlight_paints));
    part!("histogram", |app, _| std::mem::take(&mut app.histogram));
    part!("filter_index", |app, _| std::mem::take(&mut app.filter_index));
    part!("view", |app, _| std::mem::take(&mut app.view));
    part!("parsed_files", |app, _| std::mem::take(&mut app.parsed_files));
    println!("     {:<18} {:>7.1} MB", "left", in_use_mb() - base);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.iter().skip(1).find_map(|a| a.parse().ok()).unwrap_or(3_000_000);
    if let Some(at) = args.iter().position(|a| a == "--view") {
        return run(&args[at + 1], n);
    }
    // Each view in a fresh process: what one leaves in malloc does not
    // count toward the next.
    for view in ["file", "space1", "space3"] {
        let status = std::process::Command::new(&args[0]).args([n.to_string().as_str(), "--view", view]).status().unwrap();
        assert!(status.success(), "{view} failed");
    }
}
