//! Logic and UI: the Go to bar above the view (Go to Line).
//!
//! Enter makes the typed line the current line of the open Focus Tab, as a
//! Search jump does; Escape closes the bar without moving. Nothing else
//! changes: Search, Filter and Time Range stay as they are, and the bar's
//! text is not remembered.

use super::AppView;
use super::focus::Origin;
use crate::focus::LineRef;
use crate::go_to::{self, GoToError};
use crate::workspace::working_set::ViewMode;
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Subscription,
    Window, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::assets::IconName;
use std::sync::Arc;

pub struct GoToState {
    pub input: Entity<InputState>,
    /// Why the last Enter did not jump, and the text it was pressed on.
    pub error: Option<(GoToError, String)>,
    /// The file a line number is read in, as found when the bar opened.
    pub file: Option<LineFile>,
    _subscription: Subscription,
}

/// The file Go to Line counts lines in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineFile {
    pub file_id: String,
    pub name: String,
    pub generation: u32,
    pub line_count: usize,
}

impl AppView {
    /// Open the Go to bar in Line mode, or focus it again when it is open.
    pub fn open_go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active_tab_is_log_file() || self.empty_canvas().is_some() {
            return;
        }
        let file = self.go_to_line_file();
        if let Some(state) = self.go_to.as_mut() {
            state.file = file;
            state.input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
            cx.notify();
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Line number"));
        input.update(cx, |input, cx| input.focus(window, cx));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.commit_go_to(window, cx),
                InputEvent::Change => {
                    // The error goes once the text it was about is changed.
                    let Some(state) = this.go_to.as_mut() else { return };
                    let text = state.input.read(cx).value().to_string();
                    if state.error.as_ref().is_some_and(|(_, at)| *at != text) {
                        state.error = None;
                        cx.notify();
                    }
                }
                _ => {}
            },
        );
        self.go_to = Some(GoToState {
            input,
            error: None,
            file,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Close the bar without moving; the keyboard goes back to the view.
    pub fn close_go_to(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.go_to.take().is_some() {
            window.focus(&self.focus_handle, cx);
            cx.notify();
        }
    }

    /// Jump to the typed line. The bar stays open with an error when the
    /// text is not a line number.
    pub fn commit_go_to(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.go_to.as_ref() else { return };
        let text = state.input.read(cx).value().to_string();
        // Read again: a live file may have grown since the bar opened.
        let file = self.go_to_line_file();
        let landed = file.ok_or(GoToError::NoLines).and_then(|f| {
            let target = go_to::resolve_line(f.line_count, go_to::parse_line(&text)?)?;
            Ok((f, target))
        });
        let (file, target) = match landed {
            Ok(landed) => landed,
            Err(error) => {
                if let Some(state) = self.go_to.as_mut() {
                    state.error = Some((error, text));
                }
                cx.notify();
                return;
            }
        };
        self.close_go_to(window, cx);
        let line = LineRef {
            file_id: Arc::from(file.file_id.as_str()),
            line: target.line,
            generation: file.generation,
        };
        // Moving the current line drops the note of an earlier Go to.
        self.set_current_line(line, Origin::Jump, cx);
        if target.past_end {
            let note = format!(
                "{} has {} lines; went to the last one.",
                file.name,
                super::format_count(file.line_count)
            );
            self.notice = Some(note.clone());
            self.go_to_notice = Some(note);
        }
    }

    /// Drop the note a Go to left, once the current line moves on. A notice
    /// from anything else stays.
    pub(super) fn drop_go_to_notice(&mut self) {
        if let Some(note) = self.go_to_notice.take() {
            if self.notice.as_ref() == Some(&note) {
                self.notice = None;
            }
        }
    }

    /// The file a typed line number is read in: the current line's file,
    /// else the first file of the view. Its Generation is the current
    /// line's when it is in that file, else the newest one read.
    pub fn go_to_line_file(&self) -> Option<LineFile> {
        let sections = self.raw_sections();
        let current = self.current_line();
        let section = current
            .as_ref()
            .and_then(|c| sections.iter().find(|s| *s.file_id == *c.file_id))
            .or(sections.first())?;
        let of_file = |e: &&crate::model::LogEntry| e.file_id.as_deref() == Some(section.file_id.as_str());
        let generation = current
            .as_ref()
            .filter(|c| *c.file_id == *section.file_id)
            .map(|c| c.generation)
            .unwrap_or_else(|| self.all_entries.iter().filter(of_file).map(|e| e.generation).max().unwrap_or(0));
        let line_count = match self.view_mode {
            ViewMode::Raw => section.lines.len(),
            ViewMode::Parsed => self
                .all_entries
                .iter()
                .filter(of_file)
                .filter(|e| e.generation == generation)
                .map(|e| e.index)
                .max()
                .unwrap_or(0),
        };
        Some(LineFile {
            file_id: section.file_id.clone(),
            name: section.display_name.clone(),
            generation,
            line_count,
        })
    }

    /// The Go to bar, between the toolbar and the view, when it is open.
    pub(super) fn render_go_to_bar(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let state = self.go_to.as_ref()?;
        let theme = cx.theme();
        let hint: SharedString = match &state.file {
            Some(f) if f.line_count > 0 => {
                format!("{} · lines 1–{}", f.name, super::format_count(f.line_count)).into()
            }
            Some(f) => format!("{} · no lines", f.name).into(),
            None => "No file".into(),
        };
        // An error takes the hint's place, right beside the field it is about.
        let (hint, hint_color) = match &state.error {
            Some((error, _)) => (SharedString::from(error.message()), theme.danger),
            None => (hint, theme.muted_foreground),
        };
        Some(
            h_flex()
                .w_full()
                .px_3()
                .py_1()
                .gap_2()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .text_sm()
                .child(div().flex_shrink_0().text_color(theme.muted_foreground).child("Go to Line"))
                .child(
                    div().w(px(160.0)).child(
                        Input::new(&state.input).small().h(px(26.0)),
                    ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_color(hint_color)
                        .child(hint),
                )
                .child(
                    Button::new("go-to-close")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(|this, _, window, cx| this.close_go_to(window, cx))),
                )
                .into_any_element(),
        )
    }
}
