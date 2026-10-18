//! UI: the floating bar of colors that follows a selection, with the
//! Highlight Palette and the color picker one click away.

use super::AppView;
use super::highlights_panel::swatch;
use crate::app::color::{ColorBar, ColorTarget};
use crate::palette::{HighlightColor, Hue, Shade, Tone};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::color_picker::ColorPicker;
use gpui_component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;

impl AppView {
    fn bar_swatch(&self, id: String, color: &HighlightColor, tone: Tone, size: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let pick = color.clone();
        swatch(ElementId::from(id), color, tone, size)
            .on_click(cx.listener(move |this, _, _window, cx| this.apply_bar_color(pick.clone(), cx)))
            .into_any_element()
    }

    /// The 30 palette cells, ten hues across and three shades down, and the
    /// Recent row above them.
    fn render_palette_grid(&self, bar: &ColorBar, cx: &mut Context<Self>) -> gpui::AnyElement {
        let tone = self.tone(cx);
        let _ = bar;
        let recent: Vec<HighlightColor> = self
            .active_project()
            .map(|p| p.recent_colors.colors().to_vec())
            .unwrap_or_default();
        let mut grid = v_flex().gap_1();
        if !recent.is_empty() {
            let mut row = h_flex().gap_1().items_center();
            row = row.child(div().w(px(46.0)).text_xs().text_color(cx.theme().muted_foreground).child("Recent"));
            for (i, c) in recent.iter().enumerate() {
                row = row.child(self.bar_swatch(format!("recent-{i}"), c, tone, 18.0, cx));
            }
            grid = grid.child(row);
        }
        for (si, shade) in Shade::ALL.into_iter().enumerate() {
            let mut row = h_flex().gap_1().items_center();
            for (hi, hue) in Hue::ALL.into_iter().enumerate() {
                let color = HighlightColor::palette(hue, shade);
                row = row.child(self.bar_swatch(format!("cell-{si}-{hi}"), &color, tone, 18.0, cx));
            }
            grid = grid.child(row);
        }
        grid.into_any_element()
    }

    /// The bar, or `None` when it is closed.
    pub(super) fn render_color_bar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let bar = self.color_bar.clone()?;
        let tone = self.tone(cx);
        let is_rule = matches!(bar.target, ColorTarget::Rule(_));
        let recolor_removal = match &bar.target {
            ColorTarget::RecolorMarker { row, byte } => Some((Some((*row, *byte)), None)),
            ColorTarget::RawRecolor { file_id, line, line_text, byte } => {
                Some((None, Some((file_id.clone(), *line, line_text.clone(), *byte))))
            }
            _ => None,
        };

        let mut swatches = h_flex().gap_1().items_center();
        if !is_rule {
            for (i, c) in self.bar_colors().iter().enumerate() {
                swatches = swatches.child(self.bar_swatch(format!("bar-{i}"), c, tone, 20.0, cx));
            }
            swatches = swatches
                .child(
                    Button::new("bar-palette")
                        .ghost()
                        .xsmall()
                        .label("Palette")
                        .selected(bar.palette_open)
                        .on_click(cx.listener(|this, _, _window, cx| this.toggle_palette_open(cx))),
                )
                .child(ColorPicker::new(&self.color_picker).small());
            if let Some((parsed, raw)) = recolor_removal {
                swatches = swatches.child(
                    Button::new("bar-remove")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .label("Remove")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            match (&parsed, &raw) {
                                (Some((row, byte)), _) => this.remove_marker_under(*row, *byte, cx),
                                (_, Some((file, line, text, byte))) => this.remove_raw_marker(file, *line, text, *byte, cx),
                                _ => {}
                            }
                            this.color_bar = None;
                            cx.notify();
                        })),
                );
            }
        }

        let theme = cx.theme();
        let mut panel = v_flex()
            .id("color-bar")
            .absolute()
            .left(bar.pos.x)
            .top(bar.pos.y + px(14.0))
            .gap_2()
            .p_2()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(swatches);
        if is_rule || bar.palette_open {
            panel = panel.child(self.render_palette_grid(&bar, cx));
        }
        if is_rule {
            panel = panel
                .child(ColorPicker::new(&self.color_picker).small())
                .child(
                    Button::new("bar-close")
                        .ghost()
                        .xsmall()
                        .label("Close")
                        .on_click(cx.listener(|this, _, _window, cx| this.close_color_bar(cx))),
                );
        }
        Some(
            gpui::deferred(panel.when(false, |p| p)).with_priority(10).into_any_element(),
        )
    }
}
