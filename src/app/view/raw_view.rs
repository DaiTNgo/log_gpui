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
        // Scrolling up to read older lines turns Follow off (L2), as in the
        // Parsed View; the Follow button or End turns it back on.
        // The editors take the wheel for themselves, so it is watched in the
        // capture phase, before them.
        let view = cx.entity().downgrade();
        let watch_wheel = gpui::canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let view = view.clone();
                window.on_mouse_event(move |event: &gpui::ScrollWheelEvent, phase, _, cx| {
                    let up = event.delta.pixel_delta(px(20.0)).y > px(0.0);
                    if phase == gpui::DispatchPhase::Capture && up && bounds.contains(&event.position) {
                        view.update(cx, |this, cx| {
                            if this.live.follow {
                                this.live.follow = false;
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                });
            },
        )
        .absolute()
        .size_full();
        // Read-only sections are one list over their files' lines; a
        // scratch file opened as a File View is edited in an editor.
        let body = if sections.iter().any(|s| s.editable) {
            self.render_scratch_sections(sections, cx)
        } else {
            self.render_raw_list(cx)
        };
        div()
            .relative()
            .size_full()
            .min_w(px(0.0))
            .flex()
            .child(body)
            .child(watch_wheel)
            .into_any_element()
    }

    /// The read-only File Sections as one virtualized list (task 8.1): each
    /// row drawn is read from its file as it is drawn, through the file's
    /// Line Index and its cache.
    fn render_raw_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::app::raw_list::{RawRow, TEXT_PAD, gutter_width};
        use gpui::{MouseButton, uniform_list};
        let (sections, rows) = self.raw_rows();
        let row_h = self.text_scale().row_height();
        let font_size = self.text_scale().font_size();
        let max_lines = sections.iter().map(|s| s.lines.len()).max().unwrap_or(0);
        let gutter = gutter_width(max_lines, font_size);
        let colors = self.canvas_colors(cx);
        let tone = colors.tone();
        let matcher = self.focus.search().matcher.clone();
        let phrase_fills = (self.phrase_colors() && matcher.is_some()).then(|| {
            let keys = matcher.as_ref().map(|m| m.phrase_keys()).unwrap_or_default();
            crate::ui::canvas::search_phrase_fills(&keys, tone)
        });
        let shared = crate::ui::canvas::PhraseFill {
            normal: gpui::hsla(0.55, 0.85, 0.55, 0.35),
            current: gpui::hsla(0.55, 0.9, 0.55, 0.7),
        };
        let current_match = self.raw.matches.get(self.raw.current).copied();
        let current_line = self.current_line();
        let selection = self.raw.list_selection;
        let flash = self.raw.list_flash.clone().filter(|(_, _, at)| at.elapsed() < crate::ui::canvas::ROW_FLASH);
        // The Markers of each section's file, by line.
        let files: Vec<Option<crate::workspace::log_space::LogFile>> = sections
            .iter()
            .map(|s| self.active_project().and_then(|p| p.log_spaces.iter().find_map(|sp| sp.file(&s.file_id))).cloned())
            .collect();
        // Headers, actions rows and Breaks are built here, for the rows of
        // this frame only.
        let mut extras: std::collections::HashMap<usize, gpui::AnyElement> = Default::default();
        let first = self.raw.scroll.0.borrow().base_handle.offset();
        let top = ((-f32::from(first.y)) / row_h).max(0.0) as usize;
        let shown = (f32::from(self.raw.list_bounds.get().size.height) / row_h).ceil() as usize + 2;
        for index in top.saturating_sub(2)..(top + shown.max(60)).min(rows.len()) {
            let element = match rows.row(index) {
                Some(RawRow::Header { section }) => {
                    let collapsed = self.is_section_collapsed(&sections[section].file_id);
                    Some(self.section_header(&sections[section], collapsed, cx))
                }
                Some(RawRow::Actions { section }) => self.live_actions_row(&sections[section], cx),
                Some(RawRow::Break { section, line }) => sections[section].breaks.get(&line).map(|mark| {
                    h_flex()
                        .px_2()
                        .gap_2()
                        .text_size(px(10.0))
                        .text_color(cx.theme().warning)
                        .child("⏤")
                        .child(mark.label.clone())
                        .into_any_element()
                }),
                _ => None,
            };
            if let Some(element) = element {
                extras.insert(index, element);
            }
        }
        let extras = std::rc::Rc::new(std::cell::RefCell::new(extras));
        // The longest line sets how far the list scrolls sideways.
        let longest = sections
            .iter()
            .enumerate()
            .filter_map(|(si, s)| {
                let (line, len) = match &s.lines {
                    crate::raw_view::RawLines::Indexed(file) => file.longest_line().map(|(l, n)| (l, n as usize))?,
                    crate::raw_view::RawLines::Held(lines) => {
                        lines.iter().enumerate().map(|(i, l)| (i, l.len())).max_by_key(|(_, n)| *n)?
                    }
                };
                Some((len, rows.index_of_line(si, line)?))
            })
            .max()
            .map(|(_, index)| index);
        let (muted, selected_bg, flash_bg, text_selection) =
            (colors.muted, colors.selected, colors.info, cx.theme().selection);
        let border = cx.theme().border;
        let list = uniform_list("raw_list", rows.len(), move |range, _window, cx| {
            let mono = cx.theme().mono_font_family.clone();
            range
                .map(|index| {
                    let base = div().id(index).h(px(row_h)).w_full().flex().items_center();
                    match rows.row(index) {
                        Some(RawRow::Line { section: si, line }) => {
                            let section = &sections[si];
                            let text = section.lines.get(line).unwrap_or_default();
                            let len = text.len();
                            let mut layers: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = Vec::new();
                            if let Some(matcher) = &matcher {
                                let is_current = current_match.is_some_and(|m| m.section == si && m.line == line);
                                for p in crate::ui::canvas::line_match_paints(matcher, &text, is_current, phrase_fills.as_deref(), shared) {
                                    layers.push((p.start..p.end, gpui::HighlightStyle { background_color: Some(p.fill), ..Default::default() }));
                                }
                            }
                            if let Some(file) = &files[si] {
                                for m in file.markers.iter().filter(|m| m.line == line + 1) {
                                    if m.line_hash.is_some_and(|h| h != crate::workspace::log_space::line_hash(&text)) {
                                        continue;
                                    }
                                    layers.push((
                                        m.range.clone(),
                                        gpui::HighlightStyle {
                                            background_color: Some(gpui::rgb(m.color.rgb(tone)).into()),
                                            color: Some(gpui::rgb(crate::palette::text_rgb(m.color.text_tone(tone))).into()),
                                            ..Default::default()
                                        },
                                    ));
                                }
                            }
                            if let Some(range) = selection.and_then(|s| s.on_line(si, line, len)) {
                                layers.push((range, gpui::HighlightStyle { background_color: Some(text_selection), ..Default::default() }));
                            }
                            let highlights = layered(&text, layers);
                            let is_current =
                                current_line.as_ref().is_some_and(|c| *c.file_id == *section.file_id && c.line == line + 1);
                            let flashed = flash.as_ref().filter(|(id, l, _)| *id == section.file_id && *l == line).map(|(_, _, at)| {
                                let t = at.elapsed().as_secs_f32() / crate::ui::canvas::ROW_FLASH.as_secs_f32();
                                flash_bg.opacity(0.45 * (1.0 - gpui::ease_in_out(t.min(1.0))))
                            });
                            base.when(is_current, |d| d.bg(selected_bg))
                                .when_some(flashed, |d, fill| d.bg(fill))
                                .font_family(mono.clone())
                                .text_size(px(font_size))
                                .child(
                                    div()
                                        .w(px(gutter))
                                        .flex_none()
                                        .pr_2()
                                        .text_right()
                                        .text_color(muted)
                                        .child(format!("{}", line + 1)),
                                )
                                .child(
                                    div()
                                        .pl(px(TEXT_PAD))
                                        .whitespace_nowrap()
                                        .child(gpui::StyledText::new(text).with_highlights(highlights)),
                                )
                                .into_any_element()
                        }
                        Some(RawRow::Header { .. }) | Some(RawRow::Actions { .. }) | Some(RawRow::Break { .. }) => {
                            match extras.borrow_mut().remove(&index) {
                                Some(element) => base.child(element).into_any_element(),
                                None => base.into_any_element(),
                            }
                        }
                        None => base.into_any_element(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .size_full()
        .with_width_from_item(longest)
        .with_horizontal_sizing_behavior(gpui::ListHorizontalSizingBehavior::Unconstrained)
        .track_scroll(&self.raw.scroll);
        let (v_down, v_move, v_up, v_out) = (cx.entity(), cx.entity(), cx.entity(), cx.entity());
        let bounds_sink = self.raw.list_bounds.clone();
        div()
            .id("raw_list_container")
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .relative()
            .border_color(border)
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                v_down.update(cx, |this, cx| this.start_raw_selection(event.position, window, cx));
            })
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    v_move.update(cx, |this, cx| this.drag_raw_selection(event.position, window, cx));
                }
            })
            .on_mouse_up(MouseButton::Left, move |event, _, cx| {
                v_up.update(cx, |this, cx| this.finish_raw_selection(event.position, cx));
            })
            .on_mouse_up_out(MouseButton::Left, move |event, _, cx| {
                v_out.update(cx, |this, cx| this.finish_raw_selection(event.position, cx));
            })
            .child(
                gpui::canvas(|bounds, _, _| bounds, move |bounds, _, _, _| bounds_sink.set(bounds))
                    .absolute()
                    .size_full(),
            )
            .child(list)
            .into_any_element()
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
                    .child(format!("{} lines", crate::app::format_count(section.lines.len()))),
            )
            .into_any_element()
    }

    /// What a Remote File holds and what the user can do about it: Clear.
    /// A file that holds no line says it waits for new ones.
    fn live_actions_row(&self, section: &RawSection, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        use gpui_component::button::{Button, ButtonVariants as _};
        use gpui_component::Sizable as _;
        let info = self.hold_info(&section.file_id)?;
        let (warning, muted) = (cx.theme().warning, cx.theme().muted_foreground);
        let cleared = self
            .live
            .cached
            .get(&section.file_id)
            .and_then(|c| c.clear_mark.as_ref())
            .filter(|_| section.lines.is_empty())
            .map(|mark| format!("{} cleared {}", section.display_name, mark.time));
        let id = |name: &str| SharedString::from(format!("raw-{name}-{}", section.file_id));
        let clear_file = section.file_id.clone();
        let row = h_flex()
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
            );
        let waiting = if info.command { "Waiting for output" } else { "Waiting for new lines" };
        Some(self.live_row_with_waiting(row, cleared, waiting, warning, muted))
    }

    /// The actions row, and under it the "cleared" line of a file that holds
    /// no line yet.
    fn live_row_with_waiting(
        &self,
        row: gpui::Div,
        cleared: Option<String>,
        waiting: &'static str,
        warning: gpui::Hsla,
        muted: gpui::Hsla,
    ) -> gpui::AnyElement {
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
                    .child(div().text_color(muted).child(waiting)),
            );
        }
        block.into_any_element()
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
                    .text_size(px(13.0 * self.text_scale().ratio()))
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
        let ratio = self.text_scale().ratio();
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
                            .h(px(section_editor_height(section.lines.len(), ratio)))
                            .border_b_1()
                            .border_color(border)
                            .text_size(px(12.0 * ratio))
                            .child(
                                Editor::new(&editor.state)
                                    .appearance(false)
                                    .bordered(false)
                                    .readonly(!section.editable)
                                    .h(px(section_editor_height(section.lines.len(), ratio) - EDITOR_PADDING / 2.0)),
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

    /// A Live Log Space whose files are indexed: their last lines, newest at
    /// the bottom, with no line numbers until Indexing gets there (L3).
    fn render_reading_tail(
        &self,
        files: &[(String, u8)],
        tail: Vec<crate::model::LogEntry>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme();
        let percent = files.iter().map(|(_, p)| *p).min().unwrap_or(0);
        let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        let mut lines = v_flex().w_full().justify_end().font_family(theme.mono_font_family.clone()).text_xs();
        for entry in &tail {
            let text = match &entry.source {
                Some(source) => format!("{source}  {}", entry.raw_line),
                None => entry.raw_line.to_string(),
            };
            lines = lines.child(div().px_2().whitespace_nowrap().overflow_hidden().child(text));
        }
        v_flex()
            .flex_1()
            .h_full()
            .min_h(px(0.0))
            .child(
                div()
                    .id("reading-tail")
                    .debug_selector(|| "reading-tail".into())
                    .w_full()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(format!(
                        "Reading {} ({percent}%): the last lines show; line numbers come once it is read.",
                        names.join(", ")
                    )),
            )
            .child(v_flex().flex_1().min_h(px(0.0)).overflow_hidden().justify_end().child(lines))
            .into_any_element()
    }

    /// Shown when no tab is open or the open Log Space has no files.
    pub(super) fn render_empty_state(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::space_view::EmptyCanvas;
        use gpui_component::button::{Button, ButtonVariants as _};
        use gpui_component::{Icon, Sizable as _};
        use gpui_kit::assets::IconName;
        let state = self.empty_canvas().unwrap_or(EmptyCanvas::NoLogSpace);
        if let EmptyCanvas::Reading { files } = &state {
            let tail = self.reading_tail();
            if !tail.is_empty() {
                return self.render_reading_tail(files, tail, cx);
            }
        }
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let (icon, icon_color) = match state {
            EmptyCanvas::NoLogSpace => (IconName::Layers, muted),
            EmptyCanvas::NoFiles { .. } => (IconName::FolderOpen, muted),
            EmptyCanvas::LoadFailed { .. } => (IconName::TriangleAlert, danger),
            EmptyCanvas::Reading { .. } => (IconName::LoaderCircle, muted),
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
                    .on_click(cx.listener(move |this, _, _, cx| this.add_files_dialog(&space_id, false, cx)))
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

/// The highlights of a line from `layers`, later layers painted over earlier
/// ones: sorted pieces that do not overlap, on character boundaries.
fn layered(
    text: &str,
    layers: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)>,
) -> Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> {
    use crate::text_selection::floor_char_boundary;
    let clamp = |b: usize| floor_char_boundary(text, b.min(text.len()));
    let layers: Vec<_> = layers
        .into_iter()
        .map(|(r, style)| (clamp(r.start)..clamp(r.end), style))
        .filter(|(r, _)| r.start < r.end)
        .collect();
    let mut cuts: Vec<usize> = layers.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = Vec::new();
    for pair in cuts.windows(2) {
        let piece = pair[0]..pair[1];
        let mut style: Option<gpui::HighlightStyle> = None;
        for (r, s) in &layers {
            if r.start <= piece.start && piece.end <= r.end {
                style = Some(match style {
                    Some(have) => have.highlight(*s),
                    None => *s,
                });
            }
        }
        if let Some(style) = style {
            out.push((piece, style));
        }
    }
    out
}
