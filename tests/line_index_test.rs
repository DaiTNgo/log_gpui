//! The Line Index and what it reads from (openspec change
//! `line-offset-index`, tasks 2.1–2.4).

use log_gpui::index::handles::HandleBudget;
use log_gpui::index::meta::{NO_TIME, micros};
use log_gpui::index::{
    BytesSource, DiskSource, FileIndex, LineIndex, LineSource, index_source, read_entries, read_lines,
};
use log_gpui::parser::text::{LineParser, parse_entries};
use log_gpui::space_view::ParsedFile;
use log_gpui::timeline::effective_timestamps;
use std::io::Write;

fn write_file(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

// ------------------------------------------------------------ 2.1 LineSource

#[test]
fn disk_source_reads_the_middle_and_stops_at_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "a.log", b"0123456789");
    let source = DiskSource::on_demand(&path, HandleBudget::new(8));
    assert_eq!(source.len().unwrap(), 10);
    assert_eq!(source.read_range(3, 4).unwrap(), b"3456");
    assert_eq!(source.read_range(8, 10).unwrap(), b"89", "a read past the end is short");
    assert_eq!(source.read_range(20, 4).unwrap(), b"", "a read after the end is empty");
}

#[test]
fn disk_source_sees_the_file_change_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "a.log", b"one\n");
    let source = DiskSource::stay_open(&path, HandleBudget::new(8)).unwrap();
    assert_eq!(source.len().unwrap(), 4);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"two\n").unwrap();
    assert_eq!(source.len().unwrap(), 8);
    assert_eq!(source.read_range(4, 4).unwrap(), b"two\n");
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(2).unwrap();
    assert_eq!(source.len().unwrap(), 2);
    assert_eq!(source.read_range(0, 8).unwrap(), b"on");
}

#[test]
fn a_source_kept_open_reads_its_file_after_a_rename() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "app.log", b"old\n");
    let source = DiskSource::stay_open(&path, HandleBudget::new(8)).unwrap();
    std::fs::rename(&path, dir.path().join("app.log.1")).unwrap();
    write_file(&dir, "app.log", b"new file\n");
    assert_eq!(source.read_range(0, 4).unwrap(), b"old\n");
}

#[test]
fn bytes_source_reads_and_grows() {
    let source = BytesSource::new(b"abc".to_vec());
    assert_eq!(source.read_range(1, 10).unwrap(), b"bc");
    source.append(b"def");
    assert_eq!(source.len().unwrap(), 6);
    assert_eq!(source.read_range(2, 3).unwrap(), b"cde");
    assert_eq!(source.read_range(9, 3).unwrap(), b"");
}

// ----------------------------------------------------------- 2.1b handles

#[test]
fn a_file_read_on_demand_closes_after_its_pass() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "a.log", b"x\n");
    let budget = HandleBudget::new(8);
    let source = DiskSource::on_demand(&path, budget.clone());
    assert!(!source.is_open(), "nothing is open before the first read");
    index_source(&source, None).unwrap();
    assert!(!source.is_open());
    assert_eq!(budget.open(), 0);
}

#[test]
fn three_hundred_files_index_within_a_budget_of_256() {
    let dir = tempfile::tempdir().unwrap();
    let budget = HandleBudget::new(256);
    let sources: Vec<DiskSource> = (0..300)
        .map(|i| {
            let path = write_file(&dir, &format!("{i}.log"), format!("2026-09-05 10:00:{:02} INFO line {i}\n", i % 60).as_bytes());
            DiskSource::on_demand(path, budget.clone())
        })
        .collect();
    let mut most = 0;
    for source in &sources {
        let index = index_source(source, None).unwrap();
        assert_eq!(index.len(), 1);
        most = most.max(budget.open());
    }
    assert!(most <= 1, "each file closes after its pass, {most} were open");
    // Reading a screen of every file again opens and closes them the same way.
    for source in &sources {
        let index = index_source(source, None).unwrap();
        read_lines(source, &index.lines, 0..1).unwrap();
        source.release();
    }
    assert_eq!(budget.open(), 0);
}

#[test]
fn the_budget_says_so_before_it_runs_out() {
    let dir = tempfile::tempdir().unwrap();
    let budget = HandleBudget::new(10);
    let kept: Vec<DiskSource> = (0..9)
        .map(|i| DiskSource::stay_open(write_file(&dir, &format!("{i}.log"), b"x\n"), budget.clone()).unwrap())
        .collect();
    assert!(budget.near_limit(), "9 of 10 open is near the limit");
    let tenth = DiskSource::stay_open(write_file(&dir, "9.log", b"x\n"), budget.clone()).unwrap();
    let error = DiskSource::stay_open(write_file(&dir, "10.log", b"x\n"), budget.clone())
        .err()
        .expect("the eleventh open fails");
    assert!(error.to_string().contains("too many files open"), "{error}");
    drop(tenth);
    drop(kept);
    assert_eq!(budget.open(), 0);
}

