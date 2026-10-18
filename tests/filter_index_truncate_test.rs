use log_gpui::filter_index::FilterIndex;
use log_gpui::space_view::ParsedFile;

fn text() -> String {
    let mut lines = Vec::new();
    for i in 0..40 {
        let level = ["INFO", "WARN", "ERROR"][i % 3];
        lines.push(format!("2024-05-01 10:00:{:02}.000 {level} [mod{}] message {i}", i, i % 4));
        if i % 7 == 0 {
            lines.push("    continuation of the line above".to_string());
        }
    }
    lines.join("\n")
}

#[test]
fn truncate_then_append_equals_a_full_build() {
    let entries = ParsedFile::from_text(&text()).entries;
    let full = FilterIndex::build(&entries);
    for cut in [0, 1, 5, 8, 20, entries.len() - 1, entries.len()] {
        let mut index = FilterIndex::build(&entries);
        index.truncate(&entries[..cut], &entries[cut..]);
        assert_eq!(index.rows(), cut);
        index.append(&entries[cut..]);
        assert_eq!(index.rows(), full.rows(), "cut {cut}");
        for col in &full.columns {
            let other = index.column(&col.key).expect("column kept");
            assert_eq!(other.values.len(), col.values.len(), "{} cut {cut}", col.key);
            for (a, b) in other.values.iter().zip(&col.values) {
                assert_eq!((a.folded.as_str(), a.count), (b.folded.as_str(), b.count), "{} cut {cut}", col.key);
            }
            for row in 0..full.rows() {
                assert_eq!(other.id_of_row(row), col.id_of_row(row), "{} row {row} cut {cut}", col.key);
            }
        }
        for row in 0..full.rows() {
            assert_eq!(index.effective_timestamp(row), full.effective_timestamp(row));
        }
        assert_eq!(index.time_hint(), full.time_hint());
        assert_eq!(index.anchor_day(), full.anchor_day());
    }
}
