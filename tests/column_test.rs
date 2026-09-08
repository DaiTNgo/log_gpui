use gpui::px;
use log_gpui::column::{ColumnLayoutState, PinnableColumn};

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
