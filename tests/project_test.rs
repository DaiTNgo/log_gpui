use log_gpui::column::PinnableColumn;
use log_gpui::workspace::log_space::LogSpaceKind;
use log_gpui::workspace::project::{ProjectColorAccent, slug_project_id};
use log_gpui::workspace::state::{ContextSidebarState, ContextSection, LeftSidebarState, WorkspaceState};
use log_gpui::workspace::working_set::{TabSessionState, WorkingSet};

fn session_with_pin(selected_row: usize) -> TabSessionState {
    let mut session = TabSessionState::new();
    session.selected_row = Some(selected_row);
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

    // Populate Payment project with a Log Space + tab
    {
        let payment = ws.project_mut("payment-service").unwrap();
        let id = payment.create_log_space("Checkout", LogSpaceKind::Scratch).unwrap().id.clone();
        payment.open_log_space(&id, session_with_pin(0));
        assert_eq!(payment.working_set.tab_count(), 1);
    }

    // Switch to E-Commerce
    ws.switch_project("e-commerce-core").unwrap();
    let ecom = ws.active_project().unwrap();
    assert_eq!(ecom.working_set.tab_count(), 0);
    assert!(ecom.log_spaces.is_empty());

    // Switch back: Payment context fully restored
    ws.switch_project("payment-service").unwrap();
    let payment = ws.active_project().unwrap();
    assert_eq!(payment.working_set.tab_count(), 1);
    let tab = payment.working_set.active_tab().unwrap();
    assert!(tab.session.column_layout.is_pinned(PinnableColumn::Timestamp));
}

#[test]
fn test_switch_project_restores_tab_sessions() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Proj A", ProjectColorAccent::Blue);

    {
        let p = ws.active_project_mut().unwrap();
        let mut s = session_with_pin(0);
        s.scroll_x = 250.0;
        s.selected_row = Some(17);
        let id = p.create_log_space("A", LogSpaceKind::Scratch).unwrap().id.clone();
        p.open_log_space(&id, s);
    }

    ws.create_project("Proj B", ProjectColorAccent::Red);
    {
        let p = ws.active_project_mut().unwrap();
        let mut s = TabSessionState::new();
        s.scroll_x = 5.0;
        let id = p.create_log_space("B", LogSpaceKind::Scratch).unwrap().id.clone();
        p.open_log_space(&id, s);
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
        .create_log_space("Payment", LogSpaceKind::Scratch)
        .unwrap();

    ws.create_project("B", ProjectColorAccent::Green);
    ws.active_project_mut()
        .unwrap()
        .create_log_space("Payment", LogSpaceKind::Scratch)
        .unwrap();

    assert!(ws.projects_are_isolated("a", "b"));
}

#[test]
fn test_slug_generates_url_safe_id() {
    assert_eq!(slug_project_id("Payment Service"), "payment-service");
    assert_eq!(slug_project_id("Payments!@#$"), "payments");
    assert_eq!(slug_project_id("^^^"), "project");
    assert_eq!(slug_project_id("Payment   Service"), "payment-service");
}

// ─── Project: Log Spaces ─────────────────────────────────────────────────────

#[test]
fn test_remove_log_space_closes_its_tabs() {
    let mut ws = WorkspaceState::new();
    ws.create_project("P", ProjectColorAccent::Blue);
    let p = ws.active_project_mut().unwrap();
    let id = p.create_log_space("Payment", LogSpaceKind::Imported).unwrap().id.clone();
    p.log_space_mut(&id)
        .unwrap()
        .add_imported_files(&["/var/log/api.log".to_string()])
        .unwrap();
    p.open_log_space(&id, session_with_pin(0));
    p.open_file_view(&id, "/var/log/api.log", session_with_pin(0));
    assert_eq!(p.working_set.tab_count(), 2);

    let removed = p.remove_log_space(&id);

    assert!(removed.is_some());
    assert_eq!(p.working_set.tab_count(), 0);
    assert!(p.log_spaces.is_empty());
}

#[test]
fn test_open_log_space_opens_or_focuses_tab() {
    let mut ws = WorkspaceState::new();
    ws.create_project("P", ProjectColorAccent::Blue);
    let p = ws.active_project_mut().unwrap();
    let id = p.create_log_space("Payment", LogSpaceKind::Scratch).unwrap().id.clone();

    p.open_log_space(&id, session_with_pin(0));
    assert_eq!(p.working_set.tab_count(), 1);

    // Opening the same Log Space focuses the existing tab, no duplicate
    assert!(p.open_log_space(&id, session_with_pin(0)).is_none());
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
fn test_context_sidebar_defaults_hidden_with_filter_and_inspector_open() {
    let cs = ContextSidebarState::new();
    assert!(!cs.visible);
    assert!(!cs.is_collapsed(ContextSection::Filter));
    assert!(cs.is_collapsed(ContextSection::Highlights));
    assert!(!cs.is_collapsed(ContextSection::Inspector));
    assert!(!cs.is_shown(true));
}

#[test]
fn test_context_sidebar_toggle_keeps_section_state() {
    let mut cs = ContextSidebarState::new();
    cs.reveal(ContextSection::Highlights);
    assert!(cs.visible);
    cs.toggle_section(ContextSection::Filter);
    cs.toggle();
    cs.toggle();
    assert!(cs.visible);
    assert!(!cs.is_collapsed(ContextSection::Highlights));
    assert!(cs.is_collapsed(ContextSection::Filter));
}

#[test]
fn test_context_sidebar_requires_a_log_file_tab() {
    let mut cs = ContextSidebarState::new();
    cs.reveal(ContextSection::Filter);
    assert!(cs.is_shown(true));
    assert!(!cs.is_shown(false));
}

#[test]
fn test_context_section_labels() {
    let labels: Vec<_> = ContextSection::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(labels, ["Filter", "Highlights", "Columns", "Inspector"]);
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
    p.open_file_view("s", "a.log", session_with_pin(0));

    ws.create_project("B", ProjectColorAccent::Red);
    let p = ws.active_project_mut().unwrap();
    p.open_file_view("s", "b.log", session_with_pin(0));

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

    assert!(!p.working_set.is_file_open("app.log"));
    assert!(!p.working_set.is_file_open("worker.log"));

    p.open_file_view("triage", "app.log", session_with_pin(0));
    assert!(p.working_set.is_file_open("app.log"));
    assert!(!p.working_set.is_file_open("worker.log"));

    p.open_file_view("triage", "worker.log", session_with_pin(0));
    assert!(p.working_set.is_file_open("app.log"));
    assert!(p.working_set.is_file_open("worker.log"));

    // Close app.log via find_tab
    let target = log_gpui::workspace::working_set::TabTarget::FileView {
        space_id: "triage".to_string(),
        file_id: "app.log".to_string(),
    };
    let tab_idx = p.working_set.find_tab(&target).expect("app.log tab should exist");
    p.working_set.close_tab(tab_idx);

    assert!(!p.working_set.is_file_open("app.log"));
    assert!(p.working_set.is_file_open("worker.log"));
}

#[test]
fn test_project_investigations_lifecycle() {
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
}
