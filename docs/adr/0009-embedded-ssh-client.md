# 0009. Remote Files Are Read Through An Embedded SSH Client

Remote Log Spaces read Remote Files through an SSH client built into the app, not by running the machine's `ssh` command. The app then behaves the same on every machine whatever `ssh` binary, `~/.ssh/config` or agent is installed there, at the cost of re-implementing what OpenSSH gives for free: the app keeps its own list of trusted host keys (confirm the fingerprint on first connect, refuse a changed key), its own saved **Servers** shared by every Project, its own **Jump Server** support, and stores passwords and key passphrases in the macOS Keychain. The first version signs in with a private key or a password; ssh-agent and keyboard-interactive (OTP/2FA) are left for later.

## Considered Options

- **Run the system `ssh` (`ssh host tail -F path`)**: rejected. Far less code and reuses the user's config, agent, ProxyJump and known_hosts, but ties behaviour to whatever is installed and configured on each machine.

## Consequences

- Aliases, ProxyJump and other settings in `~/.ssh/config` are not used; each Server and Jump Server is entered in the app.
- Servers that only allow agent or 2FA sign-in cannot be reached until those are added.
