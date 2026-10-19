//! `RemoteCommandSource`: a command run on a Server whose output is read as
//! lines, like a file that is being written.
//!
//! The command runs through an exec channel with no terminal and its input
//! closed. Each execution is a Run: a fresh Run starts a Generation (line
//! numbers from 1); a Run resumed by time after a lost connection goes on with
//! the Generation before it. A Command Preset that was not edited is resumed
//! from the time of its last line (`ResumePoint`); a typed command is run again
//! as it was typed, with a Break saying lines may be missing or repeated.

use super::resume::{ResumePoint, Verdict, parse_timestamp};
use super::source::{
    BreakKind, LineSource, RestartKind, SourceCommand, SourceEvent, SourceHandle, SourceStatus, StreamMask,
};
use super::splitter::LineSplitter;
use crate::model::Stream;
use crate::remote::{ConnectError, ExecEvent, ExecStream, PresetFields, RemoteHub, RemoteSession, ServerStore};
use async_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Reconnecting stops after this long and waits for the user.
pub const GIVE_UP_AFTER: Duration = Duration::from_secs(300);
/// First and longest wait before a Run is started again after the command
/// exited, with Restart on exit on.
pub const RESTART_BACKOFF_FIRST: Duration = Duration::from_secs(1);
pub const RESTART_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// A Run that lived this long starts the backoff over.
pub const RUN_LONG_ENOUGH: Duration = Duration::from_secs(60);
/// Runs in a row that exit with a code other than 0 within `QUICK_EXIT`
/// before Restart on exit gives up.
pub const QUICK_FAILURES_BEFORE_STOP: u32 = 5;
pub const QUICK_EXIT: Duration = Duration::from_secs(10);
/// How long a resumed Run may go without a line that has a time before it is
/// taken to have worked; until then its output is held back, so that a tool
/// that does not know the since flag shows its usage text nowhere.
pub const RESUME_PROOF_WITHIN: Duration = Duration::from_secs(3);

pub struct RemoteCommandSource {
    pub hub: Arc<RemoteHub>,
    /// The Servers as they are now (edits apply to the next connection).
    pub servers: Arc<Mutex<ServerStore>>,
    pub server_id: String,
    pub command: String,
    /// The Command Preset the command was written from, only while the
    /// command is exactly what it writes.
    pub preset: Option<PresetFields>,
    pub restart_on_exit: bool,
    /// `<server>$ <command>`.
    pub name: String,
    pub window: usize,
}

impl LineSource for RemoteCommandSource {
    fn spawn(self) -> SourceHandle {
        let (event_tx, events) = async_channel::unbounded();
        let (commands, command_rx) = async_channel::unbounded();
        let hub = self.hub.clone();
        hub.spawn(async move { self.run(event_tx, command_rx).await });
        SourceHandle { events, commands }
    }
}

/// Why serving a session ended.
enum Served {
    Closed,
    /// The connection dropped.
    Lost,
    /// Cannot go on; the user must act.
    Stopped(SourceStatus),
}

/// How the next Run starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// The first Run, as written, with no Break.
    First,
    /// The user pressed Restart: as written, a new Generation.
    Manual,
    /// The command exited and Restart on exit is on.
    OnExit,
    /// The connection came back.
    AfterDisconnect,
    /// Resuming did not work: as written, a new Generation.
    ResumeFailed,
}

/// What a wait ended with.
#[derive(Debug, PartialEq, Eq)]
enum Wake {
    Timeout,
    Restart,
    Reconnect,
    /// Restart on exit was turned on or off.
    Changed,
    Closed,
}

/// How a Run ended.
enum RunEnd {
    Closed,
    Restart,
    /// The Run was a resume and the tool did not take its since flag.
    ResumeFailed,
    Exited(Option<u32>),
}

/// How a Run is started.
struct RunStart {
    command: String,
    /// A resumed Run holds its output back until it has shown it works; the
    /// Break that goes with it waits too.
    resumed: bool,
    deferred_break: Option<BreakKind>,
}

/// What survives a lost connection and the end of a Run.
struct State {
    window: usize,
    paused: bool,
    restart_on_exit: bool,
    /// Restart was asked while no Run could be restarted.
    restart_pending: bool,
    /// Lines output while Paused, at most `window`.
    queue: VecDeque<(Stream, String)>,
    /// Lines dropped from `queue` for the Live Window, not yet announced.
    dropped: u32,
    splitters: [LineSplitter; 2],
    /// `None` for a typed or edited command, or once resuming failed.
    resume: Option<ResumePoint>,
    resume_disabled: bool,
    /// A Run has been started before.
    started: bool,
    /// The connection was lost since the last Run started.
    was_disconnected: bool,
}

