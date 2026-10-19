//! A Remote File leaving a Log Space of several files (Clear, or lines
//! leaving the window): the Filter counts follow, and a column that had
//! passed 500 distinct values takes new conditions again.

use log_gpui::filter_index::FilterIndex;
use log_gpui::model::LogEntry;
use log_gpui::space_view::ParsedFile;
use log_gpui::timeline;

fn file(name: &str, lines: usize, modules: usize) -> Vec<LogEntry> {
    let text: Vec<String> = (0..lines)
        .map(|i| format!("2024-05-01 10:{:02}:{:02}.000 INFO [{name}{}] message {i}", i / 60 % 60, i % 60, i % modules))
        .collect();
    ParsedFile::from_text(&text.join("\n")).entries
}

fn merged_index(files: &[&[LogEntry]]) -> FilterIndex {
    let timeline = timeline::merge(files);
    let named: Vec<(&str, &str, &[LogEntry])> =
        files.iter().enumerate().map(|(i, f)| (["a", "b"][i], ["a.log", "b.log"][i], *f)).collect();
    let entries = timeline::materialize(&named, &timeline);
    let fields = timeline::union_field_names(files);
    FilterIndex::build_merged(&entries, &fields, &timeline.effective_ts)
}

/// How many columns are not available for filtering.
fn closed_columns(index: &FilterIndex) -> usize {
    index.columns.iter().filter(|c| c.distinct_count().is_none()).count()
}

#[test]
fn removing_a_whole_file_gives_back_counts_and_reopens_a_column_over_500_values() {
    let busy = file("m", 1_200, 600);
    let quiet = file("q", 300, 3);
    let both = merged_index(&[&busy, &quiet]);
    assert_eq!(both.rows(), 1_500);
    assert!(closed_columns(&both) >= 1, "600 + 3 distinct values pass the limit");

    // The busy file is cleared: only the quiet file is left.
    let left = merged_index(&[&[], &quiet]);
    assert_eq!(left.rows(), 300);
    assert_eq!(closed_columns(&left), 0, "the column takes conditions again");
    let module = left
        .columns
        .iter()
        .find(|c| c.values.iter().any(|v| v.folded.starts_with("q0")))
        .expect("the quiet file's module column");
    assert_eq!(module.distinct_count(), Some(3));
    assert_eq!(module.values.iter().map(|v| v.count).sum::<usize>(), 300);
    assert!(module.values.iter().all(|v| v.folded.starts_with('q')), "values of the cleared file are gone");
}

#[test]
fn a_file_with_no_lines_left_gives_an_empty_index() {
    let none = merged_index(&[&[], &[]]);
    assert_eq!(none.rows(), 0);
    assert_eq!(closed_columns(&none), 0);
}
