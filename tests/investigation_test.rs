use log_gpui::model::LogLevel;
use log_gpui::workspace::investigation::{CaseIdGenerator, CaseStatus, InvestigationCase};

fn make_case(id: &str) -> InvestigationCase {
    InvestigationCase::new(id.to_string(), "Payment Upstream Timeout".to_string(), "P1")
}

#[test]
fn test_case_defaults_active() {
    let c = make_case("INC-1");
    assert_eq!(c.id, "INC-1");
    assert_eq!(c.status, CaseStatus::Active);
    assert!(c.anchors.is_empty());
    assert_eq!(c.display_name(), "INC-1: Payment Upstream Timeout");
    assert!(!c.is_archived());
}

#[test]
fn test_case_status_transitions() {
    let mut c = make_case("INC-2");
    c.status = CaseStatus::RootCauseIdentified;
    assert!(matches!(c.status, CaseStatus::RootCauseIdentified));

    c.status = CaseStatus::Mitigated;
    assert!(matches!(c.status, CaseStatus::Mitigated));

    c.status = CaseStatus::Archived;
    assert!(c.is_archived());

    // String round-trip
    assert_eq!(CaseStatus::from_str("Archived"), CaseStatus::Archived);
    assert_eq!(CaseStatus::from_str("Root Cause Identified"), CaseStatus::RootCauseIdentified);
    assert_eq!(CaseStatus::from_str("junk"), CaseStatus::Active);
    assert_eq!(CaseStatus::Active.as_str(), "Active");
}

#[test]
fn test_anchor_line_across_files() {
    let mut c = make_case("INC-3");
    c.anchor_line(
        "a1".into(),
        "api.log",
        42,
        1000.0,
        LogLevel::Error,
        "upstream timeout".into(),
        None,
    );
    c.anchor_line(
        "a2".into(),
        "worker.log",
        87,
        1005.0,
        LogLevel::Warn,
        "retrying request".into(),
        Some("backoff observed".into()),
    );
    c.anchor_line(
        "a3".into(),
        "nginx.log",
        12,
        998.0,
        LogLevel::Info,
        "GET /pay".into(),
        None,
    );

    assert_eq!(c.anchors.len(), 3);
    assert_eq!(c.anchor_count_for_file("api.log"), 1);
    assert_eq!(c.anchor_count_for_file("worker.log"), 1);
}

#[test]
fn test_chronological_trail_sorts_across_files() {
    let mut c = make_case("INC-4");
    // Insert out of chronological order across three files
    c.anchor_line("c".into(), "nginx.log", 5, 3000.0, LogLevel::Info, "late".into(), None);
    c.anchor_line("a".into(), "api.log", 5, 1000.0, LogLevel::Error, "early".into(), None);
    c.anchor_line("b".into(), "worker.log", 5, 2000.0, LogLevel::Warn, "middle".into(), None);

    let trail = c.chronological_trail();
    let payloads: Vec<&str> = trail.iter().map(|a| a.payload.as_str()).collect();
    assert_eq!(payloads, vec!["early", "middle", "late"]);
}

#[test]
fn test_remove_anchor_by_id() {
    let mut c = make_case("INC-5");
    c.anchor_line("keep".into(), "a.log", 1, 1.0, LogLevel::Info, "keep".into(), None);
    c.anchor_line("drop".into(), "b.log", 1, 2.0, LogLevel::Info, "drop".into(), None);

    assert!(c.remove_anchor("drop"));
    assert!(!c.remove_anchor("nonexistent"));
    assert_eq!(c.anchors.len(), 1);
    assert_eq!(c.anchors[0].id, "keep");
}

#[test]
fn test_set_anchor_note() {
    let mut c = make_case("INC-6");
    c.anchor_line("a1".into(), "api.log", 10, 1.0, LogLevel::Error, "err".into(), None);

    assert!(c.set_anchor_note("a1", Some("Stripe TCP SYN timeout".into())));
    assert_eq!(c.anchors[0].note.as_deref(), Some("Stripe TCP SYN timeout"));

    assert!(!c.set_anchor_note("missing", Some("nope".into())));
}

#[test]
fn test_case_level_notes() {
    let mut c = make_case("INC-7");
    c.add_case_note("Hypothesis: cache stampede".into());
    c.add_case_note("Confirmed via trace id t-123".into());
    assert_eq!(c.notes.len(), 2);
}

#[test]
fn test_export_post_mortem_markdown() {
    let mut c = make_case("INC-142");
    c.status = CaseStatus::RootCauseIdentified;
    c.add_case_note("Upstream Stripe connection reset observed".into());
    c.anchor_line(
        "a1".into(),
        "api.log",
        42,
        1000.123,
        LogLevel::Error,
        "upstream request failed: connection reset".to_string(),
        Some("First failure at gateway".into()),
    );
    c.anchor_line(
        "a2".into(),
        "worker.log",
        7,
        1001.5,
        LogLevel::Warn,
        "retrying in 2s (attempt 2/5)".to_string(),
        None,
    );

    let md = c.export_post_mortem_markdown();

    assert!(md.contains("# INC-142: Payment Upstream Timeout"));
    assert!(md.contains("Root Cause Identified"));
    assert!(md.contains("**Severity**: P1"));
    assert!(md.contains("**Evidence entries**: 2"));
    assert!(md.contains("## Diagnostic Notes"));
    assert!(md.contains("1. Upstream Stripe connection reset observed"));
    assert!(md.contains("## Chronological Evidence Trail"));
    assert!(md.contains("**File**: `api.log` (line 42)"));
    assert!(md.contains("```log"));
    assert!(md.contains("upstream request failed: connection reset"));
    assert!(md.contains("> First failure at gateway"));
    assert!(md.contains("**File**: `worker.log` (line 7)"));

    // Anchors appear chronologically: api.log before worker.log
    let api_pos = md.find("api.log").unwrap();
    let worker_pos = md.find("worker.log").unwrap();
    assert!(api_pos < worker_pos);
}

#[test]
fn test_case_id_generator_no_collisions() {
    let mut cases: Vec<InvestigationCase> = vec![];
    let mut generator = CaseIdGenerator::new(1);

    let id1 = generator.next(&cases.iter().collect::<Vec<_>>());
    assert_eq!(id1, "INC-1");
    cases.push(InvestigationCase::new(id1, "A".into(), "P2"));

    let id2 = generator.next(&cases.iter().collect::<Vec<_>>());
    assert_eq!(id2, "INC-2");
    cases.push(InvestigationCase::new(id2, "B".into(), "P2"));

    // Simulate a persisted case that occupies INC-4
    cases.push(InvestigationCase::new("INC-4".into(), "C".into(), "P2"));

    // Generator skips occupied INC-4
    let id3 = generator.next(&cases.iter().collect::<Vec<_>>());
    assert_eq!(id3, "INC-3");
    let id4 = generator.next(&cases.iter().collect::<Vec<_>>());
    assert_eq!(id4, "INC-5");
}

#[test]
fn test_display_name_format() {
    let c = InvestigationCase::new("INC-9".into(), "Cache Stampede".into(), "P3");
    assert_eq!(c.display_name(), "INC-9: Cache Stampede");
}