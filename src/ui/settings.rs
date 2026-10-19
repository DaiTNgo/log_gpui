//! Settings dialog (gpui-kit `Dialog`): a page per section on the right, a
//! list of sections and a search field on the left. Each setting is a row with
//! its name and what it does on the left and its control on the right.
//! Opened from the title bar.

use crate::app::AppView;
use crate::app::MatchKind;
use crate::density::Density;
use crate::palette::Hue;
use gpui::{
    div, px, AnyElement, App, Entity, InteractiveElement, IntoElement, Keystroke, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::kbd::Kbd;
use gpui_component::switch::Switch;
use gpui_component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _,
    WindowExt as _,
};
use gpui_kit::assets::IconName;

/// Space kept between the Settings dialog and the window edge, in pixels.
const DIALOG_MARGIN: f32 = 48.0;
/// Width of the list of sections.
const NAV_WIDTH: f32 = 240.0;

const SHORTCUTS: [(&str, &str); 11] = [
    ("cmd-o", "New Log Space"),
    ("cmd-s", "Save scratch files"),
    ("cmd-b", "Projects sidebar"),
    ("cmd-alt-b", "Context sidebar"),
    ("cmd-shift-b", "Context: Files"),
    ("cmd-shift-i", "Context: Investigations"),
    ("cmd-shift-s", "Context: Saved Views"),
    ("cmd-p", "Project switcher"),
    ("cmd-k", "Command palette"),
    ("cmd-f", "Search logs"),
    ("cmd-1", "Switch tab"),
];

/// A page of the Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    General,
    Appearance,
    RawView,
    Keymap,
}

impl SettingsSection {
    const ALL: [SettingsSection; 4] = [
        SettingsSection::General,
        SettingsSection::Appearance,
        SettingsSection::RawView,
        SettingsSection::Keymap,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsSection::General => "General",
            SettingsSection::Appearance => "Appearance",
            SettingsSection::RawView => "Raw View",
            SettingsSection::Keymap => "Keymap",
        }
    }
}

/// One setting: its name, what it does and the control that changes it.
struct Row {
    title: &'static str,
    description: &'static str,
    control: AnyElement,
}

/// What the rows read from the app, taken once per frame.
struct Values {
    live_window: usize,
    shown: bool,
    /// The Stream Column is on, and whether the open Log Space has a command.
    stream: bool,
    has_command: bool,
    minimap: bool,
    in_space: bool,
    wrap: bool,
    density: Density,
    colors: crate::palette::MatchColors,
}

