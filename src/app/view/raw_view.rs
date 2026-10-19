//! UI: the Raw View. Every file of the Log Space is a File Section stacked
//! one under the other, each headed by its name and folder and collapsible.
//! Imported files are read-only lines in a virtualized list; a scratch file
//! is a real editor.

use super::AppView;
use crate::app::raw::{
    EDITOR_PADDING, RAW_ROW_HEIGHT, RawSection, section_editor_height,
};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::input::Editor;
use gpui_component::{ActiveTheme as _, Icon, h_flex, v_flex};
use gpui_kit::assets::IconName;

impl AppView {
    pub(super) fn render_raw_view(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let sections = self.raw_sections();
        if sections.is_empty() {
            return self.render_empty_state(cx);
        }
        self.render_scratch_sections(sections, cx)
    }

    /// One File Section header: chevron, file name, folder, line count.
    fn section_header(
        &self,
        section: &RawSection,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let view = cx.entity();
        let file_id = section.file_id.clone();
        let theme = cx.theme();
        let (bg, border, muted) =
            (crate::ui::main_view::background(cx), theme.border, theme.muted_foreground);
        h_flex()
            .id(SharedString::from(format!("raw-section-{}", section.file_id)))
            .h(px(RAW_ROW_HEIGHT))
            .w_full()
            .px_2()
            .gap_2()
            .items_center()
            .cursor_pointer()
            .bg(bg)
            .border_b_1()
            .border_color(border)
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .on_click(move |_, _, app| {
                let id = file_id.clone();
                view.update(app, |this, cx| this.toggle_section(&id, cx));
            })
            .child(
                Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
                    .size_3(),
            )
            .child(section.display_name.clone())
            .when_some(section.folder.clone(), |this, folder| {
                this.child(
                    div()
                        .text_color(muted)
                        .font_weight(gpui::FontWeight::NORMAL)
                        .truncate()
                        .child(folder),
                )
            })
            .child(
                div()
                    .ml_auto()
                    .text_color(muted)
                    .font_weight(gpui::FontWeight::NORMAL)
                    .child(format!("{} lines", section.lines.len())),
            )
            .into_any_element()
    }

    /// What a Remote File holds and what the user can do about it: Clear,
    /// Load more, Back to Live Window. A file that holds no line says it
    /// waits for new ones.
    fn live_actions_row(&self, section: &RawSection, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use gpui_component::button::{Button, ButtonVariants as _};
        use gpui_component::{Disableable as _, Sizable as _};
        let info = self.hold_info(&section.file_id)?;
        let step = self.live.load_more_step;
        let (warning, muted) = (cx.theme().warning, cx.theme().muted_foreground);
        let cleared = self
            .parsed_files
            .get(&section.file_id)
            .and_then(|p| p.live.as_ref())
            .and_then(|b| b.clear_mark.as_ref())
            .filter(|_| section.lines.is_empty())
            .map(|mark| format!("{} cleared {}", section.display_name, mark.time));
        let id = |name: &str| SharedString::from(format!("raw-{name}-{}", section.file_id));
        let (clear_file, more_file, back_file) =
            (section.file_id.clone(), section.file_id.clone(), section.file_id.clone());
        let load_label = if info.loading {
            "Loading…".to_string()
        } else {
            format!("Load more ({})", info.amount_label(step))
        };
        let cancel_file = section.file_id.clone();
        let mut row = h_flex()
            .px_2()
            .gap_2()
            .items_center()
            .text_size(px(10.0))
            .child(div().text_color(muted).child(info.tooltip()))
            .child(
                Button::new(id("clear"))
                    .ghost()
                    .xsmall()
                    .label("Clear")
                    .on_click(cx.listener(move |this, _, _, cx| this.request_clear_file(&clear_file, cx))),
            )
            .child(
                Button::new(id("more"))
                    .ghost()
                    .xsmall()
                    .label(load_label)
                    .disabled(info.loading || info.load_more_off.is_some())
                    .tooltip(info.load_more_off.unwrap_or("Read older lines from the Server"))
                    .on_click(cx.listener(move |this, _, _, cx| this.load_more(&more_file, None, cx))),
            );
        if info.loading {
            row = row.child(
                Button::new(id("cancel"))
                    .ghost()
                    .xsmall()
                    .label("Cancel")
                    .on_click(cx.listener(move |this, _, _, _| this.cancel_load_more(&cancel_file))),
            );
        }
        if info.extended {
            row = row.child(
                Button::new(id("back"))
                    .ghost()
                    .xsmall()
                    .label("Back to Live Window")
                    .on_click(cx.listener(move |this, _, _, cx| this.back_to_live_window(&back_file, cx))),
            );
        }
        let mut block = v_flex().w_full().child(row);
        if let Some(text) = cleared {
            block = block.child(
                h_flex()
                    .px_2()
                    .gap_2()
                    .text_size(px(10.0))
                    .text_color(warning)
                    .child("⏤")
                    .child(text)
                    .child(div().text_color(muted).child("Waiting for new lines")),
            );
        }
        Some(block.into_any_element())
    }

