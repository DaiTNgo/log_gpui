use log_gpui::column::{ColumnLayoutState, PinnableColumn};
use log_gpui::filter::{ConditionField, ConditionOp, FilterEngine};
use log_gpui::search::SearchState;
use log_gpui::workspace::{ProjectColorAccent, TabSessionState, TabTarget, WorkingSet, WorkspaceState};

fn make_session(filter_level: &str, search_query: &str, scroll_x: f32) -> TabSessionState {
    let mut filter = FilterEngine::new();
    if !filter_level.is_empty() {
        filter.add_condition(
            ConditionField::Level,
            ConditionOp::Is,
            filter_level.to_string(),
        );
    }
    let mut search = SearchState::new();
    search.query = search_query.to_string();
    TabSessionState {
        filter,
        search,
        column_layout: ColumnLayoutState::new(),
        scroll_x,
        scroll_top: 0.0,
        selected_row: None,
    }
}

// ─── Tab Addition & Deduplication ────────────────────────────────────────────

#[test]
fn test_open_tab_adds_to_working_set() {
    let mut ws = WorkingSet::new();
    let session = make_session("ERROR", "", 0.0);
    ws.open_tab(TabTarget::LogFile {
        file_id: "api.log".into(),
    }, session);

    assert_eq!(ws.tabs().len(), 1);
    assert_eq!(ws.active_tab_index(), Some(0));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile {
            file_id: "api.log".into()
        }
    );
}

#[test]
fn test_open_duplicate_tab_focuses_existing() {
    let mut ws = WorkingSet::new();
    let mut session1 = make_session("ERROR", "", 0.0);
    session1.scroll_x = 50.0;
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        session1,
    );

    // Second open of same file should focus existing tab, not create duplicate
    let session2 = make_session("WARN", "test", 0.0);
    let result = ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        session2,
    );

    assert!(result.is_none()); // No new tab created
    assert_eq!(ws.tabs().len(), 1);
    // The original session state is preserved
    assert_eq!(ws.active_tab().unwrap().session.scroll_x, 50.0);
}

#[test]
fn test_open_different_files_creates_multiple_tabs() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("ERROR", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("WARN", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "nginx.log".into(),
        },
        make_session("", "timeout", 0.0),
    );

    assert_eq!(ws.tabs().len(), 3);
    assert_eq!(ws.active_tab_index(), Some(2)); // Last opened is active
}

#[test]
fn test_open_tab_switches_to_existing_if_present() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("ERROR", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("WARN", "", 0.0),
    );

    // Re-open api.log: should switch focus back to it
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("", "", 0.0),
    );

    assert_eq!(ws.tabs().len(), 2); // Still only 2 tabs
    assert_eq!(ws.active_tab_index(), Some(0)); // Switched to api.log
}

// ─── Tab Closure ──────────────────────────────────────────────────────────────

#[test]
fn test_close_tab_removes_from_working_set() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("ERROR", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("WARN", "", 0.0),
    );

    let closed = ws.close_tab(0);
    assert!(closed.is_some());
    assert_eq!(closed.unwrap().target, TabTarget::LogFile { file_id: "api.log".into() });
    assert_eq!(ws.tabs().len(), 1);
    assert_eq!(ws.active_tab_index(), Some(0));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile {
            file_id: "worker.log".into()
        }
    );
}

#[test]
fn test_close_active_tab_switches_to_adjacent() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "nginx.log".into(),
        },
        make_session("", "", 0.0),
    );
    assert_eq!(ws.active_tab_index(), Some(2));

    // Close active tab (nginx.log at index 2) -> switches to worker.log (index 1)
    ws.close_active_tab();
    assert_eq!(ws.tabs().len(), 2);
    assert_eq!(ws.active_tab_index(), Some(1));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile {
            file_id: "worker.log".into()
        }
    );
}

#[test]
fn test_close_last_tab_results_in_empty_working_set() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.close_active_tab();
    assert!(ws.tabs().is_empty());
    assert_eq!(ws.active_tab_index(), None);
    assert!(ws.active_tab().is_none());
}

#[test]
fn test_close_first_tab_switches_to_next() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("", "", 0.0),
    );
    // Switch to first tab
    ws.set_active_tab(0);
    assert_eq!(ws.active_tab_index(), Some(0));

    // Close first tab -> should go to index 0 (which is now worker.log)
    ws.close_active_tab();
    assert_eq!(ws.tabs().len(), 1);
    assert_eq!(ws.active_tab_index(), Some(0));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile {
            file_id: "worker.log".into()
        }
    );
}

