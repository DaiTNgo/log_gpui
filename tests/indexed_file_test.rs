//! A local file read through its Line Index (task 4b.2): its entries,
//! read again from the file, are those parsing it whole gives.

use log_gpui::index::file::{IndexedFile, READ_AROUND};
use log_gpui::index::indexing::IndexOptions;
use log_gpui::index::{BytesSource, DiskSource, HandleBudget, LineSource};
use log_gpui::parser::text::LineConfig;
use log_gpui::space_view::ParsedFile;
use std::sync::Arc;

fn text() -> String {
    let mut out = String::new();
    for i in 0..2000 {
        if i % 9 == 4 {
            out += "java.lang.IllegalStateException: boom\n\tat a.b(C.java:12)\n";
        }
        out += &format!("2026-09-05 10:{:02}:{:02}.{:03} [INFO] [api] request {i} user={}\n", i / 60 % 60, i % 60, i % 1000, i % 7);
    }
    out
}

fn debug(e: &log_gpui::model::LogEntry) -> String {
    format!("{e:?}")
}

#[test]
fn entries_read_again_are_those_of_the_whole_file() {
    let text = text();
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.clone().into_bytes()));
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    let want = ParsedFile::from_bytes(text.as_bytes()).entries();
    assert_eq!(file.len(), want.len());
    // Out of order, across block edges, and twice (the second from the cache).
    let order: Vec<usize> = (0..want.len()).rev().chain([READ_AROUND - 1, READ_AROUND, READ_AROUND + 1]).collect();
    for n in order {
        assert_eq!(debug(&file.entry(n).unwrap()), debug(&want[n]), "line {n}");
    }
    assert!(file.entry(want.len()).is_none());
    assert!(file.meta(5).is_some());
}

#[test]
fn detected_fields_are_read_with_the_lines() {
    let text = text();
    let config = Arc::new(LineConfig::new(&["user".to_string()]));
    let options = IndexOptions { config: config.clone(), ..Default::default() };
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.clone().into_bytes()));
    let file = IndexedFile::index_now(source, options).unwrap();
    let mut want = ParsedFile::from_bytes(text.as_bytes()).entries();
    config.apply(&mut want);
    for n in [0, 7, 500, want.len() - 1] {
        assert_eq!(debug(&file.entry(n).unwrap()), debug(&want[n]), "line {n}");
    }
    let user = file.indexed.data.read_recursive().filter.column("user").map(|c| c.values.len());
    assert_eq!(user, Some(7), "the Filter column of the Detected Field");
}

#[test]
fn the_cache_keeps_a_few_screens_only() {
    let text = text();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, &text).unwrap();
    let source: Arc<dyn LineSource> = Arc::new(DiskSource::stay_open(&path, HandleBudget::new(4)).unwrap());
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    for n in 0..file.len() {
        file.entry(n).unwrap();
    }
    let shard = log_gpui::index::view_rows::ROW_CACHE_BYTES / 16;
    assert!(file.cached_weight() <= shard * 5 / 4, "{} bytes kept", file.cached_weight());
}

#[test]
fn the_raw_view_reads_an_indexed_file_s_lines() {
    use log_gpui::raw_view::RawLines;
    let text = text();
    let source: Arc<dyn LineSource> = Arc::new(BytesSource::new(text.clone().into_bytes()));
    let file = Arc::new(IndexedFile::index_now(source, IndexOptions::default()).unwrap());
    let held = ParsedFile::from_bytes(text.as_bytes()).lines();
    let lines = RawLines::Indexed(file.clone());
    assert_eq!(lines.len(), held.len());
    assert_eq!(lines.get(7), held.get(7).cloned(), "one line, read from the file");
    let all = lines.all();
    assert_eq!(*all, *held, "every line");
    assert!(!Arc::ptr_eq(&all, &lines.all()), "read again each time: the file keeps no copy of its text");
    assert_eq!(lines.get(9), held.get(9).cloned());
}

