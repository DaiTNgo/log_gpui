//! A Remote File read through its Remote Cache (openspec change
//! `remote-local-cache`) gives the rows `LiveBuffer` gave in memory for the
//! same events: the same numbers, Generations, text and Breaks, kept in
//! `tests/golden/cached_file/`.

use log_gpui::index::HandleBudget;
use log_gpui::live::cached::CachedFile;
use log_gpui::live::source::{EarlierFile, EarlierStart};
use log_gpui::live::BreakKind;
use log_gpui::remote::cache::{CacheTarget, RemoteCache, dir_for};

/// What a row shows: its number, Generation, text and the Break above it.
type Row = (usize, u32, String, Option<String>);

/// A Remote File read through its cache, and what it showed at each step:
/// compared with `tests/golden/cached_file/<name>.txt`, what `LiveBuffer`
/// showed in memory for the same events (recorded once the two were checked
/// equal, before `LiveBuffer` was removed). With `UPDATE_GOLDEN` set, the
/// file is written instead.
struct Both {
    cached: CachedFile,
    name: &'static str,
    log: String,
    _dir: tempfile::TempDir,
}

fn both(name: &'static str, segment_lines: usize) -> Both {
    let dir = tempfile::tempdir().unwrap();
    let target = CacheTarget::File { path: "/var/log/app.log".into() };
    let cache = RemoteCache::open(&dir_for(dir.path(), "srv", &target), "srv", target, segment_lines).unwrap();
    let cached = CachedFile::new("web1:app.log", cache, Default::default(), HandleBudget::new(256)).unwrap();
    Both { cached, name, log: String::new(), _dir: dir }
}

impl Drop for Both {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/cached_file").join(format!("{}.txt", self.name));
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &self.log).unwrap();
            return;
        }
        let want = std::fs::read_to_string(&path).unwrap();
        let line = want.lines().zip(self.log.lines()).position(|(a, b)| a != b);
        assert!(
            line.is_none() && want.lines().count() == self.log.lines().count(),
            "{}: first difference at line {line:?}: want {:?}, have {:?}",
            self.name,
            line.and_then(|l| want.lines().nth(l)),
            line.and_then(|l| self.log.lines().nth(l)),
        );
    }
}

fn lines(range: std::ops::Range<usize>) -> Vec<String> {
    range
        .flat_map(|i| {
            let line = format!("2026-09-05 10:{:02}:{:02} [INFO] [api] request {i}", i / 60 % 60, i % 60);
            // Every tenth line has a stack line under it.
            if i % 10 == 0 { vec![line, format!("    at Handler.run(Handler.java:{i})")] } else { vec![line] }
        })
        .collect()
}

fn bytes(lines: &[String]) -> Vec<u32> {
    lines.iter().map(|l| l.len() as u32 + 1).collect()
}

impl Both {
    fn cached_rows(&self) -> Vec<Row> {
        let file = self.cached.file();
        (0..file.len())
            .map(|i| {
                let e = file.entry(i).unwrap();
                (e.index, e.generation, e.raw_line.to_string(), e.break_before.as_ref().map(|m| m.label.clone()))
            })
            .collect()
    }

    fn same(&mut self, what: &str) {
        let rows = self.cached_rows();
        self.log.push_str(&format!(
            "## {what}\nheld start {:?}, top bytes {}, at start {}\n",
            self.cached.held_start(),
            self.cached.top_generation_bytes(),
            self.cached.at_generation_start()
        ));
        for (index, generation, text, mark) in rows {
            if let Some(mark) = mark {
                self.log.push_str(&format!("  -- {mark}\n"));
            }
            self.log.push_str(&format!("{index} g{generation} {text}\n"));
        }
    }

    fn note(&mut self, what: impl std::fmt::Display) {
        self.log.push_str(&format!("{what}\n"));
    }

    fn tail(&mut self, l: &[String], end: u64) {
        self.cached.apply_tail(l, &bytes(l), end).unwrap();
    }

    fn backfill(&mut self, l: &[String]) {
        self.cached.apply_backfill(l, &bytes(l)).unwrap();
    }

