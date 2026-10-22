//! Chips showing the Phrases of a Search Expression (or Focus Find
//! expression) next to its field, each filled with the color its Matches have.
//! The text of the field cannot be colored, so the chips carry the legend.

use crate::palette::Tone;
use crate::search_expr::Matcher;
use crate::ui::canvas::{find_phrase_fills, search_phrase_fills, PhraseFill};
use gpui::prelude::FluentBuilder as _;
use gpui::{div, px, AnyElement, App, Hsla, IntoElement, ParentElement, Styled};
use gpui_component::{h_flex, ActiveTheme as _};

/// One chip: a distinct Phrase (case-folded) and its Match fill.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseChip {
    pub label: String,
    pub fill: Hsla,
}

fn chips_of(keys: &[&str], fills: Vec<PhraseFill>) -> Vec<PhraseChip> {
    keys.iter()
        .zip(fills)
        .map(|(k, f)| PhraseChip { label: (*k).to_string(), fill: f.normal })
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
pub fn find_chips(keys: &[&str], search_keys: &[&str], tone: Tone, phrase_colors: bool) -> Vec<PhraseChip> {
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
            .children(chips.iter().take(max).map(|c| chip(c.label.clone(), Some(c.fill))))
            .when(chips.len() > max, |row| row.child(chip(format!("+{}", chips.len() - max), None)))
            .into_any_element(),
    )
}

/// Chips shown in the toolbar's Search field before the Search clear button.
const TOOLBAR_MAX_CHIPS: usize = 4;
/// Chips shown beside the Focus Find field.
pub const FIND_MAX_CHIPS: usize = 3;

impl crate::app::AppView {
    /// Chips of the open Focus Tab's Matcher in use (the last expression that
    /// parsed), so they agree with the Match colors on screen.
    pub(crate) fn search_phrase_chips(&self, cx: &gpui::Context<Self>) -> Option<AnyElement> {
        let tone = if cx.theme().is_dark() { Tone::Dark } else { Tone::Light };
        let chips = search_chips(self.focus.search().matcher.as_ref(), tone, self.phrase_colors());
        render_chips(&chips, TOOLBAR_MAX_CHIPS, cx)
    }
}
