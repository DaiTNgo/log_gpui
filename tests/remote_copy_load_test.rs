use log_gpui::live::{BreakKind, is_real_line_number};
use log_gpui::remote::copy::{self, CopyMeta, GenerationMeta, StopReason};
use log_gpui::space_view::ParsedFile;
use log_gpui::workspace::log_space::LogFile;

fn generation(distance: u32, name: &str, first: Option<usize>, lines: usize) -> GenerationMeta {
    GenerationMeta {
        distance,
        source_name: name.into(),
        compressed: false,
        start_offset: 0,
        end_offset: None,
        first_line_no: first,
        from: None,
        to: None,
        complete: true,
        stop: None,
        lines,
        older_not_read: false,
        file: GenerationMeta::file_name(distance),
    }
}

fn text(range: std::ops::RangeInclusive<usize>) -> String {
    range.map(|i| format!("2026-10-19 10:00:{:02} INFO line {i}\n", i % 60)).collect()
}

fn write_copy(generations: Vec<(GenerationMeta, String)>, limit: usize) -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let dir = copy::create_dir(root.path()).unwrap();
    for (g, content) in &generations {
        std::fs::write(dir.join(&g.file), content).unwrap();
    }
    let meta = CopyMeta {
        server_id: "s1".into(),
        server_name: "web1".into(),
        path: "/app/app.log".into(),
        read_at: "2026-10-19 10:00:00".into(),
        start_text: None,
        to_text: None,
        limit_lines: limit,
        generations: generations.into_iter().map(|(g, _)| g).collect(),
    };
    copy::write_meta(&dir, &meta).unwrap();
    (root, dir)
}

#[test]
fn generations_come_oldest_first_with_a_break_and_numbers_from_their_files() {
    let (_root, dir) = write_copy(
        vec![
            (generation(1, "app.log.1", Some(1), 3), text(1..=3)),
            (generation(0, "app.log", Some(1_500_001), 2), text(10..=11)),
        ],
        2_000_000,
    );
    let parsed = ParsedFile::from_remote_copy(&dir, "web1:/app/app.log").unwrap();
    let seen: Vec<(u32, usize)> = parsed.entries.iter().map(|e| (e.generation, e.index)).collect();
    assert_eq!(seen, [(0, 1), (0, 2), (0, 3), (1, 1_500_001), (1, 1_500_002)]);
    let mark = parsed.entries[3].break_before.as_ref().unwrap();
    assert_eq!(mark.kind, BreakKind::Rotated);
    assert_eq!(mark.label, "web1:/app/app.log rotated");
    assert_eq!(parsed.lines.len(), 5);
    assert_eq!(parsed.live.as_ref().unwrap().generation, 1);
}

#[test]
fn a_stopped_generation_gets_a_break_below_its_last_line() {
    let mut g = generation(0, "app.log", Some(1), 3);
    g.complete = false;
    g.stop = Some(StopReason::FullReadLimit);
    let (_root, dir) = write_copy(vec![(g, text(1..=3))], 2_000_000);
    let parsed = ParsedFile::from_remote_copy(&dir, "web1:/app/app.log").unwrap();
    let mark = parsed.entries.last().unwrap().break_after.as_ref().unwrap();
    assert_eq!(mark.kind, BreakKind::ReadLimit);
    assert_eq!(mark.label, "stopped at Full Read Limit (2,000,000 lines)");
    assert!(parsed.entries[0].break_after.is_none());
}

#[test]
fn older_lines_not_read_are_said_above_the_first_line() {
    let mut g = generation(0, "app.log", Some(8_000), 3);
    g.older_not_read = true;
    let (_root, dir) = write_copy(vec![(g, text(1..=3))], 2_000_000);
    let parsed = ParsedFile::from_remote_copy(&dir, "web1:/app/app.log").unwrap();
    assert_eq!(parsed.entries[0].break_before.as_ref().unwrap().kind, BreakKind::OlderNotRead);
    assert_eq!(parsed.entries[0].index, 8_000);
}

#[test]
fn numbers_that_could_not_be_counted_are_not_real() {
    let (_root, dir) = write_copy(vec![(generation(0, "app.log", None, 2), text(1..=2))], 2_000_000);
    let parsed = ParsedFile::from_remote_copy(&dir, "web1:/app/app.log").unwrap();
    assert!(!is_real_line_number(parsed.entries[0].index));
}

#[test]
fn a_copy_whose_read_never_finished_is_read_as_cancelled() {
    let root = tempfile::tempdir().unwrap();
    let dir = copy::create_dir(root.path()).unwrap();
    std::fs::write(dir.join("gen-1.log"), text(1..=2)).unwrap();
    std::fs::write(dir.join("gen-0.log"), text(3..=4)).unwrap();
    let parsed = ParsedFile::from_remote_copy(&dir, "web1:/app/app.log").unwrap();
    assert_eq!(parsed.entries.len(), 4);
    assert_eq!(parsed.entries[1].break_after.as_ref().unwrap().kind, BreakKind::ReadCancelled);
    assert_eq!(parsed.entries[3].break_after.as_ref().unwrap().kind, BreakKind::ReadCancelled);
    // An empty folder is an error, not an empty file.
    let empty = copy::create_dir(root.path()).unwrap();
    assert!(ParsedFile::from_remote_copy(&empty, "x").is_err());
}

#[test]
fn the_file_is_a_remote_copy_named_after_the_server_and_path() {
    let file = LogFile::remote_copy("/data/remote-copies/abc-1", "web1", "/app/app.log");
    assert!(file.is_remote_copy());
    assert_eq!(file.display_name, "web1:/app/app.log");
    assert_eq!(file.file_id, "remote-copy:abc-1");
    assert!(file.path.is_none() && file.remote.is_none());
    let json = serde_json::to_string(&file).unwrap();
    let back: LogFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.copy.as_deref(), Some("/data/remote-copies/abc-1"));
    // Sessions from before have no copy.
    let old: LogFile = serde_json::from_str(r#"{"file_id":"a","display_name":"a.log"}"#).unwrap();
    assert!(old.copy.is_none());
}
