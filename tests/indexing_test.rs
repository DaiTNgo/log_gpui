//! Indexing in the background (openspec change `line-offset-index`, tasks
//! 3.1–3.2): the Line Index, `LineMeta`, Filter columns and column widths
//! built in parallel blocks are those the whole-file parse gives.

use log_gpui::column::FieldMaxChars;
use log_gpui::filter_index::{FilterIndex, LineFacts};
use log_gpui::index::fast;
use log_gpui::index::indexing::{IndexOptions, IndexedData, Indexing, Outcome, index_all};
use log_gpui::index::{BytesSource, LineSource, index_source};
use log_gpui::log_format::compile_template;
use log_gpui::model::LogEntry;
use log_gpui::parser::text::{LineConfig, LineParser, parse_line_with};
use std::sync::Arc;

/// The entries the app reads from `bytes` today, line by line.
fn entries_of(bytes: &[u8], options: &IndexOptions) -> Vec<LogEntry> {
    let mut parser = LineParser::new();
    parser.set_config(options.config.clone());
    parser.set_format(options.format.clone());
    String::from_utf8_lossy(bytes).lines().enumerate().map(|(i, l)| parser.parse(i + 1, l)).collect()
}

/// Everything a `FilterIndex` shows, to compare two.
fn describe(f: &FilterIndex, ts: impl Fn(usize) -> Option<f64>) -> String {
    let mut out = format!(
        "rows {} parents {} timed {} anchor {} hint {:?}\n",
        f.rows(),
        f.parent_lines(),
        f.timed_lines(),
        f.anchor_day(),
        f.time_hint()
    );
    let mut kv: Vec<_> = f.kv_counts().iter().map(|(k, n)| format!("{k}={n}")).collect();
    kv.sort();
    out += &format!("kv {kv:?}\n");
    for row in 0..f.rows() {
        out += &format!("ts {row} {:?}\n", ts(row));
    }
    for c in &f.columns {
        out += &format!(
            "col {} {} overflow {} covered {} ids {} values {:?}\n",
            c.key,
            c.label,
            c.overflow,
            c.covered(),
            c.has_row_ids(),
            c.values
        );
        if c.has_row_ids() {
            let ids: Vec<_> = (0..f.rows()).map(|r| c.id_of_row(r)).collect();
            out += &format!("  {ids:?}\n");
        }
    }
    out
}

fn check(name: &str, bytes: &[u8], options: &IndexOptions) -> IndexedData {
    let source = BytesSource::new(bytes.to_vec());
    let (data, outcome) = index_all(&source, options);
    assert_eq!(outcome, Outcome::Done, "{name}");
    let entries = entries_of(bytes, options);
    assert_eq!(data.file.len(), entries.len(), "{name}: lines");

    let reference = index_source(&source, options.format.as_deref()).unwrap();
    assert_eq!(data.file.lines, reference.lines, "{name}: Line Index");
    assert_eq!(data.file.meta.as_slice(), reference.meta.as_slice(), "{name}: LineMeta");
    for (n, entry) in entries.iter().enumerate() {
        let meta = data.file.meta.get(n).unwrap();
        assert_eq!(meta.parent_of(n).map(|p| p + 1), entry.parent, "{name}: Parent Line of {n}");
        assert_eq!(meta.level(), entry.level, "{name}: level of {n}");
    }

    let want = FilterIndex::build(&entries);
    let (have, want) = (describe(&data.filter, |r| data.effective_timestamp(r)), describe(&want, |r| want.read().effective_timestamp(r)));
    assert_eq!(have, want, "{name}: FilterIndex");
    assert_eq!(data.max_chars, FieldMaxChars::scan_entries(&entries), "{name}: column widths");
    data
}

/// Every way of reading `bytes`: with and without the lock, in one block
/// and in blocks of a few lines.
fn check_all(name: &str, bytes: &[u8], options: IndexOptions) {
    for block_bytes in [None, Some(64), Some(301)] {
        for no_lock in [false, true] {
            let options = IndexOptions { block_bytes, no_lock, ..options.clone() };
            check(&format!("{name} {block_bytes:?} no_lock={no_lock}"), bytes, &options);
        }
    }
}

fn standard(lines: usize) -> String {
    let levels = ["INFO", "DEBUG", "WARN", "ERROR"];
    let tags = ["auth", "api", "db"];
    let mut out = String::new();
    for i in 0..lines {
        if i % 11 == 5 {
            out += "java.lang.IllegalStateException: boom\n\tat a.b(C.java:12)\n";
        }
        out += &format!(
            "2026-09-05 10:{:02}:{:02}.{:03} [{}] [{}] request {i} user={} took={}ms\n",
            i / 60 % 60,
            i % 60,
            i % 1000,
            levels[i % 4],
            tags[i % 3],
            i % 7,
            i % 300
        );
    }
    out
}

