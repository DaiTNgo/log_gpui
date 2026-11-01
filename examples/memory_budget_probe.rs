//! What the memory-budget change costs and saves: `memory_budget_probe
//! [lines]` (3,000,000 by default). Run it in release:
//! `cargo run --release --example memory_budget_probe`.
//!
//! - Switching back to a tab left, Warm (budget 512 MB) and Dormant
//!   (budget 0), with the RSS each keeps.
//! - How close `IndexedFile::memory_bytes` is to the RSS a file adds.
//! - The bytes the Row Cache keeps after reading every line of a Log Space
//!   of 50 files.
//! - The time the UI thread takes choosing a Scratch tab of 50 MB and a tab
//!   of a ~500 MB Archive Entry, next to the time of the work in the
//!   background (what the UI thread did before this change).
//!
//! This binary uses the system allocator; `mimalloc_probe` runs the same
//! with mimalloc, as the app does. `--cycle` opens 8 large files in a row,
//! `--budget N` only switches tabs with that budget, `--malloc` shows what
//! the system allocator gives back with no app code.

#[path = "../tests/common/mod.rs"]
mod common;

use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::io::Write;
use std::time::Instant;

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1024.0
}

/// Bytes malloc hands out now, in MB: what is in use, not what the
/// allocator keeps (RSS).
#[cfg(target_os = "macos")]
fn malloc_in_use_mb() -> f64 {
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
    if std::env::var_os("PROBE_ZONES").is_some() {
        eprintln!("    (malloc zones hold {:.0} MB, {} blocks in use)", stats.size_allocated as f64 / 1e6, stats.blocks_in_use);
    }
    stats.size_in_use as f64 / 1e6
}

#[cfg(not(target_os = "macos"))]
fn malloc_in_use_mb() -> f64 {
    0.0
}

/// The process's physical footprint in MB, as Activity Monitor shows it:
/// unlike RSS, without the pages malloc freed and marked reusable.
#[cfg(target_os = "macos")]
fn footprint_mb() -> f64 {
    let mut info = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
    // SAFETY: `info` is the buffer of the flavor asked for.
    let ok = unsafe { libc::proc_pid_rusage(std::process::id() as i32, libc::RUSAGE_INFO_V2, info.as_mut_ptr().cast()) };
    if ok != 0 {
        return 0.0;
    }
    // SAFETY: filled by the call above.
    unsafe { info.assume_init() }.ri_phys_footprint as f64 / 1e6
}

#[cfg(not(target_os = "macos"))]
fn footprint_mb() -> f64 {
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

fn write_log(path: &std::path::Path, n: usize) {
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    for i in 0..n {
        out.write_all(line(i).as_bytes()).unwrap();
    }
}

fn space(h: &mut common::Harness, name: &str, kind: LogSpaceKind, sources: &[ImportSource]) -> String {
    h.view.update(&mut h.cx, |app, cx| app.create_log_space(name, kind, sources, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.active_tab().unwrap().target.space_id().unwrap().to_string()
    })
}

fn switch_to(h: &mut common::Harness, space_id: &str) -> (std::time::Duration, std::time::Duration) {
    let at = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.tabs().iter().position(|t| t.target.space_id() == Some(space_id)).unwrap()
    });
    let t = Instant::now();
    h.view.update(&mut h.cx, |app, cx| app.switch_to_tab(at, cx));
    let ui = t.elapsed();
    h.cx.run_until_parked();
    (ui, t.elapsed())
}

