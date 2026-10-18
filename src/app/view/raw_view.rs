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

    /// The Breaks of a live file, listed above its text: the editor holds
    /// only the lines, so a Break cannot sit between two of them, and it is
    /// never part of what is copied.
    fn break_strip(&self, section: &RawSection, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if section.breaks.is_empty() {
            return None;
        }
        let warning = cx.theme().warning;
        let border = cx.theme().border;
        let mut strip = v_flex().w_full().border_b_1().border_color(border).py_0p5();
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
        use gpui_component::button::{Button, ButtonVariants as _};
        use gpui_component::Sizable as _;
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let space = self.active_target().and_then(|t| match t {
            crate::workspace::working_set::TabTarget::LogSpace { space_id } => Some(space_id),
            _ => None,
        });
        let imported_space = space.as_ref().and_then(|id| {
            self.active_project()
                .and_then(|p| p.log_space(id))
                .filter(|s| s.kind == crate::workspace::log_space::LogSpaceKind::Imported)
                .map(|s| s.id.clone())
        });
        let (message, button) = match imported_space {
            Some(space_id) => (
                "This Log Space has no files.",
                Button::new("empty-add-files")
                    .primary()
                    .small()
                    .label("Add files")
                    .on_click(cx.listener(move |this, _, _, cx| this.add_files_dialog(&space_id, cx))),
            ),
            None => (
                "No Log Space open.",
                Button::new("empty-new-space")
                    .primary()
                    .small()
                    .label("New Log Space")
                    .on_click(cx.listener(|this, _, _, cx| this.open_new_space_dialog(cx))),
            ),
        };
        v_flex()
            .flex_1()
            .h_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(div().text_color(muted).child(message))
            .child(button)
            .into_any_element()
    }
}