fn samples() -> Vec<(&'static str, String)> {
    vec![
        ("standard", standard(300)),
        ("empty", String::new()),
        ("no break at the end", "2026-09-05 10:00:00 [INFO] [a] last".into()),
        ("untimed head", "banner\nINFO not a time\n---\n".to_string() + &standard(80)),
        (
            "mixed",
            "2026-09-05T10:00:00Z INFO iso\n[05/Sep/2026:10:00:01 +0000] GET /\n2026-09-05 10:00:02,123 ERROR log4j [main] x\njunk in between\nINFO after a time\n{\"time\":\"2026-09-05T10:00:03Z\",\"level\":\"warn\",\"msg\":\"json\",\"user\":\"bob\"}\n{\"level\":\"info\",\"user\":\"amy\"}\n2026/09/05 10:00:04 WARN [db] slash date\n2026-09-05 10:00:05 - pay - WARNING - python\n1969-12-31 23:59:59 [INFO] [old] before 1970\n2026-09-05 10:00:06 [INFO]: colon\n2026-09-05 10:00:07 [VERBOSEX] long\n2026-09-05 10:00:08\tINFO tab\n".to_string()
                + &standard(100),
        ),
        ("only continuations", "  at a\n  at b\n\n".to_string()),
    ]
}

#[test]
fn blocks_give_what_the_whole_file_gives() {
    for (name, text) in samples() {
        check_all(name, text.as_bytes(), IndexOptions::default());
    }
}

#[test]
fn detected_fields_are_read_while_indexing() {
    let config = Arc::new(LineConfig::new(&["user".to_string(), "took".to_string(), "module".to_string()]));
    for (name, text) in samples() {
        check_all(name, text.as_bytes(), IndexOptions { config: config.clone(), ..Default::default() });
    }
    // A `module=` Detected Field gives a line without a tag its tag.
    let text = "2026-09-05 10:00:00 INFO module=billing charged\n".repeat(100);
    let data = check("module field", text.as_bytes(), &IndexOptions { config, ..Default::default() });
    assert!(data.filter.column("module").unwrap().values.iter().any(|v| v.display == "billing"));
}

#[test]
fn a_log_format_is_read_while_indexing() {
    let format = Arc::new(compile_template("%{TIMESTAMP:timestamp} [%{LEVEL:level}] %{WORD:op} %{GREEDY:message}").unwrap());
    let text = "2026-10-20 10:00:01 [INFO] charge ok\nnot matching\n2026-10-20 10:00:02 [WARN] refund late\n".repeat(50);
    check_all("format", text.as_bytes(), IndexOptions { format: Some(format), ..Default::default() });
}

#[test]
fn a_column_overflows_while_indexing() {
    let mut text = String::new();
    for i in 0..1500 {
        text += &format!("2026-09-05 10:00:{:02} [INFO] [t{}] id={i} in {}\n", i % 60, i % 1200, i % 3);
    }
    let config = Arc::new(LineConfig::new(&["id".to_string(), "in".to_string()]));
    let options = IndexOptions { config, block_bytes: Some(4096), ..Default::default() };
    let data = check("overflow", text.as_bytes(), &options);
    assert!(data.filter.column("module").unwrap().overflow);
}

#[test]
fn a_standard_file_locks_on_the_fast_reader() {
    let text = standard(200);
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.into_bytes()));
    let indexed = Indexing::start(source.clone(), IndexOptions::default()).wait();
    assert!(indexed.locked.load(std::sync::atomic::Ordering::Acquire));
    // Lines of other layouts among them do not stop the lock.
    let mixed: Arc<dyn LineSource> = Arc::new(BytesSource::new(samples()[4].1.clone().into_bytes()));
    let indexed = Indexing::start(mixed, IndexOptions::default()).wait();
    assert!(indexed.locked.load(std::sync::atomic::Ordering::Acquire));
    // A file of JSON lines has nothing to lock on.
    let json = "{\"time\":\"2026-09-05T10:00:03Z\",\"level\":\"warn\",\"msg\":\"x\"}\n".repeat(200);
    let json: Arc<dyn LineSource> = Arc::new(BytesSource::new(json.into_bytes()));
    let indexed = Indexing::start(json, IndexOptions::default()).wait();
    assert!(!indexed.locked.load(std::sync::atomic::Ordering::Acquire));
    // Nor does a file with a Log Format.
    let format = Arc::new(compile_template("%{TIMESTAMP:timestamp} %{GREEDY:message}").unwrap());
    let indexed = Indexing::start(source, IndexOptions { format: Some(format), ..Default::default() }).wait();
    assert!(!indexed.locked.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn indexing_reports_progress_and_ends() {
    let text = standard(20_000);
    let total = text.len() as u64;
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.into_bytes()));
    let indexing = Indexing::start(source, IndexOptions { block_bytes: Some(16 << 10), ..Default::default() });
    let updates = indexing.updates.clone();
    let indexed = indexing.wait();
    let all: Vec<_> = std::iter::from_fn(|| updates.try_recv().ok()).collect();
    let last = all.last().expect("at least the end is told");
    assert_eq!(last.outcome, Some(Outcome::Done));
    assert_eq!((last.bytes, last.total), (total, total));
    assert!(all.windows(2).all(|w| w[0].lines <= w[1].lines), "lines only grow");
    assert_eq!(indexed.progress(), (total, total));
}

