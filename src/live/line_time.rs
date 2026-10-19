//! The time a line says, on the file's own clock. Only a Parent Line has one:
//! a Continuation Line, or a line without a time, has none of its own.

use crate::parser::text::LineParser;

/// The time written in `line`, as seconds on the file's clock.
pub fn line_timestamp(line: &str) -> Option<f64> {
    let entry = LineParser::new().parse(0, line);
    entry.has_time.then_some(entry.timestamp)
}

/// The newest time among `lines`, which is the clock of the file at its end.
pub fn newest_timestamp<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<f64> {
    lines.into_iter().filter_map(line_timestamp).fold(None, |best, t| Some(best.map_or(t, |b: f64| b.max(t))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_lines_have_a_time_and_others_do_not() {
        assert!(line_timestamp("2026-10-19 10:00:00 INFO started").is_some());
        assert_eq!(line_timestamp("    at com.example.Main(Main.java:10)"), None);
        assert_eq!(line_timestamp(""), None);
    }

    #[test]
    fn newest_wins_even_when_the_last_line_has_no_time() {
        let lines = [
            "2026-10-19 10:00:00 INFO a",
            "2026-10-19 10:00:05 INFO b",
            "  continued",
        ];
        let newest = newest_timestamp(lines).unwrap();
        assert_eq!(Some(newest), line_timestamp(lines[1]));
        assert_eq!(newest_timestamp(["no time here"]), None);
    }
}
