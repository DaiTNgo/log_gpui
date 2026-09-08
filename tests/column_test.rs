use gpui::px;
use log_gpui::column::{
    compute_column_width, ColumnLayoutState, FieldMaxChars, IconName, PinToggleState, PinnableColumn,
};
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::theme::Density;

#[test]
fn test_default_column_layout_state() {
    let state = ColumnLayoutState::new();

    // Domain rule 1 & 2: Only Gutter is permanently anchored by default.
    assert!(state.is_gutter_anchored());
    assert!(state.pinned_columns.is_empty());
    assert_eq!(state.gutter_width(), px(88.0));

    // Default metadata column widths
    assert_eq!(state.width(PinnableColumn::Timestamp), px(120.0));
    assert_eq!(state.width(PinnableColumn::Level), px(68.0));
    assert_eq!(state.width(PinnableColumn::Module), px(100.0));

    // Hitbox safety floor min_widths (36-48px)
    assert!(state.min_width(PinnableColumn::Timestamp) >= px(36.0));
    assert!(state.min_width(PinnableColumn::Level) >= px(36.0));
    assert!(state.min_width(PinnableColumn::Module) >= px(36.0));
}

#[test]
fn test_gutter_stationary_anchor_invariant() {
    let state = ColumnLayoutState::new();

    let scroll_positions = [px(0.0), px(12.5), px(50.0), px(88.0), px(240.0), px(1024.0)];

    for scroll_x in scroll_positions {
        // Sticky offset within horizontally shifted row exactly counteracts scroll_x
        assert_eq!(state.gutter_sticky_offset(scroll_x), scroll_x);

        // Resulting screen position for Gutter must strictly remain at x = 0
        assert_eq!(state.screen_gutter_offset(scroll_x), px(0.0));

        // Unpinned columns shift by -scroll_x
        assert_eq!(state.columns_scroll_offset(scroll_x), -scroll_x);
    }
}

#[test]
fn test_density_uniform_row_heights() {
    use log_gpui::theme::Density;

    // Fixed single-line row heights ensuring virtualization performance across all densities
    assert_eq!(Density::Comfortable.row_height(), 30.0);
    assert_eq!(Density::Compact.row_height(), 24.0);
    assert_eq!(Density::UltraCompact.row_height(), 20.0);

    // Font sizes scaled appropriately per density
    assert_eq!(Density::Comfortable.font_size(), 13.0);
    assert_eq!(Density::Compact.font_size(), 12.0);
    assert_eq!(Density::UltraCompact.font_size(), 11.0);
}

#[test]
fn test_unpinned_column_positions_at_various_scroll_offsets() {
    let state = ColumnLayoutState::new();

    // When scroll_x == 0, data columns start directly after Gutter (88px)
    let natural_timestamp_start = state.gutter_width();
    let natural_level_start = natural_timestamp_start + state.width(PinnableColumn::Timestamp);
    let natural_module_start = natural_level_start + state.width(PinnableColumn::Level);

    assert_eq!(natural_timestamp_start, px(88.0));
    assert_eq!(natural_level_start, px(208.0)); // 88 + 120
    assert_eq!(natural_module_start, px(276.0)); // 208 + 68

    // When scrolled by 50px:
    let scroll_x = px(50.0);
    let shifted_timestamp = natural_timestamp_start + state.columns_scroll_offset(scroll_x);
    let shifted_level = natural_level_start + state.columns_scroll_offset(scroll_x);
    let shifted_module = natural_module_start + state.columns_scroll_offset(scroll_x);

    assert_eq!(shifted_timestamp, px(38.0));
    assert_eq!(shifted_level, px(158.0));
    assert_eq!(shifted_module, px(226.0));

    // Gutter remains at x = 0
    assert_eq!(state.screen_gutter_offset(scroll_x), px(0.0));
}

#[test]
fn test_field_max_chars_scan() {
    let entries = vec![
        LogEntry::new(
            1,
            0.0,
            "12:34:56.789".to_string(), // 12 chars
            "auth".to_string(),          // 4 chars
            LogLevel::Info,             // "I" -> 1 char
            "short msg".to_string(),
            "".to_string(),
        ),
        LogEntry::new(
            2,
            1.0,
            "2026-09-08 12:34:56.789123".to_string(), // 26 chars
            "com.cloud.gateway.proxy.SessionManager".to_string(), // 38 chars
            LogLevel::Error, // "E" -> 1 char
            "another payload".to_string(),
            "".to_string(),
        ),
    ];

    let max_chars = FieldMaxChars::scan_entries(&entries);
    assert_eq!(max_chars.timestamp, 26);
    assert_eq!(max_chars.module, 38);
    assert_eq!(max_chars.level, 1);
}