// --------------------------------------------------------- 2.2 LineIndex

fn index_of(bytes: &[u8]) -> FileIndex {
    index_source(&BytesSource::new(bytes.to_vec()), None).unwrap()
}

fn texts(bytes: &[u8]) -> Vec<String> {
    let index = index_of(bytes);
    read_lines(&BytesSource::new(bytes.to_vec()), &index.lines, 0..index.len()).unwrap()
}

/// The lines as `str::lines` reads them, which the parser uses.
fn str_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes).lines().map(str::to_string).collect()
}

#[test]
fn lines_read_back_as_str_lines_reads_them() {
    let cases: [&[u8]; 8] = [
        b"",
        b"\n",
        b"a\n\nb\n",
        b"no break at the end",
        b"crlf\r\nline\r\n",
        b"lone\rcr\n",
        b"ends in cr\r",
        b"bad \xff utf8\nok\n",
    ];
    for bytes in cases {
        assert_eq!(texts(bytes), str_lines(bytes), "{:?}", String::from_utf8_lossy(bytes));
    }
}

#[test]
fn an_empty_file_has_no_lines() {
    let index = index_of(b"");
    assert_eq!(index.len(), 0);
    assert_eq!(index.lines.span(0), None);
}

#[test]
fn a_line_of_one_megabyte() {
    let mut bytes = vec![b'x'; 1 << 20];
    bytes.extend_from_slice(b"\nshort\n");
    let index = index_of(&bytes);
    assert_eq!(index.len(), 2);
    assert_eq!(index.lines.span(0), Some((0, (1 << 20) + 1)));
    assert_eq!(index.lines.span(1), Some(((1 << 20) + 1, 6)));
    assert_eq!(texts(&bytes)[1], "short");
}

#[test]
fn spans_across_many_blocks_match_plain_offsets() {
    let mut index = LineIndex::new();
    let mut offsets = Vec::new();
    let mut end = 0u64;
    for i in 0..1000u64 {
        let len = (i * 37) % 300 + if i % 97 == 0 { 70_000 } else { 0 };
        offsets.push((end, len));
        index.push(len);
        end += len;
    }
    for (n, &span) in offsets.iter().enumerate() {
        assert_eq!(index.span(n), Some(span), "line {n}");
    }
    assert_eq!(index.lens(126, 131), offsets[126..131].iter().map(|s| s.1).collect::<Vec<_>>());
    assert_eq!(index.byte_range(10, 300), Some(offsets[10].0..offsets[299].0 + offsets[299].1));
    assert!(index.heap_bytes() < 1000 * 8, "smaller than a u64 per line");
}

#[test]
fn pop_and_truncate_give_back_the_index_as_it_was() {
    let mut index = LineIndex::new();
    let mut before = None;
    for i in 0..300u64 {
        if i == 128 {
            before = Some(index.clone());
        }
        index.push(i + 1);
    }
    index.truncate(128);
    assert_eq!(Some(index.clone()), before);
    assert_eq!(index.pop(), Some(128));
    assert_eq!(index.len(), 127);
    assert_eq!(index.end(), (1..=127).sum::<u64>());
}

// ---------------------------------------------------------- 2.3 LineMeta

/// Files of every shape the parser knows, and some it does not.
fn samples() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formats");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        out.push((path.display().to_string(), std::fs::read(&path).unwrap()));
    }
    let made: [(&str, &str); 5] = [
        (
            "stack trace",
            "2026-09-05 10:00:00.120 [ERROR] [api] failed\njava.lang.Exception: boom\n\tat a.b(C.java:1)\n\n2026-09-05 10:00:01 [INFO] [api] next\n",
        ),
        ("untimed head", "banner\n---\n2026-09-05 10:00:00 [INFO] [db] up\ncontinued\n"),
        ("no time at all", "just text\nmore text\n\n"),
        ("crlf", "2026-09-05 10:00:00 [WARN] [x] a\r\n  more\r\n2026-09-05 10:00:02 [DEBUG] [x] b\r\n"),
        (
            "mixed formats",
            "2026-09-05T10:00:00Z INFO iso line\n[05/Sep/2026:10:00:01 +0000] GET /\n2026-09-05 10:00:02,123 ERROR log4j-ish\njunk in between\n{\"time\":\"2026-09-05T10:00:03Z\",\"level\":\"warn\",\"msg\":\"json\"}\n",
        ),
    ];
    for (name, text) in made {
        out.push((name.to_string(), text.as_bytes().to_vec()));
    }
    let mut big = String::new();
    for i in 0..1000 {
        if i % 7 == 3 {
            big.push_str("    at continuation of the line above\n");
        } else {
            big.push_str(&format!("2026-09-05 10:{:02}:{:02}.{:03} [INFO] [w] line {i} user={}\n", i / 60 % 60, i % 60, i, i % 13));
        }
    }
    out.push(("1000 lines".into(), big.into_bytes()));
    out
}

