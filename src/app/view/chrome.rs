//! UI: window title bar built from gpui-kit components.

use super::AppView;
use gpui::prelude::FluentBuilder as _;
use gpui::{div, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Selectable as _, Sizable as _, TitleBar};
use gpui_kit::assets::IconName;

impl AppView {
    /// Title bar: only the two sidebar toggles. The left toggle sits next to the macOS
    /// traffic lights (TitleBar reserves the inset and handles drag / double-click zoom);
    /// the right toggle shows only while a log file tab is focused.
    pub(super) fn render_window_chrome(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let show_context_toggle = self.active_tab_is_log_file();
        let context_visible = self.context_sidebar.is_shown(show_context_toggle);

        // TitleBar reserves 80px on macOS for the traffic lights. They disappear in full screen,
        // so drop that inset (the bar's own pl_3 then keeps the toggle 12px from the edge).
        let mut bar = TitleBar::new();
        if window.is_fullscreen() {
            bar = bar.pl_0();
        }

        bar.child(
                Button::new("toggle-left-sidebar")
                    .ghost()
                    .small()
                    .icon(IconName::PanelLeft)
                    .selected(self.sidebar_state.visible)
                    .on_click(cx.listener(|this, _, _window, cx| this.toggle_sidebar(cx))),
            )
            .child(div().flex().items_center().pr_2().when(show_context_toggle, |this| {
                this.child(
                    Button::new("toggle-context-sidebar")
                        .ghost()
                        .small()
                        .icon(IconName::PanelRight)
                        .selected(context_visible)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.toggle_context_sidebar(cx)
                        })),
                )
            }))
            .into_any_element()
    }
}
