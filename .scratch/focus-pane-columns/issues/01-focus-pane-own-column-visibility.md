# 01: Focus Pane has its own column visibility

**What to build:** The Columns section of the context sidebar shows and hides columns for the Focus Pane only, stored per Project. The Source Column and Stream column become ordinary columns of the Focus Pane, switched in the Columns section like the others. The Canvas is not changed yet: this is the expand step, so the Canvas keeps working as before.

**Blocked by:** None (can start immediately)

**Status:** resolved (branch `focus-pane-columns`)

- [ ] Switching a column off in the Columns section hides it in the Focus Pane and not in the Canvas
- [ ] Source and Stream have a switch in the Columns section and no longer a separate display switch
- [ ] The choice is remembered per Project across restarts
- [ ] Detected Fields are listed in the Columns section and are off by default for the Focus Pane
