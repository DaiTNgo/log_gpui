//! Colors of the main view (tabs, search toolbar, log table, Raw View), taken
//! from the theme. Everything that paints the main view's background goes
//! through here, so changing which theme token it follows is a one-line edit.

use gpui::{App, Hsla};
use gpui_component::ActiveTheme as _;

/// Background of the main view: the tone of the title bar and the status bar,
/// so only the sidebars stay darker.
pub fn background(cx: &App) -> Hsla {
    cx.theme().status_bar
}
