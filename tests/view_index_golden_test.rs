//! What a view's Filter columns and the rows' times give, row by row, compared
//! with `tests/golden/view_index/<name>.txt` (lean-view-memory, group 2):
//! written from the index the view kept before that change, so the views
//! that read their files' index instead give exactly the same. With
//! `UPDATE_GOLDEN` set, the files are written again.

mod common;

use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
use std::fmt::Write as _;

/// `a.log`: three lines before the first timestamp, Continuation Lines,
/// four levels, a module column with few values.
fn a_lines() -> Vec<String> {
    let mut out = vec!["banner line one".to_string(), "banner line two".into(), "  still banner".into()];
    for i in 0..400 {
        let level = ["INFO", "DEBUG", "WARN", "ERROR"][i % 4];
        out.push(format!("2026-09-05 10:{:02}:{:02}.{:03} [{level}] [mod{}] step {i} user=u{}", (i * 3) / 60, (i * 3) % 60, i, i % 3, i % 7));
        if i % 50 == 7 {
            out.push("    at frame one".into());
            out.push("    at frame two".into());
        }
    }
    out
}

/// `b.log`: timestamps between `a.log`'s, only ERROR and FATAL, and a module
/// column with more values than a column keeps (it overflows).
fn b_lines() -> Vec<String> {
    (0..600)
        .map(|i| {
            let level = if i % 5 == 0 { "FATAL" } else { "ERROR" };
            format!("2026-09-05 10:{:02}:{:02}.{:03} [{level}] [svc{i}] failure {i}", (i * 2) / 60, (i * 2) % 60, 500 + i)
        })
        .collect()
}

/// `c.log`: JSON lines with no time at all, a Request ID only from line 50.
fn c_lines() -> Vec<String> {
    (0..120)
        .map(|i| {
            if i < 50 {
                format!(r#"{{"level":"info","msg":"warm up {i}"}}"#)
            } else {
                format!(r#"{{"level":"warn","msg":"call {i}","requestId":"r{}"}}"#, i % 4)
            }
        })
        .collect()
}

fn write(dir: &std::path::Path, name: &str, lines: &[String]) -> ImportSource {
    let path = dir.join(name);
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    ImportSource::Disk(path.to_string_lossy().into_owned())
}

/// Everything the view's index answers, as text, then the Time Histogram's
/// counts; then the same with a Filter on Level.
fn snapshot(h: &mut common::Harness) -> String {
    let mut out = h.view.read_with(&h.cx, |app, _| index_text(app) + &histogram_text(app));
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    h.cx.run_until_parked();
    out += &h.view.read_with(&h.cx, |app, _| {
        let pass = app.filter_pass.to_vec();
        format!("level=error passes {} rows: {:?}\n", pass.len(), pass) + &histogram_text(app)
    });
    out
}

fn histogram_text(app: &log_gpui::app::AppView) -> String {
    let s = &app.histogram.shown;
    format!(
        "histogram counts {:?}\n  filtered {:?}\n  matched {:?}\n  untimed {} first {:?}\n",
        s.counts, s.filtered, s.matched, s.untimed, s.first_untimed
    )
}

/// The columns, their values and Coverage, the counts and time span, then
/// each row's file, line, effective timestamp and value id in every column.
fn index_text(app: &log_gpui::app::AppView) -> String {
    let mut out = String::new();
    let index = app.index();
    let rows = app.view.len();
    writeln!(out, "rows {rows} parent_lines {} timed_lines {}", index.parent_lines(), index.timed_lines()).unwrap();
    writeln!(out, "time_span {:?} anchor_day {}", index.time_span(), index.anchor_day()).unwrap();
    let mut kv: Vec<_> = index.kv_counts().iter().map(|(k, n)| format!("{}={n}", k.as_str())).collect();
    kv.sort();
    writeln!(out, "kv_counts {}", kv.join(" ")).unwrap();
    for col in &index.columns {
        writeln!(
            out,
            "column {} {:?} overflow {} filterable {} coverage {:?} distinct {:?}",
            col.key,
            col.label,
            col.overflow,
            col.is_filterable(),
            index.coverage(&col.key),
            col.distinct_count()
        )
        .unwrap();
        for v in &col.values {
            writeln!(out, "  value {:?} {:?} {}", v.display, v.folded, v.count).unwrap();
        }
    }
    let ids: Vec<_> = index.columns.iter().map(|c| index.ids(&c.key)).collect();
    for row in 0..rows {
        let (file, line) = app.view.get(row).unwrap();
        let ids: Vec<String> =
            ids.iter().map(|ids| ids.as_ref().and_then(|ids| ids.id_of_row(row)).map_or("-".into(), |id| id.to_string())).collect();
        writeln!(out, "{row} f{file}:{line} ts {:?} {}", index.effective_timestamp(row), ids.join(" ")).unwrap();
    }
    out
}

fn check(name: &str, have: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/view_index").join(format!("{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, have).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let line = want.lines().zip(have.lines()).position(|(a, b)| a != b);
    assert!(
        line.is_none() && want.lines().count() == have.lines().count(),
        "{name}: first difference at line {line:?}: want {:?}, have {:?}",
        line.and_then(|l| want.lines().nth(l)),
        line.and_then(|l| have.lines().nth(l)),
    );
}

fn open_space(h: &mut common::Harness, sources: &[ImportSource]) -> String {
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("golden", LogSpaceKind::Static, sources, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().working_set.active_tab().unwrap().target.space_id().unwrap().to_string()
    })
}

#[gpui::test]
fn file_view(cx: &mut gpui::TestAppContext) {
    let mut h = common::open_app(cx);
    let a = write(h.dir.path(), "a.log", &a_lines());
    let space = open_space(&mut h, &[a]);
    let file = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_space(&space).unwrap().files[0].file_id.clone());
    h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space, &file, cx));
    h.cx.run_until_parked();
    check("file_view", &snapshot(&mut h));
}

#[gpui::test]
fn space_of_one_file(cx: &mut gpui::TestAppContext) {
    let mut h = common::open_app(cx);
    let a = write(h.dir.path(), "a.log", &a_lines());
    open_space(&mut h, &[a]);
    check("space_one_file", &snapshot(&mut h));
}

#[gpui::test]
fn space_of_three_files(cx: &mut gpui::TestAppContext) {
    let mut h = common::open_app(cx);
    let sources = [write(h.dir.path(), "a.log", &a_lines()), write(h.dir.path(), "b.log", &b_lines()), write(h.dir.path(), "c.log", &c_lines())];
    open_space(&mut h, &sources);
    check("space_three_files", &snapshot(&mut h));
}
