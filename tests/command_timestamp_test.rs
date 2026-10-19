//! The line parser must read the time at the start of the lines a Command
//! Preset prints, or the lines of a Remote Command would sink to the end of
//! the merged timeline (rule 22). Times are compared as written, with no
//! timezone conversion (rule 19, L8): the offset is read past, not applied.

use log_gpui::parser::LineParser;

/// 2026-10-19T10:02:03 read as if it were UTC.
const WALL_CLOCK_SECONDS: f64 = 1_792_404_123.0;

fn timestamp_of(line: &str) -> Option<f64> {
    let entry = LineParser::new().parse(1, line);
    entry.has_time.then_some(entry.timestamp)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn journalctl_short_iso_precise_with_offset_without_colon() {
    let ts = timestamp_of("2026-10-19T10:02:03.123456+0700 web1 nginx[812]: GET / 200").expect("has a time");
    assert!(close(ts, WALL_CLOCK_SECONDS + 0.123456), "{ts}");
}

#[test]
fn journalctl_short_iso_precise_with_utc_offset() {
    let ts = timestamp_of("2026-10-19T10:02:03.123456+0000 web1 nginx[812]: GET / 200").expect("has a time");
    assert!(close(ts, WALL_CLOCK_SECONDS + 0.123456), "{ts}");
}

#[test]
fn docker_and_kubectl_rfc3339_nano() {
    let ts = timestamp_of("2026-10-19T10:02:03.123456789Z GET /health 200").expect("has a time");
    assert!(close(ts, WALL_CLOCK_SECONDS + 0.123456789), "{ts}");
}

#[test]
fn a_time_with_a_colon_in_the_offset() {
    let ts = timestamp_of("2026-10-19T10:02:03.5+07:00 x").expect("has a time");
    assert!(close(ts, WALL_CLOCK_SECONDS + 0.5), "{ts}");
}

#[test]
fn times_are_compared_as_written_whatever_the_offset() {
    let a = timestamp_of("2026-10-19T10:02:03.000000+0700 a").unwrap();
    let b = timestamp_of("2026-10-19T10:02:03.000000000Z b").unwrap();
    assert!(close(a, b), "{a} vs {b}");
}
