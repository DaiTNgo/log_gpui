//! Chips showing the Phrases of a Search Expression (or Focus Find
//! expression) next to its field, each filled with the color its Matches have.
//! The text of the field cannot be colored, so the chips carry the legend.

use crate::palette::Tone;
use crate::search_expr::Matcher;
use crate::ui::canvas::{PhraseFill, find_phrase_fills, search_phrase_fills};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};

/// One chip: a distinct Phrase (case-folded) and its Match fill.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseChip {
    pub label: String,
    pub fill: Hsla,
}

fn chips_of(keys: &[&str], fills: Vec<PhraseFill>) -> Vec<PhraseChip> {
    keys.iter()
        .zip(fills)
        .map(|(k, f)| PhraseChip {
            label: (*k).to_string(),
            fill: f.normal,
        })
        .collect()
}

/// Chips of the Matcher in use, in order of first appearance. None when
/// there is no expression or `phrase_colors` is off.
pub fn search_chips(matcher: Option<&Matcher>, tone: Tone, phrase_colors: bool) -> Vec<PhraseChip> {
    let Some(m) = matcher.filter(|_| phrase_colors) else {
        return Vec::new();
    };
    let keys = m.phrase_keys();
    chips_of(&keys, search_phrase_fills(&keys, tone))
}

/// Chips of a Focus Find expression; a Phrase also in Search has Search's color.
pub fn find_chips(
    keys: &[&str],
    search_keys: &[&str],
    tone: Tone,
    phrase_colors: bool,
) -> Vec<PhraseChip> {
    if !phrase_colors {
        return Vec::new();
    }
    chips_of(keys, find_phrase_fills(keys, search_keys, tone))
}

/// A row of at most `max` chips, then a "+N" chip. Each label is clipped with
/// an ellipsis. `None` when there are no chips.
pub fn render_chips(chips: &[PhraseChip], max: usize, cx: &App) -> Option<AnyElement> {
    if chips.is_empty() {
        return None;
    }
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let chip = |label: String, fill: Option<Hsla>| {
        div()
            .flex_none()
            .max_w(px(90.0))
            .px_1p5()
            .rounded_sm()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_xs()
            .text_color(if fill.is_some() { fg } else { muted })
            .when_some(fill, |d, f| d.bg(f))
            .child(label)
    };
    Some(
        h_flex()
            .flex_none()
            .gap_1()
            .items_center()
            .children(
                chips
                    .iter()
                    .take(max)
                    .map(|c| chip(c.label.clone(), Some(c.fill))),
            )
            .when(chips.len() > max, |row| {
                row.child(chip(format!("+{}", chips.len() - max), None))
            })
            .into_any_element(),
    )
}

/// Chips shown beside the Focus Find field.
pub const FIND_MAX_CHIPS: usize = 3;