#[test]
fn test_dynamic_column_width_formula() {
    // Formula: Width = max(min_width, (max_chars * W_char) + padding)
    // where W_char = 0.6 * font_size
    let font_size = Density::Compact.font_size(); // 12.0
    let char_width = 0.6 * font_size; // 7.2

    // 1. Long timestamp (26 chars)
    // Expected: 26 * 7.2 + 16.0 = 187.2 + 16.0 = 203.2 px
    let ts_width = compute_column_width(PinnableColumn::Timestamp, 26, font_size, px(48.0));
    assert_eq!(ts_width, px(26.0 * char_width + 16.0));
    assert!(ts_width > px(120.0)); // Auto-expanded beyond legacy 120px

    // 2. Long module (38 chars)
    // Expected: 38 * 7.2 + 16.0 = 273.6 + 16.0 = 289.6 px
    let mod_width = compute_column_width(PinnableColumn::Module, 38, font_size, px(48.0));
    assert_eq!(mod_width, px(38.0 * char_width + 16.0));
    // 3. Safety floor and title floor enforcement:
    // 0 chars floored by title length so column headers never truncate
    let floored_ts = compute_column_width(PinnableColumn::Timestamp, 0, font_size, px(48.0));
    assert_eq!(floored_ts, px(9.0 * char_width + 16.0)); // 80.8px >= 48.0px

    let floored_lvl = compute_column_width(PinnableColumn::Level, 0, font_size, px(36.0));
    assert_eq!(floored_lvl, px(5.0 * char_width + 24.0)); // 60.0px >= 36.0px

    let floored_mod = compute_column_width(PinnableColumn::Module, 0, font_size, px(48.0));
    assert_eq!(floored_mod, px(6.0 * char_width + 16.0)); // 59.2px >= 48.0px
}

#[test]
fn test_dynamic_width_across_densities() {
    let max_chars = 25;

    let width_comfortable = compute_column_width(
        PinnableColumn::Timestamp,
        max_chars,
        Density::Comfortable.font_size(), // 13.0 -> W_char = 7.8
        px(48.0),
    );
    let width_compact = compute_column_width(
        PinnableColumn::Timestamp,
        max_chars,
        Density::Compact.font_size(), // 12.0 -> W_char = 7.2
        px(48.0),
    );
    let width_ultra = compute_column_width(
        PinnableColumn::Timestamp,
        max_chars,
        Density::UltraCompact.font_size(), // 11.0 -> W_char = 6.6
        px(48.0),
    );

    // Comfortable > Compact > UltraCompact
    assert!(width_comfortable > width_compact);
    assert!(width_compact > width_ultra);
    assert_eq!(width_comfortable, px(25.0 * (0.6 * 13.0) + 16.0)); // 211.0px
    assert_eq!(width_compact, px(25.0 * (0.6 * 12.0) + 16.0)); // 196.0px
    assert_eq!(width_ultra, px(25.0 * (0.6 * 11.0) + 16.0)); // 181.0px

    // LEVEL column width also properly scales with density
    let lvl_comfortable = compute_column_width(PinnableColumn::Level, 1, Density::Comfortable.font_size(), px(36.0));
    let lvl_compact = compute_column_width(PinnableColumn::Level, 1, Density::Compact.font_size(), px(36.0));
    let lvl_ultra = compute_column_width(PinnableColumn::Level, 1, Density::UltraCompact.font_size(), px(36.0));
    assert!(lvl_comfortable > lvl_compact);
    assert!(lvl_compact > lvl_ultra);
    assert_eq!(lvl_comfortable, px(5.0 * (0.6 * 13.0) + 24.0)); // 63.0px
    assert_eq!(lvl_compact, px(5.0 * (0.6 * 12.0) + 24.0)); // 60.0px
    assert_eq!(lvl_ultra, px(5.0 * (0.6 * 11.0) + 24.0)); // 57.0px
}
#[test]
fn test_column_layout_state_update_from_entries() {
    let mut state = ColumnLayoutState::new();

    let entries = vec![
        LogEntry::new(
            1,
            0.0,
            "2026-09-08 12:34:56.789".to_string(), // 23 chars
            "api-service".to_string(),             // 11 chars
            LogLevel::Info,
            "msg".to_string(),
            "".to_string(),
        ),
    ];

    state.update_from_entries(&entries, Density::Compact);

    // Widths must expand to fit longest entries
    let expected_ts = px(23.0 * (0.6 * 12.0) + 16.0); // 23 * 7.2 + 16 = 181.6
    let expected_mod = px(11.0 * (0.6 * 12.0) + 16.0); // 11 * 7.2 + 16 = 95.2 -> max(48, 95.2) = 95.2

    assert_eq!(state.width(PinnableColumn::Timestamp), expected_ts);
    assert_eq!(state.width(PinnableColumn::Module), expected_mod);

    // Switching density recomputes widths automatically
    state.update_density(Density::Comfortable);
    let expected_ts_comfortable = px(23.0 * (0.6 * 13.0) + 16.0); // 23 * 7.8 + 16 = 195.4
    assert_eq!(state.width(PinnableColumn::Timestamp), expected_ts_comfortable);
}

