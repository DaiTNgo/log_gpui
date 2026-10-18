
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
    SetComfortable,
    SetCompact,
    SetUltraCompact,
    ToggleTail,
    ToggleInspector,
    LoadDemo,
    NewLogSpace,
    ToggleRawParsed,
    SaveScratch,
    UnpinAllColumns,
    ToggleProjectSwitcher,
    ToggleMarkerPanel,
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
                name: "Marker: Open Marker Panel",
                shortcut: "⌘⇧M",
                action: PaletteAction::ToggleMarkerPanel,
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
                name: "Log Space: New Log Space...",
                shortcut: "⌘O",
                action: PaletteAction::NewLogSpace,
            },
            PaletteCommand {
                name: "View: Toggle Raw / Parsed",
                shortcut: "⌘⇧R",
                action: PaletteAction::ToggleRawParsed,
            },
            PaletteCommand {
                name: "Log Space: Save Scratch Files",
                shortcut: "⌘S",
                action: PaletteAction::SaveScratch,
            },
            PaletteCommand {
                name: "Project: Switch Project",
                shortcut: "⌘P",
                action: PaletteAction::ToggleProjectSwitcher,
            },
            PaletteCommand {
                name: "Data: Load Demo Log Space",
                shortcut: "",
                action: PaletteAction::LoadDemo,
            },
            PaletteCommand {
                name: "Columns: Unpin All",
                shortcut: "",
                action: PaletteAction::UnpinAllColumns,
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
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(cmd.name),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(".AppleSystemUIFontMonospaced")
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