#[test]
fn lines_written_since_are_indexed_on() {
    use log_gpui::index::file::Extended;
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.log");
    let first = text();
    // The last line is still being written: no line break yet.
    std::fs::write(&path, format!("{first}2026-09-05 11:00:00 [ERROR] [db] cut")).unwrap();
    let source: Arc<dyn LineSource> = Arc::new(DiskSource::stay_open(&path, HandleBudget::new(4)).unwrap());
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    let before = file.len();
    assert_eq!(file.extend().unwrap(), Extended::Same, "nothing new");

    // More of the cut line, still without its line break: it waits.
    let mut out = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    out.write_all(b" off").unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Same);

    // Its line break and two more lines (one a stack line).
    out.write_all(b" here\n\tat x.y(Z.java:1)\n2026-09-05 11:00:01 [INFO] [db] next user=3\n").unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Appended { range: before - 1..before + 2, replaced_last: true, more: false });
    let all = std::fs::read(&path).unwrap();
    let want = ParsedFile::from_bytes(&all).entries();
    assert_eq!(file.len(), want.len());
    for n in [0, before - 2, before - 1, before, before + 1] {
        assert_eq!(debug(&file.entry(n).unwrap()), debug(&want[n]), "line {n}");
    }
    // The Filter columns are those of the whole file.
    let whole = log_gpui::filter_index::FilterIndex::build(&want);
    let got = file.indexed.data.read_recursive();
    for key in ["level", "module", "user"] {
        let (a, b) = (got.filter.column(key).map(|c| c.values.clone()), whole.column(key).map(|c| c.values.clone()));
        assert_eq!(a, b, "{key}");
    }
    assert_eq!(got.filter.parent_lines(), whole.parent_lines());
    drop(got);

    // Cut shorter than what was read: told, not read wrongly.
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(10).unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Shrunk);
}

#[test]
fn a_file_written_over_in_place_and_longer_is_not_read_on() {
    use log_gpui::index::file::Extended;
    use std::io::Write;
    let old: String = (0..50).map(|i| format!("2026-09-05 10:00:{:02} [INFO] old {i}\n", i % 60)).collect();
    let (_dir, path, file) = indexed_on_disk(&old);
    // `cp bigger.log app.log`: the same inode, longer, other text.
    let bigger: String = (0..80).map(|i| format!("2026-09-05 11:00:{:02} [WARN] new line {i}\n", i % 60)).collect();
    std::fs::OpenOptions::new().write(true).truncate(false).open(&path).unwrap().write_all(bigger.as_bytes()).unwrap();
    assert_eq!(file.extend().unwrap(), Extended::Rewritten);
    assert_eq!(file.len(), 50, "nothing is read at the old offsets");
}

/// `text` written to `app.log` in a fresh folder, indexed and kept open.
fn indexed_on_disk(text: &str) -> (tempfile::TempDir, std::path::PathBuf, IndexedFile) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.log");
    std::fs::write(&path, text).unwrap();
    let source: Arc<dyn LineSource> = Arc::new(DiskSource::stay_open(&path, HandleBudget::new(4)).unwrap());
    let file = IndexedFile::index_now(source, IndexOptions::default()).unwrap();
    (dir, path, file)
}

#[test]
fn a_file_changed_on_disk_is_told_apart_from_one_written_on() {
    use log_gpui::index::file::OnDisk;
    use std::io::Write;
    let first = text();

    // Unchanged, then written on.
    let (_dir, path, file) = indexed_on_disk(&first);
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Same);
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Same, "the second time from metadata only");
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"2026-09-05 11:00:00 [INFO] [api] more\n").unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Appended);
    file.extend().unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Same, "the new lines are indexed");

    // `mv other app.log`: another inode at the path.
    let (dir, path, file) = indexed_on_disk(&first);
    let other = dir.path().join("other.log");
    std::fs::write(&other, "2026-09-05 12:00:00 [INFO] [api] another file\n").unwrap();
    std::fs::rename(&other, &path).unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Replaced);

    // `cp bigger app.log`: the same inode, longer, other bytes.
    let (_dir, path, file) = indexed_on_disk(&first);
    std::fs::write(&path, first.replace("request", "REQUEST") + &first).unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Replaced);

    // The same size, only the last line other.
    let (_dir, path, file) = indexed_on_disk(&first);
    let mut same_size = first.clone().into_bytes();
    let at = same_size.len() - 3;
    same_size[at] = b'X';
    std::fs::write(&path, same_size).unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Replaced);

    // Cut shorter (copytruncate), then deleted.
    let (_dir, path, file) = indexed_on_disk(&first);
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Replaced);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(file.changed_on_disk(&path).unwrap(), OnDisk::Gone);
}

