#!/usr/bin/env python3
"""Strip every explicit theme/colour styling from the UI so gpui-kit defaults apply.

Removes, from all `src/**/*.rs`:
  * style calls whose arguments reference `theme.`:
      .bg(..) .text_color(..) .border_color(..) .hover(..) .group_hover(..)
  * the colour argument of `render_icon(svg, size, <colour>)`
  * `let theme = cx.theme().colors;` bindings and the `ActiveTheme` imports

Usage:
    python3 scripts/strip_theme.py            # dry run: prints what would change
    python3 scripts/strip_theme.py --apply    # rewrite the files

Anything left that still mentions `theme.` (data-driven colours such as log-level
badges) must be removed by hand; `cargo check` points at it.
"""
import glob
import re
import sys

STYLE_CALLS = ("bg", "text_color", "border_color", "hover", "group_hover")


def match_paren(s, i):
    """Index of the ')' closing the '(' at s[i]."""
    depth = 0
    in_str = False
    j = i
    while j < len(s):
        c = s[j]
        if in_str:
            if c == "\\":
                j += 1
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return j
        j += 1
    raise ValueError("unbalanced parentheses")


def split_args(args):
    out, depth, cur, in_str = [], 0, "", False
    i = 0
    while i < len(args):
        c = args[i]
        if in_str:
            cur += c
            if c == "\\":
                i += 1
                cur += args[i]
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
            cur += c
        elif c in "([{":
            depth += 1
            cur += c
        elif c in ")]}":
            depth -= 1
            cur += c
        elif c == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        out.append(cur)
    return out


def strip_style_calls(s):
    pat = re.compile(r"\s*\.(%s)\(" % "|".join(STYLE_CALLS))
    pos = 0
    while True:
        m = pat.search(s, pos)
        if not m:
            return s
        open_i = m.end() - 1
        close_i = match_paren(s, open_i)
        if "theme." in s[open_i:close_i]:
            s = s[: m.start()] + s[close_i + 1 :]
            pos = m.start()
        else:
            pos = m.end()


def strip_icon_colour(s):
    pat = re.compile(r"(?<!fn )\brender_icon\(")
    pos = 0
    while True:
        m = pat.search(s, pos)
        if not m:
            return s
        open_i = m.end() - 1
        close_i = match_paren(s, open_i)
        args = split_args(s[open_i + 1 : close_i])
        if len(args) == 3:
            new = "render_icon(" + ",".join(args[:2]) + ")"
            s = s[:m.start()] + new + s[close_i + 1 :]
            pos = m.start() + len(new)
        else:
            pos = m.end()


def process(s):
    s = strip_style_calls(s)
    s = strip_icon_colour(s)
    s = re.sub(r"\n[ \t]*let theme = cx\.theme\(\)\.colors;", "", s)
    s = re.sub(r"\n[ \t]*use gpui_component::ActiveTheme as _;", "", s)
    return s


def main():
    apply = "--apply" in sys.argv
    changed = 0
    for path in sorted(glob.glob("src/**/*.rs", recursive=True)):
        src = open(path).read()
        out = process(src)
        if out != src:
            changed += 1
            left = out.count("theme.")
            print(f"{path}: {src.count('theme.')} -> {left} remaining `theme.` references")
            if apply:
                open(path, "w").write(out)
    print(f"{changed} file(s) {'rewritten' if apply else 'would change (dry run)'}")


if __name__ == "__main__":
    main()
