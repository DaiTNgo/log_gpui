# 03: Home folder and local user resolve on Windows for Servers

**What to build:** On Windows, importing Servers from `~/.ssh/config` finds `%USERPROFILE%\.ssh\config`, and `~/…` in identity files, `IdentityAgent` and key paths expands to the user's profile folder. `%u` in ssh config and the default user for Quick Connect come from `%USERNAME%`. Today these read `HOME`/`USER` only, which Windows usually does not set, so `~` expands to an empty string.

Use the app's single home-folder helper (which already falls back to `USERPROFILE`) everywhere instead of reading `HOME` directly.

**Blocked by:** 01 (Windows build goes green on CI)

**Status:** resolved (windows-build-integration)

Note: touches the same ssh-config reading code as 04; don't run both in parallel on the same branch.

- [ ] With only `USERPROFILE`/`USERNAME` set, ssh config import lists the user's Hosts
- [ ] `~` and `~/…` expand to the profile folder in key paths and agent paths
- [ ] `%u` and the Quick Connect default user use the Windows user name
- [ ] Existing Unix behaviour (and tests using `HOME`/`USER`) unchanged