    /// The Breaks of a live file, listed above its text: the editor holds
    /// only the lines, so a Break cannot sit between two of them, and it is
    /// never part of what is copied.
    fn break_strip(&self, section: &RawSection, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let live = self.live_actions_row(section, cx);
        if section.breaks.is_empty() && live.is_none() {
            return None;
        }
        let warning = cx.theme().warning;
        let border = cx.theme().border;
        let mut strip = v_flex().w_full().border_b_1().border_color(border).py_0p5().children(live);
        for (line, mark) in section.breaks.iter() {
            strip = strip.child(
                h_flex()
                    .px_2()
                    .gap_2()
                    .text_size(px(10.0))
                    .text_color(warning)
                    .child("⏤")
                    .child(mark.label.clone())
                    .child(format!("(above line {})", line + 1)),
            );
        }
        Some(strip.into_any_element())
    }

    /// One editor per File Section (read-only unless it is a scratch File View), so
    /// text can be selected and copied.
    fn render_scratch_sections(
        &mut self,
        sections: Vec<RawSection>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        // One file: the editor fills the whole area, like a text editor.
        if sections.len() == 1 {
            if let Some(editor) = self.raw.editors.get(&sections[0].file_id) {
                let strip = self.break_strip(&sections[0], cx);
                return v_flex()
                    .flex_1()
                    .h_full()
                    .min_w(px(0.0))
                    .text_size(px(13.0))
                    .children(strip)
                    .child(div().flex_1().min_h(px(0.0)).child(
                        Editor::new(&editor.state)
                            .appearance(false)
                            .bordered(false)
                            .readonly(!sections[0].editable)
                            .h(gpui::relative(1.0)),
                    ))
                    .into_any_element();
            }
            return div().flex_1().into_any_element();
        }
        let theme = cx.theme();
        let border = theme.border;
        let mut column = v_flex().w_full();
        for section in &sections {
            let single = sections.len() == 1;
            let collapsed = !single && self.is_section_collapsed(&section.file_id);
            let mut block = v_flex().w_full();
            if !single {
                block = block.child(self.section_header(section, collapsed, cx));
            }
            if !collapsed {
                if let Some(strip) = self.break_strip(section, cx) {
                    block = block.child(strip);
                }
                if let Some(editor) = self.raw.editors.get(&section.file_id) {
                    block = block.child(
                        div()
                            .w_full()
                            .h(px(section_editor_height(section.lines.len())))
                            .border_b_1()
                            .border_color(border)
                            .text_size(px(12.0))
                            .child(
                                Editor::new(&editor.state)
                                    .appearance(false)
                                    .bordered(false)
                                    .readonly(!section.editable)
                                    .h(px(section_editor_height(section.lines.len()) - EDITOR_PADDING / 2.0)),
                            ),
                    );
                }
            }
            column = column.child(block);
        }
        div()
            .id("raw_scratch_scroll")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.raw.section_scroll)
            .child(column)
            .into_any_element()
    }

    /// Shown when no tab is open or the open Log Space has no files.
    pub(super) fn render_empty_state(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::space_view::EmptyCanvas;
        use gpui_component::button::{Button, ButtonVariants as _};
        use gpui_component::{Icon, Sizable as _};
        use gpui_kit::assets::IconName;
        let state = self.empty_canvas().unwrap_or(EmptyCanvas::NoLogSpace);
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let (icon, icon_color) = match state {
            EmptyCanvas::NoLogSpace => (IconName::Layers, muted),
            EmptyCanvas::NoFiles { .. } => (IconName::FolderOpen, muted),
            EmptyCanvas::LoadFailed { .. } => (IconName::TriangleAlert, danger),
        };
        let button = match state {
            EmptyCanvas::NoLogSpace => Some(
                Button::new("empty-new-space")
                    .primary()
                    .small()
                    .label("New Log Space")
                    .on_click(cx.listener(|this, _, _, cx| this.open_new_space_dialog(cx))),
            ),
            EmptyCanvas::NoFiles { can_add: true } => self.active_space().map(|space| {
                let space_id = space.id.clone();
                Button::new("empty-add-files")
                    .primary()
                    .small()
                    .label("Add files")
                    .on_click(cx.listener(move |this, _, _, cx| this.add_files_dialog(&space_id, cx)))
            }),
            _ => None,
        };
        v_flex()
            .flex_1()
            .h_full()
            .items_center()
            .justify_center()
            .gap_2()
            .px_6()
            .child(Icon::new(icon).size(px(32.0)).text_color(icon_color))
            .child(div().text_base().font_weight(gpui::FontWeight::SEMIBOLD).child(state.title()))
            .child(
                div()
                    .max_w(px(420.0))
                    .text_sm()
                    .text_center()
                    .text_color(muted)
                    .whitespace_normal()
                    .child(state.detail()),
            )
            .children(button.map(|b| div().pt_2().child(b)))
            .into_any_element()
    }
}
