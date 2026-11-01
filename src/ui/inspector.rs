//! Inspector: the selected line, as its columns (Fields) or as the stored
//! text exactly as in the file with the Continuation Lines that follow it
//! (Raw). The Raw text sits in a read-only editor, so it can be selected and
//! copied. Pointing at a field offers Show only, Hide and Highlight rows.

use crate::app::AppView;
use crate::app::context_layout::InspectorField;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement, SharedString, StatefulInteractiveElement as _,
    Styled, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Editor, EditorState};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::assets::IconName;

pub struct InspectorView;

impl InspectorView {
    fn empty() -> gpui::AnyElement {
        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(div().text_xs().child("Select a line to inspect it"))
            .into_any_element()
    }

    /// The stored text of the line and its Continuation Lines.
    pub fn render_raw(has_line: bool, editor: &Entity<EditorState>, cx: &gpui::App) -> gpui::AnyElement {
        if !has_line {
            return Self::empty();
        }
        div()
            .size_full()
            .min_h(px(0.0))
            .p_1()
            .font_family(cx.theme().mono_font_family.clone())
            .text_xs()
            .child(Editor::new(editor).appearance(false).bordered(false).readonly(true).h_full())
            .into_any_element()
    }

    /// The columns of the line, one per row; `actions` adds the buttons
    /// shown when pointing at a field (not on TIMESTAMP).
    pub fn render_fields(fields: &[InspectorField], actions: bool, cx: &mut Context<AppView>) -> gpui::AnyElement {
        let muted = cx.theme().muted_foreground;
        let hover_bg = cx.theme().accent;
        let mut list = v_flex().w_full().px_2().pb_2();
        for (i, field) in fields.iter().enumerate() {
            let group = SharedString::from(format!("inspector-field-{i}"));
            let filterable = actions && field.key != "timestamp";
            let row = h_flex()
                .id(group.clone())
                .group(group.clone())
                .w_full()
                .min_h(px(22.0))
                .px_1()
                .gap_2()
                .items_start()
                .rounded(px(4.0))
                .hover(|s| s.bg(hover_bg))
                .child(div().w(px(84.0)).flex_shrink_0().pt(px(3.0)).text_xs().text_color(muted).truncate().child(field.label.clone()))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .pt(px(3.0))
                        .text_xs()
                        .font_family(cx.theme().mono_font_family.clone())
                        .child(field.value.clone()),
                )
                .when(filterable, |row| {
                    let mut buttons = h_flex().flex_shrink_0().invisible().group_hover(group.clone(), |s| s.visible());
                    for (n, (icon, tip)) in
                        [(IconName::Funnel, "Show only"), (IconName::EyeOff, "Hide"), (IconName::Highlighter, "Highlight rows")]
                            .into_iter()
                            .enumerate()
                    {
                        let (key, value) = (field.key.clone(), field.value.clone());
                        buttons = buttons.child(
                            Button::new(SharedString::from(format!("inspector-field-{i}-{n}")))
                                .ghost()
                                .xsmall()
                                .icon(icon)
                                .tooltip(tip)
                                .on_click(cx.listener(move |this, _, _, cx| match n {
                                    0 => this.filter_from_selection(&key, &value, true, false, cx),
                                    1 => this.filter_from_selection(&key, &value, true, true, cx),
                                    _ => this.open_rule_form_with(&key, &value, cx),
                                })),
                        );
                    }
                    row.child(buttons)
                });
            list = list.child(row);
        }
        div()
            .id("inspector-fields")
            .size_full()
            .overflow_y_scroll()
            .child(list)
            .into_any_element()
    }
}
