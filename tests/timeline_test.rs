use log_gpui::filter::FilterState;
use log_gpui::model::LogEntry;
use log_gpui::space_view::{
    ParsedFile, apply_markers_merged, build_file_view, build_space_view,
};
use log_gpui::timeline::{effective_timestamps, materialize, merge, union_field_names};
use log_gpui::workspace::log_space::{LogSpace, LogSpaceKind};
use std::collections::HashMap;

fn entries(text: &str) -> Vec<LogEntry> {
    ParsedFile::from_text(text).entries
}

/// The merged rows as `(file index, line number)` pairs.
fn order(files: &[&[LogEntry]]) -> Vec<(u32, usize)> {
    let merged = merge(files);
    merged
        .rows
        .iter()
        .map(|r| (r.file, files[r.file as usize][r.line as usize].index))
        .collect()
}

#[test]
fn rows_of_two_files_interleave_by_timestamp() {
    let api = entries(
        "2026-09-05 10:00:01 [INFO] [api] first\n2026-09-05 10:00:03 [INFO] [api] third\n",
    );
    let worker = entries("2026-09-05 10:00:02 [INFO] [job] second\n");

    assert_eq!(order(&[&api, &worker]), vec![(0, 1), (1, 1), (0, 2)]);
}

#[test]
fn equal_timestamps_follow_the_file_order_then_the_line() {
    let api = entries("2026-09-05 10:00:05 [INFO] [api] a1\n2026-09-05 10:00:05 [INFO] [api] a2\n");
    let worker = entries("2026-09-05 10:00:05 [INFO] [job] w1\n");

    assert_eq!(order(&[&api, &worker]), vec![(0, 1), (0, 2), (1, 1)]);
    // With the files the other way round, the worker's line comes first.
    assert_eq!(order(&[&worker, &api]), vec![(0, 1), (1, 1), (1, 2)]);
}

#[test]
fn a_stack_trace_stays_with_its_parent_row() {
    let api = entries(
        "2026-09-05 10:00:01.000 [ERROR] [api] boom\n    at a.b.C(C.java:1)\n    at a.b.D(D.java:2)\n",
    );
    let worker = entries("2026-09-05 10:00:01.500 [INFO] [job] later\n");

    assert_eq!(order(&[&api, &worker]), vec![(0, 1), (0, 2), (0, 3), (1, 1)]);
}

#[test]
fn lines_before_the_first_timestamp_take_the_first_timestamp() {
    let log = entries("header line\n2026-09-05 10:00:09 [INFO] [api] first\n");
    let ts = effective_timestamps(&log).unwrap();
    assert_eq!(ts[0], ts[1]);

    let other = entries("2026-09-05 10:00:05 [INFO] [job] earlier\n");
    // The header sorts with its file's first row, after the other file's earlier row.
    assert_eq!(order(&[&log, &other]), vec![(1, 1), (0, 1), (0, 2)]);
}

#[test]
fn a_file_without_timestamps_goes_last_and_is_flagged() {
    let notes = entries("just a note\nanother note\n");
    let api = entries("2026-09-05 10:00:01 [INFO] [api] x\n2026-09-05 10:00:02 [INFO] [api] y\n");

    let merged = merge(&[&notes, &api]);
    let files: Vec<u32> = merged.rows.iter().map(|r| r.file).collect();
    assert_eq!(files, vec![1, 1, 0, 0]);
    assert_eq!(merged.no_timestamp_files, vec![0]);
    assert!(merged.effective_ts[2].is_nan());
}

#[test]
fn timestamps_are_compared_as_written_without_timezone_conversion() {
    // 10:00 UTC and 17:00 Vietnam time are the same instant, but the clocks differ.
    let utc = entries("2026-09-05T10:00:00Z [INFO] [a] utc\n");
    let vn = entries("2026-09-05 17:00:00 [INFO] [b] vietnam\n");
    assert_eq!(order(&[&vn, &utc]), vec![(1, 1), (0, 1)]);
}

