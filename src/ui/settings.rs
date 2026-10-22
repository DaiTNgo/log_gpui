//! Settings dialog (gpui-kit `Dialog`): a page per section on the right, a
//! list of sections and a search field on the left. Each setting is a row with
//! its name and what it does on the left and its control on the right.
//! Opened from the title bar.

use crate::app::AppView;
use crate::app::MatchKind;
use crate::density::Density;
use crate::palette::Hue;
use gpui::{
    div, px, AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement,
    prelude::FluentBuilder as _, SharedString, StatefulInteractiveElement, Styled, Window,
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

/// A page of the Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    General,
    Appearance,
    RawView,
    /// The Log Format library, shared by every Project.
    LogFormats,
    Keymap,
}

impl SettingsSection {
    const ALL: [SettingsSection; 5] = [
        SettingsSection::General,
        SettingsSection::Appearance,
        SettingsSection::RawView,
        SettingsSection::LogFormats,
        SettingsSection::Keymap,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsSection::General => "General",
            SettingsSection::Appearance => "Appearance",
            SettingsSection::RawView => "Raw View",
            SettingsSection::LogFormats => "Log Formats",
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
    load_more_step: usize,
    full_read_limit: usize,
    glob_limit: usize,
    glob_interval: u64,
    shown: bool,
    /// The Stream Column is on, and whether the open Log Space has a command.
    stream: bool,
    has_command: bool,
    minimap: bool,
    search_highlight: bool,
    phrase_colors: bool,
    in_space: bool,
    wrap: bool,
    density: Density,
    colors: crate::palette::MatchColors,
    /// The Log Format library: id, name, template.
    formats: Vec<(String, String, String)>,
    keymap: crate::keymap::Keymap,
    recording: Option<crate::keymap::Command>,
    keymap_message: Option<String>,
    record_focus: gpui::FocusHandle,
}

pub fn open_settings_dialog(view: Entity<AppView>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, window, cx| {
        let (values, section, query) = {
            let v = view.read(cx);
            (
                Values {
                    live_window: v.live.window,
                    load_more_step: v.live.load_more_step,
                    full_read_limit: v.live.full_read_limit,
                    glob_limit: v.live.glob_limit,
                    glob_interval: v.live.glob_interval,
                    shown: v.source_column_shown(),
                    stream: v.stream_column_setting(),
                    has_command: v.active_space_has_command(),
                    minimap: v.show_minimap,
                    search_highlight: v.workspace_state.show_search_highlight,
                    phrase_colors: v.phrase_colors(),
                    in_space: v.active_is_log_space(),
                    wrap: v.raw.soft_wrap,
                    density: v.density,
                    colors: v.workspace_state.match_colors,
                    formats: v.log_formats().iter().map(|f| (f.id.clone(), f.name.clone(), f.template.clone())).collect(),
                    keymap: v.keymap.clone(),
                    recording: v.keymap_recording,
                    keymap_message: v.keymap_message.clone(),
                    record_focus: v.keymap_record_focus.clone(),
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
                            .when(!row.description.is_empty(), |col| {
                                col.child(div().text_sm().text_color(muted).child(row.description))
                            }),
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
/// Open the Log Format editor on `id` (or a new format), closing Settings
/// first so the editor is not under it.
fn edit_format(view: &Entity<AppView>, id: Option<String>, window: &mut Window, app: &mut App) {
    window.close_all_dialogs(app);
    let view = view.clone();
    window.defer(app, move |window, app| {
        view.update(app, |this, cx| this.open_format_editor(id.as_deref(), None, window, cx));
    });
}

fn log_format_rows(view: &Entity<AppView>, v: &Values, cx: &App) -> Vec<Row> {
    let muted = cx.theme().muted_foreground;
    let new_view = view.clone();
    let mut list = v_flex().w(px(440.0)).gap_1();
    if v.formats.is_empty() {
        list = list.child(div().text_sm().text_color(muted).child("No Log Formats yet."));
    }
    for (id, name, template) in &v.formats {
        let (edit_view, dup_view, del_view) = (view.clone(), view.clone(), view.clone());
        let (edit_id, dup_id, del_id) = (id.clone(), id.clone(), id.clone());
        list = list.child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(div().text_sm().child(name.clone()))
                        .child(div().text_xs().text_color(muted).truncate().child(template.clone())),
                )
                .child(Button::new(SharedString::from(format!("fmt-edit-{id}"))).xsmall().label("Edit").on_click(
                    move |_, window, app| edit_format(&edit_view, Some(edit_id.clone()), window, app),
                ))
                .child(Button::new(SharedString::from(format!("fmt-dup-{id}"))).xsmall().label("Duplicate").on_click(
                    move |_, window, app| {
                        dup_view.update(app, |this, cx| this.duplicate_log_format(&dup_id, cx));
                        window.refresh();
                    },
                ))
                .child(Button::new(SharedString::from(format!("fmt-del-{id}"))).xsmall().label("Delete").on_click(
                    move |_, window, app| {
                        // Asking first closes Settings, so the question shows.
                        if !del_view.read(app).log_format_uses(&del_id).is_empty() {
                            window.close_all_dialogs(app);
                        }
                        del_view.update(app, |this, cx| this.request_delete_log_format(&del_id, cx));
                        window.refresh();
                    },
                )),
        );
    }
    vec![
        Row {
            title: "Log Formats",
            description: "Templates that read a whole line into columns. Shared by every Project; choose one for a Log Space in its Columns section.",
            control: Button::new("fmt-new")
                .small()
                .label("New Log Format…")
                .on_click(move |_, window, app| edit_format(&new_view, None, window, app))
                .into_any_element(),
        },
        Row { title: "Library", description: "", control: list.into_any_element() },
    ]
}

fn keymap_rows(view: &Entity<AppView>, v: &Values, cx: &App) -> Vec<Row> {
    use crate::keymap::Command;
    let (muted, accent) = {
        let theme = cx.theme();
        (theme.muted_foreground, theme.ring)
    };
    let reset_view = view.clone();
    let mut rows = vec![Row {
        title: "Keyboard shortcuts",
        description: "Press + and then the keys to add a shortcut; a shortcut another command had moves here. Shortcuts without ⌘ or ⌥ work while no text field has the cursor. Esc, the arrow keys, ⌘C, ⌘1–⌘9 and ⌘Tab cannot be changed.",
        control: Button::new("keymap-reset-all")
            .small()
            .label("Reset all")
            .disabled(!v.keymap.has_changes())
            .on_click(move |_, window, app| {
                reset_view.update(app, |this, cx| this.reset_all_shortcuts(cx));
                window.refresh();
            })
            .into_any_element(),
    }];
    if let Some(message) = &v.keymap_message {
        rows.push(Row {
            title: "Last change",
            description: "",
            control: div().max_w(px(440.0)).text_sm().text_color(muted).child(message.clone()).into_any_element(),
        });
    }
    for command in Command::ALL {
        let id = command.id();
        let mut control = h_flex().gap_1().items_center().justify_end();
        for shortcut in v.keymap.shortcuts(command) {
            let del_view = view.clone();
            let text = shortcut.text();
            control = control.child(
                h_flex()
                    .gap_0p5()
                    .items_center()
                    .child(Kbd::new(shortcut.keystroke()))
                    .child(
                        Button::new(SharedString::from(format!("keymap-del-{id}-{text}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .on_click(move |_, window, app| {
                                del_view.update(app, |this, cx| this.remove_shortcut(command, &shortcut, cx));
                                window.refresh();
                            }),
                    ),
            );
        }
        if v.recording == Some(command) {
            let rec_view = view.clone();
            control = control.child(
                div()
                    .id("keymap-recorder")
                    .track_focus(&v.record_focus)
                    .px_2()
                    .py_0p5()
                    .border_1()
                    .border_color(accent)
                    .rounded_md()
                    .text_sm()
                    .child("Press keys… (Esc cancels)")
                    .on_key_down(move |event: &gpui::KeyDownEvent, window, app| {
                        rec_view.update(app, |this, cx| this.record_shortcut(&event.keystroke, cx));
                        app.stop_propagation();
                        window.refresh();
                    }),
            );
        } else {
            let add_view = view.clone();
            control = control.child(
                Button::new(SharedString::from(format!("keymap-add-{id}")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .on_click(move |_, window, app| {
                        add_view.update(app, |this, cx| this.start_recording_shortcut(command, window, cx));
                        window.refresh();
                    }),
            );
        }
        if !v.keymap.is_default(command) {
            let reset_view = view.clone();
            control = control.child(
                Button::new(SharedString::from(format!("keymap-reset-{id}")))
                    .ghost()
                    .xsmall()
                    .label("Reset")
                    .on_click(move |_, window, app| {
                        reset_view.update(app, |this, cx| this.reset_shortcuts(command, cx));
                        window.refresh();
                    }),
            );
        }
        rows.push(Row { title: command.name(), description: "", control: control.into_any_element() });
    }
    rows
}

fn rows_of(section: SettingsSection, view: &Entity<AppView>, v: &Values, cx: &App) -> Vec<Row> {
    match section {
        SettingsSection::LogFormats => log_format_rows(view, v, cx),
        SettingsSection::General => {
            let view = view.clone();
            let step_view = view.clone();
            let windows = [10_000usize, 50_000, 100_000, 500_000, 1_000_000];
            let steps = [1_000usize, 10_000, 100_000, 1_000_000];
            let current = v.live_window;
            let current_step = v.load_more_step;
            let (limit_view, interval_view, read_view) = (view.clone(), view.clone(), view.clone());
            let read_limits = [500_000usize, 1_000_000, 2_000_000, 5_000_000, 10_000_000];
            let current_read = v.full_read_limit;
            let limits = [10usize, 25, 50, 100, 200, 500];
            let intervals = [2u64, 5, 10, 30, 60, 300];
            let (current_limit, current_interval) = (v.glob_limit, v.glob_interval);
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
            },
            Row {
                title: "Load more step",
                description: "How many older lines Load more reads from the Server each time (about 13 MB of memory per 10,000 lines).",
                control: ButtonGroup::new("settings-load-more-step")
                    .small()
                    .children(steps.map(|n| {
                        Button::new(SharedString::from(format!("load-more-step-{n}")))
                            .label(format!("{n}"))
                            .selected(current_step == n)
                    }))
                    .on_click(move |ix: &Vec<usize>, window, app| {
                        if let Some(&i) = ix.first() {
                            step_view.update(app, |this, cx| this.set_load_more_step(steps[i], cx));
                            window.refresh();
                        }
                    })
                    .into_any_element(),
            },
            Row {
                title: "Full Read Limit",
                description: "The most lines one Open in full reads, and the most a Remote File holds once Load more made its window longer. About 1.3 KB of memory per line.",
                control: ButtonGroup::new("settings-full-read-limit")
                    .small()
                    .children(read_limits.map(|n| {
                        Button::new(SharedString::from(format!("full-read-limit-{n}")))
                            .label(format!("{}M", n as f64 / 1_000_000.0))
                            .selected(current_read == n)
                    }))
                    .on_click(move |ix: &Vec<usize>, window, app| {
                        if let Some(&i) = ix.first() {
                            read_view.update(app, |this, cx| this.set_full_read_limit(read_limits[i], cx));
                            window.refresh();
                        }
                    })
                    .into_any_element(),
            },
            Row {
                title: "Live Glob file limit",
                description: "The most files one Live Glob reads at once. When more files match, the most recently modified are read and the status bar says how many are not.",
                control: ButtonGroup::new("settings-glob-limit")
                    .small()
                    .children(limits.map(|n| {
                        Button::new(SharedString::from(format!("glob-limit-{n}")))
                            .label(format!("{n}"))
                            .selected(current_limit == n)
                    }))
                    .on_click(move |ix: &Vec<usize>, window, app| {
                        if let Some(&i) = ix.first() {
                            limit_view.update(app, |this, cx| this.set_glob_limit(limits[i], cx));
                            window.refresh();
                        }
                    })
                    .into_any_element(),
            },
            Row {
                title: "Live Glob scan interval",
                description: "Seconds between two checks of a Live Glob's folder for new files (at least 30 for a folder of more than 10,000 entries).",
                control: ButtonGroup::new("settings-glob-interval")
                    .small()
                    .children(intervals.map(|n| {
                        Button::new(SharedString::from(format!("glob-interval-{n}")))
                            .label(format!("{n} s"))
                            .selected(current_interval == n)
                    }))
                    .on_click(move |ix: &Vec<usize>, window, app| {
                        if let Some(&i) = ix.first() {
                            interval_view.update(app, |this, cx| this.set_glob_interval(intervals[i], cx));
                            window.refresh();
                        }
                    })
                    .into_any_element(),
            }]
        }
        SettingsSection::Appearance => {
            let (v_source, v_density, v_minimap) = (view.clone(), view.clone(), view.clone());
            let v_phrase_colors = view.clone();
            let v_stream = view.clone();
            let v_highlight = view.clone();
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
                    title: "Color Matches by Phrase",
                    description: "Paint each Phrase of a Search in its own color. Off uses the single Match color.",
                    control: Switch::new("phrase-colors")
                        .checked(v.phrase_colors)
                        .on_click(move |checked: &bool, window, app| {
                            let on = *checked;
                            v_phrase_colors.update(app, |this, cx| this.set_phrase_colors(on, cx));
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
                    title: "Search Highlight in the Focus Pane",
                    description: "Paint Search Matches in the Focus Pane. Every line there already matches the Search, so the paint can be turned off; Focus Find matches stay painted.",
                    control: Switch::new("show-search-highlight")
                        .checked(v.search_highlight)
                        .on_click(move |checked: &bool, window, app| {
                            let on = *checked;
                            v_highlight.update(app, |this, cx| this.set_show_search_highlight(on, cx));
                            window.refresh();
                        })
                        .into_any_element(),
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
        SettingsSection::Keymap => keymap_rows(view, v, cx),
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
