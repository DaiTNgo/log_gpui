use log_gpui::live::{SourceStatus, connection_label};
use log_gpui::remote::latency_text;
use std::time::Duration;

fn ms(n: u64) -> Option<Duration> {
    Some(Duration::from_millis(n))
}

#[test]
fn live_members_show_the_latency_of_the_slowest_server() {
    assert_eq!(connection_label(&SourceStatus::Live, 3, 3, ms(42)), "Live · 42 ms");
}

#[test]
fn without_a_measurement_the_label_has_no_latency() {
    assert_eq!(connection_label(&SourceStatus::Live, 1, 1, None), "Live");
}

#[test]
fn the_worst_status_counts_the_members_that_have_it() {
    assert_eq!(connection_label(&SourceStatus::Reconnecting, 1, 3, None), "Reconnecting (1 of 3)");
    assert_eq!(connection_label(&SourceStatus::Reconnecting, 1, 1, None), "Reconnecting");
}

#[test]
fn an_exited_command_shows_exited_without_its_code() {
    assert_eq!(connection_label(&SourceStatus::Exited(Some(1)), 1, 3, None), "Exited (1 of 3)");
    assert_eq!(connection_label(&SourceStatus::Exited(Some(0)), 1, 2, None), "Exited (1 of 2)");
}

#[test]
fn latency_is_written_in_milliseconds_or_seconds_and_a_dash_when_unknown() {
    assert_eq!(latency_text(None), "—");
    assert_eq!(latency_text(ms(12)), "12 ms");
    assert_eq!(latency_text(Some(Duration::from_micros(150))), "1 ms");
    assert_eq!(latency_text(Some(Duration::from_millis(1_250))), "1.2 s");
}