pub fn open_settings_dialog(view: Entity<AppView>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, window, cx| {
        let (values, section, query) = {
            let v = view.read(cx);
            (
                Values {
                    live_window: v.live.window,
                    shown: v.source_column_shown(),
                    stream: v.stream_column_setting(),
                    has_command: v.active_space_has_command(),
                    minimap: v.show_minimap,
                    in_space: v.active_is_log_space(),
                    wrap: v.raw.soft_wrap,
                    density: v.density,
                    colors: v.workspace_state.match_colors,
                },
                v.settings_section,
                v.settings_search.read(cx).value().trim().to_lowercase(),
            )
        };
        let size = window.viewport_size();
        let width = (size.width - px(2.0 * DIALOG_MARGIN)).max(px(560.0));
        let height = (size.height - px(2.0 * DIALOG_MARGIN + 48.0)).max(px(320.0));
        let (border, muted, active_bg) = {
            let theme = cx.theme();
            (theme.border, theme.muted_foreground, theme.list_active)
        };

        // The search looks through every section; without it, one page shows.
        let searching = !query.is_empty();
        let (heading, rows): (String, Vec<Row>) = if searching {
            let rows = SettingsSection::ALL
                .into_iter()
                .flat_map(|s| rows_of(s, &view, &values, cx))
                .filter(|r| {
                    r.title.to_lowercase().contains(&query)
                        || r.description.to_lowercase().contains(&query)
                })
                .collect();
            (format!("Results for “{query}”"), rows)
        } else {
            (section.label().to_string(), rows_of(section, &view, &values, cx))
        };

        let mut nav = v_flex().gap_1().mt_3();
        for s in SettingsSection::ALL {
            let v = view.clone();
            nav = nav.child(
                h_flex()
                    .id(SharedString::from(format!("settings-nav-{}", s.label())))
                    .h(px(34.0))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .when_selected(!searching && s == section, active_bg)
                    .child(Icon::new(IconName::ChevronRight).size_3().text_color(muted))
                    .child(s.label())
                    .on_click(move |_, window, app| {
                        v.update(app, |this, cx| {
                            this.settings_section = s;
                            cx.notify();
                        });
                        window.refresh();
                    }),
            );
        }

        let mut page = v_flex().w_full();
        if rows.is_empty() {
            page = page.child(div().py_4().text_color(muted).child("No settings match."));
        }
        for row in rows {
            page = page.child(
                h_flex()
                    .w_full()
                    .py_4()
                    .gap_8()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.0))
                            .gap_1()
                            .child(div().text_base().child(row.title))
                            .child(div().text_sm().text_color(muted).child(row.description)),
                    )
                    .child(div().flex_shrink_0().child(row.control)),
            );
        }

        let body = h_flex()
            .w_full()
            .h(height)
            .gap_6()
            .child(
                v_flex()
                    .w(px(NAV_WIDTH))
                    .flex_shrink_0()
                    .h_full()
                    .child(
                        h_flex()
                            .h(px(34.0))
                            .px_2()
                            .gap_2()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(border)
                            .child(Icon::new(IconName::Search).size_4().text_color(muted))
                            .child(
                                Input::new(&view.read(cx).settings_search)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false),
                            ),
                    )
                    .child(nav),
            )
            .child(
                v_flex()
                    .id("settings-page")
                    .flex_1()
                    .min_w(px(0.0))
                    .h_full()
                    .overflow_y_scroll()
                    .child(div().pb_2().text_xl().child(heading))
                    .child(page),
            );
        dialog.title("Settings").w(width).child(body)
    });
}

