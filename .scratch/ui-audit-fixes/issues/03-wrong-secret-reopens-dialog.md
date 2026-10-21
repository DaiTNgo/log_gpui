# 03: Wrong password or passphrase reopens the secret dialog with an error

**What to build:** When someone connects to a Server with the wrong password or key passphrase, the secret dialog comes back saying "Wrong password — try again" or "Wrong passphrase — try again", with their Remember choice kept. Failures that aren't about the secret (host unreachable, timeout) keep today's status-bar error and don't reopen the dialog.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness).

**Status:** resolved

- [x] The remote hub reports an authentication failure as its own source status (e.g. `AuthFailed`), separate from the generic error.
- [x] When a Remote File of a Server reaches it, the app reopens the secret dialog for that Server with the right message and the previous Remember choice.
- [x] A remembered secret for that Server is dropped after an auth failure.
- [x] Non-auth failures still show in the status bar and don't open the dialog.
- [x] Env-gated remote hub test (`scripts/ssh-test-env.sh`, `SSH_TEST_DIR`/`SSH_TEST_PORT`): a wrong key passphrase reaches `AuthFailed`; the test is skipped when the env is unset.
- [x] Harness test with a stub hub reporting `AuthFailed`: the dialog reopens with the error and Remember kept (covers the password path, which can't be tested locally without PAM).

## Comments

- 2026-10-19: Done in d275612. `SourceStatus::AuthFailed` comes from `ConnectError::AuthFailed` (Remote Files and Remote Commands). The hub forgets the refused secret, typed and remembered (Keychain); before, a wrong passphrase stayed typed and was retried. After each batch of source events, `ask_again_after_auth_failure` reopens the secret dialog for the first Server with an AuthFailed file, says "Wrong password/passphrase — try again", and turns its files to NeedsSecret so Cancel leaves them on Connect instead of reopening the dialog.
- Tests: `tests/secret_retry_test.rs` (4 harness tests: password, passphrase, other failure, cancel) and `a_refused_passphrase_is_forgotten_so_the_user_is_asked_again` in `remote_hub_test` (ran against the local sshd: passes). `remote_source_test::load_more_reads_older_lines_above_the_oldest_held_one` failed once in the full SSH run and passed alone, with and without this change: it looks flaky under load.