    fn append(&mut self, l: &[String]) {
        self.cached.apply_append(l, &bytes(l)).unwrap();
    }

    fn total(&mut self, n: usize) {
        let shift = self.cached.apply_total(Some(n)).unwrap();
        self.note(format!("total {n}: shift {shift:?}"));
    }

    fn brk(&mut self, kind: BreakKind, time: &str) {
        self.cached.apply_break(kind, time).unwrap();
    }

    fn trim(&mut self, window: usize) {
        let cached = self.cached.trim_to_window(window).unwrap().map(|t| (t.removed, t.generation, t.first_kept));
        self.note(format!("trim {window}: {cached:?}"));
    }
}

#[test]
fn tail_total_backfill_and_append_number_the_lines_alike() {
    let mut f = both("tail_total_backfill_append", 50);
    // The last lines of a file of 400, then their real numbers.
    let all = lines(0..400);
    let n = all.len();
    f.tail(&all[n - 60..], 9_999);
    f.same("tail");
    f.total(n);
    f.same("total");
    f.backfill(&all[n - 150..n - 60]);
    f.same("backfill");
    f.append(&lines(400..430));
    f.same("append");
    f.append(&lines(430..520));
    f.same("append across segments");
}

#[test]
fn breaks_and_generations_are_alike() {
    let mut f = both("breaks_and_generations", 40);
    let first = lines(0..50);
    f.tail(&first, 5_000);
    f.total(first.len());
    f.brk(BreakKind::Disconnected, "10:00:00");
    f.brk(BreakKind::Reconnected, "10:00:05");
    f.append(&lines(50..60));
    f.same("reconnected");
    f.brk(BreakKind::Rotated, "10:01:00");
    f.append(&lines(0..30));
    f.same("rotated");
    f.brk(BreakKind::Truncated, "10:02:00");
    f.append(&lines(0..5));
    f.same("truncated");
}

#[test]
fn the_live_window_drops_the_same_lines() {
    let mut f = both("live_window", 25);
    let first = lines(0..100);
    f.tail(&first, 20_000);
    f.total(first.len());
    for batch in 0..6 {
        f.append(&lines(100 + batch * 20..120 + batch * 20));
        f.trim(80);
        f.same(&format!("batch {batch}"));
    }
}

#[test]
fn an_earlier_generation_goes_above_the_file_held_from_its_first_line() {
    let mut f = both("earlier_generation", 30);
    let all = lines(0..300);
    let n = all.len();
    f.tail(&all[n - 50..], 30_000);
    f.total(n);
    // Backfill to the start of the file, then a Rotated File above.
    f.backfill(&all[..n - 50]);
    f.cached.set_earlier_available(Some(&EarlierFile { name: "app.log.1".into(), size: 4096, compressed: false })).unwrap();
    f.same("offered");
    let earlier = lines(1000..1040);
    let start = EarlierStart { name: "app.log.1".into(), modified: Some("2026-09-04 23:59:00".into()) };
    assert!(f.cached.apply_earlier(Some(&start), &earlier, &bytes(&earlier), Some(earlier.len())).unwrap().generations_moved);
    f.same("earlier generation");
    f.append(&lines(300..310));
    f.same("append after");
}

#[test]
fn clear_is_alike() {
    let mut f = both("clear", 30);
    let first = lines(0..60);
    f.tail(&first, 7_000);
    f.total(first.len());
    f.cached.clear("10:05:00").unwrap();
    f.same("cleared");
    f.append(&lines(60..70));
    f.same("after clear");
}

