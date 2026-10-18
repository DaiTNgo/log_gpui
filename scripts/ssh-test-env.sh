#!/bin/sh
# Starts a throwaway sshd on 127.0.0.1:2222 for the SSH integration tests and
# prints the environment to run them with:
#
#   eval "$(scripts/ssh-test-env.sh start)"
#   cargo test --test remote_hub_test --test remote_source_test
#   scripts/ssh-test-env.sh stop
#
# Needs /usr/sbin/sshd and /usr/libexec/sftp-server (macOS has both). The
# server accepts the keys it generates (`userkey`, and `userkey_pp` with the
# passphrase `secretpass`), allows TCP forwarding (for Jump Servers), and
# has no password login (it has no PAM).
set -e
DIR="${SSH_TEST_DIR:-${TMPDIR:-/tmp}/log_gpui_sshd}"
PORT="${SSH_TEST_PORT:-2222}"
case "$1" in
start)
  rm -rf "$DIR"; mkdir -p "$DIR"
  ssh-keygen -q -t ed25519 -N "" -f "$DIR/hostkey"
  ssh-keygen -q -t ed25519 -N "secretpass" -f "$DIR/userkey_pp"
  ssh-keygen -q -t ed25519 -N "" -f "$DIR/userkey"
  cat "$DIR/userkey.pub" "$DIR/userkey_pp.pub" > "$DIR/authorized_keys"
  chmod 600 "$DIR/authorized_keys"
  cat > "$DIR/sshd_config" <<CONF
Port $PORT
ListenAddress 127.0.0.1
HostKey $DIR/hostkey
PidFile $DIR/sshd.pid
AuthorizedKeysFile $DIR/authorized_keys
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
UsePAM no
StrictModes no
AllowTcpForwarding yes
Subsystem sftp /usr/libexec/sftp-server
LogLevel ERROR
CONF
  /usr/sbin/sshd -f "$DIR/sshd_config" -E "$DIR/sshd.log"
  echo "export SSH_TEST_DIR=$DIR SSH_TEST_PORT=$PORT"
  ;;
stop)
  [ -f "$DIR/sshd.pid" ] && kill "$(cat "$DIR/sshd.pid")" && rm -rf "$DIR"
  ;;
*)
  echo "usage: $0 start|stop" >&2; exit 1 ;;
esac
