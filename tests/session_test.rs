use log_gpui::column::PinnableColumn;
use log_gpui::marker::MarkerType;
use log_gpui::workspace::log_space::LogSpaceKind;
use log_gpui::workspace::project::{ProjectColorAccent, StoredMarker};
use log_gpui::workspace::session::{SessionManifest, SessionPersistence, default_session_path};
use log_gpui::workspace::state::WorkspaceState;
use log_gpui::workspace::working_set::{TabSessionState, TabTarget};

/// A session with a search and column layout variations, so round-tripping
/// through JSON actually exercises every field. The Search Expression is never
/// written to disk.
fn rich_session() -> TabSessionState {
    let mut s = TabSessionState::new();
    s.search.query = "gateway".into();
    s.column_layout.pin(PinnableColumn::Timestamp);
    s.column_layout.pin(PinnableColumn::Module);
    s.column_layout.set_width(PinnableColumn::Level, gpui::px(88.0));
    s
}

/// A workspace with one project that has an Imported Log Space and one open tab.
fn build_workspace() -> WorkspaceState {
    let mut ws = WorkspaceState::new();
    ws.create_project("Payment Service", ProjectColorAccent::Blue);
    let proj = ws.active_project_mut().unwrap();
    let space = proj.create_log_space("Payment", LogSpaceKind::Imported).unwrap();
    space.add_imported_files(&["/tmp/api.log".to_string()]).unwrap();
    space.filter.toggle_value("level", "WARN");
    space.filter.add_custom("module", "pay");
    space.filter.set_hide("module", true);
    space.filter.set_time_range("10:00:00", "10:05:00");
    space.file_mut("/tmp/api.log").unwrap().markers.push(StoredMarker {
        line: 42,
        marker: Some(MarkerType::Bookmark),
        note: Some("check this".into()),
        content_hash: None,
    });
    let space_id = space.id.clone();
    let _ = proj.open_log_space(&space_id, rich_session());
    ws
}

#[test]
fn test_manifest_round_trips_project_and_working_set() {
    let ws = build_workspace();
    let manifest = SessionManifest::from_workspace(&ws);

    let json = manifest.to_json().unwrap();
    let parsed = SessionManifest::from_json(&json);
    assert!(parsed.is_ok(), "round trip error: {:?}", parsed.err());

    let restored = parsed.unwrap();
    assert_eq!(restored.version, 3);
    assert_eq!(restored.active_project_id.as_deref(), Some("payment-service"));
    assert_eq!(restored.projects.len(), 1);

    let project = &restored.projects[0];
    assert_eq!(project.name, "Payment Service");
    assert_eq!(project.log_spaces.len(), 1);
    let space = &project.log_spaces[0];
    assert_eq!(space.name, "Payment");
    assert_eq!(space.kind, LogSpaceKind::Imported);
    assert_eq!(space.files.len(), 1);
    assert_eq!(space.files[0].file_id, "/tmp/api.log");

    // The Log Space keeps its own Filter; the file keeps its Markers.
    let filter = &space.filter;
    assert!(filter.is_value_ticked("level", "warn"));
    assert_eq!(filter.column("module").unwrap().custom, vec!["pay".to_string()]);
    assert!(filter.column("module").unwrap().hide);
    assert_eq!(filter.time_range.from, "10:00:00");
    assert_eq!(filter.time_range.to, "10:05:00");
    assert_eq!(space.files[0].markers.len(), 1);
    assert_eq!(space.files[0].markers[0].line, 42);
    assert_eq!(space.files[0].markers[0].note.as_deref(), Some("check this"));

    // Working set lives inside the project and preserves the tab session.
    assert_eq!(project.working_set.tab_count(), 1);
    let tab = &project.working_set.tabs()[0];
    assert!(matches!(tab.target, TabTarget::LogSpace { ref space_id } if space_id == &space.id));
    assert_eq!(tab.session.search.query, "", "Search is not remembered");
    assert!(tab.session.column_layout.is_pinned(PinnableColumn::Timestamp));
    assert!(tab.session.column_layout.is_pinned(PinnableColumn::Module));
    assert_eq!(tab.session.column_layout.width(PinnableColumn::Level).as_f32(), 88.0);
    assert_eq!(project.working_set.active_tab_index(), Some(0));
}

