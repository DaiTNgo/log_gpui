use log_gpui::live::{ResumePoint, Verdict, parse_timestamp};
use log_gpui::model::Stream::{Stderr, Stdout};
use log_gpui::remote::PresetFields;

fn journal() -> ResumePoint {
    ResumePoint::new(PresetFields::journal("nginx"))
}

fn docker() -> ResumePoint {
    ResumePoint::new(PresetFields::docker("api"))
}

fn fields() -> PresetFields {
    PresetFields::journal("nginx")
}

#[test]
fn journalctl_times_with_an_offset_become_exact_epoch_times() {
    // 10:02:03.123456 at +07:00 is 03:02:03 UTC.
    let ts = parse_timestamp(&fields(), "2026-10-19T10:02:03.123456+0700 web1 nginx[1]: hi").unwrap();
    let utc = parse_timestamp(&fields(), "2026-10-19T03:02:03.123456Z hi").unwrap();
    assert_eq!(ts.nanos, utc.nanos);
    assert_eq!(ts.seconds_of_day, 10 * 3600 + 2 * 60 + 3, "the Server's own clock");
    // The colon form of the offset is read too.
    let colon = parse_timestamp(&fields(), "2026-10-19T10:02:03.123456+07:00 x").unwrap();
    assert_eq!(colon.nanos, ts.nanos);
    // 2026-10-19T03:02:03Z.
    assert_eq!(utc.nanos / 1_000_000_000, 1_792_378_923);
}

#[test]
fn docker_nanosecond_times_are_read_whole_and_kept_verbatim() {
    let ts = parse_timestamp(&PresetFields::docker("a"), "2026-10-19T03:02:03.123456789Z GET /").unwrap();
    assert_eq!(ts.text, "2026-10-19T03:02:03.123456789Z");
    assert_eq!(ts.nanos % 1_000_000_000, 123_456_789);
}

#[test]
fn a_line_without_a_time_with_an_offset_has_no_timestamp() {
    for line in [
        "Oct 19 10:02:03 web1 nginx: hi",
        "2026-10-19 10:02:03 hi",
        "2026-10-19T10:02:03 no offset",
        "  at handler (a.js:3)",
        "",
        "2026-13-19T10:02:03Z bad month",
    ] {
        assert!(parse_timestamp(&fields(), line).is_none(), "{line:?}");
    }
}

#[test]
fn nothing_to_resume_from_before_a_timestamped_line() {
    let mut p = journal();
    assert!(!p.can_resume());
    p.record(Stdout, "no time here");
    assert!(!p.can_resume());
    assert_eq!(p.command(), None);
    p.record(Stdout, "2026-10-19T10:02:03.400000+0700 web1 nginx: a");
    assert!(p.can_resume());
}

#[test]
fn journalctl_resumes_from_the_second_of_the_last_line() {
    let mut p = journal();
    p.record(Stdout, "2026-10-19T10:02:03.400000+0700 a");
    assert_eq!(
        p.command().unwrap(),
        "journalctl --no-pager -o short-iso-precise -f --since @1792378923 -u 'nginx'"
    );
    assert_eq!(p.last_seconds_of_day(), Some(10 * 3600 + 2 * 60 + 3));
}

#[test]
fn docker_resumes_from_the_exact_time_of_the_last_line() {
    let mut p = docker();
    p.record(Stdout, "2026-10-19T03:02:03.123456789Z GET /");
    assert_eq!(
        p.command().unwrap(),
        "docker logs -f --timestamps --since 2026-10-19T03:02:03.123456789Z 'api'"
    );
}

#[test]
fn lines_already_received_at_the_boundary_second_are_dropped() {
    let mut p = journal();
    for line in [
        "2026-10-19T10:02:02.100000+0700 earlier second",
        "2026-10-19T10:02:03.100000+0700 first in the second",
        "2026-10-19T10:02:03.400000+0700 last received",
    ] {
        p.record(Stdout, line);
    }
    p.begin_resume();
    // journalctl --since works in seconds: it prints the whole second again,
    // and one line that was written while the connection was down.
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.100000+0700 first in the second"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.400000+0700 last received"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.700000+0700 new in the same second"), Verdict::Keep);
    assert!(!p.is_filtering(), "the comparison ended at the first new line");
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.100000+0700 first in the second"), Verdict::Keep);
}

