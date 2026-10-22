use log_gpui::search_color;
use gpui::hsla;
use log_gpui::focus::FocusFind;
use log_gpui::search_expr::{Matcher, parse};
use log_gpui::ui::canvas::CanvasColors;
use log_gpui::ui::focus_pane::match_paints;

fn matcher(q: &str) -> Matcher {
    Matcher::compile(&parse(q).unwrap().unwrap())
}

fn colors() -> CanvasColors {
    let c = hsla(0.0, 0.0, 0.5, 1.0);
    CanvasColors {
        background: c,
        header_bg: c,
        header_fg: c,
        border: c,
        selected: c,
        search: hsla(0.15, 0.9, 0.5, 1.0),
        find: hsla(0.55, 0.9, 0.5, 1.0),
        muted: c,
        danger: c,
        warning: c,
        info: c,
        stderr: c,
        text_selection: c,
        dark: false,
    }
}

const LINE: &str = "timeout then panic";

#[test]
fn search_matches_use_one_color_per_phrase() {
    let m = matcher("timeout || panic");
    let p = match_paints(LINE, Some(&m), &FocusFind::default(), true, &colors());
    assert_eq!(p.search.len(), 2);
    assert_eq!((p.search[0].start, p.search[0].end), (0, 7));
    assert_ne!(p.search[0].fill.h, p.search[1].fill.h);
    assert!(p.find.is_empty());
}

#[test]
fn focus_find_phrases_follow_the_same_rules() {
    let find = FocusFind::with_query("panic || timeout");
    let p = match_paints(LINE, None, &find, true, &colors());
    assert_eq!(p.find.len(), 2);
    assert_ne!(p.find[0].fill.h, p.find[1].fill.h);
    // The longer Phrase wins where phrases overlap.
    let find = FocusFind::with_query("tim || timeout");
    let p = match_paints(LINE, None, &find, true, &colors());
    assert_eq!(p.find.len(), 1);
    assert_eq!((p.find[0].start, p.find[0].end), (0, 7));
}

#[test]
fn a_phrase_has_the_same_color_in_search_and_focus_find() {
    let m = matcher("timeout");
    let find = FocusFind::with_query("timeout");
    let p = match_paints(LINE, Some(&m), &find, true, &colors());
    assert_eq!(p.search[0].fill, p.find[0].fill);
}

#[test]
fn a_line_that_fails_the_expression_has_no_paint() {
    let m = matcher("timeout && absent");
    let find = FocusFind::with_query("timeout && absent");
    let p = match_paints(LINE, Some(&m), &find, true, &colors());
    assert!(p.search.is_empty() && p.find.is_empty());
}

#[test]
fn toggle_off_restores_the_single_colors_and_keeps_search_and_find_apart() {
    let c = colors();
    let m = matcher("timeout || panic");
    let find = FocusFind::with_query("timeout || panic");
    let p = match_paints(LINE, Some(&m), &find, false, &c);
    assert_eq!(p.search.len(), 2);
    assert!(p.search.iter().all(|s| s.fill == c.search.opacity(search_color::SHARED_SEARCH_OPACITY)));
    assert!(p.find.iter().all(|s| s.fill == c.find.opacity(search_color::FIND_OPACITY)));
}

#[test]
fn a_phrase_in_both_expressions_has_the_same_color_even_when_separation_pushes_it() {
    // `x || y` with colliding plain hues: `y` is pushed in Search; Focus Find
    // for `y` alone must show Search's pushed color.
    let keys: Vec<String> = (0..200).map(|i| format!("k{i}")).collect();
    let (x, y) = keys
        .iter()
        .flat_map(|a| keys.iter().map(move |b| (a, b)))
        .find(|(a, b)| {
            a != b
                && log_gpui::search_color::hue_distance(
                    log_gpui::search_color::hue_for_key(a),
                    log_gpui::search_color::hue_for_key(b),
                ) < 30.0
        })
        .unwrap();
    let line = format!("{x} {y}");
    let m = matcher(&format!("{x} || {y}"));
    let find = FocusFind::with_query(y);
    let p = match_paints(&line, Some(&m), &find, true, &colors());
    assert_eq!(p.find.len(), 1);
    assert_eq!(p.find[0].fill, p.search[1].fill);
}
