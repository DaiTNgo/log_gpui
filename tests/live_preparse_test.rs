//! Big batches of Live lines are read on a worker thread before the app
//! applies them: the entries are the same ones, and a burst never holds up a
//! frame for long.
mod common;

use common::fake_live::{lines, open};
use gpui::TestAppContext;
use log_gpui::space_view::ParsedFile;

fn same_entries(app: &log_gpui::app::AppView, expected: &ParsedFile, what: &str) {
    let held = app.parsed_files.values().next().expect("one file");
    assert_eq!(held.entries.len(), expected.entries.len(), "{what}: lines");
    for (i, (a, b)) in held.entries.iter().zip(&expected.entries).enumerate() {
        assert_eq!(
            (a.index, &a.raw_line, a.parent, a.has_time, a.timestamp.to_bits(), &a.payload, &a.level_text, &a.tag, &a.time_str),
            (b.index, &b.raw_line, b.parent, b.has_time, b.timestamp.to_bits(), &b.payload, &b.level_text, &b.tag, &b.time_str),
            "{what}: entry {i}"
        );
        assert_eq!(format!("{:?}", a.spans), format!("{:?}", b.spans), "{what}: spans of entry {i}");
    }
}

#[gpui::test]
fn a_big_batch_is_read_ahead_and_applied_as_if_read_inline(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 50, 1_000_000);
    f.h.view.update(&mut f.h.cx, |app, _| app.live.preparse_min = 1);
    let mut all = lines("alpha", 0..50, 0);
    // Batches of several shapes: continuation lines, a split Parent Line.
    for (start, end) in [(50, 120), (120, 121), (121, 300)] {
        let batch = lines("alpha", start..end, 0);
        all.extend(batch.iter().cloned());
        f.send("alpha", batch);
        f.pump();
        // The file waits for its batch to be read.
        f.h.cx.run_until_parked();
    }
    let expected = ParsedFile::from_text(&format!("{}\n", all.join("\n")));
    f.h.view.read_with(&f.h.cx, |app, _| same_entries(app, &expected, "read ahead"));
}

#[gpui::test]
fn lines_sent_while_a_batch_is_read_keep_their_order(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 20, 1_000_000);
    f.h.view.update(&mut f.h.cx, |app, _| app.live.preparse_min = 1);
    let mut all = lines("alpha", 0..20, 0);
    for (start, end) in [(20, 80), (80, 100), (100, 160)] {
        let batch = lines("alpha", start..end, 0);
        all.extend(batch.iter().cloned());
        f.send("alpha", batch);
    }
    // Pumps without waiting: the file is busy until its batch is applied.
    for _ in 0..6 {
        f.pump();
        f.h.cx.run_until_parked();
    }
    let expected = ParsedFile::from_text(&format!("{}\n", all.join("\n")));
    f.h.view.read_with(&f.h.cx, |app, _| same_entries(app, &expected, "queued batches"));
}

#[gpui::test]
fn a_burst_is_applied_over_several_pumps_each_short(cx: &mut TestAppContext) {
    let mut f = open(cx, &[("alpha", 0)], 10, 10_000_000);
    let total = 200_000;
    let start = f.h.view.read_with(&f.h.cx, |app, _| app.all_entries.len());
    let burst: Vec<String> = (0..total).map(|i| format!("2026-10-19 11:00:00.{:03} INFO burst line {i}", i % 1000)).collect();
    f.send("alpha", burst);
    let mut slowest = std::time::Duration::ZERO;
    let mut pumps = 0;
    loop {
        let started = std::time::Instant::now();
        let more = f.h.view.update(&mut f.h.cx, |app, cx| app.pump_live(cx));
        slowest = slowest.max(started.elapsed());
        f.h.cx.run_until_parked();
        // The batch is applied once its lines are read (here, on the main thread).
        slowest = slowest.max(started.elapsed());
        pumps += 1;
        if !more || pumps > 200 {
            break;
        }
    }
    let rows = f.h.view.read_with(&f.h.cx, |app, _| app.all_entries.len());
    assert!(pumps > 3, "a burst this size takes several pumps, took {pumps}");
    assert_eq!(rows, start + total, "every line arrived");
    eprintln!("burst: {pumps} pumps, slowest {slowest:?}");
    assert!(slowest < std::time::Duration::from_millis(250), "slowest pump {slowest:?}");
}

#[test]
fn lines_read_ahead_for_another_state_are_read_again() {
    use log_gpui::live::{BreakKind, LiveBuffer, SourceEvent};
    let batch: Vec<String> = lines("alpha", 0..30, 0);
    let event = SourceEvent::Append { lines: batch.clone(), bytes: Vec::new(), streams: None };

    // Read ahead for the state the buffer is in, then Lines are skipped
    // before the batch is applied: the numbers moved, the entries must too.
    let mut parsed = ParsedFile::empty();
    let mut buf = LiveBuffer::command("cmd");
    buf.set_prepared(buf.prepare_job(None).run(std::slice::from_ref(&event)));
    buf.apply_break(BreakKind::Skipped(7), "10:00:00");
    buf.apply_append(&mut parsed, &batch);

    let mut direct = ParsedFile::empty();
    let mut other = LiveBuffer::command("cmd");
    other.apply_break(BreakKind::Skipped(7), "10:00:00");
    other.apply_append(&mut direct, &batch);
    let numbers = |p: &ParsedFile| p.entries.iter().map(|e| (e.index, e.parent)).collect::<Vec<_>>();
    assert_eq!(numbers(&parsed), numbers(&direct));
    assert_eq!(parsed.entries[0].index, 8);

    // And when it fits, the same entries come out.
    let mut ahead = ParsedFile::empty();
    let mut buf = LiveBuffer::command("cmd");
    buf.set_prepared(buf.prepare_job(None).run(std::slice::from_ref(&event)));
    buf.apply_append(&mut ahead, &batch);
    let mut inline = ParsedFile::empty();
    LiveBuffer::command("cmd").apply_append(&mut inline, &batch);
    assert_eq!(numbers(&ahead), numbers(&inline));
}