#[test]
fn merging_again_with_another_file_order_does_not_parse_again() {
    let a = entries("2026-09-05 10:00:00 [INFO] [a] x\n");
    let b = entries("2026-09-05 10:00:00 [INFO] [b] y\n");
    let ab = merge(&[&a, &b]);
    let ba = merge(&[&b, &a]);
    assert_eq!(ab.rows.len(), ba.rows.len());
    assert_eq!(ab.rows[0].file, 0);
    assert_eq!(ba.rows[0].file, 0); // file 0 is now b
}

#[test]
fn each_file_keeps_its_own_line_order_even_when_it_goes_back_in_time() {
    let a = entries(
        "2026-09-05 10:00:05 [INFO] [a] late\n2026-09-05 10:00:01 [INFO] [a] early\n",
    );
    let merged = merge(&[&a]);
    let lines: Vec<u32> = merged.rows.iter().map(|r| r.line).collect();
    assert_eq!(lines, vec![0, 1]);
}

// ─── Columns and the Filter index ────────────────────────────────────────────

fn space_with(files: &[(&str, &str)]) -> (LogSpace, HashMap<String, ParsedFile>) {
    let mut space = LogSpace::new("s".into(), "Payment".into(), LogSpaceKind::Imported);
    let paths: Vec<String> = files.iter().map(|(p, _)| p.to_string()).collect();
    space.add_imported_files(&paths).unwrap();
    let parsed = files
        .iter()
        .map(|(p, text)| (p.to_string(), ParsedFile::from_text(text)))
        .collect();
    (space, parsed)
}

#[test]
fn columns_are_united_by_name() {
    let json = r#"{"time":"2026-09-05T10:00:00Z","level":"info","msg":"x","region":"eu"}"#;
    let plain = "2026-09-05 10:00:01 [INFO] [nginx] y";
    let (space, parsed) = space_with(&[("/a.log", json), ("/b.log", plain)]);
    let view = build_space_view(&space, &parsed);

    // `region` exists only in the first file: the rows of the other file leave it empty.
    let region = view.index.column("region").expect("union has the column");
    let empty_rows = view
        .entries
        .iter()
        .enumerate()
        .filter(|(row, _)| region.id_of_row(*row) == Some(u16::MAX))
        .count();
    assert_eq!(empty_rows, 1);

    let names = union_field_names(&[&parsed["/a.log"].entries, &parsed["/b.log"].entries]);
    assert!(names.contains(&"region".to_string()));
}

#[test]
fn the_first_files_column_order_comes_first() {
    let a = r#"{"time":"2026-09-05T10:00:00Z","zeta":"1","alpha":"1"}"#;
    let b = r#"{"time":"2026-09-05T10:00:01Z","beta":"1","zeta":"1"}"#;
    let (_, parsed) = space_with(&[("/a.log", a), ("/b.log", b)]);
    let names = union_field_names(&[&parsed["/a.log"].entries, &parsed["/b.log"].entries]);
    let pos = |n: &str| names.iter().position(|x| x == n).unwrap();
    assert!(pos("zeta") < pos("alpha") || pos("alpha") < pos("zeta")); // both from file one
    assert!(pos("alpha").max(pos("zeta")) < pos("beta"), "new columns come after: {names:?}");
}

#[test]
fn the_source_column_is_filterable_by_file_name() {
    let (space, parsed) = space_with(&[
        ("/logs/api.log", "2026-09-05 10:00:01 [INFO] [api] a\n"),
        ("/logs/worker.log", "2026-09-05 10:00:02 [INFO] [job] b\n"),
    ]);
    let mut view = build_space_view(&space, &parsed);

    let source = view.index.column("source").expect("Source is a column");
    assert!(source.is_filterable());
    assert_eq!(source.values.len(), 2);

    let mut filter = FilterState::new();
    filter.toggle_value("source", "api.log");
    let visible = filter.evaluate(&view.index, &mut view.entries);
    assert_eq!(visible.len(), 1);
    assert_eq!(view.entries[visible[0]].source.as_deref(), Some("api.log"));
}

#[test]
fn the_filter_values_come_from_every_file() {
    let (space, parsed) = space_with(&[
        ("/a.log", "2026-09-05 10:00:01 [INFO] [API] a\n"),
        ("/b.log", "2026-09-05 10:00:02 [INFO] [JOB] b\n"),
    ]);
    let view = build_space_view(&space, &parsed);
    let module = view.index.column("module").unwrap();
    let values: Vec<&str> = module.values.iter().map(|v| v.display.as_str()).collect();
    assert!(values.contains(&"API") && values.contains(&"JOB"));
}

