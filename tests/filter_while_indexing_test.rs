//! The Filter picker while a file is indexed (openspec change
//! `line-offset-index`, task 5.2): the values and counts of the lines read
//! so far, growing as Indexing reads on, with a mark that the file is not
//! read whole yet.

mod common;

use common::{Harness, open_app};
use gpui::TestAppContext;
use log_gpui::filter_index::{FilterIndex, MergeFile};
use log_gpui::index::indexing::{IndexOptions, IndexedData, index_more};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

/// Lines of module `API` and `DB`, then a second half that also has `SEARCH`.
fn halves() -> (String, String) {
    let line = |i: usize, module: &str| format!("2026-09-05 10:00:{:02} [INFO] [{module}] line {i}\n", i % 60);
    let first: String = (0..100).map(|i| line(i, if i % 2 == 0 { "API" } else { "DB" })).collect();
    let second: String = (100..200).map(|i| line(i, if i % 4 == 0 { "SEARCH" } else { "API" })).collect();
    (first, second)
}

fn counts(index: &FilterIndex, key: &str) -> Vec<(String, usize)> {
    index.column(key).map_or(Vec::new(), |c| c.values.iter().map(|v| (v.display.clone(), v.count)).collect())
}

#[test]
fn the_values_so_far_grow_with_the_lines_read() {
    let (first, second) = halves();
    let options = IndexOptions::default();
    let mut data = IndexedData::default();
    index_more(&mut data, first.clone().into_bytes(), &options, false);
    let so_far = FilterIndex::values_so_far(&[MergeFile { index: &data.filter, source: None }]);
    assert_eq!(counts(&so_far, "module"), [("API".to_string(), 50), ("DB".to_string(), 50)]);

    index_more(&mut data, second.clone().into_bytes(), &options, false);
    let so_far = FilterIndex::values_so_far(&[MergeFile { index: &data.filter, source: None }]);
    assert_eq!(counts(&so_far, "module"), counts(&data.filter, "module"), "once read, the counts are the file's");
    assert_eq!(counts(&so_far, "module").last(), Some(&("SEARCH".to_string(), 25)));
    assert_eq!(so_far.coverage("module"), data.filter.coverage("module"));
}

#[test]
fn the_values_so_far_of_several_files_add_up() {
    let (first, second) = halves();
    let options = IndexOptions::default();
    let (mut a, mut b) = (IndexedData::default(), IndexedData::default());
    index_more(&mut a, first.into_bytes(), &options, false);
    index_more(&mut b, second.into_bytes(), &options, false);
    let files = [MergeFile { index: &a.filter, source: Some("a.log") }, MergeFile { index: &b.filter, source: Some("b.log") }];
    let so_far = FilterIndex::values_so_far(&files);
    assert_eq!(
        counts(&so_far, "module"),
        [("API".to_string(), 125), ("DB".to_string(), 50), ("SEARCH".to_string(), 25)],
        "values in file order, counts added",
    );
    assert_eq!(counts(&so_far, "source"), [("a.log".to_string(), 100), ("b.log".to_string(), 100)]);
}

fn write_log(h: &Harness, name: &str, text: &str) -> ImportSource {
    let path = h.dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    ImportSource::Disk(path.to_string_lossy().into_owned())
}

#[gpui::test]
fn the_picker_shows_the_values_read_so_far_and_keeps_what_was_ticked(cx: &mut TestAppContext) {
    let mut h = open_app(cx);
    let (first, second) = halves();
    let source = write_log(&h, "a.log", &format!("{first}{second}"));
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("A", LogSpaceKind::Static, &[source], cx);
        assert!(app.is_reading());
        // Indexing has read the first half of the file.
        let indexed = app.loads.values().next().and_then(|l| l.indexed().cloned()).expect("read through its Line Index");
        index_more(&mut indexed.data.write(), first.clone().into_bytes(), &IndexOptions::default(), false);
        app.show_values_read_so_far();
        assert_eq!(counts(&app.index(), "module"), [("API".to_string(), 50), ("DB".to_string(), 50)]);
        assert!(app.filter_read_so_far().is_some(), "the picker tells the file is not read whole");

        app.pick_filter_column("module", cx);
        app.filter_toggle_value("module", "api", cx);
        // What the test read by hand goes: Indexing reads the file itself.
        *indexed.data.write() = IndexedData::default();
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.is_reading());
        assert_eq!(app.filter_read_so_far(), None);
        assert_eq!(counts(&app.index(), "module").last(), Some(&("SEARCH".to_string(), 25)));
        assert!(app.filter_state.is_value_ticked("module", "api"), "the value ticked while reading stays");
        assert_eq!(app.filter_pass.len(), 125, "and filters the file once it is read");
        assert_eq!(app.filter_ui.expanded.as_deref(), Some("module"), "the picker stays open");
    });
}
