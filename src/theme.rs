//! The color themes the app offers. Each one is a gpui-component mode plus an
//! optional lift of the dark palette; the choice is an app setting.

use gpui::{App, Hsla};
use gpui_component::{theme::ThemeMode, Theme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppTheme {
    #[default]
    Dark,
    /// The dark theme with lighter surfaces, borders and text.
    DarkBright,
    Light,
}

impl AppTheme {
    pub const ALL: [AppTheme; 3] = [AppTheme::Dark, AppTheme::DarkBright, AppTheme::Light];

    pub fn label(self) -> &'static str {
        match self {
            AppTheme::Dark => "Dark",
            AppTheme::DarkBright => "Dark Bright",
            AppTheme::Light => "Light",
        }
    }

    /// Switch the whole app to this theme.
    pub fn apply(self, cx: &mut App) {
        let mode = if self == AppTheme::Light { ThemeMode::Light } else { ThemeMode::Dark };
        Theme::change(mode, None, cx);
        if self == AppTheme::DarkBright {
            brighten(cx);
        }
        cx.refresh_windows();
    }
}

/// Lift the dark palette a notch: lighter surfaces and borders, brighter text.
fn brighten(cx: &mut App) {
    let colors = &mut Theme::global_mut(cx).colors;
    let lift = |c: &mut Hsla, by: f32| c.l = (c.l + by).min(0.95);
    for c in [
        &mut colors.background,
        &mut colors.status_bar,
        &mut colors.title_bar,
        &mut colors.sidebar,
        &mut colors.popover,
        &mut colors.muted,
        &mut colors.secondary,
        &mut colors.table,
        &mut colors.table_head,
        &mut colors.border,
        &mut colors.title_bar_border,
        &mut colors.status_bar_border,
        &mut colors.input,
    ] {
        lift(c, 0.07);
    }
    lift(&mut colors.muted_foreground, 0.12);
    lift(&mut colors.foreground, 0.05);
}
