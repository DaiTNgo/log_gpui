use gpui::{rgb, rgba, Rgba};
use serde::{Deserialize, Serialize};

/// Zed Design System Theme Tokens
/// Based on Zed's official schema (v0.2.0) and minimalist dark UI principles.
#[derive(Debug, Clone)]
pub struct ZedTheme {
    // Surface Elevations
    pub background: Rgba,              // Base window canvas (#18181b)
    pub surface_background: Rgba,      // Grounded surfaces: titlebar, sidebar, rail, status bar (#1f1f23)
    pub editor_background: Rgba,       // Main log canvas background (#18181b)
    pub elevated_surface: Rgba,        // Floating modals, command palette, popovers (#232328)

    // Interactive Element States
    pub element_background: Rgba,      // Solid elements, badges, inputs (#27272a)
    pub element_hover: Rgba,           // Soft hover state for rows and buttons (#2e2e35)
    pub element_selected: Rgba,        // Active line selection, selected tab (#34343e)
    pub element_active: Rgba,          // Pressed state (#3d3d48)
    pub element_disabled: Rgba,        // Disabled element bg (#202024)

    // Ghost Elements (borderless buttons/icons that light up on hover)
    pub ghost_hover: Rgba,             // Subtle hover pill (#2b2b32)
    pub ghost_active: Rgba,            // Pressed ghost pill (#35353e)
    pub ghost_selected: Rgba,          // Selected ghost button / rail icon (#303038)

    // Borders & Structural Dividers
    pub border_variant: Rgba,          // Deemphasized 1px divider between panels (#27272e)
    pub border: Rgba,                  // Standard structural border for cards/popovers (#383842)
    pub border_focused: Rgba,          // Keyboard / input focus ring (#4f7be8)
    pub border_selected: Rgba,         // Active item border (#3e4756)
    pub border_transparent: Rgba,      // Zero-shift placeholder border

    // Typography
    pub text: Rgba,                    // Primary text (#f4f4f5)
    pub text_accent: Rgba,             // Highlight / match / link text (#60a5fa)
    pub text_muted: Rgba,              // Secondary metadata, gutter line numbers (#8a8a93)
    pub text_placeholder: Rgba,        // Inactive input placeholder (#52525b)
    pub text_disabled: Rgba,           // Disabled text (#3f3f46)

    // Icons
    pub icon: Rgba,                    // Default icon fill (#a1a1aa)
    pub icon_hover: Rgba,              // Hovered icon fill (#f4f4f5)
    pub icon_accent: Rgba,             // Selected icon fill (#60a5fa)
    pub icon_muted: Rgba,              // Subdued icon fill (#71717a)

    // Restrained Semantic Status (Soft backgrounds with crisp foreground text)
    pub error: Rgba,                   // Text / badge border (#f87171)
    pub error_bg: Rgba,                // Tinted error badge bg (rgba(248, 113, 113, 0.12))
    pub error_border: Rgba,            // (#5c1d24)

    pub warning: Rgba,                 // Text / badge border (#fbbf24)
    pub warning_bg: Rgba,              // Tinted warning badge bg (rgba(251, 191, 36, 0.12))
    pub warning_border: Rgba,          // (#543e0e)

    pub info: Rgba,                    // Text / badge border (#60a5fa)
    pub info_bg: Rgba,                 // Tinted info badge bg (rgba(96, 165, 250, 0.12))
    pub info_border: Rgba,             // (#1e3a5f)

    pub success: Rgba,                 // Text / badge border (#4ade80)
    pub success_bg: Rgba,              // Tinted success badge bg (rgba(74, 222, 128, 0.12))
    pub success_border: Rgba,          // (#184728)

    pub debug: Rgba,                   // Text / badge border (#94a3b8)
    pub debug_bg: Rgba,                // Tinted debug badge bg (rgba(148, 163, 184, 0.10))

    // Search & Highlights
    pub search_match_bg: Rgba,         // Muted yellow highlight for matches (#423922)
    pub search_current_bg: Rgba,       // Active match focus (#254e80)
    pub search_current_border: Rgba,   // Active match border (#60a5fa)

    // Editor Gutter & Canvas Specifics
    pub gutter_line_number: Rgba,      // Inactive line number (#52525b)
    pub gutter_active_line_number: Rgba,// Current active line number (#f4f4f5)
    pub active_line_bg: Rgba,          // Current row highlight bar (#212127)
    pub transparent: Rgba,
}

impl Default for ZedTheme {
    fn default() -> Self {
        Self {
            // Dark Neutral Charcoal Canvas & Grounded Surfaces
            background: rgb(0x18181b),
            surface_background: rgb(0x1f1f23),
            editor_background: rgb(0x18181b),
            elevated_surface: rgb(0x232328),

            // Element States
            element_background: rgb(0x27272a),
            element_hover: rgb(0x2e2e35),
            element_selected: rgb(0x34343e),
            element_active: rgb(0x3d3d48),
            element_disabled: rgb(0x202024),

            // Ghost States
            ghost_hover: rgb(0x2b2b32),
            ghost_active: rgb(0x35353e),
            ghost_selected: rgb(0x303038),

            // Borders
            border_variant: rgb(0x27272e),
            border: rgb(0x383842),
            border_focused: rgb(0x4f7be8),
            border_selected: rgb(0x3e4756),
            border_transparent: rgba(0x00000000),

            // Typography
            text: rgb(0xf4f4f5),
            text_accent: rgb(0x60a5fa),
            text_muted: rgb(0x8a8a93),
            text_placeholder: rgb(0x52525b),
            text_disabled: rgb(0x3f3f46),

            // Icons
            icon: rgb(0xa1a1aa),
            icon_hover: rgb(0xf4f4f5),
            icon_accent: rgb(0x60a5fa),
            icon_muted: rgb(0x71717a),

            // Semantic Status - Restrained Soft Tints
            error: rgb(0xf87171),
            error_bg: rgba(0x3b1418cc),
            error_border: rgb(0x5c1d24),

            warning: rgb(0xfbbf24),
            warning_bg: rgba(0x362a0acc),
            warning_border: rgb(0x543e0e),

            info: rgb(0x60a5fa),
            info_bg: rgba(0x0e2a4acc),
            info_border: rgb(0x1e3a5f),

            success: rgb(0x4ade80),
            success_bg: rgba(0x0d2818cc),
            success_border: rgb(0x184728),

            debug: rgb(0x94a3b8),
            debug_bg: rgba(0x1e2430cc),

            // Search
            search_match_bg: rgb(0x423922),
            search_current_bg: rgb(0x254e80),
            search_current_border: rgb(0x60a5fa),

            // Gutter & Canvas
            gutter_line_number: rgb(0x52525b),
            gutter_active_line_number: rgb(0xf4f4f5),
            active_line_bg: rgb(0x212127),
            transparent: rgba(0x00000000),
        }
    }
}

pub static ZED: std::sync::LazyLock<ZedTheme> = std::sync::LazyLock::new(ZedTheme::default);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Density {
    Comfortable,
    #[default]
    Compact,
    UltraCompact,
}

impl Density {
    pub fn row_height(&self) -> f32 {
        match self {
            Density::Comfortable => 30.0,
            Density::Compact => 24.0,
            Density::UltraCompact => 20.0,
        }
    }

    pub fn font_size(&self) -> f32 {
        match self {
            Density::Comfortable => 13.0,
            Density::Compact => 12.0,
            Density::UltraCompact => 11.0,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Density::Comfortable => "Comfortable",
            Density::Compact => "Compact",
            Density::UltraCompact => "Ultra Compact",
        }
    }
}
