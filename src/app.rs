use crate::filter::{FilterEngine, FilterRule, FilterType};
use crate::model::{LogEntry, LogLevel};
use crate::parser::parse_file;
use crate::sample::generate_sample_logs;
use crate::ui::inspector::InspectorView;
use crate::ui::table_view::LogTableDelegate;
use gpui::{
    div, px, rgb, App, AppContext, Context, ElementId, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement as _,
    Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    table::{DataTable, TableEvent, TableState},
    ActiveTheme, Selectable,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct AppView {
    pub all_entries: Vec<LogEntry>,
    pub visible_indices: Arc<Vec<usize>>,
    pub filter_rules: Vec<FilterRule>,
    pub table_state: Entity<TableState<LogTableDelegate>>,
    pub search_input: Entity<InputState>,
    pub selected_row_index: Option<usize>,
    pub inspector_tab: usize, // 0: Payload, 1: Raw Line, 2: Hex Dump
    pub selected_tag: Option<String>,
    pub file_name: String,
    pub file_format: String,
    pub active_sidebar_tab: usize, // 0: Filters, 1: Tags / Services
    pub focus_handle: FocusHandle,
}

impl Focusable for AppView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // 1. Initial sample logs
        let mut all_entries = generate_sample_logs();
        let mut filter_rules = Vec::new();

        // Apply filters initially
        let visible = FilterEngine::apply_filters(&mut filter_rules, &mut all_entries, "");
        let visible_indices = Arc::new(visible);
        let entries_arc = Arc::new(all_entries.clone());

        // 2. Setup table state & delegate
        let delegate = LogTableDelegate::new(entries_arc.clone(), visible_indices.clone());
        let table_state = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .col_resizable(true)
        });

        // Subscribe to table events (selection)
        cx.subscribe(
            &table_state,
            |this: &mut Self, _table, event: &TableEvent, cx| match event {
                TableEvent::SelectRow(row_ix) => {
                    this.selected_row_index = Some(*row_ix);
                    cx.notify();
                }
                TableEvent::DoubleClickedRow(row_ix) => {
                    this.selected_row_index = Some(*row_ix);
                    this.inspector_tab = (this.inspector_tab + 1) % 3;
                    cx.notify();
                }
                _ => {}
            },
        )
        .detach();

        // 3. Search input
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("🔍 Quick search in message, tag, or raw line...")
        });

        cx.subscribe(
            &search_input,
            |this: &mut Self, input, _event: &InputEvent, cx| {
                let text = input.read(cx).text().to_string();
                this.on_search_changed(&text, cx);
            },
        )
        .detach();

        Self {
            all_entries,
            visible_indices,
            filter_rules,
            table_state,
            search_input,
            selected_row_index: if entries_arc.is_empty() { None } else { Some(0) },
            inspector_tab: 0,
            selected_tag: None,
            file_name: "Demo Server Log Stream (Text)".to_string(),
            file_format: "Text Log".to_string(),
            active_sidebar_tab: 0,
            focus_handle,
        }
    }

    pub fn on_search_changed(&mut self, query: &str, cx: &mut Context<Self>) {
        self.apply_current_filters(query, cx);
    }

    pub fn apply_current_filters(&mut self, query: &str, cx: &mut Context<Self>) {
        let visible =
            FilterEngine::apply_filters(&mut self.filter_rules, &mut self.all_entries, query);
        self.visible_indices = Arc::new(visible);
        let entries_arc = Arc::new(self.all_entries.clone());

        // Update table state
        let visible_arc = self.visible_indices.clone();
        self.table_state.update(cx, |state, cx| {
            state.delegate_mut().entries = entries_arc;
            state.delegate_mut().visible_indices = visible_arc;
            state.refresh(cx);
        });

        cx.notify();
    }

    pub fn open_file_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(file_path) = rfd::FileDialog::new()
            .add_filter(
                "Text Log Files (*.log, *.txt, *.json, *.out)",
                &["log", "txt", "json", "jsonl", "out", "csv"],
            )
            .pick_file()
        {
            let path_buf = file_path.clone();
            let name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown File".to_string());

            match parse_file(&path_buf) {
                Ok(entries) => {
                    self.all_entries = entries;
                    self.file_name = name;
                    self.file_format = "Text Log".to_string();
                    self.selected_row_index = if self.all_entries.is_empty() {
                        None
                    } else {
                        Some(0)
                    };

                    let query = self.search_input.read(cx).text().to_string();
                    self.apply_current_filters(&query, cx);
                }
                Err(e) => {
                    eprintln!("Failed to parse file: {:?}", e);
                }
            }
        }
    }

    pub fn load_demo_data(&mut self, cx: &mut Context<Self>) {
        self.all_entries = generate_sample_logs();
        self.file_name = "Demo Server Log Stream (Text)".to_string();
        self.file_format = "Text Log".to_string();
        self.selected_row_index = Some(0);

        let query = self.search_input.read(cx).text().to_string();
        self.apply_current_filters(&query, cx);
    }

    pub fn toggle_filter(&mut self, rule_id: usize, cx: &mut Context<Self>) {
        if let Some(r) = self.filter_rules.iter_mut().find(|r| r.id == rule_id) {
            r.enabled = !r.enabled;
        }
        let query = self.search_input.read(cx).text().to_string();
        self.apply_current_filters(&query, cx);
    }

    pub fn delete_filter(&mut self, rule_id: usize, cx: &mut Context<Self>) {
        self.filter_rules.retain(|r| r.id != rule_id);
        let query = self.search_input.read(cx).text().to_string();
        self.apply_current_filters(&query, cx);
    }

    pub fn add_preset(&mut self, rule: FilterRule, cx: &mut Context<Self>) {
        self.filter_rules.push(rule);
        let query = self.search_input.read(cx).text().to_string();
        self.apply_current_filters(&query, cx);
    }

    pub fn select_tag(&mut self, tag_opt: Option<String>, cx: &mut Context<Self>) {
        self.selected_tag = tag_opt.clone();
        self.filter_rules.retain(|r| r.name != "Tag Filter");
        if let Some(tag) = tag_opt {
            let mut rule = FilterRule::new(9991, "Tag Filter".to_string(), FilterType::Include);
            rule.tag = Some(tag);
            self.filter_rules.push(rule);
        }
        let query = self.search_input.read(cx).text().to_string();
        self.apply_current_filters(&query, cx);
    }

    fn render_filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let muted_color = theme.muted;
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;
        let sec_color = theme.secondary;
        let sec_fg = theme.secondary_foreground;
        let danger_color = theme.danger;

        let mut list = div().flex().flex_col().gap_1p5().w_full();

        for rule in &self.filter_rules {
            let rule_id = rule.id;
            let enabled = rule.enabled;

            let (type_label, type_color, type_bg) = match rule.filter_type {
                FilterType::Include => ("INC", rgb(0x60a5fa), rgb(0x172554)),
                FilterType::Exclude => ("EXC", rgb(0xf87171), rgb(0x450a0a)),
                FilterType::Marker => ("MARK", rgb(0xfacc15), rgb(0x422006)),
            };

            let rule_item = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .p_2()
                .rounded_md()
                .bg(if enabled { bg_color } else { muted_color })
                .border_1()
                .border_color(border_color)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id(ElementId::from(format!("toggle_filter_{}", rule_id)))
                                .cursor_pointer()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .bg(if enabled {
                                    rgb(0x22c55e).into()
                                } else {
                                    muted_fg
                                })
                                .text_color(rgb(0xffffff))
                                .child(if enabled { "ON" } else { "OFF" })
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.toggle_filter(rule_id, cx);
                                })),
                        )
                        .child(
                            div()
                                .px_1()
                                .rounded_xs()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(type_color)
                                .bg(type_bg)
                                .child(type_label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(if enabled { fg } else { muted_fg })
                                .child(rule.name.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_full()
                                .text_xs()
                                .bg(sec_color)
                                .text_color(sec_fg)
                                .child(format!("{} hits", rule.hit_count)),
                        )
                        .child(
                            div()
                                .id(ElementId::from(format!("del_filter_{}", rule_id)))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(danger_color)
                                .child("✕")
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.delete_filter(rule_id, cx);
                                })),
                        ),
                );

            list = list.child(rule_item);
        }

        let presets = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .mt_3()
            .pt_3()
            .border_t_1()
            .border_color(border_color)
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(muted_fg)
                    .child("QUICK FILTER PRESETS"),
            )
            .child(
                Button::new("preset_errors")
                    .label("+ Only Errors & Warnings")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        let mut rule = FilterRule::new(
                            rand_id(),
                            "Only Errors & Warnings".to_string(),
                            FilterType::Include,
                        );
                        rule.min_level = Some(LogLevel::Warn);
                        this.add_preset(rule, cx);
                    })),
            )
            .child(
                Button::new("preset_marker_err")
                    .label("+ Highlight Errors (Red)")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        let mut rule = FilterRule::new(
                            rand_id(),
                            "Highlight Errors".to_string(),
                            FilterType::Marker,
                        );
                        rule.exact_level = Some(LogLevel::Error);
                        rule.marker_color = Some("red".to_string());
                        this.add_preset(rule, cx);
                    })),
            )
            .child(
                Button::new("preset_marker_warn")
                    .label("+ Highlight Warnings (Yellow)")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        let mut rule = FilterRule::new(
                            rand_id(),
                            "Highlight Warnings".to_string(),
                            FilterType::Marker,
                        );
                        rule.exact_level = Some(LogLevel::Warn);
                        rule.marker_color = Some("yellow".to_string());
                        this.add_preset(rule, cx);
                    })),
            )
            .child(
                Button::new("preset_hide_verb")
                    .label("- Hide Verbose & Debug")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        let mut rule = FilterRule::new(
                            rand_id(),
                            "Exclude Verbose".to_string(),
                            FilterType::Exclude,
                        );
                        rule.max_level = Some(LogLevel::Debug);
                        this.add_preset(rule, cx);
                    })),
            );

        div()
            .w(px(260.0))
            .h_full()
            .border_r_1()
            .border_color(border_color)
            .bg(bg_color)
            .p_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(muted_fg)
                    .mb_3()
                    .child("FILTERS & MARKERS"),
            )
            .child(div().flex_1().overflow_y_scrollbar().child(list))
            .child(presets)
    }

    fn render_tags(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;
        let primary = theme.primary;
        let primary_fg = theme.primary_foreground;

        let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();
        for entry in &self.all_entries {
            *tag_counts.entry(entry.tag.clone()).or_insert(0) += 1;
        }

        let mut list = div().flex().flex_col().gap_1p5().w_full();

        let is_all_selected = self.selected_tag.is_none();
        let all_item = div()
            .id("all_tag_item")
            .cursor_pointer()
            .p_1p5()
            .rounded_sm()
            .bg(if is_all_selected { primary } else { bg_color })
            .text_color(if is_all_selected { primary_fg } else { fg })
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(format!("🌐 All Messages ({} lines)", self.all_entries.len()))
            .on_click(cx.listener(|this, _, _window, cx| {
                this.select_tag(None, cx);
            }));

        list = list.child(all_item);

        for (tag, count) in tag_counts {
            let is_tag_selected = self.selected_tag.as_deref() == Some(&tag);
            let tag_clone = tag.clone();

            let tag_item = div()
                .id(ElementId::from(format!("tag_item_{}", tag)))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if is_tag_selected { primary } else { bg_color })
                .border_1()
                .border_color(border_color)
                .text_xs()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(if is_tag_selected { primary_fg } else { fg })
                        .child(format!("🏷️ {}", tag)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(if is_tag_selected { primary_fg } else { muted_fg })
                        .child(format!("{}", count)),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    if is_tag_selected {
                        this.select_tag(None, cx);
                    } else {
                        this.select_tag(Some(tag_clone.clone()), cx);
                    }
                }));

            list = list.child(tag_item);
        }

        div()
            .w(px(220.0))
            .h_full()
            .border_r_1()
            .border_color(border_color)
            .bg(bg_color)
            .p_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(muted_fg)
                    .mb_2()
                    .child("TAGS & SERVICES"),
            )
            .child(div().flex_1().overflow_y_scrollbar().child(list))
    }
}

