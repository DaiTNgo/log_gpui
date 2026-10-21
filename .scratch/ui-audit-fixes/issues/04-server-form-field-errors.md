# 04: Server form shows errors next to each field, at the right time

**What to build:** Adding or editing a Server no longer greets the person with "Enter a name" in red. An error appears under its field, with an error icon and the field in its error state, once the field has lost focus or after Save is pressed. Save flags every invalid field at once, and an error disappears as soon as its field is valid. An invalid Port is an error instead of silently becoming 22.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness).

**Status:** resolved

- [x] Server validation returns all errors, each tagged with its field (Name, Host, Port, User, Jump Server), instead of only the first.
- [x] Port: empty means 22; anything else must be a number from 1 to 65535, otherwise "Port must be 1–65535".
- [x] Duplicate name sits on Name; Jump Server loop and too many Jump Servers sit on Jump Server.
- [x] A freshly opened form shows no errors; a field's error shows after it loses focus or after a Save attempt.
- [x] Errors are recomputed from current values, so they clear as soon as the field is valid; an old Save error no longer overrides them. Store-level errors (in use, not found) stay as a form-level message.
- [x] Field errors render with an icon and the input's error state; New Log Space's name error uses the same row.
- [x] Tests: field-tagged validation directly (all errors returned, Port rules); harness tests for no errors on open, error after blur, all errors on Save, error clears when fixed.

## Comments

- 2026-10-19: Done in 8bdc62c. `ServerStore::validate_all` returns every error, each with `ServerError::field()`; `parse_port` makes empty mean 22 and anything else 1–65535. The form tracks touched fields (Blur) and Save attempts; `visible_server_form_errors` shows a field's error only then, recomputed from the current values. Save is no longer disabled: pressing it reveals every error. Errors render under their field via the shared `field_error_row` (icon + text) with a danger border; New Log Space's name error uses it too. `dialog.error` is now only for whole-Server errors (a failed save).
- Blur is driven through `touch_server_field` in tests (the subscription calls it), not a real focus change.
- Incident: `a_valid_form_saves_with_its_port` saved a Server through code that writes `servers.json` in the real config dir, overwriting the user's file. Fixed in e9631f7: the harness gives the test process its own `XDG_CONFIG_HOME` and refuses to run otherwise. The user's file was reset to an empty list at their request; the previous Servers could not be recovered.
