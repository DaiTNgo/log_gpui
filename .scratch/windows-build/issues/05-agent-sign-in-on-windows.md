# 05: Agent sign-in on Windows

**What to build:** A Server using Agent auth signs in on Windows. With no `SSH_AUTH_SOCK`, the System agent means the Windows OpenSSH agent's named pipe (`\\.\pipe\openssh-ssh-agent`), which 1Password also serves. Pageant is offered as a choice. The 1Password preset resolves to the right pipe on Windows; the Secretive preset (macOS-only) is hidden. The preferred-key filtering and "agent has no keys" / "preferred key is not in the agent" errors behave as on macOS.

Replaces the "agent unavailable" stub from 01.

**Blocked by:** 01 (Windows build goes green on CI)

**Status:** ready-for-agent

- [ ] Agent sign-in works through the OpenSSH named pipe on Windows
- [ ] Agent sign-in works through Pageant
- [ ] The Server dialog shows only agent presets that exist on the platform (no Secretive on Windows); 1Password resolves to its Windows pipe
- [ ] An explicit `IdentityAgent` pipe path from ssh config is honoured
- [ ] Missing agent still reports "agent unavailable" with the path that was tried