fn tab_switch(n: usize, budgets: &[u32]) {
    for &budget in budgets {
        let mut cx = gpui::TestAppContext::single();
        let mut h = common::open_app(&mut cx);
        h.view.update(&mut h.cx, |app, _| app.workspace_state.warm_tab_budget_mb = budget);
        let (a, b) = (h.dir.path().join("a.log"), h.dir.path().join("b.log"));
        write_log(&a, n);
        write_log(&b, 1000);
        let (before, used_before, foot_before) = (rss_mb(), malloc_in_use_mb(), footprint_mb());
        let first = space(&mut h, "a", LogSpaceKind::Static, &[ImportSource::Disk(a.to_string_lossy().into_owned())]);
        let (open, used_open, foot_open) = (rss_mb() - before, malloc_in_use_mb() - used_before, footprint_mb());
        let memory = h.view.read_with(&h.cx, |app, _| {
            app.parsed_files.values().filter_map(|p| p.indexed.as_ref()).map(|f| f.memory_bytes()).sum::<usize>()
        });
        let weak = h.view.read_with(&h.cx, |app, _| {
            app.parsed_files.values().find_map(|p| p.indexed.as_ref()).map(std::sync::Arc::downgrade).unwrap()
        });
        space(&mut h, "b", LogSpaceKind::Static, &[ImportSource::Disk(b.to_string_lossy().into_owned())]);
        let (left, used_left) = (rss_mb() - before, malloc_in_use_mb() - used_before);
        println!(
            "  malloc in use: open +{used_open:.0} MB, after leaving +{used_left:.0} MB; file still held: {}",
            weak.strong_count() > 0
        );
        println!("  footprint: before {foot_before:.0} MB, open {foot_open:.0} MB, after leaving {:.0} MB", footprint_mb());
        std::thread::sleep(std::time::Duration::from_secs(3));
        if std::env::var_os("PROBE_VMMAP").is_some() {
            let out = std::process::Command::new("vmmap").args(["--summary", &std::process::id().to_string()]).output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines().filter(|l| l.contains("MALLOC") || l.contains("VM_ALLOCATE") || l.contains("TOTAL") || l.contains("REGION TYPE") || l.contains("Stack")) {
                eprintln!("    {line}");
            }
        }
        println!("  footprint 3 s later: {:.0} MB, RSS +{:.0} MB, malloc in use +{:.0} MB", footprint_mb(), rss_mb() - before, malloc_in_use_mb() - used_before);
        let (ui, all) = switch_to(&mut h, &first);
        println!(
            "budget {budget:>3} MB: open +{open:.0} MB (memory_bytes {:.0} MB, {:.0} B/line), after leaving +{left:.0} MB, switch back UI {ui:.2?}, until shown {all:.2?}",
            memory as f64 / 1e6,
            memory as f64 / n as f64
        );
    }
}

fn many_files() {
    let mut cx = gpui::TestAppContext::single();
    let mut h = common::open_app(&mut cx);
    let sources: Vec<ImportSource> = (0..50)
        .map(|f| {
            let path = h.dir.path().join(format!("f{f}.log"));
            write_log(&path, 20_000);
            ImportSource::Disk(path.to_string_lossy().into_owned())
        })
        .collect();
    space(&mut h, "many", LogSpaceKind::Static, &sources);
    let before = rss_mb();
    h.view.read_with(&h.cx, |app, _| {
        for parsed in app.parsed_files.values() {
            if let Some(file) = &parsed.indexed {
                for n in 0..file.len() {
                    file.entry(n);
                }
            }
        }
    });
    let kept = log_gpui::index::view_rows::rows().weight();
    println!(
        "50 files x 20,000 lines read whole: Row Cache {:.1} MB (budget {} MB; 4096 rows a file before: ~{:.0} MB), RSS +{:.0} MB",
        kept as f64 / 1e6,
        log_gpui::index::view_rows::ROW_CACHE_BYTES >> 20,
        50.0 * 4096.0 * log_gpui::index::view_rows::row_weight(110) as f64 / 1e6,
        rss_mb() - before
    );
}

fn scratch() {
    let mut cx = gpui::TestAppContext::single();
    let mut h = common::open_app(&mut cx);
    h.view.update(&mut h.cx, |app, _| app.workspace_state.warm_tab_budget_mb = 0);
    let notes = space(&mut h, "notes", LogSpaceKind::Scratch, &[]);
    h.view.update(&mut h.cx, |app, cx| app.new_scratch_file_in(&notes, cx));
    h.cx.run_until_parked();
    let path = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_space(&notes).unwrap().files[0].path.clone().unwrap()
    });
    write_log(std::path::Path::new(&path), 500_000);
    let size = std::fs::metadata(&path).unwrap().len();
    let other = h.dir.path().join("other.log");
    write_log(&other, 10);
    space(&mut h, "other", LogSpaceKind::Static, &[ImportSource::Disk(other.to_string_lossy().into_owned())]);
    let before = rss_mb();
    let (ui, all) = switch_to(&mut h, &notes);
    println!(
        "Scratch {:.0} MB: choosing its tab UI {ui:.2?}, until shown {all:.2?}, RSS +{:.0} MB",
        size as f64 / 1e6,
        rss_mb() - before
    );
}