/// Text of a Phrase field drawn invisibly, each Phrase in a pill of the fill
/// its Matches have, so the color sits on the typed word. Returns the layer to
/// put under the field (the pills) and the one over it (the same row,
/// unpainted, whose Phrases call `on_menu` with the click position and the
/// Phrase key on a right click). `fills_of` gives the fill of each distinct
/// Phrase key, in order. `None` when no Phrase is typed.
pub fn phrase_text_layers(
    text: &str,
    fills_of: impl Fn(&[&str]) -> Vec<PhraseFill>,
    on_menu: impl Fn(gpui::Point<gpui::Pixels>, String, &mut App) + Clone + 'static,
    _id: &'static str,
) -> Option<(AnyElement, AnyElement)> {
    let spans = crate::search_expr::phrase_spans(text);
    if spans.is_empty() {
        return None;
    }
    let mut keys: Vec<&str> = Vec::new();
    for (_, k) in &spans {
        if !keys.contains(&k.as_str()) {
            keys.push(k);
        }
    }
    let fills = fills_of(&keys);
    let pills: Vec<(std::ops::Range<usize>, String, Hsla)> = spans
        .iter()
        .filter_map(|(range, k)| {
            let i = keys.iter().position(|x| *x == k.as_str())?;
            Some((range.clone(), k.clone(), fills.get(i)?.normal))
        })
        .collect();
    let text = text.to_string();
    // The pills are placed from the text shaped with the field's own font, so
    // they sit under the typed words however long the text is.
    let rects = {
        let text = text.clone();
        let pills = pills.clone();
        move |bounds: gpui::Bounds<gpui::Pixels>, window: &mut gpui::Window| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let run = gpui::TextRun {
                len: text.len(),
                font: style.font(),
                color: gpui::transparent_black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window
                .text_system()
                .shape_line(text.clone().into(), size, &[run], None);
            let left = bounds.origin.x + gpui_component::Size::Medium.input_px();
            pills
                .iter()
                .map(|(range, key, fill)| {
                    let x0 = left + line.x_for_index(range.start);
                    let x1 = left + line.x_for_index(range.end);
                    let rect = gpui::Bounds {
                        origin: gpui::point(
                            x0 - PILL_PAD,
                            bounds.origin.y + (bounds.size.height - PILL_HEIGHT) / 2.0,
                        ),
                        size: gpui::size(x1 - x0 + PILL_PAD * 2.0, PILL_HEIGHT),
                    };
                    (rect, key.clone(), *fill)
                })
                .collect::<Vec<_>>()
        }
    };
    let under_rects = rects.clone();
    let under = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _cx| {
            for (rect, _, fill) in under_rects(bounds, window) {
                window.paint_quad(
                    gpui::fill(rect, fill)
                        .corner_radii(PILL_RADIUS)
                        .border_widths(px(1.0))
                        .border_color(fill.opacity(2.0)),
                );
            }
        },
    )
    .size_full();
    let over = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _cx| {
            let hit = rects(bounds, window);
            let on_menu = on_menu.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, _window, cx| {
                if phase != gpui::DispatchPhase::Capture || e.button != MouseButton::Right {
                    return;
                }
                if let Some((_, key, _)) = hit.iter().find(|(r, _, _)| r.contains(&e.position)) {
                    on_menu(e.position, key.clone(), cx);
                    cx.stop_propagation();
                }
            });
        },
    )
    .size_full();
    // A div carries the field's text size, which a canvas does not apply.
    let host = |canvas: gpui::Canvas<()>| {
        div()
            .absolute()
            .inset_0()
            .text_sm()
            .child(canvas)
            .into_any_element()
    };
    Some((host(under), host(over)))
}

/// A Phrase pill under the typed text: its height, and how far it extends past the word.
const PILL_HEIGHT: gpui::Pixels = px(20.0);
const PILL_PAD: gpui::Pixels = px(2.0);
const PILL_RADIUS: gpui::Pixels = px(5.0);

/// Opens the hue menu of a Phrase at a window position, in the main window or
/// in the Focus window.
fn menu_opener(
    view: gpui::Entity<crate::app::AppView>,
    focus_window: bool,
) -> impl Fn(gpui::Point<gpui::Pixels>, String, &mut App) + Clone + 'static {
    move |pos, key, cx| {
        view.update(cx, |this, cx| {
            this.phrase_menu = Some((pos, key, focus_window));
            cx.notify();
        })
    }
}

/// The text layers of the Search field in the toolbar.
pub fn toolbar_search_layers(
    view: &crate::app::AppView,
    cx: &mut gpui::Context<crate::app::AppView>,
) -> Option<(AnyElement, AnyElement)> {
    if !view.phrase_colors() {
        return None;
    }
    let text = view.search_input.read(cx).value();
    let tone = if cx.theme().is_dark() {
        Tone::Dark
    } else {
        Tone::Light
    };
    phrase_text_layers(
        &text,
        |keys| search_phrase_fills(keys, tone),
        menu_opener(cx.entity(), false),
        "search-phrase",
    )
}

/// The text layers of the Focus Pane's Find field, or of its Search field in
/// the Focus window (`search_keys` empty: its Phrases are Search's own).
pub fn focus_field_layers(
    text: &str,
    search_keys: Option<&[&str]>,
    tone: Tone,
    phrase_colors: bool,
    view: &gpui::Entity<crate::app::AppView>,
    focus_window: bool,
    id: &'static str,
) -> Option<(AnyElement, AnyElement)> {
    if !phrase_colors {
        return None;
    }
    phrase_text_layers(
        text,
        |keys| match search_keys {
            Some(sk) => find_phrase_fills(keys, sk, tone),
            None => search_phrase_fills(keys, tone),
        },
        menu_opener(view.clone(), focus_window),
        id,
    )
}

