use log_gpui::raw_view::{RawMatch, RawRow, index_of_line, row_at, row_count, search_sections};
use log_gpui::search_expr::{Matcher, parse};
use log_gpui::space_view::{ParsedFile, split_lines};

fn matcher(query: &str) -> Matcher {
    Matcher::compile(&parse(query).unwrap().unwrap())
}

#[test]
fn raw_view_shows_every_line_as_stored() {
    let text = "first\n\n   indented\nlast";
    let parsed = ParsedFile::from_text(text);
    // Empty lines are kept in the Raw View, unlike the parsed rows.
    assert_eq!(parsed.lines.len(), 4);
    assert_eq!(parsed.lines[1], "");
    assert_eq!(parsed.lines[2], "   indented");
    assert_eq!(parsed.entries.len(), 3);
}

#[test]
fn search_finds_matches_on_every_line_of_every_section() {
    // 5 lines hidden by a Filter in the Parsed View still count here.
    let a: Vec<String> = (0..10)
        .map(|i| if i < 5 { format!("DEBUG timeout {i}") } else { format!("INFO ok {i}") })
        .collect();
    let b: Vec<String> = (0..12).map(|i| format!("ERROR timeout {i}")).collect();

    let matches = search_sections(&matcher("timeout"), &[a.as_slice(), b.as_slice()]);

    assert_eq!(matches.len(), 17);
    assert_eq!(matches[0], RawMatch { section: 0, line: 0 });
    assert_eq!(matches[5], RawMatch { section: 1, line: 0 });
}

#[test]
fn search_in_raw_view_is_case_insensitive_and_diacritic_sensitive() {
    let lines = vec!["Lỗi kết nối".to_string(), "loi ket noi".to_string()];
    let m = search_sections(&matcher("lỗi"), &[lines.as_slice()]);
    assert_eq!(m, vec![RawMatch { section: 0, line: 0 }]);
}

#[test]
fn flat_rows_are_one_header_then_the_lines_unless_collapsed() {
    let sections = [(3, false), (2, true), (1, false)];
    assert_eq!(row_count(&sections), (1 + 3) + 1 + (1 + 1));

    assert_eq!(row_at(&sections, 0), Some(RawRow::Header { section: 0 }));
    assert_eq!(row_at(&sections, 3), Some(RawRow::Line { section: 0, line: 2 }));
    assert_eq!(row_at(&sections, 4), Some(RawRow::Header { section: 1 }));
    // The collapsed section hides its lines.
    assert_eq!(row_at(&sections, 5), Some(RawRow::Header { section: 2 }));
    assert_eq!(row_at(&sections, 6), Some(RawRow::Line { section: 2, line: 0 }));
    assert_eq!(row_at(&sections, 7), None);
}

#[test]
fn a_match_in_a_collapsed_section_has_no_row_until_it_is_opened() {
    let sections = [(3, false), (2, true)];
    assert_eq!(index_of_line(&sections, 0, 1), Some(2));
    assert_eq!(index_of_line(&sections, 1, 0), None);

    let opened = [(3, false), (2, false)];
    assert_eq!(index_of_line(&opened, 1, 0), Some(5));
    assert_eq!(index_of_line(&opened, 1, 5), None);
}

#[test]
fn splitting_text_keeps_blank_lines() {
    assert_eq!(split_lines("a\n\nb\n"), vec!["a", "", "b"]);
    assert!(split_lines("").is_empty());
}

#[test]
fn reparsing_after_an_edit_reads_the_new_text() {
    let before = ParsedFile::from_text("2026-09-05 10:00:00 [INFO] [api] one\n");
    assert_eq!(before.entries.len(), 1);
    let after = ParsedFile::from_text(
        "2026-09-05 10:00:00 [INFO] [api] one\n2026-09-05 10:00:01 [ERROR] [api] two\n",
    );
    assert_eq!(after.entries.len(), 2);
    assert_eq!(after.entries[1].tag, "api");
}
