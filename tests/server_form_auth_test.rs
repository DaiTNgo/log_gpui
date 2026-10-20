//! The Server form: the four ways to sign in, Server Group and tags, and
//! Test connection with what is typed (nothing is saved by it).

mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::server_dialog::{AgentChoice, AuthKind, test_lines};
use log_gpui::remote::{AgentSocket, AuthMethod, Server, TestStep, TestStepKind};
use std::time::{Duration, Instant};

fn open_new(cx: &mut TestAppContext) -> Harness {
    let mut h = common::open_app(cx);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_servers_dialog(cx);
        app.new_server_form(cx);
        app.sync_server_dialog(window, cx);
    });
    h
}

fn fill_basics(h: &mut Harness, name: &str, host: &str, port: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let form = app.server_dialog.as_ref().unwrap().form.as_ref().unwrap();
        let f = form.fields.as_ref().unwrap();
        for (input, text) in [(&f.name, name), (&f.host, host), (&f.port, port), (&f.user, "deploy")] {
            input.update(cx, |state, cx| state.set_value(text.to_string(), window, cx));
        }
    });
}

#[gpui::test]
fn an_agent_server_is_saved_with_socket_preferred_key_group_and_tags(cx: &mut TestAppContext) {
    let mut h = open_new(cx);
    fill_basics(&mut h, "web1", "10.0.0.5", "22");
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.set_server_auth(AuthKind::Agent, cx);
        app.set_agent_choice(AgentChoice::Path, cx);
        let extra = app.server_dialog.as_ref().unwrap().form.as_ref().unwrap().extra.as_ref().unwrap();
        for (input, text) in [
            (&extra.agent_path, "/tmp/agent.sock"),
            (&extra.preferred_key, "SHA256:abc"),
            (&extra.group, " prod "),
            (&extra.tags, "eu, web, eu,, "),
        ] {
            input.update(cx, |state, cx| state.set_value(text.to_string(), window, cx));
        }
        app.save_server_form(cx);
    });
    let saved: Server = h.view.read_with(&h.cx, |app, _| app.remote.servers.by_name("web1").cloned().unwrap());
    assert_eq!(
        saved.auth,
        AuthMethod::Agent { socket: AgentSocket::Path("/tmp/agent.sock".into()), preferred_key: Some("SHA256:abc".into()) }
    );
    assert_eq!(saved.group.as_deref(), Some("prod"));
    assert_eq!(saved.tags, vec!["eu", "web"]);
}

#[gpui::test]
fn keyboard_interactive_needs_no_secret(cx: &mut TestAppContext) {
    let mut h = open_new(cx);
    fill_basics(&mut h, "web1", "10.0.0.5", "22");
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let f = app.server_dialog.as_ref().unwrap().form.as_ref().unwrap().fields.as_ref().unwrap();
        // Typed while Password was chosen, then the sign-in was changed.
        f.secret.update(cx, |state, cx| state.set_value("leftover".to_string(), window, cx));
        app.set_server_auth(AuthKind::Interactive, cx);
        let (server, secret) = app.server_from_form(cx).unwrap();
        assert_eq!(server.auth, AuthMethod::KeyboardInteractive);
        assert_eq!(secret, None);
    });
}

#[gpui::test]
fn editing_an_agent_server_keeps_its_sign_in(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let mut server = Server::new(
        "web1",
        "10.0.0.5",
        "deploy",
        AuthMethod::Agent { socket: AgentSocket::Secretive, preferred_key: None },
    );
    server.group = Some("prod".into());
    server.tags = vec!["eu".into()];
    let id = h.view.update(&mut h.cx, |app, _| app.remote.servers.save(server).unwrap());
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_servers_dialog(cx);
        app.edit_server_form(&id, cx);
        app.sync_server_dialog(window, cx);
        let form = app.server_dialog.as_ref().unwrap().form.as_ref().unwrap();
        assert_eq!((form.auth, form.agent), (AuthKind::Agent, AgentChoice::Secretive));
        let (again, _) = app.server_from_form(cx).unwrap();
        assert_eq!(again.auth, AuthMethod::Agent { socket: AgentSocket::Secretive, preferred_key: None });
        assert_eq!(again.group.as_deref(), Some("prod"));
        assert_eq!(again.tags, vec!["eu"]);
    });
}

fn step(hop: usize, name: &str, kind: TestStepKind, result: Result<&str, &str>) -> TestStep {
    TestStep {
        hop,
        server_name: name.into(),
        kind,
        result: result.map(str::to_string).map_err(str::to_string),
        elapsed: Duration::ZERO,
    }
}

#[test]
fn the_result_has_one_line_per_server_with_the_failing_step_marked() {
    use TestStepKind::*;
    let steps = vec![
        step(0, "bastion", Tcp, Ok("connected")),
        step(0, "bastion", HostKey, Ok("host key trusted")),
        step(0, "bastion", Auth, Ok("signed in")),
        step(1, "db", Tcp, Ok("connected")),
        step(1, "db", HostKey, Ok("host key trusted")),
        step(1, "db", Auth, Ok("signed in")),
        step(1, "db", Sftp, Err("SFTP not available")),
    ];
    assert_eq!(
        test_lines(&steps),
        vec![
            ("bastion: connected, host key trusted, signed in".to_string(), true),
            ("db: connected, host key trusted, signed in, \u{2717} SFTP not available".to_string(), false),
        ]
    );
    let refused = vec![step(0, "db", Tcp, Err("Connection refused"))];
    assert_eq!(test_lines(&refused), vec![("db: \u{2717} connection: Connection refused".to_string(), false)]);
}

#[gpui::test]
fn test_connection_reports_a_refused_port_and_saves_nothing(cx: &mut TestAppContext) {
    let mut h = open_new(cx);
    // A port nothing listens on.
    let port = {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.local_addr().unwrap().port().to_string()
    };
    fill_basics(&mut h, "db", "127.0.0.1", &port);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.set_server_auth(AuthKind::Agent, cx);
        let _ = window;
        app.test_server_connection(cx);
    });
    h.cx.executor().allow_parking();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.cx.run_until_parked();
        let done = h.view.read_with(&h.cx, |app, _| {
            app.server_dialog.as_ref().and_then(|d| d.form.as_ref()).and_then(|f| f.test.as_ref()).is_some_and(|t| !t.running)
        });
        if done {
            break;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
    let (lines, saved) = h.view.read_with(&h.cx, |app, _| {
        let test = app.server_dialog.as_ref().unwrap().form.as_ref().unwrap().test.as_ref().unwrap();
        (test_lines(&test.steps), app.remote.servers.servers.len())
    });
    assert_eq!(lines, vec![("db: \u{2717} connection: Connection refused".to_string(), false)]);
    assert_eq!(saved, 0);
}
