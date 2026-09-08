use gpui::{rgb, Rgba};

/// Carbon Design System Color Tokens (Gray 100 Dark Theme)
pub struct CarbonTheme {
    // Surfaces
    pub bg: Rgba,
    pub layer_01: Rgba,
    pub layer_02: Rgba,
    pub layer_03: Rgba,
    pub layer_hover: Rgba,
    pub layer_selected: Rgba,
    pub layer_active: Rgba,

    // Borders
    pub border_subtle: Rgba,
    pub border_strong: Rgba,
    pub border_interactive: Rgba,

    // Text
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,
    pub text_on_color: Rgba,

    // Accents
    pub blue_primary: Rgba,
    pub blue_hover: Rgba,
    pub cyan_accent: Rgba,
    pub purple_accent: Rgba,

    // Semantic Status
    pub error: Rgba,
    pub error_bg: Rgba,
    pub warning: Rgba,
    pub warning_bg: Rgba,
    pub info: Rgba,
    pub info_bg: Rgba,
    pub success: Rgba,
    pub success_bg: Rgba,

    // Search highlights
    pub search_match_bg: Rgba,
    pub search_current_bg: Rgba,
    pub search_current_border: Rgba,
    pub transparent: Rgba,
}

impl Default for CarbonTheme {
    fn default() -> Self {
        Self {
            // Dark Carbon Gray 100
            bg: rgb(0x161616),
            layer_01: rgb(0x262626),
            layer_02: rgb(0x393939),
            layer_03: rgb(0x525252),
            layer_hover: rgb(0x2f2f2f),
            layer_selected: rgb(0x353535),
            layer_active: rgb(0x404040),

            border_subtle: rgb(0x2a2a2a),
            border_strong: rgb(0x474747),
            border_interactive: rgb(0x0f62fe),

            text_primary: rgb(0xf4f4f4),
            text_secondary: rgb(0xc6c6c6),
            text_muted: rgb(0x8d8d8d),
            text_disabled: rgb(0x525252),
            text_on_color: rgb(0xffffff),

            blue_primary: rgb(0x0f62fe),
            blue_hover: rgb(0x0353e9),
            cyan_accent: rgb(0x1192e8),
            purple_accent: rgb(0xa56eff),

            error: rgb(0xfa4d56),
            error_bg: rgb(0x3b1215),
            warning: rgb(0xf1c21b),
            warning_bg: rgb(0x372905),
            info: rgb(0x4589ff),
            info_bg: rgb(0x001d6c),
            success: rgb(0x42be65),
            success_bg: rgb(0x072b14),

            search_match_bg: rgb(0x3d3522),
            search_current_bg: rgb(0x0f62fe),
            search_current_border: rgb(0x78a9ff),
            transparent: gpui::rgba(0x00000000),
        }
    }
}

pub static CARBON: std::sync::LazyLock<CarbonTheme> =
    std::sync::LazyLock::new(CarbonTheme::default);

pub use super::zed::Density;