#[test]
fn lines_before_the_boundary_second_are_dropped_whatever_they_say() {
    let mut p = journal();
    p.record(Stdout, "2026-10-19T10:02:03.400000+0700 last received");
    p.begin_resume();
    // A tool that rounds the start down prints an earlier second.
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:02.900000+0700 older"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.400000+0700 last received"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:04.000000+0700 next"), Verdict::Keep);
}

#[test]
fn two_identical_lines_are_dropped_as_many_times_as_they_were_received() {
    let mut p = journal();
    p.record(Stdout, "2026-10-19T10:02:03.100000+0700 GET /health 200");
    p.begin_resume();
    // The new Run printed the line twice: one was already received.
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.100000+0700 GET /health 200"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.100000+0700 GET /health 200"), Verdict::Keep);
    assert!(!p.is_filtering());
}

#[test]
fn a_continuation_line_after_a_dropped_line_is_dropped_too() {
    let mut p = journal();
    p.record(Stdout, "2026-10-19T10:02:03.100000+0700 error");
    p.record(Stdout, "  at handler (a.js:3)");
    p.begin_resume();
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.100000+0700 error"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "  at handler (a.js:3)"), Verdict::Drop);
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:05.000000+0700 next"), Verdict::Keep);
}

#[test]
fn a_continuation_line_that_was_not_received_ends_the_comparison() {
    let mut p = journal();
    p.record(Stdout, "2026-10-19T10:02:03.100000+0700 error");
    p.begin_resume();
    assert_eq!(p.filter(Stdout, "2026-10-19T10:02:03.100000+0700 error"), Verdict::Drop);
    // Written after the connection dropped: new.
    assert_eq!(p.filter(Stdout, "  at handler (a.js:3)"), Verdict::Keep);
}

#[test]
fn each_stream_has_its_own_boundary() {
    let mut p = docker();
    p.record(Stdout, "2026-10-19T03:02:03.000000001Z out");
    p.record(Stderr, "2026-10-19T03:02:03.000000002Z err");
    p.begin_resume();
    // stderr does not match stdout's line.
    assert_eq!(p.filter(Stdout, "2026-10-19T03:02:03.000000001Z out"), Verdict::Drop);
    assert_eq!(p.filter(Stderr, "2026-10-19T03:02:03.000000002Z err"), Verdict::Drop);
}

#[test]
fn a_clock_that_went_back_stops_the_comparison_and_says_so() {
    let mut p = journal();
    p.record(Stdout, "2026-10-19T10:02:03.100000+0700 last received");
    p.begin_resume();
    let mut verdicts = Vec::new();
    for i in 0..1_002 {
        verdicts.push(p.filter(Stdout, &format!("2026-10-19T09:00:00.{i:06}+0700 old {i}")));
    }
    assert_eq!(verdicts.iter().filter(|v| **v == Verdict::Drop).count(), 1_000);
    assert_eq!(verdicts[1_000], Verdict::KeepClockWentBack);
    assert_eq!(verdicts[1_001], Verdict::Keep);
    assert!(!p.is_filtering());
}

#[test]
fn a_boundary_that_grows_past_the_limit_gives_up_resuming() {
    let mut p = journal();
    // 10,001 lines in one second.
    for i in 0..10_001 {
        p.record(Stdout, &format!("2026-10-19T10:02:03.{i:06}+0700 line {i}"));
    }
    assert!(!p.can_resume());
    assert_eq!(p.command(), None);
}

#[test]
fn the_boundary_only_keeps_the_lines_from_the_last_second() {
    let mut p = journal();
    for second in 0..50 {
        for i in 0..1_000 {
            p.record(Stdout, &format!("2026-10-19T10:02:{second:02}.{i:06}+0700 line {second} {i}"));
        }
    }
    // 50,000 lines in all, but only the last second is held.
    assert!(p.can_resume());
}