impl State {
    /// Apply what does not end the Run; `Close` and `Restart` are for the
    /// caller.
    fn handle(&mut self, command: SourceCommand, events: &Sender<SourceEvent>) -> Wake {
        match command {
            SourceCommand::Close => return Wake::Closed,
            SourceCommand::Restart => return Wake::Restart,
            SourceCommand::Reconnect => return Wake::Reconnect,
            // The app drops the lines it holds; the command goes on, numbers
            // keep counting. A command has no older lines to load.
            SourceCommand::Clear | SourceCommand::CancelLoadMore => {}
            SourceCommand::LoadMore { .. } => {
                let _ = events.try_send(SourceEvent::OlderUnavailable);
            }
            SourceCommand::Pause => self.paused = true,
            SourceCommand::Resume => {
                self.paused = false;
                self.flush_queue(events);
            }
            SourceCommand::SetWindow(n) => {
                self.window = n;
                self.trim_queue();
            }
            SourceCommand::SetRestartOnExit(on) => {
                self.restart_on_exit = on;
                return Wake::Changed;
            }
        }
        Wake::Timeout
    }

    fn trim_queue(&mut self) {
        let keep = self.window.max(1);
        while self.queue.len() > keep {
            self.queue.pop_front();
            self.dropped += 1;
        }
    }

    /// Send what was held while Paused.
    fn flush_queue(&mut self, events: &Sender<SourceEvent>) {
        if self.dropped > 0 {
            let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Skipped(self.dropped) });
            self.dropped = 0;
        }
        if !self.queue.is_empty() {
            let batch: Vec<(Stream, String)> = self.queue.drain(..).collect();
            send_lines(events, batch);
        }
    }

    /// Give lines to the app now, or hold them while Paused.
    fn deliver(&mut self, events: &Sender<SourceEvent>, batch: Vec<(Stream, String)>) {
        if batch.is_empty() {
            return;
        }
        if self.paused {
            self.queue.extend(batch);
            self.trim_queue();
        } else {
            send_lines(events, batch);
        }
    }
}

fn send_lines(events: &Sender<SourceEvent>, batch: Vec<(Stream, String)>) {
    let streams: StreamMask = batch.iter().map(|(s, _)| *s).collect();
    let lines: Vec<String> = batch.into_iter().map(|(_, l)| l).collect();
    let _ = events.try_send(SourceEvent::Append { lines, bytes: Vec::new(), streams: Some(streams) });
}