/// Leaving the tab keeps the lines up to the last Checkpoint; opening it
/// again shows them, numbered as on the Server, and reads on from there.
#[test]
fn closed_and_restored_the_lines_end_at_the_checkpoint() {
    use log_gpui::remote::cache::Checkpoint;
    let dir = tempfile::tempdir().unwrap();
    let target = CacheTarget::File { path: "/var/log/app.log".into() };
    let at = dir_for(dir.path(), "srv", &target);
    let cache = RemoteCache::open(&at, "srv", target.clone(), 20).unwrap();
    let mut f = CachedFile::new("web1:app.log", cache, Default::default(), HandleBudget::new(64)).unwrap();
    let first = lines(0..100);
    let end: u64 = bytes(&first).iter().map(|b| u64::from(*b)).sum();
    f.apply_tail(&first, &bytes(&first), end).unwrap();
    f.apply_total(Some(first.len() + 500)).unwrap();
    f.set_checkpoint(Checkpoint { generation: 0, inode: Some(7), server_end: end, head_hash: Some(1), tail_hash: Some(2) }).unwrap();
    // Received after the Checkpoint: sent again next time.
    let later = lines(100..120);
    f.apply_append(&later, &bytes(&later)).unwrap();
    let want: Vec<String> = (0..first.len()).map(|i| f.file().line(i).unwrap().to_string()).collect();
    f.close().unwrap();

    let cache = RemoteCache::open(&at, "srv", target, 20).unwrap();
    let (f, from) = CachedFile::restore("web1:app.log", cache, Default::default(), HandleBudget::new(64)).unwrap().unwrap();
    assert_eq!(f.len(), first.len());
    let file = f.file();
    assert_eq!(file.entry(0).unwrap().index, 501, "numbered as on the Server");
    assert_eq!((0..f.len()).map(|i| file.line(i).unwrap().to_string()).collect::<Vec<_>>(), want);
    assert_eq!((from.server_end, from.inode, from.head_hash, from.tail_hash), (end, Some(7), 1, 2));
    assert_eq!(f.held_start().offset, Some(0), "the oldest line is at the start of the file");
}

/// A Remote Command's output is kept when its tab is left, and shows again
/// under a Break "previous run" with its streams; the command runs again
/// below it as a new Generation (task 6.1).
#[test]
fn a_commands_last_run_shows_again_under_a_break_and_the_next_run_goes_below() {
    use log_gpui::live::source::{RestartKind, StreamMask};
    use log_gpui::model::Stream;
    let dir = tempfile::tempdir().unwrap();
    let target = CacheTarget::Command { command: "remote-cmd:srv-1:1".into() };
    let at = dir_for(dir.path(), "commands", &target);
    let cache = RemoteCache::open(&at, "commands", target.clone(), 20).unwrap();
    let mut f = CachedFile::command("web1$ tail", cache, Default::default(), HandleBudget::new(64)).unwrap();
    let out = lines(0..30);
    let stderr = |i: usize| i % 4 == 0;
    let mask: StreamMask = (0..out.len()).map(|i| if stderr(i) { Stream::Stderr } else { Stream::Stdout }).collect();
    f.apply_append_streams(&out, &[], Some(&mask)).unwrap();
    f.apply_break(BreakKind::Skipped(3), "10:00:00").unwrap();
    f.apply_append_streams(&out[..2], &[], None).unwrap();
    let held = f.len();
    f.close_command("10:30:00").unwrap();

    let cache = RemoteCache::open(&at, "commands", target, 20).unwrap();
    let (mut f, previous) = CachedFile::restore_command("web1$ tail", cache, Default::default(), HandleBudget::new(64)).unwrap().unwrap();
    assert_eq!(f.len(), held);
    let file = f.file();
    let first = file.entry(0).unwrap();
    assert_eq!((first.index, first.stream), (1, Some(Stream::Stderr)));
    assert_eq!(first.break_before.as_ref().unwrap().label, "web1$ tail previous run 10:30:00");
    // Numbered on after the lines dropped while Paused.
    let after_skip = file.entry(out.len()).unwrap();
    assert_eq!(after_skip.index, out.len() + 4);
    assert_eq!(previous.len(), held, "the whole Run, for resuming");
    assert_eq!(previous[0], (Stream::Stderr, out[0].clone()));

    f.apply_break(BreakKind::Restarted(RestartKind::Reopened(Some(36_000))), "10:31:00").unwrap();
    f.apply_append_streams(&out[..1], &[], None).unwrap();
    let file = f.file();
    let new = file.entry(held).unwrap();
    assert_eq!((new.index, new.generation), (1, 1));
    assert_eq!(new.break_before.as_ref().unwrap().label, "web1$ tail run again 10:31:00, resumed from 10:00:00");
}
