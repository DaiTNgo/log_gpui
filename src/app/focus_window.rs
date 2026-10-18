//! The Focus Window: the Focus Pane in an operating-system window of its own.
//! The state stays in `AppView`; the window is a thin view that draws the
//! pane from it and sends every action back.

use super::AppView;
use crate::ui::focus_pane::{FocusPane, PaneMode};
use gpui::{
    AppContext as _, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Styled,
    Window, WindowBounds, WindowHandle, WindowOptions, div, px, size,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Root};

const WINDOW_WIDTH: f32 = 1000.0;
const WINDOW_HEIGHT: f32 = 600.0;

pub struct FocusWindowView {
    app: Entity<AppView>,
    search_input: Entity<InputState>,
    find_input: Entity<InputState>,
}

impl FocusWindowView {
    fn new(app: Entity<AppView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search lines…  e.g. timeout && (db || cache)"));
        let find_input = cx.new(|cx| InputState::new(window, cx).placeholder("Find in this tab…"));

        // Render again whenever the app changes.
        cx.observe(&app, |_, _, cx| cx.notify()).detach();

        let target = app.clone();
        cx.subscribe_in(&search_input, window, move |this: &mut Self, input, event: &InputEvent, _w, cx| match event {
            InputEvent::Change => {
                let text = input.read(cx).value().to_string();
                this.app.update(cx, |app, cx| app.set_search_text_from_window(&text, cx));
            }
            InputEvent::PressEnter { shift, .. } => {
                let shift = *shift;
                target.update(cx, |app, cx| {
                    if shift {
                        app.jump_prev_match(cx)
                    } else {
                        app.jump_next_match(cx)
                    }
                });
            }
            _ => {}
        })
        .detach();

        let target = app.clone();
        cx.subscribe_in(&find_input, window, move |this: &mut Self, input, event: &InputEvent, _w, cx| match event {
            InputEvent::Change => {
                let text = input.read(cx).value().to_string();
                this.app.update(cx, |app, cx| app.set_focus_find_text_from_window(&text, cx));
            }
            InputEvent::PressEnter { shift, .. } => {
                let shift = *shift;
                target.update(cx, |app, cx| {
                    if shift {
                        app.focus_find_prev(cx)
                    } else {
                        app.focus_find_next(cx)
                    }
                });
            }
            _ => {}
        })
        .detach();

        Self { app, search_input, find_input }
    }

    /// Put into a field the text the open tab holds, when it holds other text.
    fn sync_input(input: &Entity<InputState>, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if input.read(cx).value().as_ref() != text.as_str() {
            input.update(cx, |state, cx| state.set_value(gpui::SharedString::from(text), window, cx));
        }
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
        let shift = event.keystroke.modifiers.shift;
        if !cmd {
            return;
        }
        match (shift, key) {
            (false, "f") => self.search_input.update(cx, |state, cx| state.focus(window, cx)),
            (false, "c") => self.app.update(cx, |app, cx| app.copy_pane_selection(cx)),
            (false, "w") => self.app.update(cx, |app, cx| app.move_focus_back(window, cx)),
            (shift, "g") | (shift, "G") => self.app.update(cx, |app, cx| {
                if shift {
                    app.focus_find_prev(cx)
                } else {
                    app.focus_find_next(cx)
                }
            }),
            _ => {}
        }
    }
}

impl Render for FocusWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (search, find) = {
            let app = self.app.read(cx);
            (app.focus.search().query.clone(), app.focus.find().query.clone())
        };
        Self::sync_input(&self.search_input, search, window, cx);
        Self::sync_input(&self.find_input, find, window, cx);

        let view = self.app.clone();
        let (search_input, find_input) = (&self.search_input, &self.find_input);
        // Inside the app's own context, as the docked pane is drawn.
        let (colors, body) = view.update(cx, |app, cx| {
            let colors = app.canvas_colors(cx);
            if app.active_tab_is_log_file() {
                let parsed = app.view_mode == crate::workspace::working_set::ViewMode::Parsed;
                let state = app.pane_state(
                    cx.entity(),
                    PaneMode::Window,
                    find_input,
                    Some(search_input),
                    parsed,
                    colors,
                    0.0,
                    true,
                );
                return (colors, FocusPane::render(state, cx));
            }
            let muted = cx.theme().muted_foreground;
            let v_back = cx.entity();
            let empty = div()
                .size_full()
                .flex()
                .flex_col()
                .gap_3()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("Open a log file in the main window to use the Focus Pane")
                .child(
                    gpui_component::button::Button::new("focus-window-empty-back")
                        .label("Move back")
                        .on_click(move |_, window, cx| {
                            v_back.update(cx, |this, cx| this.move_focus_back(window, cx))
                        }),
                )
                .into_any_element();
            (colors, empty)
        });

        div()
            .id("focus-window")
            .size_full()
            .bg(colors.background)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .child(body)
    }
}

impl AppView {
    /// "Move to window": open the Focus Window on the screen of the main
    /// window, or bring the one already open to the front.
    pub fn move_focus_to_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.activate_focus_window(cx) {
            return;
        }
        let display_id = window.display(cx).map(|d| d.id());
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
                display_id,
                size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                cx,
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Focus Pane".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        // The new window draws at once and reads the app, so it opens after
        // this update has ended.
        cx.spawn(async move |this, cx| {
            let Some(app) = this.upgrade() else { return };
            let weak = this.clone();
            let opened = cx.update(|cx| {
                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| FocusWindowView::new(app, window, cx));
                    // The operating system's close button sends the pane home.
                    window.on_window_should_close(cx, move |_window, cx| {
                        weak.update(cx, |this, cx| {
                            this.focus_window = None;
                            cx.notify();
                        })
                        .ok();
                        true
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                })
            });
            this.update(cx, |this, cx| match opened {
                Ok(handle) => {
                    this.focus_window = Some(handle);
                    cx.notify();
                }
                Err(error) => this.notice = Some(format!("Could not open the Focus Window: {error}")),
            })
            .ok();
        })
        .detach();
    }

    /// "Move back" (from inside the Focus Window): close it, and the pane
    /// returns under the main window at the height it had.
    pub fn move_focus_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_window = None;
        window.remove_window();
        cx.notify();
    }

    /// Close the Focus Window from outside it (the main window is closing).
    pub fn close_focus_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.focus_window.take() {
            handle.update(cx, |_, window, _| window.remove_window()).ok();
            cx.notify();
        }
    }

    /// Bring the Focus Window to the front. False when there is none.
    pub fn activate_focus_window(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(handle) = self.focus_window else { return false };
        if handle.update(cx, |_, window, _| window.activate_window()).is_err() {
            // The window is gone without telling us.
            self.focus_window = None;
            cx.notify();
            return false;
        }
        true
    }

    /// The Search field of the Focus Window changed.
    pub fn set_search_text_from_window(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.focus.search().query == text {
            return;
        }
        let search = self.active_search_mut();
        search.query = text.to_string();
        if !search.query.trim().is_empty() {
            search.is_open = true;
        }
        self.search_input_dirty = true;
        self.run_search(cx);
    }

    /// The Focus Find field of the Focus Window changed.
    pub fn set_focus_find_text_from_window(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.focus.find().query == text {
            return;
        }
        self.focus_find_dirty = true;
        self.on_focus_find_changed(text, cx);
    }
}

pub type FocusWindowHandle = WindowHandle<Root>;