#[test]
fn a_file_view_shows_only_its_file_and_has_no_source() {
    let (space, parsed) = space_with(&[
        ("/a.log", "2026-09-05 10:00:01 [INFO] [API] a\n"),
        ("/b.log", "2026-09-05 10:00:02 [INFO] [JOB] b\n"),
    ]);
    let view = build_file_view(space.file("/a.log").unwrap(), &parsed["/a.log"]);
    assert_eq!(view.entries.len(), 1);
    assert!(view.entries[0].source.is_none());
    assert!(view.index.columns_in_use().all(|c| c.key != "source"));
}

#[test]
fn a_file_without_timestamps_is_flagged_in_the_view() {
    let (space, parsed) = space_with(&[
        ("/a.log", "2026-09-05 10:00:01 [INFO] [API] a\n"),
        ("/notes.txt", "remember to check\n"),
    ]);
    let view = build_space_view(&space, &parsed);
    assert_eq!(view.no_timestamp_files, vec!["/notes.txt".to_string()]);
    assert_eq!(view.entries.last().unwrap().source.as_deref(), Some("notes.txt"));
}

// ─── Markers belong to the line of a file ────────────────────────────────────

#[test]
fn markers_painted_on_a_file_show_in_the_merged_view_a_file_view_and_a_rebuild() {
    let (mut space, parsed) = space_with(&[
        ("/a.log", "2026-09-05 10:00:01 [INFO] [API] a\n2026-09-05 10:00:03 [INFO] [API] c\n"),
        ("/b.log", "2026-09-05 10:00:02 [INFO] [JOB] b\n"),
    ]);
    let line = parsed["/b.log"].entries[0].raw_line.clone();
    let color = log_gpui::palette::HighlightColor::palette(
        log_gpui::palette::Hue::Blue,
        log_gpui::palette::Shade::Medium,
    );
    // Paint `INFO` on the only line of b.log.
    let at = line.find("INFO").unwrap();
    assert!(space.file_mut("/b.log").unwrap().paint_marker(1, &line, at..at + 4, color.clone()));

    // The merged view: the row of b.log is the second one.
    let view = build_space_view(&space, &parsed);
    assert!(view.entries[0].markers.is_empty());
    assert_eq!(view.entries[1].markers.len(), 1);
    assert_eq!(view.entries[1].markers[0].range, at..at + 4);

    // A File View of b.log shows the same marker.
    let file_view = build_file_view(space.file("/b.log").unwrap(), &parsed["/b.log"]);
    assert_eq!(file_view.entries[0].markers[0].color, color);

    // Taking the marker off clears it from a rebuilt view.
    assert!(space.file_mut("/b.log").unwrap().remove_marker_at(1, &line, at));
    let mut rebuilt = build_space_view(&space, &parsed);
    apply_markers_merged(&space, &mut rebuilt.entries);
    assert!(rebuilt.entries[1].markers.is_empty());
}

#[test]
fn the_filter_of_a_space_survives_in_the_space_not_the_file_view() {
    let (mut space, _) = space_with(&[("/a.log", ""), ("/b.log", "")]);
    space.filter.toggle_value("level", "ERROR");
    assert!(space.file_filter("/a.log").is_empty());
    let mut f = space.file_filter("/a.log");
    f.toggle_value("module", "API");
    space.set_file_filter("/a.log", f);
    assert!(space.filter.column("module").is_none());
}

#[test]
fn materialize_tags_rows_with_their_source_file() {
    let a = entries("2026-09-05 10:00:01 [INFO] [API] a\n");
    let b = entries("2026-09-05 10:00:02 [INFO] [JOB] b\n");
    let merged = merge(&[&a, &b]);
    let rows = materialize(&[("id-a", "a.log", &a), ("id-b", "b.log", &b)], &merged);
    assert_eq!(rows[0].file_id.as_deref(), Some("id-a"));
    assert_eq!(rows[1].source.as_deref(), Some("b.log"));
}