impl RemoteCommandSource {
    async fn run(self, events: Sender<SourceEvent>, commands: Receiver<SourceCommand>) {
        let mut st = State {
            window: self.window,
            paused: false,
            restart_on_exit: self.restart_on_exit,
            restart_pending: false,
            queue: VecDeque::new(),
            dropped: 0,
            splitters: [LineSplitter::new(), LineSplitter::new()],
            resume: self.preset.clone().map(ResumePoint::new),
            resume_disabled: false,
            started: false,
            was_disconnected: false,
        };
        let mut backoff = Duration::from_secs(1);
        let mut failing_since: Option<Instant> = None;
        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
        loop {
            let store = self.servers.lock().unwrap().clone();
            match self.hub.session(&store, &self.server_id).await {
                Ok(session) => {
                    backoff = Duration::from_secs(1);
                    failing_since = None;
                    match self.serve(&session, &mut st, &events, &commands).await {
                        Served::Closed => return,
                        Served::Lost => {
                            if st.started {
                                let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Disconnected });
                                st.was_disconnected = true;
                            }
                            let _ = events.try_send(SourceEvent::Status(SourceStatus::Reconnecting));
                            failing_since = Some(Instant::now());
                            self.hub.drop_session(&self.server_id).await;
                        }
                        Served::Stopped(status) => {
                            let _ = events.try_send(SourceEvent::Status(status));
                            if !self.wait_for_reconnect(&commands, &mut st, &events).await {
                                return;
                            }
                            let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        }
                    }
                }
                Err(e) => {
                    let status = match &e {
                        ConnectError::NeedsSecret { .. } => Some(SourceStatus::NeedsSecret),
                        ConnectError::AuthFailed { .. } => Some(SourceStatus::AuthFailed),
                        ConnectError::Network(_) | ConnectError::Other(_) => None,
                        other => Some(SourceStatus::Error(other.message())),
                    };
                    if let Some(status) = status {
                        let _ = events.try_send(SourceEvent::Status(status));
                        if !self.wait_for_reconnect(&commands, &mut st, &events).await {
                            return;
                        }
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        continue;
                    }
                    let since = *failing_since.get_or_insert_with(Instant::now);
                    let _ = events.try_send(SourceEvent::Problem(e.message()));
                    if st.started {
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Reconnecting));
                    }
                    if since.elapsed() > GIVE_UP_AFTER {
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Disconnected));
                        failing_since = None;
                        backoff = Duration::from_secs(1);
                        if !self.wait_for_reconnect(&commands, &mut st, &events).await {
                            return;
                        }
                        let _ = events.try_send(SourceEvent::Status(SourceStatus::Connecting));
                        continue;
                    }
                }
            }
            // Wait before the next try, still hearing the app.
            if !self.sleep_while_down(backoff, &commands, &mut st, &events).await {
                return;
            }
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    /// Wait until the app says Reconnect. `false` when it closed the source.
    async fn wait_for_reconnect(
        &self,
        commands: &Receiver<SourceCommand>,
        st: &mut State,
        events: &Sender<SourceEvent>,
    ) -> bool {
        loop {
            match self.wait(None, commands, st, events).await {
                Wake::Reconnect => return true,
                Wake::Closed => return false,
                Wake::Restart => st.restart_pending = true,
                Wake::Changed | Wake::Timeout => {}
            }
        }
    }

    /// Sleep while there is no connection; a Reconnect ends it early. `false`
    /// when the source was closed.
    async fn sleep_while_down(
        &self,
        duration: Duration,
        commands: &Receiver<SourceCommand>,
        st: &mut State,
        events: &Sender<SourceEvent>,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            match self.wait(Some(left), commands, st, events).await {
                Wake::Timeout | Wake::Reconnect => return true,
                Wake::Closed => return false,
                Wake::Restart => st.restart_pending = true,
                Wake::Changed => {}
            }
        }
    }

    /// Wait for `duration` (for ever when `None`), handling what the app says
    /// that does not end the wait.
    async fn wait(
        &self,
        duration: Option<Duration>,
        commands: &Receiver<SourceCommand>,
        st: &mut State,
        events: &Sender<SourceEvent>,
    ) -> Wake {
        let deadline = duration.map(|d| tokio::time::Instant::now() + d);
        loop {
            let timer = async {
                match deadline {
                    Some(d) => tokio::time::sleep_until(d).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                _ = timer => return Wake::Timeout,
                cmd = commands.recv() => match cmd {
                    Ok(command) => match st.handle(command, events) {
                        Wake::Timeout => {}
                        other => return other,
                    },
                    Err(_) => return Wake::Closed,
                },
            }
        }
    }

    /// Run the command on `session`, again and again as the plan says, until
    /// the connection is lost, the source closed, or the user must act.
    async fn serve(
        &self,
        session: &Arc<RemoteSession>,
        st: &mut State,
        events: &Sender<SourceEvent>,
        commands: &Receiver<SourceCommand>,
    ) -> Served {
        let mut plan = if st.restart_pending {
            Plan::Manual
        } else if st.started {
            Plan::AfterDisconnect
        } else {
            Plan::First
        };
        if st.restart_pending {
            st.restart_pending = false;
            self.reset_resume(st);
        }
        let mut backoff = RESTART_BACKOFF_FIRST;
        let mut quick_failures = 0u32;
        loop {
            if events.is_closed() {
                return Served::Closed;
            }
            let start = self.prepare(plan, st, events);
            let mut stream = match session.exec_stream(&start.command).await {
                Ok(stream) => stream,
                Err(_) if session.is_closed() => return Served::Lost,
                Err(e) => {
                    return Served::Stopped(SourceStatus::Error(format!("{}: {e}", self.name)));
                }
            };
            let _ = events.try_send(SourceEvent::Run { command: start.command.clone() });
            let _ = events.try_send(SourceEvent::Status(SourceStatus::Live));
            st.started = true;
            st.was_disconnected = false;
            let started = Instant::now();
            match self.pump(&mut stream, st, events, commands, start).await {
                RunEnd::Closed => {
                    self.finish(stream, events, session).await;
                    return Served::Closed;
                }
                RunEnd::Restart => {
                    self.finish(stream, events, session).await;
                    plan = Plan::Manual;
                    quick_failures = 0;
                    backoff = RESTART_BACKOFF_FIRST;
                    self.reset_resume(st);
                }
                RunEnd::ResumeFailed => {
                    // The tool did not take the since flag: run it as written.
                    self.finish(stream, events, session).await;
                    st.resume_disabled = true;
                    st.resume = None;
                    let _ = events.try_send(SourceEvent::Problem("resume not supported".into()));
                    plan = Plan::ResumeFailed;
                }
                RunEnd::Exited(code) => {
                    if session.is_closed() {
                        return Served::Lost;
                    }
                    let _ = events.try_send(SourceEvent::Break { kind: BreakKind::Exited(code) });
                    let _ = events.try_send(SourceEvent::Status(SourceStatus::Exited(code)));
                    let lived = started.elapsed();
                    if code != Some(0) && lived < QUICK_EXIT {
                        quick_failures += 1;
                    } else {
                        quick_failures = 0;
                    }
                    if lived > RUN_LONG_ENOUGH {
                        backoff = RESTART_BACKOFF_FIRST;
                    }
                    // Wait for the backoff when Restart on exit is on, or for
                    // the user.
                    loop {
                        let automatic = st.restart_on_exit && quick_failures < QUICK_FAILURES_BEFORE_STOP;
                        match self.wait(automatic.then_some(backoff), commands, st, events).await {
                            Wake::Closed => return Served::Closed,
                            Wake::Restart => {
                                plan = Plan::Manual;
                                quick_failures = 0;
                                backoff = RESTART_BACKOFF_FIRST;
                                self.reset_resume(st);
                                break;
                            }
                            // Reconnect is for a connection, not for a Run that exited.
                            Wake::Reconnect | Wake::Changed => {}
                            Wake::Timeout => {
                                if automatic {
                                    backoff = (backoff * 2).min(RESTART_BACKOFF_MAX);
                                    plan = Plan::OnExit;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Start counting from a fresh start of the command.
    fn reset_resume(&self, st: &mut State) {
        st.resume = self.preset.clone().filter(|_| !st.resume_disabled).map(ResumePoint::new);
    }

    /// The command line of the next Run, and the Break that goes before it.
    fn prepare(&self, plan: Plan, st: &mut State, events: &Sender<SourceEvent>) -> RunStart {
        let as_written = |break_kind: Option<BreakKind>| {
            if let Some(kind) = break_kind {
                let _ = events.try_send(SourceEvent::Break { kind });
            }
            RunStart { command: self.command.clone(), resumed: false, deferred_break: None }
        };
        match plan {
            Plan::First => as_written(None),
            Plan::Manual => as_written(Some(BreakKind::Restarted(RestartKind::Manual))),
            Plan::ResumeFailed => as_written(Some(BreakKind::Restarted(RestartKind::ResumeFailed))),
            Plan::OnExit | Plan::AfterDisconnect => {
                let resumable = st.resume.as_ref().filter(|_| !st.resume_disabled).and_then(|rp| {
                    rp.command().map(|command| (command, rp.last_seconds_of_day().unwrap_or(0)))
                });
                match resumable {
                    Some((command, seconds)) => {
                        if let Some(rp) = st.resume.as_mut() {
                            rp.begin_resume();
                        }
                        let kind = if plan == Plan::OnExit {
                            BreakKind::Restarted(RestartKind::OnExit)
                        } else {
                            BreakKind::Resumed(seconds)
                        };
                        RunStart { command, resumed: true, deferred_break: Some(kind) }
                    }
                    None => {
                        self.reset_resume(st);
                        as_written(Some(BreakKind::Restarted(if plan == Plan::OnExit {
                            RestartKind::OnExit
                        } else {
                            RestartKind::AfterDisconnect
                        })))
                    }
                }
            }
        }
    }

    /// Read the Run until it ends.
    async fn pump(
        &self,
        stream: &mut ExecStream,
        st: &mut State,
        events: &Sender<SourceEvent>,
        commands: &Receiver<SourceCommand>,
        start: RunStart,
    ) -> RunEnd {
        let mut proof = Proof {
            pending: start.resumed,
            deferred_break: start.deferred_break,
            held: Vec::new(),
            deadline: tokio::time::Instant::now() + RESUME_PROOF_WITHIN,
        };
        st.splitters = [LineSplitter::new(), LineSplitter::new()];
        loop {
            let proof_timer = async {
                if proof.pending {
                    tokio::time::sleep_until(proof.deadline).await
                } else {
                    std::future::pending().await
                }
            };
            tokio::select! {
                event = stream.next() => match event {
                    Some(ExecEvent::Stdout(bytes)) => {
                        let lines = st.splitters[0].push(&bytes);
                        self.take_lines(Stream::Stdout, lines, &mut proof, st, events);
                    }
                    Some(ExecEvent::Stderr(bytes)) => {
                        let lines = st.splitters[1].push(&bytes);
                        self.take_lines(Stream::Stderr, lines, &mut proof, st, events);
                    }
                    ended => {
                        let code = match ended {
                            Some(ExecEvent::Exited(code)) => code,
                            _ => None,
                        };
                        // What was written without a newline is a line now.
                        for (i, which) in [Stream::Stdout, Stream::Stderr].into_iter().enumerate() {
                            if let Some(last) = st.splitters[i].finish() {
                                self.take_lines(which, vec![last], &mut proof, st, events);
                            }
                        }
                        if proof.pending {
                            // It ended before showing a line with a time.
                            if code != Some(0) {
                                return RunEnd::ResumeFailed;
                            }
                            self.release(&mut proof, st, events);
                        }
                        if let Some(rp) = st.resume.as_mut() {
                            rp.end_filtering();
                        }
                        return RunEnd::Exited(code);
                    }
                },
                command = commands.recv() => match command {
                    Ok(command) => match st.handle(command, events) {
                        Wake::Closed => return RunEnd::Closed,
                        Wake::Restart => return RunEnd::Restart,
                        _ => {}
                    },
                    Err(_) => return RunEnd::Closed,
                },
                _ = proof_timer => self.release(&mut proof, st, events),
            }
        }
    }

    /// Lines that came out of one of the outputs.
    fn take_lines(
        &self,
        stream: Stream,
        lines: Vec<String>,
        proof: &mut Proof,
        st: &mut State,
        events: &Sender<SourceEvent>,
    ) {
        if lines.is_empty() {
            return;
        }
        if proof.pending {
            // A real log line of a preset has a time; an error text does not.
            let works = self
                .preset
                .as_ref()
                .is_some_and(|p| lines.iter().any(|l| parse_timestamp(p, l).is_some()));
            proof.held.extend(lines.into_iter().map(|l| (stream, l)));
            if works {
                self.release(proof, st, events);
            }
            return;
        }
        let batch: Vec<(Stream, String)> = lines.into_iter().map(|l| (stream, l)).collect();
        self.accept(batch, st, events);
    }

    /// A resumed Run has shown it works (or time ran out without it failing):
    /// its Break and the lines held are let through.
    fn release(&self, proof: &mut Proof, st: &mut State, events: &Sender<SourceEvent>) {
        if !proof.pending {
            return;
        }
        proof.pending = false;
        if let Some(kind) = proof.deferred_break.take() {
            let _ = events.try_send(SourceEvent::Break { kind });
        }
        let held = std::mem::take(&mut proof.held);
        self.accept(held, st, events);
    }

    /// Drop the lines already received (a resumed Run), note them for the next
    /// resume, and deliver the rest.
    fn accept(&self, batch: Vec<(Stream, String)>, st: &mut State, events: &Sender<SourceEvent>) {
        let mut kept: Vec<(Stream, String)> = Vec::with_capacity(batch.len());
        for (stream, line) in batch {
            let verdict = match st.resume.as_mut() {
                Some(rp) => rp.filter(stream, &line),
                None => Verdict::Keep,
            };
            match verdict {
                Verdict::Drop => continue,
                Verdict::KeepClockWentBack => {
                    let before = std::mem::take(&mut kept);
                    st.deliver(events, before);
                    let _ = events.try_send(SourceEvent::Break { kind: BreakKind::ClockWentBack });
                }
                Verdict::Keep => {}
            }
            if let Some(rp) = st.resume.as_mut() {
                rp.record(stream, &line);
            }
            kept.push((stream, line));
        }
        st.deliver(events, kept);
    }

    /// End a Run on the Server: ask it to stop, and say so when it may not have.
    async fn finish(&self, stream: ExecStream, events: &Sender<SourceEvent>, session: &RemoteSession) {
        if !stream.terminate().await {
            let _ = events.try_send(SourceEvent::Problem(format!(
                "{} may still be running on {}",
                self.command, session.server.name
            )));
        }
    }
}

/// The wait of a resumed Run to show that it works.
struct Proof {
    pending: bool,
    deferred_break: Option<BreakKind>,
    held: Vec<(Stream, String)>,
    deadline: tokio::time::Instant,
}