#[test]
fn line_meta_matches_the_parsed_entries() {
    for (name, bytes) in samples() {
        let index = index_of(&bytes);
        let entries = parse_entries(&String::from_utf8_lossy(&bytes));
        assert_eq!(index.len(), entries.len(), "{name}");
        let effective = effective_timestamps(&entries);
        assert_eq!(index.meta.has_any_time(), effective.is_some(), "{name}");
        for (n, entry) in entries.iter().enumerate() {
            let meta = index.meta.get(n).unwrap();
            let ts = effective.as_ref().map_or(NO_TIME, |ts| micros(ts[n]));
            assert_eq!(meta.ts_us, ts, "{name}: timestamp of line {n}");
            assert_eq!(meta.has_time(), entry.has_time, "{name}: has_time of line {n}");
            assert_eq!(meta.level(), entry.level, "{name}: level of line {n}");
            assert_eq!(meta.parent_of(n).map(|p| p + 1), entry.parent, "{name}: Parent Line of line {n}");
        }
    }
}

#[test]
fn bytes_that_are_not_utf8_are_flagged() {
    let index = index_of(b"fine\nbad \xfe\n");
    let not_utf8 = |n: usize| index.meta.get(n).unwrap().flags & log_gpui::index::meta::flags::NOT_UTF8 != 0;
    assert!(!not_utf8(0));
    assert!(not_utf8(1));
}

#[test]
fn generations_and_streams_are_runs_of_lines() {
    use log_gpui::model::Stream;
    let mut meta = log_gpui::index::MetaTable::new();
    meta.generations.set(0, 0);
    meta.generations.set(50, 0);
    meta.generations.set(100, 1);
    meta.streams.set(0, Some(Stream::Stdout));
    meta.streams.set(10, Some(Stream::Stderr));
    meta.streams.set(12, Some(Stream::Stdout));
    assert_eq!(meta.generations.runs().len(), 2);
    assert_eq!(meta.generations.get(99), Some(0));
    assert_eq!(meta.generations.get(100), Some(1));
    assert_eq!(meta.streams.get(11), Some(Some(Stream::Stderr)));
    assert_eq!(meta.streams.get(500), Some(Some(Stream::Stdout)));
    meta.generations.truncate(100);
    assert_eq!(meta.generations.get(150), Some(0));
}

#[test]
fn truncating_meta_unwinds_the_head_timestamp() {
    let bytes = b"head\n2026-09-05 10:00:00 [INFO] [a] first\n";
    let mut index = index_of(bytes);
    assert_ne!(index.meta.get(0).unwrap().ts_us, NO_TIME, "the head took the first timestamp");
    index.meta.truncate(1);
    assert_eq!(index.meta.get(0).unwrap().ts_us, NO_TIME);
    assert!(!index.meta.has_any_time());
}

// ------------------------------------------------ 2.4 reading lines again

fn debug(entries: &[log_gpui::model::LogEntry]) -> Vec<String> {
    entries.iter().map(|e| format!("{e:?}")).collect()
}

#[test]
fn lines_read_again_parse_as_the_whole_file_did() {
    for (name, bytes) in samples() {
        let parsed = ParsedFile::from_bytes(&bytes);
        let source = BytesSource::new(bytes.clone());
        let index = index_source(&source, None).unwrap();
        let whole = read_entries(&source, &index, 0..index.len(), &mut LineParser::new()).unwrap();
        assert_eq!(debug(&whole), debug(&parsed.entries), "{name}: every line");
        // Any stretch, starting on a Continuation Line or not.
        for start in 0..index.len() {
            let end = (start + 5).min(index.len());
            let part = read_entries(&source, &index, start..end, &mut LineParser::new()).unwrap();
            assert_eq!(debug(&part), debug(&parsed.entries[start..end]), "{name}: lines {start}..{end}");
        }
    }
}

#[test]
fn lines_read_again_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (_, bytes) = samples().pop().unwrap();
    let path = write_file(&dir, "big.log", &bytes);
    let source = DiskSource::on_demand(&path, HandleBudget::new(4));
    let index = index_source(&source, None).unwrap();
    let parsed = ParsedFile::from_bytes(&bytes);
    let screen = read_entries(&source, &index, 500..560, &mut LineParser::new()).unwrap();
    assert_eq!(debug(&screen), debug(&parsed.entries[500..560]));
}

#[test]
fn a_file_that_shrank_does_not_read_wrong_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_file(&dir, "a.log", b"one\ntwo\nthree\n");
    let source = DiskSource::on_demand(&path, HandleBudget::new(4));
    let index = index_source(&source, None).unwrap();
    std::fs::write(&path, b"one\n").unwrap();
    assert_eq!(read_lines(&source, &index.lines, 0..1).unwrap(), ["one"]);
    let error = read_lines(&source, &index.lines, 1..3).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
}
