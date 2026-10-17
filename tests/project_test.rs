use log_gpui::column::PinnableColumn;
use log_gpui::filter::{ConditionField, ConditionOp};
use log_gpui::workspace::project::{LogFileDescriptor, ProjectColorAccent, slug_project_id};
use log_gpui::workspace::state::{ContextSidebarState, ContextTab, LeftSidebarState, WorkspaceState};
use log_gpui::workspace::working_set::{TabSessionState, WorkingSet};

fn session_with_filter(level: &str) -> TabSessionState {
    let mut session = TabSessionState::new();
    if !level.is_empty() {
        session.filter.add_condition(ConditionField::Level, ConditionOp::Is, level.to_string());
    }
    session.column_layout.pin(PinnableColumn::Timestamp);
    session
}

// ─── WorkspaceState: Project Creation & Switching ────────────────────────────

#[test]
fn test_workspace_creates_project_active() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Payment Service", ProjectColorAccent::Blue);

    assert_eq!(ws.project_count(), 1);
    assert_eq!(ws.active_project_id.as_deref(), Some("payment-service"));
    let project = ws.active_project().unwrap();
    assert_eq!(project.name, "Payment Service");
    assert_eq!(project.color, ProjectColorAccent::Blue);
}

#[test]
fn test_switch_project_isolates_contexts() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Payment Service", ProjectColorAccent::Blue);
    ws.create_project("E-Commerce Core", ProjectColorAccent::Green);

    // Populate Payment project with a file + tab
    {
        let payment = ws.project_mut("payment-service").unwrap();
        payment.register_file(LogFileDescriptor::new("api.log".into(), "api.log".into()));
        payment.open_file("api.log", session_with_filter("ERROR"));
        assert!(payment.working_set.is_file_open("api.log"));
    }

    // Switch to E-Commerce
    ws.switch_project("e-commerce-core").unwrap();
    let ecom = ws.active_project().unwrap();
    assert!(!ecom.working_set.is_file_open("api.log"));
    assert!(ecom.files.is_empty());

    // Switch back: Payment context fully restored
    ws.switch_project("payment-service").unwrap();
    let payment = ws.active_project().unwrap();
    assert!(payment.working_set.is_file_open("api.log"));
    let tab = payment.working_set.active_tab().unwrap();
    assert!(tab.session.column_layout.is_pinned(PinnableColumn::Timestamp));
}

#[test]
fn test_switch_project_restores_tab_sessions() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Proj A", ProjectColorAccent::Blue);

    {
        let p = ws.active_project_mut().unwrap();
        let mut s = session_with_filter("ERROR");
        s.scroll_x = 250.0;
        s.selected_row = Some(17);
        p.register_file(LogFileDescriptor::new("a.log".into(), "a.log".into()));
        p.open_file("a.log", s);
    }

    ws.create_project("Proj B", ProjectColorAccent::Red);
    {
        let p = ws.active_project_mut().unwrap();
        let mut s = TabSessionState::new();
        s.scroll_x = 5.0;
        p.register_file(LogFileDescriptor::new("b.log".into(), "b.log".into()));
        p.open_file("b.log", s);
    }

    // Round trip A -> B -> A
    ws.switch_project("proj-a").unwrap();
    let p = ws.active_project().unwrap();
    let tab = p.working_set.active_tab().unwrap();
    assert_eq!(tab.session.scroll_x, 250.0);
    assert_eq!(tab.session.selected_row, Some(17));
}

#[test]
fn test_switch_to_unknown_project_is_noop() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Proj A", ProjectColorAccent::Blue);
    assert!(ws.switch_project("does-not-exist").is_none());
    assert_eq!(ws.active_project_id.as_deref(), Some("proj-a"));
}

#[test]
fn test_rename_project_inline() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Payment", ProjectColorAccent::Blue);
    assert!(ws.rename_project("payment", "Payment Gateway"));
    assert_eq!(ws.active_project().unwrap().name, "Payment Gateway");

    assert!(!ws.rename_project("nope", "X"));
}

#[test]
fn test_remove_project_without_restructuring_files() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Payment", ProjectColorAccent::Blue);
    ws.create_project("E-Commerce", ProjectColorAccent::Green);

    let removed = ws.remove_project("payment").unwrap();
    assert_eq!(removed.name, "Payment");
    assert_eq!(ws.project_count(), 1);
    // Active falls back to remaining project
    assert_eq!(ws.active_project_id.as_deref(), Some("e-commerce"));

    // Removing last project leaves no active
    ws.remove_project("e-commerce").unwrap();
    assert_eq!(ws.project_count(), 0);
    assert_eq!(ws.active_project_id, None);
}

