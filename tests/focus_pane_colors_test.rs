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
fn search_matches_use_one_color_per_group() {
    let m = matcher("timeout || panic");
    let p = match_paints(LINE, Some(&m), &FocusFind::default(), true, &colors());
    assert_eq!(p.search.len(), 2);
    assert_eq!((p.search[0].start, p.search[0].end), (0, 7));
    assert_ne!(p.search[0].fill.h, p.search[1].fill.h);
    assert!(p.find.is_empty());
}

#[test]
fn focus_find_groups_follow_the_same_rules() {
    let find = FocusFind::with_query("panic || timeout");
    let p = match_paints(LINE, None, &find, true, &colors());
    assert_eq!(p.find.len(), 2);
    assert_ne!(p.find[0].fill.h, p.find[1].fill.h);
    // The longer Phrase wins where groups overlap.
    let find = FocusFind::with_query("tim || timeout");
    let p = match_paints(LINE, None, &find, true, &colors());
    assert_eq!(p.find.len(), 1);
    assert_eq!((p.find[0].start, p.find[0].end), (0, 7));
}

#[test]
fn a_keyword_has_the_same_color_in_search_and_focus_find() {
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
    assert!(p.search.iter().all(|s| s.fill == c.search.opacity(0.55)));
    assert!(p.find.iter().all(|s| s.fill == c.find.opacity(0.6)));
}
