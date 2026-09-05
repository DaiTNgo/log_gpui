use crate::model::LogEntry;
use gpui::{div, px, rgb, App, IntoElement, ParentElement, Styled, Window};
use gpui_component::{scroll::ScrollableElement as _, ActiveTheme};

pub struct InspectorView;

impl InspectorView {
    pub fn render(
        entry: Option<&LogEntry>,
        active_tab: usize, // 0: Payload, 1: Raw Line, 2: Hex Dump
        _window: &mut Window,
        cx: &App,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
        let bg_color = theme.background;
        let muted_color = theme.muted;
        let muted_fg = theme.muted_foreground;
        let fg = theme.foreground;
        let primary = theme.primary;
        let primary_fg = theme.primary_foreground;

        let Some(entry) = entry else {
            return div()
                .h(px(220.0))
                .w_full()
                .border_t_1()
                .border_color(border_color)
                .bg(muted_color)
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted_fg)
                .child("Select a log row to inspect payload, raw line, and hex dump")
                .into_any_element();
        };

        // Header info bar
        let delta_str = entry
            .delta_time_ms
            .map(|d| format!("+{:.2} ms", d))
            .unwrap_or_else(|| "0.00 ms".to_string());

        let header_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_4()
            .px_3()
            .py_1p5()
            .bg(muted_color)
            .border_b_1()
            .border_color(border_color)
            .text_xs()
            .child(info_badge("Line", &format!("#{}", entry.index), cx))
            .child(info_badge("Time", &entry.time_str, cx))
            .child(info_badge("Δt", &delta_str, cx))
            .child(info_badge("Tag / Service", &entry.tag, cx))
            .child(info_badge("Level", entry.level.as_str(), cx))
            .child(info_badge("Bytes", &format!("{} bytes", entry.raw_line.len()), cx));

        // Tab bar
        let tab_bar = div()
            .flex()
            .flex_row()
            .gap_2()
            .px_3()
            .py_1()
            .bg(bg_color)
            .border_b_1()
            .border_color(border_color)
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if active_tab == 0 { primary_fg } else { muted_fg })
                    .bg(if active_tab == 0 { primary } else { muted_color })
                    .child("Message Payload"),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if active_tab == 1 { primary_fg } else { muted_fg })
                    .bg(if active_tab == 1 { primary } else { muted_color })
                    .child("Raw Line"),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if active_tab == 2 { primary_fg } else { muted_fg })
                    .bg(if active_tab == 2 { primary } else { muted_color })
                    .child("Hex / ASCII Dump"),
            );

        // Content body
        let content_body = match active_tab {
            0 => {
                // Formatted message payload
                div()
                    .flex_1()
                    .p_3()
                    .overflow_y_scrollbar()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_xs()
                    .text_color(fg)
                    .child(entry.payload.clone())
                    .into_any_element()
            }
            1 => {
                // Raw Line
                div()
                    .flex_1()
                    .p_3()
                    .overflow_y_scrollbar()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_xs()
                    .text_color(rgb(0x93c5fd))
                    .child(entry.raw_line.clone())
                    .into_any_element()
            }
            _ => {
                // Hex dump view
                let hex_text = format_hex_dump(entry.raw_line.as_bytes());
                div()
                    .flex_1()
                    .p_3()
                    .overflow_y_scrollbar()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_xs()
                    .text_color(rgb(0x38bdf8))
                    .child(hex_text)
                    .into_any_element()
            }
        };

        div()
            .h(px(220.0))
            .w_full()
            .border_t_1()
            .border_color(border_color)
            .bg(bg_color)
            .flex()
            .flex_col()
            .child(header_row)
            .child(tab_bar)
            .child(content_body)
            .into_any_element()
    }
}

fn info_badge(label: &'static str, val: &str, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap_1()
        .child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{}:", label)),
        )
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
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
