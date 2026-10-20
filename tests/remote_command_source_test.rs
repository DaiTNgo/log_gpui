//! Needs a local sshd with the stand-in tools of `scripts/ssh-test-env.sh`:
//! `SSH_TEST_DIR`, `SSH_TEST_PORT` (see `remote_hub_test.rs`).

use log_gpui::live::{
    BreakKind, LineSource, RemoteCommandSource, RestartKind, SourceCommand, SourceEvent, SourceHandle,
    SourceStatus,
};
use log_gpui::model::Stream;
use log_gpui::remote::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Env {
    hub: Arc<RemoteHub>,
    store: ServerStore,
    server_id: String,
    dir: String,
}

fn env() -> Option<Env> {
    let dir = std::env::var("SSH_TEST_DIR").ok()?;
    let port: u16 = std::env::var("SSH_TEST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let user = std::env::var("USER").ok()?;
    let mut server = Server::new("web1", "127.0.0.1", &user, AuthMethod::Key { path: format!("{dir}/userkey") });
    server.port = port;
    let mut store = ServerStore::default();
    let server_id = store.save(server).unwrap();
    let (hub, requests) = RemoteHub::new(Arc::new(MemorySecrets::default()), Arc::new(Mutex::new(KnownHosts::default())));
    std::thread::spawn(move || {
        while let Ok(req) = requests.recv_blocking() {
            let _ = req.reply.send(true);
        }
    });
    Some(Env { hub, store, server_id, dir })
}

impl Env {
    fn start(&self, command: &str, preset: Option<PresetFields>, restart_on_exit: bool, window: usize) -> SourceHandle {
        RemoteCommandSource {
            hub: self.hub.clone(),
            servers: Arc::new(Mutex::new(self.store.clone())),
            server_id: self.server_id.clone(),
            command: command.to_string(),
            preset,
            restart_on_exit,
            name: format!("web1$ {command}"),
            window,
        }
        .spawn()
    }

    fn plain(&self, command: &str) -> SourceHandle {
        self.start(command, None, false, 100_000)
    }

    /// A line of the stand-in tools' "logs".
    fn append_fake(&self, file: &str, lines: &[&str]) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/fake/{file}", self.dir))
            .unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
    }

    fn drop_sessions(&self) {
        let pid = std::fs::read_to_string(format!("{}/sshd.pid", self.dir)).unwrap();
        let status = std::process::Command::new("pkill").args(["-P", pid.trim()]).status().unwrap();
        assert!(status.success() || status.code() == Some(1));
    }
}