// ─── Tab Switching ────────────────────────────────────────────────────────────

#[test]
fn test_set_active_tab_by_index() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("ERROR", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("WARN", "timeout", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "nginx.log".into(),
        },
        make_session("", "", 100.0),
    );

    ws.set_active_tab(0);
    assert_eq!(ws.active_tab_index(), Some(0));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile {
            file_id: "api.log".into()
        }
    );

    ws.set_active_tab(2);
    assert_eq!(ws.active_tab_index(), Some(2));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile {
            file_id: "nginx.log".into()
        }
    );
}

#[test]
fn test_next_tab_wraps_around() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "a.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "b.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.set_active_tab(0);

    ws.next_tab();
    assert_eq!(ws.active_tab_index(), Some(1));

    ws.next_tab();
    assert_eq!(ws.active_tab_index(), Some(0)); // Wraps around
}

#[test]
fn test_prev_tab_wraps_around() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "a.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "b.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.set_active_tab(0);

    ws.prev_tab();
    assert_eq!(ws.active_tab_index(), Some(1)); // Wraps to end
}

#[test]
fn test_select_tab_by_number_shortcut() {
    let mut ws = WorkingSet::new();
    for i in 0..9 {
        ws.open_tab(
            TabTarget::LogFile {
                file_id: format!("file{}.log", i),
            },
            make_session("", "", 0.0),
        );
    }

    // Cmd+1 through Cmd+9
    ws.select_tab_by_number(1);
    assert_eq!(ws.active_tab_index(), Some(0));

    ws.select_tab_by_number(9);
    assert_eq!(ws.active_tab_index(), Some(8));

    // Out of range should be no-op
    let prev = ws.active_tab_index();
    ws.select_tab_by_number(10);
    assert_eq!(ws.active_tab_index(), prev);
}

#[test]
fn test_empty_working_set_tab_operations_are_noops() {
    let mut ws = WorkingSet::new();

    assert_eq!(ws.active_tab_index(), None);
    assert!(ws.active_tab().is_none());

    ws.close_active_tab();
    ws.set_active_tab(0);
    ws.next_tab();
    ws.prev_tab();
    ws.select_tab_by_number(1);

    assert!(ws.tabs().is_empty());
}

// ─── Independent Session State ────────────────────────────────────────────────

#[test]
fn test_tab_sessions_are_independent() {
    let mut ws = WorkingSet::new();

    let mut session_a = make_session("ERROR", "timeout", 50.0);
    session_a.column_layout.pin(PinnableColumn::Timestamp);
    session_a.selected_row = Some(42);
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        session_a,
    );

    let mut session_b = make_session("WARN", "connection", 200.0);
    session_b.column_layout.pin(PinnableColumn::Module);
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        session_b,
    );

    // Verify api.log session
    ws.set_active_tab(0);
    let tab_a = ws.active_tab().unwrap();
    assert_eq!(tab_a.session.scroll_x, 50.0);
    assert_eq!(tab_a.session.selected_row, Some(42));
    assert!(tab_a.session.column_layout.is_pinned(PinnableColumn::Timestamp));
    assert!(!tab_a.session.column_layout.is_pinned(PinnableColumn::Module));

    // Verify worker.log session
    ws.set_active_tab(1);
    let tab_b = ws.active_tab().unwrap();
    assert_eq!(tab_b.session.scroll_x, 200.0);
    assert!(!tab_b.session.column_layout.is_pinned(PinnableColumn::Timestamp));
    assert!(tab_b.session.column_layout.is_pinned(PinnableColumn::Module));
}

#[test]
fn test_session_state_survives_tab_switching() {
    let mut ws = WorkingSet::new();

    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("ERROR", "search_term", 100.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "worker.log".into(),
        },
        make_session("WARN", "other_search", 200.0),
    );

    // Switch back and forth multiple times
    for _ in 0..10 {
        ws.set_active_tab(0);
        assert_eq!(ws.active_tab().unwrap().session.scroll_x, 100.0);
        ws.set_active_tab(1);
        assert_eq!(ws.active_tab().unwrap().session.scroll_x, 200.0);
    }
}

// ─── Investigation Board Tabs ────────────────────────────────────────────────

#[test]
fn test_open_investigation_board_tab() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::InvestigationBoard {
            case_id: "INC-142".into(),
        },
        make_session("", "", 0.0),
    );

    assert_eq!(ws.tabs().len(), 1);
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::InvestigationBoard {
            case_id: "INC-142".into()
        }
    );
}