/// The rows of one section.
fn rows_of(section: SettingsSection, view: &Entity<AppView>, v: &Values, cx: &App) -> Vec<Row> {
    match section {
        SettingsSection::General => {
            let view = view.clone();
            let windows = [10_000usize, 50_000, 100_000, 500_000, 1_000_000];
            let current = v.live_window;
            vec![Row {
                title: "Live Window",
                description: "How many of the newest lines each Remote File keeps. Older lines leave the view as new ones arrive.",
                control: ButtonGroup::new("settings-live-window")
                    .small()
                    .children(windows.map(|n| {
                        Button::new(SharedString::from(format!("live-window-{n}")))
                            .label(format!("{n}"))
                            .selected(current == n)
                    }))
                    .on_click(move |ix: &Vec<usize>, window, app| {
                        if let Some(&i) = ix.first() {
                            view.update(app, |this, cx| this.set_live_window(windows[i], cx));
                            window.refresh();
                        }
                    })
                    .into_any_element(),
            }]
        }
        SettingsSection::Appearance => {
            let (v_source, v_density, v_minimap) = (view.clone(), view.clone(), view.clone());
            let v_stream = view.clone();
            let densities = [
                (Density::Comfortable, "Comfortable"),
                (Density::Compact, "Compact"),
                (Density::UltraCompact, "Ultra"),
            ];
            let density = v.density;
            vec![
                Row {
                    title: "Row density",
                    description: "How tall the rows of the log table are, and the size of their text.",
                    control: ButtonGroup::new("settings-density")
                        .small()
                        .children(densities.map(|(d, label)| {
                            Button::new(SharedString::from(format!("density-{label}")))
                                .label(label)
                                .selected(density == d)
                        }))
                        .on_click(move |ix: &Vec<usize>, window, app| {
                            if let Some(&i) = ix.first() {
                                v_density.update(app, |this, cx| this.set_density(densities[i].0, cx));
                                window.refresh();
                            }
                        })
                        .into_any_element(),
                },
                Row {
                    title: "Show minimap",
                    description: "A thin overview of the whole log beside the table; click or drag it to scroll.",
                    control: Switch::new("show-minimap")
                        .checked(v.minimap)
                        .on_click(move |checked: &bool, window, app| {
                            let on = *checked;
                            v_minimap.update(app, |this, cx| this.set_show_minimap(on, cx));
                            window.refresh();
                        })
                        .into_any_element(),
                },
                Row {
                    title: "Show source file",
                    description: "Add a column with the file each line came from. Applies to a Log Space.",
                    control: Switch::new("show-source")
                        .checked(v.shown)
                        .disabled(!v.in_space)
                        .on_click(move |checked: &bool, window, app| {
                            let on = *checked;
                            v_source.update(app, |this, cx| this.set_show_source(on, cx));
                            window.refresh();
                        })
                        .into_any_element(),
                },
                Row {
                    title: "Show stream",
                    description: "Add a column saying whether each line of a remote command came from its standard output or its standard error. Shows in a Log Space that has a remote command.",
                    control: Switch::new("show-stream")
                        .checked(v.stream)
                        .disabled(!v.in_space || !v.has_command)
                        .on_click(move |checked: &bool, window, app| {
                            let on = *checked;
                            v_stream.update(app, |this, cx| this.set_show_stream(on, cx));
                            window.refresh();
                        })
                        .into_any_element(),
                },
                Row {
                    title: "Search match color",
                    description: "Paint of the lines a Search Expression finds, in the view and in the Focus Pane.",
                    control: match_color_swatches(view, MatchKind::Search, v.colors.search, cx),
                },
                Row {
                    title: "Focus Find match color",
                    description: "Paint of the text Focus Find finds in the Focus Pane.",
                    control: match_color_swatches(view, MatchKind::Find, v.colors.find, cx),
                },
            ]
        }
        SettingsSection::RawView => {
            let view = view.clone();
            vec![Row {
                title: "Wrap long lines",
                description: "Wrap lines longer than the window in the Raw View instead of scrolling sideways.",
                control: Switch::new("raw-soft-wrap")
                    .checked(v.wrap)
                    .on_click(move |checked: &bool, window, app| {
                        let on = *checked;
                        view.update(app, |this, cx| this.set_raw_soft_wrap(on, window, cx));
                        window.refresh();
                    })
                    .into_any_element(),
            }]
        }
        SettingsSection::Keymap => SHORTCUTS
            .into_iter()
            .filter_map(|(keys, label)| {
                // Shortcuts take Ctrl in place of Cmd off macOS.
                let keys = if cfg!(target_os = "macos") { keys.to_string() } else { keys.replace("cmd-", "ctrl-") };
                Keystroke::parse(&keys).ok().map(|stroke| Row {
                    title: label,
                    description: "",
                    control: Kbd::new(stroke).into_any_element(),
                })
            })
            .collect(),
    }
}

/// "Default" and one swatch per hue.
fn match_color_swatches(
    view: &Entity<AppView>,
    kind: MatchKind,
    current: Option<Hue>,
    cx: &App,
) -> AnyElement {
    let (border, selected_border) = {
        let theme = cx.theme();
        (theme.border, theme.foreground)
    };
    let tag = if kind == MatchKind::Search { "search" } else { "find" };
    let v_default = view.clone();
    let mut swatches = h_flex().gap_1().items_center();
    swatches = swatches.child(
        Button::new(SharedString::from(format!("match-{tag}-default")))
            .ghost()
            .xsmall()
            .label("Default")
            .selected(current.is_none())
            .on_click(move |_, window, app| {
                v_default.update(app, |this, cx| this.set_match_color(kind, None, cx));
                window.refresh();
            }),
    );
    for hue in Hue::ALL {
        let (h, s, l) = hue.vivid_hsl();
        let v = view.clone();
        swatches = swatches.child(
            div()
                .id(SharedString::from(format!("match-{tag}-{}", hue.name())))
                .size(px(20.0))
                .rounded_sm()
                .cursor_pointer()
                .bg(gpui::hsla(h, s, l, 1.0))
                .border_1()
                .border_color(if current == Some(hue) { selected_border } else { border })
                .on_click(move |_, window, app| {
                    v.update(app, |this, cx| this.set_match_color(kind, Some(hue), cx));
                    window.refresh();
                }),
        );
    }
    swatches.into_any_element()
}

/// Paint a nav entry's background when it is the open page.
trait WhenSelected: Styled + Sized {
    fn when_selected(self, selected: bool, bg: gpui::Hsla) -> Self {
        if selected { self.bg(bg) } else { self }
    }
}
impl<T: Styled + Sized> WhenSelected for T {}