fn rand_id() -> usize {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as usize
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let muted_color = theme.muted;
        let muted_fg = theme.muted_foreground;

        // 1. Top Navigation & Toolbar
        let top_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .bg(bg_color)
            .border_b_1()
            .border_color(border_color)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("btn_open")
                            .primary()
                            .label("📂 Open File")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.open_file_dialog(cx);
                            })),
                    )
                    .child(
                        Button::new("btn_demo")
                            .outline()
                            .label("⚡ Demo Data")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.load_demo_data(cx);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(self.file_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted_fg)
                                    .child(format!("Format: {}", self.file_format)),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(360.0))
                    .child(Input::new(&self.search_input)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        Button::new("tab_filters")
                            .label("Filters & Markers")
                            .selected(self.active_sidebar_tab == 0)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_sidebar_tab = 0;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tab_tags")
                            .label("Tags & Services")
                            .selected(self.active_sidebar_tab == 1)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_sidebar_tab = 1;
                                cx.notify();
                            })),
                    ),
            );

        // 2. Left Sidebar (Filters or Tags Explorer)
        let sidebar = if self.active_sidebar_tab == 0 {
            self.render_filters(cx).into_any_element()
        } else {
            self.render_tags(cx).into_any_element()
        };

        // 3. Center Log Table
        let table_view = div()
            .flex_1()
            .h_full()
            .child(DataTable::new(&self.table_state).bordered(true).stripe(true));

        // 4. Bottom Inspector View
        let selected_entry = self
            .selected_row_index
            .and_then(|row_ix| self.visible_indices.get(row_ix))
            .and_then(|&entry_ix| self.all_entries.get(entry_ix));

        let inspector_view = InspectorView::render(selected_entry, self.inspector_tab, window, cx);

        // 5. Right pane: Table + Inspector
        let right_content = div()
            .flex_1()
            .flex()
            .flex_col()
            .child(table_view)
            .child(inspector_view);

        // 6. Status Bar (Bottom)
        let total_count = self.all_entries.len();
        let visible_count = self.visible_indices.len();
        let pct = if total_count > 0 {
            (visible_count as f64 / total_count as f64) * 100.0
        } else {
            100.0
        };

        let selected_info = if let Some(row_ix) = self.selected_row_index {
            if let Some(entry) = selected_entry {
                let delta = entry
                    .delta_time_ms
                    .map(|d| format!("+{:.2} ms", d))
                    .unwrap_or_else(|| "0.00 ms".to_string());
                format!("Line #{} (Row {}) | Tag: [{}] | Δt: {}", entry.index, row_ix + 1, entry.tag, delta)
            } else {
                format!("Selected Row: {}", row_ix + 1)
            }
        } else {
            "No line selected".to_string()
        };

        let status_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .py_1()
            .bg(muted_color)
            .border_t_1()
            .border_color(border_color)
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(div().child(format!("Total: {} lines", total_count)))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x38bdf8))
                            .child(format!("Visible: {} ({:.1}%)", visible_count, pct)),
                    )
                    .child(div().text_color(muted_fg).child(selected_info)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted_fg)
                    .child("GPUI Text Log Viewer v0.1.0"),
            );

        // Main App Layout
        div()
            .size_full()
            .bg(bg_color)
            .flex()
            .flex_col()
            .child(top_bar)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .child(sidebar)
                    .child(right_content),
            )
            .child(status_bar)
    }
}