#[test]
fn test_manifest_tab_order_and_active_preserved() {
    let mut ws = WorkspaceState::new();
    ws.create_project("p1", ProjectColorAccent::Red);
    let proj = ws.active_project_mut().unwrap();
    let _ = proj.open_file_view("s", "/f1.log", TabSessionState::new());
    let _ = proj.open_file_view("s", "/f2.log", TabSessionState::new());
    proj.working_set.set_active_tab(1);

    let manifest = SessionManifest::from_workspace(&ws);
    let restored = SessionManifest::from_json(&manifest.to_json().unwrap()).unwrap();

    let ws_set = &restored.projects[0].working_set;
    assert_eq!(ws_set.tab_count(), 2);
    assert_eq!(ws_set.tabs()[0].session.search.query, "");
    assert!(matches!(ws_set.tabs()[0].target, TabTarget::FileView { ref file_id, .. } if file_id == "/f1.log"));
    assert!(matches!(ws_set.tabs()[1].target, TabTarget::FileView { ref file_id, .. } if file_id == "/f2.log"));
    assert_eq!(ws_set.active_tab_index(), Some(1));
}

#[test]
fn test_persistence_save_then_load_round_trip() {
    let ws = build_workspace();
    let dir = std::env::temp_dir().join(format!("log_gpui_session_test_{}", std::process::id()));
    let path = dir.join("session.json");

    let persistence = SessionPersistence::new(path.clone());
    persistence.save(&ws).unwrap();
    assert!(path.exists());

    let loaded = persistence.load().unwrap().expect("manifest not found");
    assert_eq!(loaded.version, 3);
    assert_eq!(loaded.projects.len(), 1);
    assert_eq!(loaded.projects[0].name, "Payment Service");
    assert_eq!(loaded.projects[0].working_set.tabs()[0].session.search.query, "");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_persistence_load_missing_file_returns_none() {
    let dir = std::env::temp_dir().join(format!("log_gpui_missing_{}", std::process::id()));
    let path = dir.join("nope.json");
    let persistence = SessionPersistence::new(path);
    assert!(persistence.load().unwrap().is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_persistence_load_corrupt_file_is_error() {
    let dir = std::env::temp_dir().join(format!("log_gpui_corrupt_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad.json");
    std::fs::write(&path, "{ not json").unwrap();

    let persistence = SessionPersistence::new(path);
    let err = persistence.load().map(|_| ()).unwrap_err();
    assert!(err.to_string().contains("line 1"), "unexpected error: {err}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_default_session_path_targets_log_gpui_session_json() {
    let path = default_session_path();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), "session.json");
    assert!(path.to_str().unwrap().contains("log_gpui"));
}

#[test]
fn test_manifest_into_workspace_restores_projects_and_all_state() {
    let ws = build_workspace();
    let manifest = SessionManifest::from_workspace(&ws);
    let mut restored_ws = manifest.into_workspace();

    assert_eq!(restored_ws.project_count(), 1);
    let active = restored_ws.active_project().expect("active project restored");
    assert_eq!(active.name, "Payment Service");
    assert_eq!(active.log_spaces.len(), 1);
    assert_eq!(active.working_set.tab_count(), 1);
    // Not written to JSON, so the in-memory Search is still there.
    assert_eq!(active.working_set.tabs()[0].session.search.query, "gateway");
    assert_eq!(restored_ws.active_project_id.as_deref(), Some("payment-service"));

    // Restored workspace stays fully functional.
    let _ = restored_ws
        .active_project_mut()
        .unwrap()
        .open_file_view("payment", "/worker.log", TabSessionState::new());
    assert_eq!(restored_ws.active_project().unwrap().working_set.tab_count(), 2);
}

#[test]
fn test_multiple_projects_each_keep_own_working_set() {
    let mut ws = WorkspaceState::new();
    ws.create_project("payment-service", ProjectColorAccent::Blue);
    let _ = ws.active_project_mut().unwrap().open_file_view("s", "/api.log", rich_session());

    ws.create_project("search", ProjectColorAccent::Green);
    let _ = ws.active_project_mut().unwrap().open_file_view("s", "/indexer.log", TabSessionState::new());
    let _ = ws.active_project_mut().unwrap().open_file_view("s", "/queue.log", TabSessionState::new());

    let manifest = SessionManifest::from_workspace(&ws);
    let json = manifest.to_json().unwrap();
    assert!(json.contains("/api.log"));
    assert!(json.contains("/indexer.log"));

    let restored = SessionManifest::from_json(&json).unwrap();
    assert_eq!(restored.projects.len(), 2);
    assert_eq!(restored.active_project_id.as_deref(), Some("search"));

    let payment = restored.projects.iter().find(|p| p.name == "payment-service").unwrap();
    assert_eq!(payment.working_set.tab_count(), 1);
    assert_eq!(payment.working_set.tabs()[0].session.search.query, "");

    let search = restored.projects.iter().find(|p| p.name == "search").unwrap();
    assert_eq!(search.working_set.tab_count(), 2);
}

#[test]
fn test_workspaces_remain_isolated_after_round_trip() {
    let mut ws = WorkspaceState::new();
    ws.create_project("a", ProjectColorAccent::Blue);
    let proj = ws.active_project_mut().unwrap();
    let id = proj.create_log_space("Shared", LogSpaceKind::Imported).unwrap().id.clone();
    proj.log_space_mut(&id).unwrap().add_imported_files(&["/shared.log".to_string()]).unwrap();
    let _ = proj.open_file_view(&id, "/shared.log", TabSessionState::new());

    ws.create_project("b", ProjectColorAccent::Green);
    let _ = ws.switch_project("a");
    let _ = ws.switch_project("b");

    let manifest = SessionManifest::from_workspace(&ws);
    let restored = SessionManifest::from_json(&manifest.to_json().unwrap()).unwrap();
    let restored_ws = restored.into_workspace();
    assert!(restored_ws.projects_are_isolated("a", "b"));
}

#[test]
fn test_investigation_board_tab_persists() {
    let mut ws = WorkspaceState::new();
    ws.create_project("incidents", ProjectColorAccent::Red);
    let _ = ws.active_project_mut().unwrap().open_investigation_board("INC-3");

    let manifest = SessionManifest::from_workspace(&ws);
    let restored = SessionManifest::from_json(&manifest.to_json().unwrap()).unwrap();
    let tab = &restored.projects[0].working_set.tabs()[0];
    assert!(matches!(tab.target, TabTarget::InvestigationBoard { ref case_id } if case_id == "INC-3"));
}

#[test]
fn test_file_view_tab_and_view_mode_persist() {
    use log_gpui::workspace::working_set::ViewMode;
    let mut ws = WorkspaceState::new();
    ws.create_project("chronology", ProjectColorAccent::Cyan);
    let mut session = TabSessionState::new();
    session.mode = ViewMode::Raw;
    session.collapsed_sections.insert("/a.log".to_string());
    let _ = ws
        .active_project_mut()
        .unwrap()
        .open_file_view("s", "/a.log", session);

    let manifest = SessionManifest::from_workspace(&ws);
    let restored = SessionManifest::from_json(&manifest.to_json().unwrap()).unwrap();
    let tab = &restored.projects[0].working_set.tabs()[0];
    assert!(matches!(tab.target, TabTarget::FileView { ref space_id, ref file_id } if space_id == "s" && file_id == "/a.log"));
    assert_eq!(tab.session.mode, ViewMode::Raw);
    assert!(tab.session.collapsed_sections.contains("/a.log"));
}

#[test]
fn test_empty_workspace_serializes() {
    let ws = WorkspaceState::new();
    let manifest = SessionManifest::from_workspace(&ws);
    let json = manifest.to_json().unwrap();
    let restored = SessionManifest::from_json(&json).unwrap();
    assert_eq!(restored.projects.len(), 0);
    assert_eq!(restored.active_project_id, None);
}

#[test]
fn test_manifest_rejects_unknown_version() {
    let ws = build_workspace();
    let mut manifest = SessionManifest::from_workspace(&ws);
    manifest.version = 999;
    assert!(manifest.to_json().unwrap().contains("\"version\": 999"));
}

#[test]
fn test_session_of_an_older_format_is_ignored() {
    // An older session keeps `files` on the project instead of Log Spaces.
    let dir = std::env::temp_dir().join(format!("log_gpui_old_session_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("session.json");
    let old = serde_json::json!({
        "version": 2,
        "active_project_id": "p",
        "projects": [{
            "id": "p", "name": "P", "color": "Blue",
            "files": [{ "file_id": "/a.log", "display_name": "a.log" }],
            "working_set": { "tabs": [], "active_index": null },
            "investigation_cases": []
        }]
    });
    std::fs::write(&path, old.to_string()).unwrap();

    let persistence = SessionPersistence::new(path);
    assert!(persistence.load().unwrap().is_none(), "old sessions are not converted");
    assert!(SessionManifest::from_json_current(&old.to_string()).is_none());
    std::fs::remove_dir_all(&dir).ok();
}