#[test]
fn test_projects_are_strictly_isolated() {
    let mut ws = WorkspaceState::new();
    ws.create_project("A", ProjectColorAccent::Blue);
    ws.active_project_mut()
        .unwrap()
        .register_file(LogFileDescriptor::new("api.log".into(), "api.log".into()));

    ws.create_project("B", ProjectColorAccent::Green);
    ws.active_project_mut()
        .unwrap()
        .register_file(LogFileDescriptor::new("worker.log".into(), "worker.log".into()));

    assert!(ws.projects_are_isolated("a", "b"));
}

#[test]
fn test_slug_generates_url_safe_id() {
    assert_eq!(slug_project_id("Payment Service"), "payment-service");
    assert_eq!(slug_project_id("Payments!@#$"), "payments");
    assert_eq!(slug_project_id("^^^"), "project");
    assert_eq!(slug_project_id("Payment   Service"), "payment-service");
}

// ─── Project: File Registry ──────────────────────────────────────────────────

#[test]
fn test_register_file_deduplicates() {
    let mut ws = WorkspaceState::new();
    ws.create_project("P", ProjectColorAccent::Blue);
    let p = ws.active_project_mut().unwrap();

    assert!(p.register_file(LogFileDescriptor::new("api.log".into(), "api.log".into())));
    assert!(!p.register_file(LogFileDescriptor::new("api.log".into(), "api.log".into())));
    assert_eq!(p.files.len(), 1);
}

#[test]
fn test_unregister_file_preserves_disk_file() {
    let mut ws = WorkspaceState::new();
    ws.create_project("P", ProjectColorAccent::Blue);
    let p = ws.active_project_mut().unwrap();
    p.register_file(LogFileDescriptor::new("db.log".into(), "db.log".into()));

    let removed = p.unregister_file("db.log");
    assert!(removed.is_some());
    assert!(p.file("db.log").is_none());
    assert!(p.files.is_empty());
}

#[test]
fn test_project_open_file_opens_or_focuses_tab() {
    let mut ws = WorkspaceState::new();
    ws.create_project("P", ProjectColorAccent::Blue);
    let p = ws.active_project_mut().unwrap();
    p.register_file(LogFileDescriptor::new("api.log".into(), "api.log".into()));

    p.open_file("api.log", session_with_filter("WARN"));
    assert_eq!(p.working_set.tab_count(), 1);

    // Opening same file focuses existing tab, no duplicate
    p.open_file("api.log", session_with_filter("ERROR"));
    assert_eq!(p.working_set.tab_count(), 1);
}

// ─── Sidebar State (Left: Projects, Right: Context) ──────────────────────────

#[test]
fn test_left_sidebar_defaults_visible() {
    let sb = LeftSidebarState::new();
    assert!(sb.visible);
    assert_eq!(sb.width, 240.0);
}

#[test]
fn test_left_sidebar_toggle_is_hidden_expanded() {
    let mut sb = LeftSidebarState::new();
    sb.toggle();
    assert!(!sb.visible);
    sb.toggle();
    assert!(sb.visible);
}

#[test]
fn test_left_sidebar_width_defaults_and_resizing() {
    let mut sb = LeftSidebarState::new();
    sb.set_width(320.0);
    assert_eq!(sb.width, 320.0);
    sb.set_width(120.0);
    assert_eq!(sb.width, 180.0);
    sb.set_width(800.0);
    assert_eq!(sb.width, 500.0);
    sb.reset_width();
    assert_eq!(sb.width, 240.0);
}

#[test]
fn test_context_sidebar_defaults_hidden_on_files() {
    let cs = ContextSidebarState::new();
    assert!(!cs.visible);
    assert_eq!(cs.active_tab, ContextTab::Files);
    assert!(!cs.is_shown(true));
}

#[test]
fn test_context_sidebar_toggle_keeps_active_tab() {
    let mut cs = ContextSidebarState::new();
    cs.show_tab(ContextTab::Markers);
    assert!(cs.visible);
    cs.toggle();
    assert!(!cs.visible);
    cs.toggle();
    assert!(cs.visible);
    assert_eq!(cs.active_tab, ContextTab::Markers);
}

#[test]
fn test_context_sidebar_requires_a_log_file_tab() {
    let mut cs = ContextSidebarState::new();
    cs.show_tab(ContextTab::Investigations);
    assert!(cs.is_shown(true));
    assert!(!cs.is_shown(false));
}