#[test]
fn cancelled_indexing_keeps_the_lines_read() {
    let text = standard(200_000);
    let bytes = text.into_bytes();
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(bytes.clone()));
    let indexing = Indexing::start(source, IndexOptions { block_bytes: Some(4096), ..Default::default() });
    // Wait for the first lines, then cancel.
    let first = indexing.updates.recv_blocking().unwrap();
    assert!(first.lines > 0);
    indexing.cancel();
    let indexed = indexing.wait();
    assert_eq!(indexed.outcome(), Some(Outcome::Cancelled));
    let data = indexed.data.read_recursive();
    let n = data.file.len();
    let reference = index_source(&BytesSource::new(bytes), None).unwrap();
    assert!(n < reference.len(), "stopped early: {n} lines");
    assert_eq!(data.file.meta.as_slice(), &reference.meta.as_slice()[..n], "the lines read are right");
    assert_eq!(data.filter.rows(), n);
}

#[test]
fn the_fast_reader_reads_as_auto_detect_or_not_at_all() {
    let config = LineConfig::new(&["user".to_string(), "module".to_string()]);
    let lines = [
        "2026-09-05 10:00:00 [INFO] [api] plain",
        "2026-09-05 10:00:00.1 [WARN] [api] one digit",
        "2026-09-05 10:00:00.123456789 [ERROR] [api] nine digits",
        "2026-09-05 10:00:00.1234567890 [ERROR] [api] ten digits",
        "2026-09-05 10:00:00,250 ERROR [main] c.a.S - comma",
        "2026-09-05T10:00:00 [D] [x] T and one letter",
        "2026/09/05 10:00:00 [INFO] [x] slashes",
        "2026/09-05 10:00:00 [INFO] [x] mixed date",
        "2026-09-05 10:00:00Z [INFO] [x] zone",
        "2026-09-05 10:00:00 [INFO]   [x]   spaces   ",
        "2026-09-05 10:00:00 [INFO]",
        "2026-09-05 10:00:00 [INFO] [x]",
        "2026-09-05 10:00:00 [INFO] [bad tag] msg",
        "2026-09-05 10:00:00 [INFO] [x]: colon",
        "2026-09-05 10:00:00 INFO user=bob module=pay no tag",
        "2026-09-05 10:00:00 [INFO] [x] user=\"a b\" quoted",
        "2026-09-05 10:00:00 Starting not a level",
        "2026-09-05 10:00:00 I think",
        "2026-02-30 10:00:00 [INFO] [x] no such day",
        "2026-09-05 23:59:60 [INFO] [x] leap second",
        "2026-09-05 10:00:00 - pay - WARNING - python",
        "2026-09-05 10:00:00 [INFO] [x] 10.0.0.1 - - [05/Sep/2026:10:00:00 +0000] \"GET /\" access inside",
        "2026-09-05 10:00:00 [INFO] [x] ünïcödé ✓",
    ];
    let mut fast_read = 0;
    for line in lines {
        let event = parse_line_with(0, line, None);
        let mut entry = LogEntry::from_event(&event);
        entry.extra = config.extra_spans(&entry);
        let slow = LineFacts::of_entry(&entry);
        if let Some(fast) = fast::read_line(line, &config) {
            assert_eq!(fast, slow, "{line}");
            fast_read += 1;
        }
    }
    assert!(fast_read >= 10, "the common lines read fast: {fast_read}");
}

// ------------------------------------------------- 3.3 the last lines first

#[test]
fn the_last_lines_read_before_indexing_reaches_them() {
    use log_gpui::index::tail::read_tail;
    let text = standard(5_000);
    let bytes = text.as_bytes();
    let source = BytesSource::new(bytes.to_vec());
    let tail = read_tail(&source, 4096, &mut LineParser::new()).unwrap();
    assert!(!tail.lines.is_empty());
    assert_eq!(tail.end, bytes.len() as u64);
    let all: Vec<&str> = text.lines().collect();
    let shown: Vec<&str> = tail.lines.iter().map(|e| e.raw_line.as_ref()).collect();
    assert_eq!(&all[all.len() - shown.len()..], &shown[..], "whole lines, the last ones");
    assert_eq!(&bytes[tail.start as usize..], (shown.join("\n") + "\n").as_bytes(), "starting at a line");
    // A file smaller than the read is read whole.
    let small = BytesSource::new(b"a\nb".to_vec());
    let tail = read_tail(&small, 4096, &mut LineParser::new()).unwrap();
    assert_eq!((tail.start, tail.lines.len()), (0, 2));
}
