use log_gpui::live::{BreakKind, RestartKind, SourceEvent, SourceHealth, SourceStatus};
use std::time::{Duration, SystemTime};

fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

fn append() -> SourceEvent {
    SourceEvent::Append { lines: vec!["x".into()], bytes: vec![], streams: None }
}

#[test]
fn a_remote_file_counts_its_reconnects_and_remembers_when_the_last_line_came() {
    let mut h = SourceHealth::default();
    h.observe(&append(), at(10));
    h.observe(&SourceEvent::Break { kind: BreakKind::Disconnected }, at(11));
    assert_eq!(h.reconnects, 0, "losing the connection is not a reconnect");
    h.observe(&SourceEvent::Break { kind: BreakKind::Reconnected }, at(20));
    h.observe(&SourceEvent::Break { kind: BreakKind::Reconnected }, at(30));
    assert_eq!(h.reconnects, 2);
    assert_eq!(h.last_line_at, Some(at(10)));
}

#[test]
fn an_empty_batch_is_not_a_line() {
    let mut h = SourceHealth::default();
    h.observe(&SourceEvent::Append { lines: vec![], bytes: vec![], streams: None }, at(5));
    assert_eq!(h.last_line_at, None);
}

#[test]
fn a_remote_command_counts_runs_and_shows_the_command_of_the_current_one() {
    let mut h = SourceHealth::default();
    h.observe(&SourceEvent::Run { command: "docker logs -f 'api'".into() }, at(1));
    h.observe(&SourceEvent::Status(SourceStatus::Exited(Some(1))), at(2));
    assert_eq!(h.exit_code, Some(1));
    h.observe(&SourceEvent::Run { command: "docker logs -f --since T 'api'".into() }, at(3));
    assert_eq!(h.runs, 2);
    assert_eq!(h.run_command.as_deref(), Some("docker logs -f --since T 'api'"));
    assert_eq!(h.exit_code, None, "it runs again: no exit code");
}

#[test]
fn a_resumed_run_after_a_lost_connection_counts_as_a_reconnect() {
    let mut h = SourceHealth::default();
    h.observe(&SourceEvent::Break { kind: BreakKind::Resumed(3600) }, at(1));
    h.observe(&SourceEvent::Break { kind: BreakKind::Restarted(RestartKind::AfterDisconnect) }, at(2));
    h.observe(&SourceEvent::Break { kind: BreakKind::Restarted(RestartKind::Manual) }, at(3));
    assert_eq!(h.reconnects, 2);
}

#[test]
fn the_last_error_comes_from_an_error_status_or_a_problem() {
    let mut h = SourceHealth::default();
    h.observe(&SourceEvent::Status(SourceStatus::Error("Permission denied".into())), at(4));
    assert_eq!(h.last_error, Some((at(4), "Permission denied".to_string())));
    h.observe(&SourceEvent::Problem("resume not supported".into()), at(9));
    assert_eq!(h.last_error, Some((at(9), "resume not supported".to_string())));
}
