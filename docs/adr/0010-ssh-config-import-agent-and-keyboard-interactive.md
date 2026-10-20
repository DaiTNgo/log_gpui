# 0010. Servers May Be Imported From ~/.ssh/config; Agent And Keyboard-Interactive Sign-in

This partly replaces [0009](0009-embedded-ssh-client.md). The SSH client stays embedded, the Server list stays the app's own, and the app keeps its own known hosts. What changes is how Servers get made and how they sign in:

- **`~/.ssh/config` is imported on request, never followed.** The user imports `Host` entries (HostName, User, Port, IdentityFile, IdentityAgent, ProxyJump) as ordinary Servers and Jump Servers, and can refresh them later; the file is read only then, never while connecting and never written. This keeps 0009's point (behaviour does not depend on the machine's configuration at run time) while saving the retyping.
- **ssh-agent sign-in** (`SSH_AUTH_SOCK`, 1Password, Secretive or a chosen socket). The app only asks the agent to sign; private keys never reach it.
- **Keyboard-interactive** (OTP/2FA), alone or after a key or the agent. The server's questions are shown as asked and the answers are never stored. A Server that needs them is not reconnected by itself.

## Considered Options

- **Read `~/.ssh/config` on every connection, like OpenSSH**: rejected. A Server would change behaviour silently whenever the file changes, and `Match`, `Include` and tokens would all have to be implemented exactly to be right.

## Consequences

- `ProxyCommand`, `Match` blocks and options outside the mapped list are not supported; the import lists what it skipped. OpenSSH's own `known_hosts` is still not used.
- A Server that needs a Sign-in Prompt waits for the user after a lost connection or when a Project is reopened (rule L17), unlike other Servers (L6).
- `servers.json` gets new fields with defaults. Before the first Server using Agent or keyboard-interactive is written, the old file is kept as `servers.v1.json` so the change can be rolled back.
