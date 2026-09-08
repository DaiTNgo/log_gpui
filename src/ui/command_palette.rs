use crate::theme::ZED;
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};

pub struct CommandPalette;

pub struct PaletteCommand {
    pub name: &'static str,
    pub shortcut: &'static str,
    pub action: PaletteAction,
}

#[derive(Clone, Copy)]
pub enum PaletteAction {
    ToggleSearch,
    ToggleFilter,
    ToggleBookmark,
    ShowContext,
    GoNextError,
    GoPrevError,
    ToggleWrap,
    SetComfortable,
    SetCompact,
    SetUltraCompact,
    ToggleTail,
    ToggleInspector,
    LoadDemo,
    OpenFile,
}

impl CommandPalette {
    pub fn list() -> Vec<PaletteCommand> {
        vec![
            PaletteCommand {
                name: "Search: Find in Log",
                shortcut: "⌘F",
                action: PaletteAction::ToggleSearch,
            },
            PaletteCommand {
                name: "Filter: Open Filter Builder",
                shortcut: "⌘⇧F",
                action: PaletteAction::ToggleFilter,
            },
            PaletteCommand {
                name: "Bookmark: Toggle Bookmark on Selected Line",
                shortcut: "B",
                action: PaletteAction::ToggleBookmark,
            },
            PaletteCommand {
                name: "Investigation: Show Surrounding Context",
                shortcut: "Space",
                action: PaletteAction::ShowContext,
            },
            PaletteCommand {
                name: "Navigate: Jump to Next Error",
                shortcut: "⌘E",
                action: PaletteAction::GoNextError,
            },
            PaletteCommand {
                name: "Navigate: Jump to Previous Error",
                shortcut: "⌘⇧E",
                action: PaletteAction::GoPrevError,
            },
            PaletteCommand {
                name: "View: Toggle Word Wrap",
                shortcut: "⌥Z",
                action: PaletteAction::ToggleWrap,
            },
            PaletteCommand {
                name: "Density: Compact (Default)",
                shortcut: "",
                action: PaletteAction::SetCompact,
            },
            PaletteCommand {
                name: "Density: Comfortable",
                shortcut: "",
                action: PaletteAction::SetComfortable,
            },
            PaletteCommand {
                name: "Density: Ultra Compact",
                shortcut: "",
                action: PaletteAction::SetUltraCompact,
            },
            PaletteCommand {
                name: "Stream: Toggle Tail Mode",
                shortcut: "T",
                action: PaletteAction::ToggleTail,
            },
            PaletteCommand {
                name: "Inspector: Toggle Slide-out Inspector",
                shortcut: "⌘I",
                action: PaletteAction::ToggleInspector,
            },
            PaletteCommand {
                name: "File: Open Log File...",
                shortcut: "⌘O",
                action: PaletteAction::OpenFile,
            },
            PaletteCommand {
                name: "Data: Reload Sample Logs",
                shortcut: "",
                action: PaletteAction::LoadDemo,
            },
        ]
    }

    pub fn render(
        is_open: bool,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !is_open {
            return div().into_any_element();
        }

        let commands = Self::list();
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
                .hover(|s| s.bg(ZED.ghost_hover).text_color(ZED.text))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(ZED.text)
                        .child(cmd.name),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(".AppleSystemUIFontMonospaced")
                        .text_color(ZED.text_muted)
                        .child(cmd.shortcut),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.execute_palette_action(act, cx);
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
                    .bg(ZED.elevated_surface)
                    .border_1()
                    .border_color(ZED.border)
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
                            .border_color(ZED.border_variant)
                            .pb_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(ZED.text_muted)
                                    .child("COMMAND PALETTE"),
                            )
                            .child(
                                div()
                                    .id("close_pal")
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(ZED.text_muted)
                                    .hover(|s| s.text_color(ZED.text))
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
