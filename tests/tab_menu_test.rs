use log_gpui::workspace::{CloseScope, TabSessionState, TabTarget, WorkingSet};

fn space(id: &str) -> TabTarget {
    TabTarget::LogSpace { space_id: id.into() }
}

/// A working set of tabs "a".."e" with the given tab active.
fn five(active: usize) -> WorkingSet {
    let mut ws = WorkingSet::new();
    for id in ["a", "b", "c", "d", "e"] {
        ws.open_tab(space(id), TabSessionState::new());
    }
    ws.set_active_tab(active);
    ws
}

fn ids(ws: &WorkingSet) -> Vec<String> {
    ws.tabs().iter().map(|t| t.target.space_id().unwrap().to_string()).collect()
}

fn active_id(ws: &WorkingSet) -> String {
    ws.active_tab().unwrap().target.space_id().unwrap().to_string()
}

#[test]
fn pinning_moves_the_tab_behind_the_pinned_group_and_keeps_it_active() {
    let mut ws = five(2);
    assert_eq!(ws.pin_tab(2), 0);
    assert_eq!(ids(&ws), ["c", "a", "b", "d", "e"]);
    assert_eq!(active_id(&ws), "c");
    assert_eq!(ws.pin_tab(4), 1);
    assert_eq!(ids(&ws), ["c", "e", "a", "b", "d"]);
    assert_eq!(active_id(&ws), "c");
    assert!(ws.is_pinned(0) && ws.is_pinned(1) && !ws.is_pinned(2));
}

#[test]
fn unpinning_moves_the_tab_to_the_front_of_the_unpinned_group() {
    let mut ws = five(0);
    ws.pin_tab(0);
    ws.pin_tab(3);
    assert_eq!(ids(&ws), ["a", "d", "b", "c", "e"]);
    assert_eq!(ws.unpin_tab(0), 1);
    assert_eq!(ids(&ws), ["d", "a", "b", "c", "e"]);
    assert!(!ws.is_pinned(1));
    assert_eq!(active_id(&ws), "a");
}

#[test]
fn pinning_a_pinned_tab_or_unpinning_an_unpinned_one_changes_nothing() {
    let mut ws = five(0);
    ws.pin_tab(0);
    assert_eq!(ws.pin_tab(0), 0);
    assert_eq!(ws.unpin_tab(3), 3);
    assert_eq!(ids(&ws), ["a", "b", "c", "d", "e"]);
}

#[test]
fn close_others_keeps_the_clicked_tab_and_pinned_tabs() {
    let mut ws = five(0);
    ws.pin_tab(4); // e a b c d
    ws.close_scope(CloseScope::Others, 3); // keep "c"
    assert_eq!(ids(&ws), ["e", "c"]);
    assert_eq!(active_id(&ws), "c");
}

#[test]
fn close_right_and_left_skip_pinned_and_keep_the_active_tab_when_it_survives() {
    let mut ws = five(1);
    ws.close_scope(CloseScope::Right, 2);
    assert_eq!(ids(&ws), ["a", "b", "c"]);
    assert_eq!(active_id(&ws), "b");

    let mut ws = five(4);
    ws.pin_tab(0); // a b c d e, "a" pinned
    ws.close_scope(CloseScope::Left, 3);
    assert_eq!(ids(&ws), ["a", "d", "e"]);
    assert_eq!(active_id(&ws), "e");
}

#[test]
fn closing_the_active_tab_activates_the_clicked_tab() {
    let mut ws = five(4);
    ws.close_scope(CloseScope::Right, 1);
    assert_eq!(ids(&ws), ["a", "b"]);
    assert_eq!(active_id(&ws), "b");
}

#[test]
fn close_all_leaves_only_pinned_tabs() {
    let mut ws = five(3);
    ws.pin_tab(1);
    ws.close_scope(CloseScope::All, 0);
    assert_eq!(ids(&ws), ["b"]);
    assert_eq!(active_id(&ws), "b");

    let mut ws = five(2);
    ws.close_scope(CloseScope::All, 0);
    assert_eq!(ws.tab_count(), 0);
    assert_eq!(ws.active_tab_index(), None);
}

#[test]
fn closable_count_ignores_pinned_tabs() {
    let mut ws = five(0);
    ws.pin_tab(0);
    ws.pin_tab(1); // a b pinned, then c d e
    assert_eq!(ws.closable_count(CloseScope::Others, 2), 2);
    assert_eq!(ws.closable_count(CloseScope::Left, 2), 0);
    assert_eq!(ws.closable_count(CloseScope::Right, 2), 2);
    assert_eq!(ws.closable_count(CloseScope::Right, 4), 0);
    assert_eq!(ws.closable_count(CloseScope::All, 0), 3);
}

#[test]
fn pinned_flag_survives_a_session_round_trip_and_old_sessions_load_unpinned() {
    let mut ws = five(0);
    ws.pin_tab(2);
    let json = serde_json::to_string(&ws).unwrap();
    let back: WorkingSet = serde_json::from_str(&json).unwrap();
    assert!(back.is_pinned(0));
    assert!(!back.is_pinned(1));

    let old = json.replace(",\"pinned\":false", "").replace(",\"pinned\":true", "");
    let back: WorkingSet = serde_json::from_str(&old).unwrap();
    assert!(!back.is_pinned(0));
}
