
use crate::keymap::Command;
use gpui_component::ActiveTheme as _;
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};

pub struct CommandPalette;

pub struct PaletteCommand {
    pub name: &'static str,
    pub action: Command,
}

impl CommandPalette {
    /// Every Command but the palette itself, in the Keymap's order.
    pub fn list() -> Vec<PaletteCommand> {
        Command::ALL
            .into_iter()
            .filter(|&c| c != Command::CommandPalette)
            .map(|c| PaletteCommand { name: c.name(), action: c })
            .collect()
    }

    /// `hint` gives the shortcut shown beside a Command, from the Keymap.
    pub fn render(
        is_open: bool,
        hint: impl Fn(Command) -> String,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !is_open {
            return div().into_any_element();
        }

        let commands = Self::list();
        let mono_font = cx.theme().mono_font_family.clone();
        let mut list = div().flex().flex_col().gap_0p5().w_full();

        for (ix, cmd) in commands.into_iter().enumerate() {
            let act = cmd.action;
            let item = div()
                .id(ElementId::from(format!("pal_cmd_{}", ix)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_md()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(cmd.name),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(mono_font.clone())
                        .child(hint(act)),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.execute_command(act, window, cx);
                }));

            list = list.child(item);
        }

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(100.0))
            .child(
                div()
                    .w(px(520.0))
                    .border_1()
                    .rounded_lg()
                    .shadow_2xl()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .pb_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("COMMAND PALETTE"),
                            )
                            .child(
                                div()
                                    .id("close_pal")
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("Esc to Close")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.is_palette_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(list),
            )
            .into_any_element()
    }
}