#[test]
fn test_context_tab_labels() {
    let labels: Vec<_> = ContextTab::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(labels, ["Files", "Investigations", "Saved Views", "Markers"]);
}

#[test]
fn test_color_accent_badge_values() {
    assert_eq!(ProjectColorAccent::Blue.as_str(), "Blue");
    assert_eq!(ProjectColorAccent::Amber.hex(), "#F59E0B");
    assert_eq!(ProjectColorAccent::ALL.len(), 6);
}

// ─── WorkingSet within Project lifecycle ─────────────────────────────────────

#[test]
fn test_project_working_set_is_independent_per_project() {
    let mut ws = WorkspaceState::new();
    ws.create_project("A", ProjectColorAccent::Blue);
    let p = ws.active_project_mut().unwrap();
    p.open_file("a.log", session_with_filter("INFO"));

    ws.create_project("B", ProjectColorAccent::Red);
    let p = ws.active_project_mut().unwrap();
    p.open_file("b.log", session_with_filter("ERROR"));

    assert_eq!(ws.project("a").unwrap().working_set.tab_count(), 1);
    assert_eq!(ws.project("b").unwrap().working_set.tab_count(), 1);
    let ws_a: &WorkingSet = &ws.project("a").unwrap().working_set;
    assert!(!ws_a.is_file_open("b.log"));
}

#[test]
fn test_file_open_state_and_tab_close_by_file_id() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Triage", ProjectColorAccent::Green);
    let p = ws.active_project_mut().unwrap();
    p.register_file(LogFileDescriptor::new("app.log".to_string(), "app.log".to_string()));
    p.register_file(LogFileDescriptor::new("worker.log".to_string(), "worker.log".to_string()));

    assert!(!p.working_set.is_file_open("app.log"));
    assert!(!p.working_set.is_file_open("worker.log"));

    // Open app.log
    p.open_file("app.log", session_with_filter("INFO"));
    assert!(p.working_set.is_file_open("app.log"));
    assert!(!p.working_set.is_file_open("worker.log"));

    // Open worker.log
    p.open_file("worker.log", session_with_filter("ERROR"));
    assert!(p.working_set.is_file_open("app.log"));
    assert!(p.working_set.is_file_open("worker.log"));

    // Close app.log via find_tab
    let target = log_gpui::workspace::working_set::TabTarget::LogFile {
        file_id: "app.log".to_string(),
    };
    let tab_idx = p.working_set.find_tab(&target).expect("app.log tab should exist");
    p.working_set.close_tab(tab_idx);

    assert!(!p.working_set.is_file_open("app.log"));
    assert!(p.working_set.is_file_open("worker.log"));
}

#[test]
fn test_project_investigations_and_saved_views_lifecycle() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Incident Ops", ProjectColorAccent::Red);
    let p = ws.active_project_mut().unwrap();

    // Create investigation case
    let mut case = log_gpui::workspace::investigation::InvestigationCase::new(
        "INC-404".to_string(),
        "Cluster Outage".to_string(),
        "P0",
    );
    case.anchor_line(
        "anc-1".to_string(),
        "cluster.log",
        15,
        1700000000.0,
        log_gpui::model::LogLevel::Error,
        "Node evicted due to memory pressure".to_string(),
        None,
    );
    p.investigation_cases.push(case);
    assert_eq!(p.investigation_cases.len(), 1);
    assert_eq!(p.investigation_cases[0].id, "INC-404");
    assert_eq!(p.investigation_cases[0].anchors.len(), 1);

    // Save view
    let mut session = TabSessionState::new();
    session.filter.add_condition(
        log_gpui::filter::ConditionField::Level,
        log_gpui::filter::ConditionOp::Is,
        "FATAL".to_string(),
    );
    let view = log_gpui::workspace::saved_view::SavedView::capture(
        "view-fatal".to_string(),
        "Fatal Alerts".to_string(),
        &session,
    );
    p.saved_views.push(view);
    assert_eq!(p.saved_views.len(), 1);
    assert_eq!(p.saved_views[0].name, "Fatal Alerts");

    // Apply saved view to a fresh session
    let mut target_session = TabSessionState::new();
    p.saved_views[0].apply_to_session(&mut target_session);
    assert_eq!(target_session.filter.conditions.len(), 1);
    assert_eq!(target_session.filter.conditions[0].value, "FATAL");
}