impl crate::app::AppView {
    /// The menu of hues for a Phrase of the Search field, or `None` when closed.
    pub(crate) fn render_phrase_menu(
        &self,
        focus_window: bool,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let (pos, key, in_focus_window) = self.phrase_menu.clone()?;
        if in_focus_window != focus_window {
            return None;
        }
        let current = crate::search_color::stored_hue_override(&key);
        let custom_rgb = crate::search_color::stored_rgb_override(&key);
        let theme_fg = cx.theme().foreground;
        let ring = |on: bool| if on { theme_fg } else { gpui::black().opacity(0.25) };
        let label = |text: &'static str| div().text_xs().text_color(cx.theme().muted_foreground).child(text);
        let hover_preview = |this: &mut Self,
                             key: &str,
                             preview: crate::search_color::Preview,
                             hovered: bool,
                             cx: &mut gpui::Context<Self>| {
            this.preview_phrase(key, hovered.then_some(preview), cx)
        };

        // Every picked color is one click away: a grid of grays and hues, light to dark.
        const FAMILIES: [Option<f32>; 10] = [
            None, Some(0.0), Some(25.0), Some(50.0), Some(95.0), Some(150.0),
            Some(185.0), Some(215.0), Some(265.0), Some(320.0),
        ];
        const LIGHTNESS: [f32; 8] = [0.92, 0.82, 0.72, 0.62, 0.52, 0.42, 0.32, 0.22];
        let mut grid = v_flex().gap(px(2.0));
        for (r, family) in FAMILIES.into_iter().enumerate() {
            let mut row = h_flex().gap(px(2.0));
            for (c, l) in LIGHTNESS.into_iter().enumerate() {
                let rgb = match family {
                    Some(h) => crate::search_color::hsl_to_rgb(h, 0.75, l),
                    None => crate::search_color::hsl_to_rgb(0.0, 0.0, l + 0.08),
                };
                let chosen = custom_rgb == Some(rgb);
                let (pick_key, hover_key) = (key.clone(), key.clone());
                row = row.child(
                    div()
                        .id(gpui::ElementId::from(("phrase-rgb", r * LIGHTNESS.len() + c)))
                        .size(px(18.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(ring(chosen))
                        .bg(gpui::rgb(rgb))
                        .cursor_pointer()
                        .on_hover(cx.listener(move |this, hovered: &bool, _w, cx| {
                            hover_preview(this, &hover_key, crate::search_color::Preview::Rgb(rgb), *hovered, cx)
                        }))
                        .on_click(cx.listener(move |this, _, _w, cx| {
                            this.set_phrase_rgb(&pick_key, rgb, cx);
                            this.close_phrase_menu(cx);
                        })),
                );
            }
            grid = grid.child(row);
        }
        let has_choice = current.is_some() || custom_rgb.is_some();

        let (reset_key, reset_hover_key) = (key.clone(), key.clone());
        let theme = cx.theme();
        let panel = v_flex()
            .id("phrase-menu")
            .absolute()
            .left(pos.x)
            .top(pos.y + px(14.0))
            .gap_2()
            .p_2()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_sm().child(key.clone()))
            .child(label("Presets"))
            .child(grid)
            .child(
                div()
                    .id("phrase-hue-reset-host")
                    .on_hover(cx.listener(move |this, hovered: &bool, _w, cx| {
                        // Hovering Reset shows the generated hue.
                        hover_preview(
                            this,
                            &reset_hover_key,
                            crate::search_color::Preview::Generated,
                            *hovered && has_choice,
                            cx,
                        )
                    }))
                    .child(
                        Button::new("phrase-hue-reset")
                            .ghost()
                            .xsmall()
                            .label("Reset color")
                            .disabled(!has_choice)
                            .on_click(cx.listener(move |this, _, _w, cx| {
                                this.set_phrase_hue(&reset_key, None, cx);
                                this.close_phrase_menu(cx);
                            })),
                    ),
            );
        Some(
            div()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _w, cx| this.close_phrase_menu(cx)),
                )
                .child(panel)
                .into_any_element(),
        )
    }
}
