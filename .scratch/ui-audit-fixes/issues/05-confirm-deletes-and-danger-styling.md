# 05: Confirm before deleting a Server or removing a Project, and style destructive actions

**What to build:** Deleting a Server and removing a Project from the Project Switcher ask first, through the same Confirm dialog Log Space deletion uses. Remove is disabled, with a tooltip saying why, when only one Project is left. Delete items in the Log Space, Source File and Project menus use the danger color. "Forget host key" confirms success in a neutral color instead of red.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness), 04 (Server form field errors — both change the Servers dialog).

**Status:** resolved

- [x] Delete on a Server opens Confirm ("Delete Server <name>?"); Cancel keeps the Server, Confirm deletes it.
- [x] A Server in use still refuses deletion with "Used by: …".
- [x] Removing a Project from the Project Switcher opens Confirm; Cancel keeps it.
- [x] Remove is disabled with a tooltip when it's the last Project.
- [x] Messages in the Servers dialog carry a tone (info or error); "Forget host key" uses info.
- [ ] Delete menu items use the theme's danger color (checked by GUI smoke).
- [x] Harness tests: Server delete and Project remove go through Confirm, Cancel keeps them, last-Project Remove is disabled, Forget host key's message isn't an error.

## Comments

- 2026-10-19: Done in 074ba19. New `ConfirmAction::DeleteServer` / `RemoveProject`; the Confirm overlay now draws after every dialog, so it shows above the Servers dialog. `request_delete_server` refuses a Server in use at once ("Used by: …") without asking. `request_remove_project` does nothing for the last Project; the Project Switcher's × is dimmed with a tooltip, and the Sidebar's "Remove from Sidebar" item is disabled. `ServerDialog.info` holds non-error messages (Forget host key), shown muted.
- Menu items Delete (Log Space, scratch file) and Remove from Sidebar use `space_menu::danger_item` (danger-colored label). Not checked by GUI smoke: opening a menu needs clicks. That box stays open.
- Verified in a clean worktree of main + these files (602 passed), because the shared tree was mid-edit by another session (AuthMethod variants).