fn next_within(h: &SourceHandle, secs: u64, mut want: impl FnMut(&SourceEvent) -> bool) -> SourceEvent {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        match h.events.try_recv() {
            Ok(e) if want(&e) => return e,
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn next(h: &SourceHandle, want: impl FnMut(&SourceEvent) -> bool) -> SourceEvent {
    next_within(h, 15, want)
}

fn is_append(e: &SourceEvent) -> bool {
    matches!(e, SourceEvent::Append { .. })
}

fn is_break(kind: BreakKind) -> impl FnMut(&SourceEvent) -> bool {
    move |e| *e == SourceEvent::Break { kind }
}

/// Lines (with their Stream) until `stop` is seen.
fn lines_until(h: &SourceHandle, mut stop: impl FnMut(&SourceEvent) -> bool) -> Vec<(Stream, String)> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match h.events.try_recv() {
            Ok(SourceEvent::Append { lines, streams, .. }) => {
                let streams = streams.expect("a command says its streams");
                assert_eq!(streams.len(), lines.len());
                for (i, line) in lines.into_iter().enumerate() {
                    seen.push((streams.get(i).unwrap(), line));
                }
            }
            Ok(e) if stop(&e) => return seen,
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out; saw {seen:?}");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn exited(code: Option<u32>) -> impl FnMut(&SourceEvent) -> bool {
    move |e| *e == SourceEvent::Status(SourceStatus::Exited(code))
}

fn sorted(mut v: Vec<(Stream, String)>) -> Vec<(Stream, String)> {
    v.sort_by(|a, b| a.1.cmp(&b.1));
    v
}

#[test]
fn output_becomes_lines_each_with_its_stream_and_the_last_line_needs_no_newline() {
    let Some(env) = env() else { return };
    let h = env.plain("printf 'a\\nb\\n'; printf 'oops\\n' 1>&2; printf 'last'");
    let lines = lines_until(&h, exited(Some(0)));
    assert_eq!(
        sorted(lines),
        vec![
            (Stream::Stdout, "a".to_string()),
            (Stream::Stdout, "b".to_string()),
            (Stream::Stdout, "last".to_string()),
            (Stream::Stderr, "oops".to_string()),
        ]
    );
    h.close();
}

#[test]
fn a_partial_line_is_not_shown_until_its_newline_comes() {
    let Some(env) = env() else { return };
    let h = env.plain("printf 'par'; sleep 1; printf 'tial\\n'; sleep 60");
    next(&h, |e| *e == SourceEvent::Status(SourceStatus::Live));
    let started = Instant::now();
    let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
    assert_eq!(lines, vec!["partial"]);
    assert!(started.elapsed() >= Duration::from_millis(800), "shown before it was whole");
    h.close();
}

#[test]
fn the_exit_code_is_shown_as_a_break_and_a_status() {
    let Some(env) = env() else { return };
    let h = env.plain("echo before; exit 3");
    next(&h, is_break(BreakKind::Exited(Some(3))));
    next(&h, exited(Some(3)));
    h.close();
}

#[test]
fn a_command_that_does_not_exist_exits_127_and_the_shell_error_is_a_line() {
    let Some(env) = env() else { return };
    let h = env.plain("definitely-not-a-command-xyz");
    let lines = lines_until(&h, exited(Some(127)));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, Stream::Stderr);
    assert!(lines[0].1.contains("not found"), "{lines:?}");
    h.close();
}

#[test]
fn a_command_that_reads_its_input_sees_it_end() {
    let Some(env) = env() else { return };
    let h = env.plain("cat; echo done");
    let lines = lines_until(&h, exited(Some(0)));
    assert_eq!(lines, vec![(Stream::Stdout, "done".to_string())]);
    h.close();
}

#[test]
fn restart_runs_the_command_again_after_a_break_and_ends_the_old_process() {
    let Some(env) = env() else { return };
    let tag = std::process::id();
    let h = env.plain(&format!("echo hi; sleep 60.{tag}"));
    assert_eq!(lines_until(&h, |e| matches!(e, SourceEvent::Status(SourceStatus::Live))).len(), 0);
    next(&h, is_append);
    h.send(SourceCommand::Restart);
    next(&h, is_break(BreakKind::Restarted(RestartKind::Manual)));
    let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
    assert_eq!(lines, vec!["hi"], "the new Run prints from its start");
    // One process on the Server, not two.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(sleepers(&env, tag), 1, "the old process was asked to stop");
    h.close();
}

/// How many processes on the Server run `sleep 60.<tag>` (a command line
/// that only the process under test has).
fn sleepers(env: &Env, tag: u32) -> usize {
    let hub = env.hub.clone();
    let store = env.store.clone();
    let id = env.server_id.clone();
    hub.handle().block_on(async move {
        let session = hub.session(&store, &id).await.unwrap();
        // Lines with "grep" are the commands of this very check.
        let out = session
            .exec(&format!("ps -axo command | grep -F 'sleep 60.{tag}' | grep -vc grep; true"))
            .await
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    })
}

#[test]
fn restart_after_the_command_exited_runs_it_again() {
    let Some(env) = env() else { return };
    let h = env.plain("echo once");
    next(&h, exited(Some(0)));
    h.send(SourceCommand::Restart);
    next(&h, is_break(BreakKind::Restarted(RestartKind::Manual)));
    let lines = lines_until(&h, exited(Some(0)));
    assert_eq!(lines, vec![(Stream::Stdout, "once".to_string())]);
    h.close();
}

#[test]
fn restart_on_exit_runs_the_command_again_after_a_second() {
    let Some(env) = env() else { return };
    let h = env.start("echo x", None, true, 100_000);
    next(&h, exited(Some(0)));
    let began = Instant::now();
    next(&h, is_break(BreakKind::Restarted(RestartKind::OnExit)));
    assert!(began.elapsed() >= Duration::from_millis(800), "waited the backoff");
    next(&h, is_append);
    h.close();
}

#[test]
fn restart_on_exit_gives_up_after_five_quick_failures() {
    let Some(env) = env() else { return };
    let h = env.start("exit 1", None, true, 100_000);
    let mut runs = 0;
    // Waits of 1, 2, 4 and 8 seconds between the five Runs.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match h.events.try_recv() {
            Ok(SourceEvent::Run { .. }) => runs += 1,
            Ok(SourceEvent::Status(SourceStatus::Exited(Some(1)))) if runs == 5 => break,
            Ok(_) => {}
            Err(_) => {
                assert!(Instant::now() < deadline, "timed out after {runs} runs");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    std::thread::sleep(Duration::from_secs(3));
    while let Ok(e) = h.events.try_recv() {
        assert!(!matches!(e, SourceEvent::Run { .. }), "ran a sixth time");
    }
    // The user can still start it.
    h.send(SourceCommand::Restart);
    next(&h, |e| matches!(e, SourceEvent::Run { .. }));
    h.close();
}

#[test]
fn pause_keeps_the_command_running_and_hands_over_the_lines_on_resume() {
    let Some(env) = env() else { return };
    let h = env.plain("sleep 1; echo a; echo b; echo c; sleep 60");
    next(&h, |e| *e == SourceEvent::Status(SourceStatus::Live));
    h.send(SourceCommand::Pause);
    std::thread::sleep(Duration::from_millis(2_500));
    while let Ok(e) = h.events.try_recv() {
        assert!(!is_append(&e), "lines came through while paused");
    }
    h.send(SourceCommand::Resume);
    let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
    assert_eq!(lines, vec!["a", "b", "c"]);
    h.close();
}

#[test]
fn a_long_pause_keeps_only_the_live_window_and_says_so() {
    let Some(env) = env() else { return };
    let h = env.start("sleep 1; seq 1 10; sleep 60", None, false, 3);
    next(&h, |e| *e == SourceEvent::Status(SourceStatus::Live));
    h.send(SourceCommand::Pause);
    std::thread::sleep(Duration::from_millis(2_500));
    h.send(SourceCommand::Resume);
    let skipped = next(&h, |e| matches!(e, SourceEvent::Break { kind: BreakKind::Skipped(_) }));
    assert_eq!(skipped, SourceEvent::Break { kind: BreakKind::Skipped(7) });
    let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
    assert_eq!(lines, vec!["8", "9", "10"]);
    h.close();
}

#[test]
fn closing_ends_the_process_on_the_server() {
    let Some(env) = env() else { return };
    let tag = std::process::id() + 1;
    let h = env.plain(&format!("echo up; sleep 60.{tag}"));
    next(&h, is_append);
    assert_eq!(sleepers(&env, tag), 1);
    h.close();
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(sleepers(&env, tag), 0, "the process was asked to stop");
}

// ── Reconnecting ────────────────────────────────────────────────────────────

const T1: &str = "2026-10-19T03:02:01.100000000Z";
const T2: &str = "2026-10-19T03:02:02.200000000Z";
const T3: &str = "2026-10-19T03:02:03.300000000Z";
const T4: &str = "2026-10-19T03:02:04.400000000Z";
const T5: &str = "2026-10-19T03:02:05.500000000Z";

#[test]
fn an_unedited_preset_resumes_after_a_lost_connection_without_losing_or_repeating_a_line() {
    let Some(env) = env() else { return };
    let log = "docker-api.log";
    let _ = std::fs::remove_file(format!("{}/fake/{log}", env.dir));
    env.append_fake(log, &[&format!("{T1} one"), &format!("{T2} two"), &format!("{T3} three")]);
    let fields = PresetFields::docker("api");
    let command = fields.command();
    let h = env.start(&command, Some(fields), false, 100_000);
    let mut got = Vec::new();
    while got.len() < 3 {
        let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
        got.extend(lines);
    }
    assert_eq!(got.len(), 3);

    env.drop_sessions();
    env.append_fake(log, &[&format!("{T4} four"), &format!("{T5} five")]);
    next(&h, is_break(BreakKind::Disconnected));
    next(&h, |e| *e == SourceEvent::Status(SourceStatus::Reconnecting));
    let run = next(&h, |e| matches!(e, SourceEvent::Run { command } if command.contains("--since")));
    let SourceEvent::Run { command: resumed } = run else { unreachable!() };
    assert_eq!(resumed, format!("docker logs -f --timestamps --since {T3} 'api'"));
    let resumed_break = next(&h, |e| matches!(e, SourceEvent::Break { kind: BreakKind::Resumed(_) }));
    // 03:02:03 on the Server's clock (it wrote Z).
    assert_eq!(resumed_break, SourceEvent::Break { kind: BreakKind::Resumed(3 * 3600 + 2 * 60 + 3) });
    let mut after = Vec::new();
    while after.len() < 2 {
        let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
        after.extend(lines);
    }
    assert_eq!(after, vec![format!("{T4} four"), format!("{T5} five")], "no loss, no repeat");
    h.close();
}

#[test]
fn a_typed_command_is_run_again_as_typed_after_a_lost_connection() {
    let Some(env) = env() else { return };
    let h = env.plain("echo hello; sleep 30");
    next(&h, is_append);
    env.drop_sessions();
    next(&h, is_break(BreakKind::Disconnected));
    next(&h, is_break(BreakKind::Restarted(RestartKind::AfterDisconnect)));
    let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
    assert_eq!(lines, vec!["hello"], "a new Run from the start");
    h.close();
}

#[test]
fn a_tool_that_does_not_know_since_runs_as_written_once_and_remembers() {
    let Some(env) = env() else { return };
    let dir = &env.dir;
    std::fs::write(format!("{dir}/fake/kube-pod1.log"), format!("{T1} from the pod\n")).unwrap();
    let fields = PresetFields::kube("pod1", "", "", "");
    let command = fields.command();
    // `kubectl` here ends after printing (no follow); Restart on exit resumes it.
    let h = env.start(&command, Some(fields), true, 100_000);
    next(&h, exited(Some(0)));
    let problem = next(&h, |e| matches!(e, SourceEvent::Problem(_)));
    assert_eq!(problem, SourceEvent::Problem("resume not supported".into()));
    next(&h, is_break(BreakKind::Restarted(RestartKind::ResumeFailed)));
    let mut seen = Vec::new();
    // The error text of the unknown flag is never shown as a log line.
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.is_empty() && Instant::now() < deadline {
        if let Ok(SourceEvent::Append { lines, .. }) = h.events.try_recv() {
            seen.extend(lines);
        } else {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    assert_eq!(seen, vec![format!("{T1} from the pod")]);
    // From now on it runs as written: no more tries with --since-time.
    let run = next(&h, |e| matches!(e, SourceEvent::Run { .. }));
    let SourceEvent::Run { command: next_command } = run else { unreachable!() };
    assert!(!next_command.contains("--since"), "{next_command}");
    h.close();
}

#[test]
fn clear_does_not_stop_the_run_and_resuming_by_time_still_starts_at_the_last_line_received() {
    let Some(env) = env() else { return };
    let log = "docker-clear.log";
    let _ = std::fs::remove_file(format!("{}/fake/{log}", env.dir));
    env.append_fake(log, &[&format!("{T1} one"), &format!("{T2} two"), &format!("{T3} three")]);
    let fields = PresetFields::docker("clear");
    let command = fields.command();
    let h = env.start(&command, Some(fields), false, 100_000);
    let mut got = Vec::new();
    while got.len() < 3 {
        let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
        got.extend(lines);
    }

    // The app drops every line it holds: the Run goes on, no Break, no restart.
    h.send(SourceCommand::Clear);
    env.append_fake(log, &[&format!("{T4} four")]);
    let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
    assert_eq!(lines, vec![format!("{T4} four")], "the same Run still delivers");

    // The connection is lost with nothing held: resuming starts after the last
    // line received, not from the lines that are held (none).
    env.drop_sessions();
    env.append_fake(log, &[&format!("{T5} five")]);
    next(&h, is_break(BreakKind::Disconnected));
    let run = next(&h, |e| matches!(e, SourceEvent::Run { command } if command.contains("--since")));
    let SourceEvent::Run { command: resumed } = run else { unreachable!() };
    assert_eq!(resumed, format!("docker logs -f --timestamps --since {T4} 'clear'"));
    let mut after = Vec::new();
    while after.is_empty() {
        let SourceEvent::Append { lines, .. } = next(&h, is_append) else { unreachable!() };
        after.extend(lines);
    }
    assert_eq!(after, vec![format!("{T5} five")], "no loss, no repeat");
    h.close();
}
