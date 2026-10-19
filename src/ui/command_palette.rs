
use gpui_component::ActiveTheme as _;
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};

pub struct CommandPalette;

/// A shortcut as written in the list (macOS symbols), spelled for this platform:
/// `⌘⇧F` stays as is on macOS and reads `Ctrl+Shift+F` elsewhere.
fn display_shortcut(shortcut: &'static str) -> gpui::SharedString {
    if cfg!(target_os = "macos") || !shortcut.contains(['⌘', '⇧', '⌥']) {
        return shortcut.into();
    }
    let mut out = String::new();
    for ch in shortcut.chars() {
        match ch {
            '⌘' => out.push_str("Ctrl+"),
            '⇧' => out.push_str("Shift+"),
            '⌥' => out.push_str("Alt+"),
            c => out.push(c),
        }
    }
    out.into()
}

pub struct PaletteCommand {
    pub name: &'static str,
    pub shortcut: &'static str,
    pub action: PaletteAction,
}

#[derive(Clone, Copy)]
pub enum PaletteAction {
    ToggleSearch,
    ToggleFilter,
    ShowContext,
    GoNextError,
    GoPrevError,
    SetComfortable,
    SetCompact,
    SetUltraCompact,
    ManageServers,
    ToggleFollow,
    TogglePause,
    ToggleInspector,
    LoadDemo,
    NewLogSpace,
    ToggleRawParsed,
    SaveScratch,
    UnpinAllColumns,
    ToggleProjectSwitcher,
    ShowHighlights,
    ToggleFocusPane,
    MoveFocusToWindow,
    ClearFile,
    LoadMore,
    BackToLiveWindow,
    OpenInFull,
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
                name: "Focus Pane: Show or Hide",
                shortcut: "⌘J",
                action: PaletteAction::ToggleFocusPane,
            },
            PaletteCommand {
                name: "Focus Pane: Move to Window",
                shortcut: "",
                action: PaletteAction::MoveFocusToWindow,
            },
            PaletteCommand {
                name: "Highlights: Open Highlight Rules",
                shortcut: "⌘⇧M",
                action: PaletteAction::ShowHighlights,
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
                name: "Servers: Manage…",
                shortcut: "",
                action: PaletteAction::ManageServers,
            },
            PaletteCommand {
                name: "Live: Toggle Follow",
                shortcut: "T",
                action: PaletteAction::ToggleFollow,
            },
            PaletteCommand {
                name: "Live: Toggle Pause",
                shortcut: "",
                action: PaletteAction::TogglePause,
            },
            PaletteCommand {
                name: "Live: Clear File",
                shortcut: "⌘⇧K",
                action: PaletteAction::ClearFile,
            },
            PaletteCommand {
                name: "Live: Load More Older Lines",
                shortcut: "⌘⇧↑",
                action: PaletteAction::LoadMore,
            },
            PaletteCommand {
                name: "Live: Open in Full…",
                shortcut: "",
                action: PaletteAction::OpenInFull,
            },
            PaletteCommand {
                name: "Live: Back to Live Window",
                shortcut: "",
                action: PaletteAction::BackToLiveWindow,
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
                        .child(display_shortcut(cmd.shortcut)),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.execute_palette_action(act, window, cx);
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
