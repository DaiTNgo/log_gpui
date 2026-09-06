use crate::filter::{ConditionField, ConditionOp, FilterEngine, LogicMode};
use crate::model::LogLevel;
use crate::theme::CARBON;
use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::{Icon, IconName};

pub struct FilterPopover;

impl FilterPopover {
    pub fn render_builder(
        filter_engine: &FilterEngine,
        is_open: bool,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        if !is_open {
            return div().into_any_element();
        }

        let mut conditions_list = div().flex().flex_col().gap_1p5().w_full();

        for cond in &filter_engine.conditions {
            let cond_id = cond.id;
            let field_str = cond.field.as_str();
            let op_str = cond.op.as_str();
            let val_str = cond.value.clone();

            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .bg(CARBON.layer_01)
                .rounded_sm()
                .border_1()
                .border_color(CARBON.border_subtle)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(CARBON.text_primary)
                                .child(field_str),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(CARBON.text_muted)
                                .child(op_str),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(CARBON.blue_primary)
                                .child(val_str),
                        ),
                )
                .child(
                    div()
                        .id(ElementId::from(format!("rm_cond_{}", cond_id)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(CARBON.error)
                        .child(Icon::new(IconName::Close).size(px(11.0)))
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.remove_filter_condition(cond_id, cx);
                        })),
                );

            conditions_list = conditions_list.child(row);
        }

        div()
            .absolute()
            .top(px(48.0))
            .left(px(240.0))
            .w(px(380.0))
            .bg(CARBON.layer_02)
            .border_1()
            .border_color(CARBON.border_strong)
            .rounded_md()
            .shadow_xl()
            .p_3()
            .flex()
            .flex_col()
            .gap_2p5()
            .child(
                // Header
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(CARBON.border_subtle)
                    .pb_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(CARBON.text_primary)
                            .child("FILTER BUILDER"),
                    )
                    .child(
                        div()
                            .id("close_filter_builder")
                            .cursor_pointer()
                            .text_color(CARBON.text_muted)
                            .hover(|s| s.text_color(CARBON.text_primary))
                            .child(Icon::new(IconName::Close).size(px(13.0)))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_filter_builder(cx);
                            })),
                    ),
            )
            .child(
                // Condition items
                if filter_engine.conditions.is_empty() {
                    div()
                        .py_3()
                        .text_center()
                        .text_xs()
                        .text_color(CARBON.text_muted)
                        .child("No filter conditions. Add one below or click a log value.")
                        .into_any_element()
                } else {
                    conditions_list.into_any_element()
                },
            )
            .child(
                // Quick add condition presets
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .pt_1()
                    .child(
                        div()
                            .id("add_err_cond")
                            .cursor_pointer()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(CARBON.layer_01)
                            .border_1()
                            .border_color(CARBON.border_subtle)
                            .text_xs()
                            .text_color(CARBON.error)
                            .hover(|s| s.bg(CARBON.layer_hover))
                            .child("+ Level is ERROR")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.filter_engine.add_condition(
                                    ConditionField::Level,
                                    ConditionOp::Is,
                                    "ERROR".to_string(),
                                );
                                this.apply_filters_and_refresh(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("add_warn_cond")
                            .cursor_pointer()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(CARBON.layer_01)
                            .border_1()
                            .border_color(CARBON.border_subtle)
                            .text_xs()
                            .text_color(CARBON.warning)
                            .hover(|s| s.bg(CARBON.layer_hover))
                            .child("+ Level is WARN")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.filter_engine.add_condition(
                                    ConditionField::Level,
                                    ConditionOp::Is,
                                    "WARN".to_string(),
                                );
                                this.apply_filters_and_refresh(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("add_api_cond")
                            .cursor_pointer()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(CARBON.layer_01)
                            .border_1()
                            .border_color(CARBON.border_subtle)
                            .text_xs()
                            .text_color(CARBON.text_secondary)
                            .hover(|s| s.bg(CARBON.layer_hover))
                            .child("+ Module is API")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.filter_engine.add_condition(
                                    ConditionField::Module,
                                    ConditionOp::Is,
                                    "API".to_string(),
                                );
                                this.apply_filters_and_refresh(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("add_db_cond")
                            .cursor_pointer()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(CARBON.layer_01)
                            .border_1()
                            .border_color(CARBON.border_subtle)
                            .text_xs()
                            .text_color(CARBON.text_secondary)
                            .hover(|s| s.bg(CARBON.layer_hover))
                            .child("+ Module is DB")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.filter_engine.add_condition(
                                    ConditionField::Module,
                                    ConditionOp::Is,
                                    "DB".to_string(),
                                );
                                this.apply_filters_and_refresh(cx);
                            })),
                    ),
            )
            .child(
                // Logic mode (ALL / ANY) + Action buttons
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .pt_2()
                    .border_t_1()
                    .border_color(CARBON.border_subtle)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(CARBON.text_muted)
                                    .child("Match:"),
                            )
                            .child(
                                div()
                                    .id("match_all_btn")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_xs()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .bg(if filter_engine.logic_mode == LogicMode::All {
                                        CARBON.blue_primary
                                    } else {
                                        CARBON.layer_01
                                    })
                                    .text_color(if filter_engine.logic_mode == LogicMode::All {
                                        CARBON.text_on_color
                                    } else {
                                        CARBON.text_muted
                                    })
                                    .child("ALL (AND)")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.filter_engine.logic_mode = LogicMode::All;
                                        this.apply_filters_and_refresh(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("match_any_btn")
                                    .cursor_pointer()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_xs()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .bg(if filter_engine.logic_mode == LogicMode::Any {
                                        CARBON.blue_primary
                                    } else {
                                        CARBON.layer_01
                                    })
                                    .text_color(if filter_engine.logic_mode == LogicMode::Any {
                                        CARBON.text_on_color
                                    } else {
                                        CARBON.text_muted
                                    })
                                    .child("ANY (OR)")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.filter_engine.logic_mode = LogicMode::Any;
                                        this.apply_filters_and_refresh(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                div()
                                    .id("clear_filters_btn")
                                    .cursor_pointer()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .text_xs()
                                    .text_color(CARBON.text_muted)
                                    .hover(|s| s.text_color(CARBON.text_primary))
                                    .child("Clear")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.filter_engine.clear();
                                        this.apply_filters_and_refresh(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("apply_filters_btn")
                                    .cursor_pointer()
                                    .px_2p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(CARBON.blue_primary)
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(CARBON.text_on_color)
                                    .hover(|s| s.bg(CARBON.blue_hover))
                                    .child("Apply")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_filter_builder(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Renders Quick Log-Click Popover when clicking on Level or Module in a log row
    pub fn render_quick_log_menu(
        active_quick_menu: &Option<QuickLogMenu>,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let Some(menu) = active_quick_menu else {
            return div().into_any_element();
        };

        let title = match &menu.kind {
            QuickLogMenuKind::Level(lvl) => lvl.as_str().to_string(),
            QuickLogMenuKind::Module(mod_name) => mod_name.clone(),
        };

        let (filter_label, hide_label, search_label) = match &menu.kind {
            QuickLogMenuKind::Level(_) => (
                format!("Show only {}", title),
                format!("Hide {}", title),
                format!("Search {}", title),
            ),
            QuickLogMenuKind::Module(_) => (
                format!("Show only {}", title),
                format!("Hide {}", title),
                format!("Search {}", title),
            ),
        };

        let kind_clone1 = menu.kind.clone();
        let kind_clone2 = menu.kind.clone();
        let kind_clone3 = menu.kind.clone();

        div()
            .absolute()
            .top(px(menu.y))
            .left(px(menu.x))
            .w(px(200.0))
            .bg(CARBON.layer_02)
            .border_1()
            .border_color(CARBON.border_strong)
            .rounded_md()
            .shadow_2xl()
            .p_1()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(CARBON.text_muted)
                    .border_b_1()
                    .border_color(CARBON.border_subtle)
                    .child(title),
            )
            .child(
                div()
                    .id("quick_show_only")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_xs()
                    .text_xs()
                    .text_color(CARBON.text_primary)
                    .hover(|s| s.bg(CARBON.layer_hover).text_color(CARBON.blue_primary))
                    .child(filter_label)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        match &kind_clone1 {
                            QuickLogMenuKind::Level(lvl) => this.filter_engine.add_quick_level(*lvl),
                            QuickLogMenuKind::Module(m) => this.filter_engine.add_quick_module(m),
                        }
                        this.active_quick_menu = None;
                        this.apply_filters_and_refresh(cx);
                    })),
            )
            .child(
                div()
                    .id("quick_hide")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_xs()
                    .text_xs()
                    .text_color(CARBON.text_primary)
                    .hover(|s| s.bg(CARBON.layer_hover).text_color(CARBON.error))
                    .child(hide_label)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        match &kind_clone2 {
                            QuickLogMenuKind::Level(lvl) => this.filter_engine.hide_level(*lvl),
                            QuickLogMenuKind::Module(m) => this.filter_engine.hide_module(m),
                        }
                        this.active_quick_menu = None;
                        this.apply_filters_and_refresh(cx);
                    })),
            )
            .child(
                div()
                    .id("quick_search")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_xs()
                    .text_xs()
                    .text_color(CARBON.text_primary)
                    .hover(|s| s.bg(CARBON.layer_hover).text_color(CARBON.cyan_accent))
                    .child(search_label)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        let query = match &kind_clone3 {
                            QuickLogMenuKind::Level(lvl) => lvl.as_str().to_string(),
                            QuickLogMenuKind::Module(m) => m.clone(),
                        };
                        this.active_quick_menu = None;
                        this.trigger_search_query(&query, cx);
                    })),
            )
            .into_any_element()
    }
}

#[derive(Debug, Clone)]
pub struct QuickLogMenu {
    pub x: f32,
    pub y: f32,
    pub kind: QuickLogMenuKind,
}

#[derive(Debug, Clone)]
pub enum QuickLogMenuKind {
    Level(LogLevel),
    Module(String),
}