/// The Generations of a file end to end: an older one keeps the bytes it had
/// (a line break added after a last line that had none), whatever is
/// written to it since, and the new one's lines follow.
#[test]
fn a_chain_reads_each_generation_through_its_own_handle() {
    use log_gpui::index::ChainSource;
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let (old, new) = (dir.path().join("app.log.1"), dir.path().join("app.log"));
    std::fs::write(&old, "a\nb").unwrap();
    std::fs::write(&new, "c\n").unwrap();
    let open = |p: &std::path::Path| Arc::new(DiskSource::stay_open(p, HandleBudget::new(4)).unwrap()) as Arc<dyn LineSource>;
    let chain = ChainSource::new(open(&old));
    assert_eq!(chain.push(open(&new)).unwrap(), 4, "b, then its added line break");
    std::fs::OpenOptions::new().append(true).open(&old).unwrap().write_all(b"late\n").unwrap();
    assert_eq!(chain.len().unwrap(), 6);
    assert_eq!(chain.read_range(0, 100).unwrap(), b"a\nb\nc\n");
    assert_eq!(chain.read_range(3, 2).unwrap(), b"\nc", "across the two");
    std::fs::OpenOptions::new().append(true).open(&new).unwrap().write_all(b"d\n").unwrap();
    assert_eq!(chain.read_range(4, 100).unwrap(), b"c\nd\n", "the newest one grows");
    assert_eq!(chain.generations(), 2);
}

/// A file that grew by more than a batch is indexed over several calls
/// (task 9.1), each a batch of whole lines.
#[test]
fn a_file_that_grew_a_lot_is_indexed_a_batch_at_a_time() {
    use log_gpui::index::file::{EXTEND_BYTES, Extended};
    let source = Arc::new(BytesSource::new(text().into_bytes()));
    let file = IndexedFile::index_now(source.clone(), IndexOptions::default()).unwrap();
    let before = file.len();
    let line = "2026-09-05 11:00:00 [INFO] [db] a line written in a burst of many lines\n";
    let lines = EXTEND_BYTES * 5 / 2 / line.len();
    source.append(line.repeat(lines).as_bytes());
    let mut calls = 0;
    loop {
        calls += 1;
        match file.extend().unwrap() {
            Extended::Appended { range, more: true, .. } => assert!(range.len() * line.len() <= EXTEND_BYTES),
            Extended::Appended { more: false, .. } => break,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(calls, 3);
    assert_eq!(file.len(), before + lines);
}

/// A source whose reads take a while once `slow` is set, saying when one
/// is under way.
struct SlowSource {
    bytes: std::sync::Mutex<Vec<u8>>,
    slow: std::sync::atomic::AtomicBool,
    reading: std::sync::atomic::AtomicBool,
}

impl log_gpui::index::LineSource for SlowSource {
    fn len(&self) -> std::io::Result<u64> {
        Ok(self.bytes.lock().unwrap().len() as u64)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::sync::atomic::Ordering;
        if self.slow.load(Ordering::SeqCst) {
            self.reading.store(true, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let bytes = self.bytes.lock().unwrap();
        let start = (offset as usize).min(bytes.len());
        let n = buf.len().min(bytes.len() - start);
        buf[..n].copy_from_slice(&bytes[start..start + n]);
        Ok(n)
    }
}

/// Lines written since are read from the file with no lock on the index:
/// it is locked to write only to add them.
#[test]
fn reading_new_lines_leaves_the_index_readable() {
    use std::sync::atomic::Ordering;
    let line = |i: usize| format!("2026-09-05 10:00:00 [INFO] line {i}\n");
    let source = std::sync::Arc::new(SlowSource {
        bytes: std::sync::Mutex::new((0..100).flat_map(|i| line(i).into_bytes()).collect()),
        slow: false.into(),
        reading: false.into(),
    });
    let file = std::sync::Arc::new(log_gpui::index::file::IndexedFile::index_now(source.clone(), Default::default()).unwrap());
    source.bytes.lock().unwrap().extend((100..200).flat_map(|i| line(i).into_bytes()));
    source.slow.store(true, Ordering::SeqCst);
    let extending = std::thread::spawn({
        let file = file.clone();
        move || file.extend().unwrap()
    });
    while !source.reading.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }
    assert!(file.indexed.data.try_write().is_some(), "not locked while the new lines are read");
    let extended = extending.join().unwrap();
    assert!(matches!(extended, log_gpui::index::file::Extended::Appended { .. }), "{extended:?}");
    assert_eq!(file.len(), 200);
}

/// A view holds the index while a row it reads locks it again; a writer
/// waiting meanwhile (Indexing) does not make that read wait for it.
#[test]
fn a_row_read_while_the_index_is_held_does_not_wait_for_a_writer() {
    let text: String = (0..1000).map(|i| format!("2026-09-05 10:00:00 [INFO] line {i}\n")).collect();
    let file = Arc::new(IndexedFile::index_now(Arc::new(BytesSource::new(text.into_bytes())), Default::default()).unwrap());
    file.forget_cached();
    let held = file.indexed.data.read_recursive();
    let writer = std::thread::spawn({
        let file = file.clone();
        move || drop(file.indexed.data.write())
    });
    // Let the writer start waiting.
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(file.entry(500).is_some(), "the row is read");
    drop(held);
    writer.join().unwrap();
}
