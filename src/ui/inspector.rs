use crate::marker::MarkerType;
use crate::model::LogEntry;

use gpui::{
    div, px, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{Icon};
use gpui_kit::assets::IconName;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Fields,
    Raw,
    Hex,
    Notes,
}

pub struct InspectorView;

impl InspectorView {
    /// Inspector body (tabs + content) for the right sidebar's Inspector section.
    pub fn render_body(
        entry: Option<&LogEntry>,
        active_tab: InspectorTab,
        cx: &mut gpui::Context<crate::app::AppView>,
    ) -> gpui::AnyElement {
        let Some(entry) = entry else {
            return div()
                .flex_1()
                .p_4()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_xs().child("Select a log row to inspect details"))
                .into_any_element();
        };

        // Tabs
        let tabs = div()
            .flex()
            .flex_row()
            .border_b_1()
            .child(tab_btn("Fields", InspectorTab::Fields, active_tab, cx))
            .child(tab_btn("Raw", InspectorTab::Raw, active_tab, cx))
            .child(tab_btn("Hex", InspectorTab::Hex, active_tab, cx))
            .child(tab_btn("Notes", InspectorTab::Notes, active_tab, cx));

        // Tab Content
        let tab_content = match active_tab {
            InspectorTab::Fields => {
                // Structured fields
                let mut field_rows = div().flex().flex_col().gap_2().w_full();

                // Standard metadata
                field_rows = field_rows.child(field_item("Line", &format!("#{}", entry.index)));
                field_rows = field_rows.child(field_item("Timestamp", &entry.time_str));
                field_rows = field_rows.child(field_item("Level", entry.level.as_str()));
                field_rows = field_rows.child(field_item("Module / Tag", &entry.tag));

                if let Some(ref req_id) = entry.request_id {
                    field_rows = field_rows.child(field_item("Request ID", req_id));
                }
                if let Some(ref trace_id) = entry.trace_id {
                    field_rows = field_rows.child(field_item("Trace ID", trace_id));
                }
                if let Some(d) = entry.delta_time_ms {
                    field_rows = field_rows.child(field_item("Delta Time", &format!("+{:.2} ms", d)));
                }

                // JSON key-values
                if !entry.json_fields.is_empty() {
                    let mut json_section = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .mt_2()
                        .pt_2()
                        .border_t_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("JSON PAYLOAD FIELDS"),
                        );

                    for (k, v) in &entry.json_fields {
                        json_section = json_section.child(field_item(k, v));
                    }
                    field_rows = field_rows.child(json_section);
                }

                // Multi-line stack trace
                if !entry.sub_lines.is_empty() {
                    let mut stack_section = div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .mt_2()
                        .pt_2()
                        .border_t_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("STACK TRACE / SUB-LINES"),
                        );

                    for sub in &entry.sub_lines {
                        stack_section = stack_section.child(
                            div()
                                .font_family(".AppleSystemUIFontMonospaced")
                                .text_xs()
                                .child(sub.clone()),
                        );
                    }
                    field_rows = field_rows.child(stack_section);
                }

                div().flex_1().overflow_y_scrollbar().p_3().child(field_rows)
            }
            InspectorTab::Raw => {
                // Raw log line
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_3()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_xs()
                    .child(entry.raw_line.clone())
            }
            InspectorTab::Hex => {
                // Hex dump
                let hex_str = format_hex_dump(entry.raw_line.as_bytes());
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_3()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_xs()
                    .child(hex_str)
            }
            InspectorTab::Notes => {
                // Notes & Annotation
                let note_text = entry.note.clone().unwrap_or_else(|| "No note attached yet. Click 'Set Note' below to annotate this log row.".to_string());
                let marker_label = entry.effective_marker().map(|m| m.label()).unwrap_or("None");
                let entry_ix = entry.index;

                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("MARKER ASSIGNMENT"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .child(format!("Current: {}", marker_label)),
                            ),
                    )
                    // Marker Chip Selector
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1p5()
                            .child(marker_chip("◆ Bookmark", Some(MarkerType::Bookmark), cx))
                            .child(marker_chip("⭐ Important", Some(MarkerType::Important), cx))
                            .child(marker_chip("📌 Note", Some(MarkerType::Note), cx))
                            .child(marker_chip("✕ Clear", None, cx)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .mt_2()
                            .child("ANNOTATION"),
                    )
                    .child(
                        div()
                            .p_2p5()
                            .rounded_md()
                            .border_1()
                            .text_xs()
                            .child(note_text),
                    )
                    // Action Buttons for Note
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                div()
                                    .id("insp_add_note_btn")
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .text_xs()
                                    .child(Icon::new(IconName::FileText).size(px(12.0)))
                                    .child("Set Note")
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.set_selected_note(Some(format!("Investigated at Ln #{}", entry_ix)), cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("insp_clear_note_btn")
                                    .cursor_pointer()
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .text_xs()
                                    .child("Clear")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.set_selected_note(None, cx);
                                    })),
                            ),
                    )
            }
        };

        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(tabs)
            .child(tab_content)
            .into_any_element()
    }
}

fn tab_btn(
    label: &'static str,
    tab: InspectorTab,
    active_tab: InspectorTab,
    cx: &mut gpui::Context<crate::app::AppView>,
) -> impl IntoElement {
    let is_active = tab == active_tab;
    div()
        .id(ElementId::from(format!("insp_tab_{:?}", tab)))
        .cursor_pointer()
        .flex_1()
        .py_1p5()
        .text_center()
        .text_xs()
        .font_weight(if is_active {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::NORMAL
        })
        .border_b_2()
        .child(label)
        .on_click(cx.listener(move |this, _, _window, cx| {
            this.inspector_tab = tab;
            cx.notify();
        }))
}

fn marker_chip(
    label: &'static str,
    marker: Option<MarkerType>,
    cx: &mut gpui::Context<crate::app::AppView>,
) -> impl IntoElement {
    div()
        .id(ElementId::from(format!("chip_mark_{:?}", marker)))
        .cursor_pointer()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(label)
        .on_click(cx.listener(move |this, _, _window, cx| {
            this.set_selected_marker(marker, cx);
        }))
}

fn field_item(label: &str, val: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .child(label.to_string()),
        )
        .child(
            div()
                .font_family(".AppleSystemUIFontMonospaced")
                .text_xs()
                .child(val.to_string()),
        )
}

pub fn format_hex_dump(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "Empty line".to_string();
    }

    let mut out = String::new();
    let chunk_size = 16;

    for (chunk_idx, chunk) in bytes.chunks(chunk_size).enumerate() {
        let offset = chunk_idx * chunk_size;
        out.push_str(&format!("{:08x}  ", offset));

        for i in 0..chunk_size {
            if i < chunk.len() {
                out.push_str(&format!("{:02x} ", chunk[i]));
            } else {
                out.push_str("   ");
            }
            if i == 7 {
                out.push(' ');
            }
        }

        out.push_str(" |");
        for &b in chunk {
            if (32..=126).contains(&b) {
                out.push(b as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }

    out
}