fn archive() {
    let mut cx = gpui::TestAppContext::single();
    let mut h = common::open_app(&mut cx);
    let entry = h.dir.path().join("big.log");
    write_log(&entry, 5_000_000);
    let entry_len = std::fs::metadata(&entry).unwrap().len();
    let archive = h.dir.path().join("logs.tar.gz");
    {
        let gz = flate2::write::GzEncoder::new(std::fs::File::create(&archive).unwrap(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        tar.append_path_with_name(&entry, "big.log").unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    std::fs::remove_file(&entry).unwrap();
    let other = h.dir.path().join("other.log");
    write_log(&other, 10);
    let other = space(&mut h, "other", LogSpaceKind::Static, &[ImportSource::Disk(other.to_string_lossy().into_owned())]);
    let source = ImportSource::Entry { archive: archive.to_string_lossy().into_owned(), entry: "big.log".into() };
    let t = Instant::now();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("bundle", LogSpaceKind::Static, &[source], cx));
    let ui = t.elapsed();
    h.cx.run_until_parked();
    let all = t.elapsed();
    println!(
        "Archive Entry {:.0} MB in {:.0} MB .tar.gz, first open: UI {ui:.2?}, until shown {all:.2?} (before: all of it on the UI thread)",
        entry_len as f64 / 1e6,
        std::fs::metadata(&archive).unwrap().len() as f64 / 1e6
    );
    // Opened again: unpacked already, only indexed.
    h.view.update(&mut h.cx, |app, _| app.workspace_state.warm_tab_budget_mb = 0);
    let bundle = h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.active_tab().unwrap().target.space_id().unwrap().to_string()
    });
    switch_to(&mut h, &other);
    let (ui, all) = switch_to(&mut h, &bundle);
    println!("Archive Entry opened again (unpacked before): UI {ui:.2?}, until shown {all:.2?}");
}

pub fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(3_000_000);
    // `--malloc`: what macOS malloc gives back after small and large
    // blocks are freed, with no app code.
    if std::env::args().any(|a| a == "--malloc") {
        let base = footprint_mb();
        let small: Vec<String> = (0..3_000_000).map(line).collect();
        let large: Vec<Vec<u8>> = (0..30).map(|_| vec![1u8; 10 << 20]).collect();
        println!("malloc test: footprint +{:.0} MB with 3M small strings and 300 MB in 10 MB blocks", footprint_mb() - base);
        drop(large);
        println!("  large blocks freed: +{:.0} MB", footprint_mb() - base);
        drop(small);
        println!("  small strings freed: +{:.0} MB", footprint_mb() - base);
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
            }
            // SAFETY: a null zone means every zone; a goal of 0, all it can.
            let freed = unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
            println!("  pressure relief: {} MB, footprint +{:.0} MB", freed >> 20, footprint_mb() - base);
        }
        return;
    }
    // `--cycle`: three large files opened one after the other with budget
    // 0: whether what malloc keeps grows, or is used again.
    if std::env::args().any(|a| a == "--cycle") {
        let mut cx = gpui::TestAppContext::single();
        let mut h = common::open_app(&mut cx);
        h.view.update(&mut h.cx, |app, _| app.workspace_state.warm_tab_budget_mb = 0);
        for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            let path = h.dir.path().join(format!("{name}.log"));
            write_log(&path, n);
            space(&mut h, name, LogSpaceKind::Static, &[ImportSource::Disk(path.to_string_lossy().into_owned())]);
            println!("opened {name}.log ({n} lines), the one before let go: footprint {:.0} MB, malloc in use {:.0} MB", footprint_mb(), malloc_in_use_mb());
        }
        return;
    }
    // `--budget N`: only the tab switch, with that budget, in a fresh
    // process (RSS is not shared with a run before).
    if let Some(at) = std::env::args().position(|a| a == "--budget") {
        let budget = std::env::args().nth(at + 1).and_then(|b| b.parse().ok()).unwrap_or(512);
        return tab_switch(n, &[budget]);
    }
    tab_switch(n, &[0, 512]);
    many_files();
    scratch();
    archive();
}
