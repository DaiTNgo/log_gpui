//! Inspector: the stored text of the selected line, exactly as in the file,
//! with the Continuation Lines that follow it.

use gpui_component::ActiveTheme as _;
use gpui::{div, px, IntoElement, ParentElement, Styled};
use gpui_component::scroll::ScrollableElement as _;

pub struct InspectorView;

impl InspectorView {
    /// Inspector body for the right sidebar's Inspector section. `lines` are
    /// the selected line and its Continuation Lines, as stored.
    pub fn render_body(lines: Option<Vec<String>>, cx: &gpui::App) -> gpui::AnyElement {
        let Some(lines) = lines else {
            return div()
                .flex_1()
                .p_4()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_xs().child("Select a line to inspect it"))
                .into_any_element();
        };
        let mut body = div().flex().flex_col().w_full();
        for line in lines {
            // An empty line still takes its row.
            body = body.child(div().min_h(px(16.0)).child(line));
        }
        div()
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scrollbar()
            .p_3()
            .font_family(cx.theme().mono_font_family.clone())
            .text_xs()
            .child(body)
            .into_any_element()
    }
}