#[test]
fn test_open_duplicate_investigation_board_focuses_existing() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::InvestigationBoard {
            case_id: "INC-142".into(),
        },
        make_session("", "", 0.0),
    );
    let result = ws.open_tab(
        TabTarget::InvestigationBoard {
            case_id: "INC-142".into(),
        },
        make_session("", "", 0.0),
    );

    assert!(result.is_none());
    assert_eq!(ws.tabs().len(), 1);
}

// ─── Chronicle Stream Tabs ───────────────────────────────────────────────────

#[test]
fn test_open_chronicle_stream_tab() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::ChronicleStream {
            file_ids: vec!["api.log".into(), "worker.log".into()],
        },
        make_session("", "", 0.0),
    );

    assert_eq!(ws.tabs().len(), 1);
    match &ws.active_tab().unwrap().target {
        TabTarget::ChronicleStream { file_ids } => {
            assert_eq!(file_ids.len(), 2);
        }
        _ => panic!("Expected ChronicleStream tab"),
    }
}

// ─── Tab Reordering ──────────────────────────────────────────────────────────

#[test]
fn test_move_tab_forward() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "a.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "b.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "c.log".into(),
        },
        make_session("", "", 0.0),
    );

    ws.move_tab(0, 2); // Move a.log to index 2
    let names: Vec<_> = ws
        .tabs()
        .iter()
        .map(|t| match &t.target {
            TabTarget::LogFile { file_id } => file_id.clone(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(names, vec!["b.log", "c.log", "a.log"]);
}

#[test]
fn test_move_tab_backward() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "a.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "b.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "c.log".into(),
        },
        make_session("", "", 0.0),
    );

    ws.move_tab(2, 0); // Move c.log to index 0
    let names: Vec<_> = ws
        .tabs()
        .iter()
        .map(|t| match &t.target {
            TabTarget::LogFile { file_id } => file_id.clone(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(names, vec!["c.log", "a.log", "b.log"]);
}

// ─── Active Tab Index After Closure ───────────────────────────────────────────

#[test]
fn test_close_middle_tab_selects_next_if_available() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile { file_id: "a.log".into() },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile { file_id: "b.log".into() },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile { file_id: "c.log".into() },
        make_session("", "", 0.0),
    );
    // Active is c.log (index 2)
    ws.set_active_tab(1); // Switch to b.log

    ws.close_active_tab(); // Close b.log
    // Should select c.log (which was at index 2, now at index 1)
    assert_eq!(ws.tabs().len(), 2);
    assert_eq!(ws.active_tab_index(), Some(1));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile { file_id: "c.log".into() }
    );
}

#[test]
fn test_close_tab_at_end_selects_previous() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile { file_id: "a.log".into() },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile { file_id: "b.log".into() },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile { file_id: "c.log".into() },
        make_session("", "", 0.0),
    );
    // Active is c.log (index 2)
    ws.close_active_tab(); // Close c.log
    assert_eq!(ws.active_tab_index(), Some(1));
    assert_eq!(
        ws.active_tab().unwrap().target,
        TabTarget::LogFile { file_id: "b.log".into() }
    );
}

// ─── File Membership Query ───────────────────────────────────────────────────

#[test]
fn test_is_file_open_in_working_set() {
    let mut ws = WorkingSet::new();
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "api.log".into(),
        },
        make_session("", "", 0.0),
    );

    assert!(ws.is_file_open("api.log"));
    assert!(!ws.is_file_open("worker.log"));
}

#[test]
fn test_tab_count() {
    let mut ws = WorkingSet::new();
    assert_eq!(ws.tab_count(), 0);

    ws.open_tab(
        TabTarget::LogFile {
            file_id: "a.log".into(),
        },
        make_session("", "", 0.0),
    );
    ws.open_tab(
        TabTarget::LogFile {
            file_id: "b.log".into(),
        },
        make_session("", "", 0.0),
    );
    assert_eq!(ws.tab_count(), 2);
}

// ─── Project Lifecycle Helpers ────────────────────────────────────────────────

#[test]
fn test_project_named_checks_display_name() {
    let mut state = WorkspaceState::new();
    state.create_project("Log A", ProjectColorAccent::Blue);
    state.create_project("Log B", ProjectColorAccent::Red);

    assert!(state.project_named("Log A"));
    assert!(state.project_named("Log B"));
    assert!(!state.project_named("Log C"));
    assert!(state.project_named("Log B"));

    let first_id = state.projects[0].id.clone();
    state.rename_project(&first_id, "Renamed A");
    assert!(!state.project_named("Log A"));
    assert!(state.project_named("Renamed A"));
}