#[test]
fn test_100k_entries_scan_performance() {
    let dummy_entry = LogEntry::new(
        1,
        0.0,
        "2026-09-08 12:34:56.789".to_string(),
        "auth-worker-node-42".to_string(),
        LogLevel::Warn,
        "A somewhat long payload for testing high volume performance".to_string(),
        "".to_string(),
    );

    let entries = vec![dummy_entry; 100_000];

    let start = std::time::Instant::now();
    let mut state = ColumnLayoutState::new();
    state.update_from_entries(&entries, Density::Compact);
    let elapsed = start.elapsed();

    // 100k entries scan must complete well under 50ms (zero measurable UI lag)
    assert!(
        elapsed < std::time::Duration::from_millis(50),
        "Scanning 100k entries took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_parser_tracks_field_max_chars_during_ingestion() {
    let raw = b"2026-09-08 12:34:56.789123 [WARN] [com.example.service.Auth] User signed in\n";
    let res = log_gpui::parser::text::parse_text_bytes_with_max_chars(raw).unwrap();
    assert_eq!(res.entries.len(), 1);
    assert_eq!(res.max_chars.timestamp, 26);
    assert_eq!(res.max_chars.module, 24);
    assert_eq!(res.max_chars.level, 1);
}

#[test]
fn test_pin_state_engine_defaults_and_transitions() {
    let mut state = ColumnLayoutState::new();

    // Acceptance criteria 1: All data columns start in the unpinned state by default
    assert!(!state.is_pinned(PinnableColumn::Timestamp));
    assert!(!state.is_pinned(PinnableColumn::Level));
    assert!(!state.is_pinned(PinnableColumn::Module));
    assert!(state.pinned_columns().is_empty());
    assert_eq!(
        state.unpinned_columns(),
        vec![
            PinnableColumn::Timestamp,
            PinnableColumn::Level,
            PinnableColumn::Module
        ]
    );

    // Pin a column
    let newly_pinned = state.pin(PinnableColumn::Timestamp);
    assert!(newly_pinned);
    assert!(state.is_pinned(PinnableColumn::Timestamp));
    assert_eq!(state.pinned_columns(), &[PinnableColumn::Timestamp]);

    // Pinning again is a no-op
    assert!(!state.pin(PinnableColumn::Timestamp));

    // Toggle pin off
    let is_pinned_now = state.toggle_pin(PinnableColumn::Timestamp);
    assert!(!is_pinned_now);
    assert!(!state.is_pinned(PinnableColumn::Timestamp));
    assert!(state.pinned_columns().is_empty());

    // Toggle pin on
    let is_pinned_now = state.toggle_pin(PinnableColumn::Level);
    assert!(is_pinned_now);
    assert!(state.is_pinned(PinnableColumn::Level));

    // Unpin
    let was_pinned = state.unpin(PinnableColumn::Level);
    assert!(was_pinned);
    assert!(!state.is_pinned(PinnableColumn::Level));
}

#[test]
fn test_pin_state_engine_schema_order_preservation() {
    let mut state = ColumnLayoutState::new();

    // Domain rule 3: When multiple columns are pinned, they always maintain their original
    // schema ordering (TIMESTAMP -> LEVEL -> MODULE) at the anchor boundary, regardless
    // of the sequence in which they were toggled.
    state.pin(PinnableColumn::Module);
    state.pin(PinnableColumn::Timestamp);

    // Toggled Module first, then Timestamp:
    // Ordered result MUST be [Timestamp, Module]
    assert_eq!(
        state.pinned_columns(),
        &[PinnableColumn::Timestamp, PinnableColumn::Module]
    );
    assert_eq!(state.unpinned_columns(), vec![PinnableColumn::Level]);

    // Pin Level as well
    state.pin(PinnableColumn::Level);
    assert_eq!(
        state.pinned_columns(),
        &[
            PinnableColumn::Timestamp,
            PinnableColumn::Level,
            PinnableColumn::Module
        ]
    );
    assert!(state.unpinned_columns().is_empty());

    // Unpin all reset
    state.unpin_all();
    assert!(state.pinned_columns().is_empty());
    assert_eq!(
        state.unpinned_columns(),
        vec![
            PinnableColumn::Timestamp,
            PinnableColumn::Level,
            PinnableColumn::Module
        ]
    );
}

#[test]
fn test_pinnability_rules() {
    // Acceptance criteria 5 & Domain rule 2: Only metadata columns are pinnable.
    // Gutter is permanently anchored; MESSAGE is permanently unpinnable.
    assert!(PinnableColumn::is_pinnable("TIMESTAMP"));
    assert!(PinnableColumn::is_pinnable("LEVEL"));
    assert!(PinnableColumn::is_pinnable("MODULE"));
    assert!(!PinnableColumn::is_pinnable("MESSAGE"));
    assert!(!PinnableColumn::is_pinnable("GUTTER"));
    assert!(!PinnableColumn::is_pinnable("#"));
}

#[test]
fn test_pin_toggle_interaction_states() {
    let mut state = ColumnLayoutState::new();

    // 1. Unpinned (Idle): Icon hidden to keep minimalist theme clean
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Timestamp),
        PinToggleState::UnpinnedIdle
    );
    assert_eq!(PinToggleState::UnpinnedIdle.icon(), None);
    assert!(!PinToggleState::UnpinnedIdle.is_visible());

    // 2. Unpinned (Hover): Header hover reveals muted pin icon (PushPinSimple, text_muted)
    state.set_header_hovered(PinnableColumn::Timestamp, true);
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Timestamp),
        PinToggleState::UnpinnedHover
    );
    assert_eq!(
        PinToggleState::UnpinnedHover.icon(),
        Some(IconName::PushPinSimple)
    );
    assert!(PinToggleState::UnpinnedHover.is_visible());

    // Other columns remain UnpinnedIdle
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Level),
        PinToggleState::UnpinnedIdle
    );

    // 3. Pinned (Active): Clicking pin marks it pinned, persistently visible with accent emphasis
    state.pin(PinnableColumn::Timestamp);
    state.set_header_hovered(PinnableColumn::Timestamp, false);
    state.set_toggle_hovered(PinnableColumn::Timestamp, false);
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Timestamp),
        PinToggleState::PinnedActive
    );
    assert_eq!(
        PinToggleState::PinnedActive.icon(),
        Some(IconName::PushPinSimple)
    );
    assert!(PinToggleState::PinnedActive.is_visible());

    // Header hovered while pinned still shows active accent pin if toggle itself is not hovered
    state.set_header_hovered(PinnableColumn::Timestamp, true);
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Timestamp),
        PinToggleState::PinnedActive
    );

    // 4. Pinned (Hover): Hovering the toggle button displays unpin release affordance (PushPinSimpleSlash)
    state.set_toggle_hovered(PinnableColumn::Timestamp, true);
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Timestamp),
        PinToggleState::PinnedHover
    );
    assert_eq!(
        PinToggleState::PinnedHover.icon(),
        Some(IconName::PushPinSimpleSlash)
    );
    assert!(PinToggleState::PinnedHover.is_visible());

    // Clicking pinned column releases pin
    state.toggle_pin(PinnableColumn::Timestamp);
    state.set_toggle_hovered(PinnableColumn::Timestamp, false);
    state.set_header_hovered(PinnableColumn::Timestamp, false);
    assert_eq!(
        state.pin_toggle_state(PinnableColumn::Timestamp),
        PinToggleState::UnpinnedIdle
    );
}
