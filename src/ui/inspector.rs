//! Inspector: the stored text of the selected line, exactly as in the file,
//! with the Continuation Lines that follow it. The text sits in a read-only
//! editor, so it can be selected and copied.

use gpui::{div, px, Entity, IntoElement, ParentElement, Styled};
use gpui_component::input::{Editor, EditorState};
use gpui_component::ActiveTheme as _;

pub struct InspectorView;

impl InspectorView {
    /// Inspector body for the right sidebar's Inspector section. `editor`
    /// holds the selected line and its Continuation Lines, as stored.
    pub fn render_body(has_line: bool, editor: &Entity<EditorState>, cx: &gpui::App) -> gpui::AnyElement {
        if !has_line {
            return div()
                .flex_1()
                .p_4()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_xs().child("Select a line to inspect it"))
                .into_any_element();
        }
        div()
            .flex_1()
            .min_h(px(0.0))
            .p_1()
            .font_family(cx.theme().mono_font_family.clone())
            .text_xs()
            .child(
                Editor::new(editor)
                    .appearance(false)
                    .bordered(false)
                    .readonly(true)
                    .h_full(),
            )
            .into_any_element()
    }
}
