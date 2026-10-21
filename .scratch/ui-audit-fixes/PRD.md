Status: ready-for-agent

# PRD: UI Audit Fixes — Feedback, Safety and Clarity

Source: design audit of 2026-10-19 against Checklist Design checklists (`.scratch/design-audit-2026-10-19.md`).

## Problem Statement

People reading logs in the app often can't tell what the app is doing or what just happened:

- When a Search Expression or a Filter matches no lines, the Focus Pane goes blank. Nothing says "no matches", so a typo looks exactly like a broken or still-loading app. The Match counter shows `0/0` both when Search is empty and when it found nothing.
- When someone types the wrong password or passphrase for a Server, the secret dialog closes and nothing tells them it failed.
- The status bar shows the line count wrongly ("0,400 events" for 400 lines, "1200,000 events" past a million).
- Some destructive actions run with no confirmation and look like any other action: deleting a Server, removing a Project from the Project Switcher, and Delete in the Log Space menu (which isn't styled as dangerous).
- A successful "Forget host key" shows its message in the danger color, so it reads as an error.
- The Server form shows "Enter a name" in red the moment it opens. It reports one error at a time, at the bottom of the form rather than next to the field. It silently turns an invalid Port into 22, and an error from an earlier Save stays on screen after the field has been fixed.
- Escape closes only some dialogs (New Log Space, Confirm). The host key, secret and Servers dialogs ignore it.
- The Command Palette's "Find in Log" command opens the Filter panel but doesn't put the cursor in the Search field.
- In the Highlight Rule form, values beyond the first 160px of the value list are cut off and can't be scrolled to or picked.
- The empty canvas is one muted line of text with no icon and no explanation of what a Log Space is, and a Filter that leaves zero rows shows a blank table.
- The Column Header for an empty Module column is truncated to "MOD…", even though Dynamic Column Width promises headers never truncate.
- The Remote File picker says only "Loading…" (the same words while listing a folder and while matching a glob), shows a blank box for an empty folder, keeps the old folder's rows clickable while the next one loads, and doesn't say how many files a glob added.

## Solution

The app always tells the person what state it is in and what just happened, and makes destructive actions deliberate:

- The Focus Pane shows a clear "no matches" state with what to try next (edit the Search Expression, loosen or clear the Filter, with a one-click Clear Filter). The Match counter tells "no Search" apart from "no matches".
- A failed login reopens the secret dialog with an error that says the password or passphrase was wrong.
- The status bar formats line counts with real thousands separators.
- Deleting a Server and removing a Project ask for confirmation through the existing Confirm dialog, and destructive menu items use the danger color.
- Success messages are no longer shown in the danger color.
- Server form errors appear next to their field, only after the field loses focus or after a Save attempt, clear as soon as the field is fixed, and an invalid Port is an error instead of a silent 22.
- Escape closes every dialog, the same as Cancel.
- "Find in Log" focuses the Search field.
- The Highlight Rule value list scrolls.
- The empty canvas gets an icon and a sentence explaining Log Spaces. A Filter that leaves zero rows shows a no-results state with Clear Filter.
- Column Headers never truncate.
- The Remote File picker says what it is loading, shows an empty-folder message, clears the old list while loading, and reports how many files a glob added.

## User Stories

1. As a log reader, I want the Focus Pane to say "No lines match" when my Search Expression finds nothing, so that I know the search ran and didn't break.
2. As a log reader, I want the no-matches state to show the Search Expression that found nothing, so that I can spot a typo.
3. As a log reader, I want the no-matches state to tell me when the Filter (not the Search Expression) is what leaves no lines, so that I know which control to loosen.
4. As a log reader, I want a Clear Filter action in the no-matches state, so that I can get my lines back in one click.
5. As a log reader, I want the existing hint ("Type a Search Expression, or pick a Filter…") to stay when nothing is narrowing the Focus Pane, so that first-time use is unchanged.
6. As a log reader, I want the Match counter to show nothing (or a neutral dash) when the Search Expression is empty and `0/0` only when it found no Matches, so that I can tell the two apart.
7. As a log reader using Focus Find, I want the same no-matches wording when Focus Find finds nothing in the listed lines, so that the two find tools behave alike.
8. As a log reader using the Parsed View, I want a Filter that leaves zero rows to show a no-results message with Clear Filter instead of a blank table, so that I don't think the log is empty.
9. As a log reader, I want the status bar to read "400 lines", "12,345 lines" and "1,200,000 lines" correctly, so that I can trust the count.
10. As someone connecting to a Server, I want a wrong password to reopen the secret dialog with "Wrong password — try again", so that I know why nothing connected.
11. As someone connecting with a key, I want a wrong passphrase to reopen the dialog with "Wrong passphrase — try again", so that I don't confuse it with a network problem.
12. As someone connecting, I want the dialog to keep my "Remember" choice when it reopens after a failure, so that I don't have to set it again.
13. As someone connecting, I want a failure that isn't about the secret (host unreachable, timeout) to keep its current status-bar error instead of reopening the dialog, so that I'm not asked for a password that won't help.
14. As someone managing Servers, I want Delete on a Server to ask "Delete Server <name>?" first, so that a misclick doesn't lose its settings.
15. As someone managing Servers, I want a Server that is in use to still refuse deletion with "Used by: …", so that Remote Log Spaces don't break.
16. As someone managing Servers, I want "Forget host key" to confirm success in a neutral color, so that I don't think it failed.
17. As someone adding a Server, I want an untouched, freshly opened form to show no errors, so that I'm not scolded before I've typed.
18. As someone adding a Server, I want an error to appear next to a field once I leave it invalid, so that I know which field is wrong.
19. As someone adding a Server, I want every invalid field flagged at once when I press Save, so that I can fix them all in one pass.
20. As someone adding a Server, I want an error to disappear as soon as the field is valid, so that stale errors don't mislead me.
21. As someone adding a Server, I want a non-numeric or out-of-range Port to be an error ("Port must be 1–65535"), so that I don't connect to port 22 by accident.
22. As someone adding a Server, I want an empty Port to mean 22, so that the common case stays one less field.
23. As someone adding a Server, I want form-level errors (duplicate name, Jump Server loop, too many Jump Servers) to sit next to the field they're about (Name, Jump Server), so that I know what to change.
24. As someone adding a Server, I want each error shown with an error icon and an error border on the field, not just red text, so that it's visible without relying on color.
25. As a keyboard user, I want Escape to close the host key dialog as Cancel (reject), so that I can back out quickly.
26. As a keyboard user, I want Escape to close the secret dialog as Cancel, so that I can back out quickly.
27. As a keyboard user, I want Escape to close the Server form first and then the Servers dialog, so that one Escape doesn't throw away the whole stack.
28. As a keyboard user, I want Escape in a dialog not to also clear the Search field or other things behind it, so that closing a dialog has no side effects.
29. As someone using the Project Switcher, I want removing a Project to ask for confirmation, so that a misclick next to Rename doesn't remove it.
30. As someone using the Project Switcher, I want Remove to be disabled with a reason when only one Project is left, so that the button doesn't silently do nothing.
31. As someone using the sidebar menus, I want Delete styled in the danger color, so that it stands apart from Rename.
32. As a Command Palette user, I want "Find in Log" to put the cursor in the Search field, so that I can type right away.
33. As a Command Palette user, I want "Find in Log" not to open the Filter panel as a side effect, so that my sidebar layout doesn't change.
34. As someone making a Highlight Rule, I want the value list to scroll, so that I can pick any value of a column with many values.
35. As a new user, I want the empty canvas to show an icon, a heading and one sentence saying a Log Space groups log files read as one timeline, so that I know what to do first.
36. As a new user, I want the empty Log Space state to keep its Add files button, so that the next step is one click.
37. As a user whose file failed to open, I want the canvas to show the error in place (not only in the Notice bar), so that I don't read a failure as an empty log.
38. As a log reader, I want every Column Header shown in full (e.g. "MODULE"), even when the column holds no values, so that I know what each column is.
39. As someone picking Remote Files, I want the picker to say "Listing /var/log…" or "Matching *.log…", so that I know what it's waiting on.
40. As someone picking Remote Files, I want the previous folder's rows cleared while the next folder loads, so that I don't click a stale row.
41. As someone picking Remote Files, I want an empty folder to say "This folder is empty", so that I don't think it's still loading or broken.
42. As someone picking Remote Files, I want a glob to report "Added 3 files (1 already chosen)", so that I know what it did.
43. As someone picking Remote Files, I want the path/glob field to keep my text after a successful add, so that I can tweak the glob without retyping.
44. As someone picking Remote Files, I want folders and files told apart by icon, not just a trailing "/", so that I can scan the list.
45. As a maintainer, I want these behaviors covered by tests that drive the real AppView, so that they don't regress.

## Implementation Decisions

- **GPUI test harness (new, the one seam).** Enable gpui's `test-support` feature for dev builds and drive the real `AppView` inside a `VisualTestContext` window. Tests dispatch actions and keystrokes and read state back from `AppView`'s entity. `AppView::new` must be constructible in tests: the config directory (session, `servers.json`, scratch store) must be injectable or pointed at a temp dir, and the OS keychain behind remembered secrets must be replaceable by an in-memory store. Prefer one constructor option or an environment override over threading new parameters through every call.
- **Focus Pane notice.** Replace the single optional hint string with a small enum that the Focus Pane renders: `Hint` (nothing narrowing — current text), `NoMatches { expression }` (Search Expression active, zero rows), `FilterExcludesAll` (Filter active in the Parsed View, zero rows, with a Clear Filter action), `FindNoMatches` (Focus Find active, zero finds). The view computes it from the active Focus Tab, Filter state and Focus Pane row count; ADR-0008's rules for when Filter applies (Parsed View only) decide which variant appears.
- **Match counter.** The counter shows `–` when the Search Expression is empty, `0/0` only when it found nothing. The unused `MatchNavigator` component is either wired up for this or deleted; don't keep two sources of "No matches" text.
- **Parsed View zero rows.** When Filter would leave the canvas with zero visible rows, the canvas shows the same `FilterExcludesAll` state, not an empty header. (ADR-0008 says Filter narrows only the Focus Pane; if the canvas can never actually be emptied by Filter, drop this item and note it in the issue.)
- **Line count formatting.** A pure thousands-separator formatter used by the status bar. The label reads "N lines" or keeps "events"; pick one word and use it consistently with CONTEXT.md vocabulary.
- **Secret failure.** The remote hub reports an authentication failure as a distinct `SourceStatus` (e.g. `AuthFailed`) separate from `Error(String)`. When a Remote File of a Server reaches it, the app reopens the secret dialog for that Server with `error` set ("Wrong password — try again" / "Wrong passphrase — try again") and the previous Remember choice. Any remembered secret for that Server is dropped. Other errors keep today's status-bar behavior.
- **Server form validation.** Validation returns a list of field-tagged errors (`Name`, `Host`, `Port`, `User`, `JumpServer`) instead of the first error. Port parsing moves into validation: empty → 22, otherwise must parse to 1–65535. The form tracks per-field "touched" (lost focus at least once) and "save attempted"; an error is shown for a field only when touched or after a Save attempt, and is recomputed from current values each render so it clears as soon as the field is valid. The stored "last Save error" no longer overrides live validation; store-level errors (`InUse`, `NotFound`) stay as a form-level message.
- **Errors shown with icon and field state.** Field errors render under their field with an error icon and the input in its error state; the shared error row is reused by New Log Space.
- **Notice tone.** Messages in the Servers dialog carry a tone (`Info` / `Error`); "Forget host key" uses `Info`.
- **Confirm before delete.** Deleting a Server and removing a Project go through the existing Confirm dialog (the same one Log Space deletion uses). Remove Project is disabled, with a tooltip, when it is the last Project.
- **Destructive menu items.** Delete items in the Log Space, Source File and Project menus use the theme's danger color.
- **Escape.** Every dialog handles Escape as its Cancel, innermost first (Server form, then Servers dialog). The host key dialog's Cancel is "reject". Escape is consumed by the dialog and doesn't reach the Search field.
- **Find in Log.** The command calls the same path as ⌘F that focuses the Search field; it no longer toggles the Filter panel.
- **Highlight Rule value list.** The list scrolls (same pattern as the Filter value list) instead of hiding overflow.
- **Empty canvas.** Icon + heading + one supporting sentence + the existing action, for both "No Log Space open" and "This Log Space has no files". When a Source File failed to load, the canvas shows that error in place.
- **Column Header width.** Dynamic Column Width's floor for a column is at least the width of its full Column Header text, including when the column has no values. Respect ADR-0003 (monospace ingestion scan); this only raises the floor.
- **Remote File picker.** Loading text names the operation and path. The row list is cleared when a new listing starts. An empty folder shows a message. A glob add reports added and skipped counts. The path/glob field keeps its text after a successful add. Rows get a folder or file icon.

## Testing Decisions

- **Good tests check external behavior.** Drive the app the way a person would (dispatch the action or keystroke, type into the input) and assert on what the person would see: the Focus Pane notice variant, which dialog is open, the field errors listed, whether a Server still exists, the status-bar text. Don't assert on private fields, render trees or pixel layout.
- **Primary seam: `AppView` in a GPUI `VisualTestContext`.** New integration tests under `tests/` cover: Focus Pane notice states (hint, no matches, Filter excludes all, Focus Find no matches) and the Match counter; Escape on each dialog and that it doesn't clear the Search field; Server delete and Project remove going through Confirm (and Cancel keeping them); Server form errors (none on open, shown after blur and after Save, all at once on Save, cleared when fixed, Port validation); Find in Log focusing the Search field; "Forget host key" message tone.
- **Secondary seam (existing): remote hub with a local sshd.** Extend the env-gated remote hub tests (`SSH_TEST_DIR` / `SSH_TEST_PORT`, started by `scripts/ssh-test-env.sh`) with a wrong key passphrase reaching `AuthFailed`. Password login can't be tested locally (no PAM), so the password path is covered only through the AppView seam with a hub stub reporting `AuthFailed`.
- **Pure checks where the logic is already pure.** The line-count formatter and field-tagged Server validation are asserted directly too, in the style of `search_state_test` and the existing Server store tests.
- **Prior art:** `tests/search_state_test.rs` and `tests/focus_tabs_test.rs` (Search and Focus Tab state), `tests/new_space_test.rs` (dialog validation), `tests/remote_hub_test.rs` / `tests/remote_source_test.rs` (env-gated SSH), `tests/project_test.rs` (Project removal rules). There are no GPUI-driven tests yet; the first one sets the pattern.
- **Manual GUI smoke** (seeded session via `examples/seed_session` and `XDG_CONFIG_HOME`) for purely visual items: danger-colored menu items, error icons and borders, empty-canvas icon, Column Header width, picker icons.

## Out of Scope

- Light theme or a theme switch.
- Exporting rows to a file, and a Copy item in the cell context menu.
- Reordering columns or hiding columns other than the Source Column.
- Recent or saved Search Expressions, and recent paths in the Remote File picker.
- Type-to-filter in the Command Palette.
- A progress bar for remote Download and a spinner for connection status (the picker's loading text is in scope; a broader loading-indicator pass isn't).
- Indicator for the background backfill of older Remote File lines.
- Responsive sizing of dialogs on very small windows.
- Tooltips on icon-only buttons and color swatches beyond the ones named above.

## Further Notes

- The audit's ❔ items (contrast of tabs and menus, menu positioning near window edges) need screenshots; recheck them after this lands using the new harness or a manual smoke.
- If the GPUI harness turns out too heavy to construct `AppView` (keychain, file watchers, SSH hub threads), the fallback is to test at the view-model level the issue names, but record that decision in the issue before falling back.
- Suggested slicing into issues: (01) GPUI test harness + first test, (02) Focus Pane notice + Match counter + Parsed View zero rows, (03) secret failure, (04) Server form validation, (05) confirmations, danger styling and notice tone, (06) Escape everywhere + Find in Log, (07) small fixes: line count, Highlight Rule list scroll, Column Header width, empty canvas, (08) Remote File picker feedback.
