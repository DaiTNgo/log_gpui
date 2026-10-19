mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::remote::{AuthMethod, Server, ServerError, ServerField, ServerStore, parse_port};

fn fields_of(errors: &[ServerError]) -> Vec<ServerField> {
    errors.iter().filter_map(|e| e.field()).collect()
}

#[test]
fn validate_all_returns_every_error_on_its_field() {
    let mut store = ServerStore::default();
    store.save(Server::new("web", "10.0.0.1", "deploy", AuthMethod::Password)).unwrap();

    let blank = Server::new("", "", "", AuthMethod::Password);
    assert_eq!(
        fields_of(&store.validate_all(&blank)),
        vec![ServerField::Name, ServerField::Host, ServerField::User]
    );

    let duplicate = Server::new("web", "10.0.0.2", "deploy", AuthMethod::Password);
    assert_eq!(store.validate_all(&duplicate), vec![ServerError::DuplicateName]);

    let mut unknown_jump = Server::new("db", "10.0.0.3", "deploy", AuthMethod::Password);
    unknown_jump.jump_server_id = Some("nope".into());
    assert_eq!(fields_of(&store.validate_all(&unknown_jump)), vec![ServerField::JumpServer]);

    // validate still reports the first error.
    assert_eq!(store.validate(&blank), Err(ServerError::BlankName));
    assert_eq!(ServerError::InUse(vec!["x".into()]).field(), None);
}

#[test]
fn port_rules() {
    assert_eq!(parse_port(""), Ok(22));
    assert_eq!(parse_port("  "), Ok(22));
    assert_eq!(parse_port("2222"), Ok(2222));
    assert_eq!(parse_port("65535"), Ok(65535));
    for bad in ["0", "65536", "22a", "-1"] {
        assert_eq!(parse_port(bad), Err(ServerError::InvalidPort), "{bad}");
    }
    assert_eq!(ServerError::InvalidPort.field(), Some(ServerField::Port));
}

/// Open the Servers dialog on a new Server form, with its fields made.
fn open_form(cx: &mut TestAppContext) -> Harness {
    let mut h = common::open_app(cx);
    h.view.update_in(&mut h.cx, |app, window, cx| {
        app.open_servers_dialog(cx);
        app.new_server_form(cx);
        app.sync_server_dialog(window, cx);
    });
    h
}

fn type_into(h: &mut Harness, field: ServerField, text: &str) {
    h.view.update_in(&mut h.cx, |app, window, cx| {
        let form = app.server_dialog.as_ref().unwrap().form.as_ref().unwrap();
        let fields = form.fields.as_ref().unwrap();
        let input = match field {
            ServerField::Name => fields.name.clone(),
            ServerField::Host => fields.host.clone(),
            ServerField::Port => fields.port.clone(),
            ServerField::User => fields.user.clone(),
            _ => unreachable!(),
        };
        input.update(cx, |state, cx| state.set_value(text.to_string(), window, cx));
    });
}

fn visible(h: &mut Harness) -> Vec<ServerField> {
    h.view.read_with(&h.cx, |app, cx| app.visible_server_form_errors(cx).into_iter().map(|(f, _)| f).collect())
}

#[gpui::test]
fn a_fresh_form_shows_no_errors(cx: &mut TestAppContext) {
    let mut h = open_form(cx);
    assert_eq!(visible(&mut h), vec![]);
    // The errors are there, just not shown yet.
    let all = h.view.read_with(&h.cx, |app, cx| app.server_form_errors(cx).len());
    assert!(all >= 3);
}

#[gpui::test]
fn an_error_shows_once_the_field_is_left(cx: &mut TestAppContext) {
    let mut h = open_form(cx);
    h.view.update(&mut h.cx, |app, cx| app.touch_server_field(ServerField::Name, cx));
    assert_eq!(visible(&mut h), vec![ServerField::Name]);
}

#[gpui::test]
fn save_flags_every_invalid_field_and_saves_nothing(cx: &mut TestAppContext) {
    let mut h = open_form(cx);
    type_into(&mut h, ServerField::Port, "99999");
    h.view.update(&mut h.cx, |app, cx| app.save_server_form(cx));
    assert_eq!(
        visible(&mut h),
        vec![ServerField::Name, ServerField::Host, ServerField::Port, ServerField::User]
    );
    let saved = h.view.read_with(&h.cx, |app, _| app.remote.servers.servers.len());
    assert_eq!(saved, 0);
}

#[gpui::test]
fn an_error_clears_as_soon_as_the_field_is_valid(cx: &mut TestAppContext) {
    let mut h = open_form(cx);
    h.view.update(&mut h.cx, |app, cx| app.save_server_form(cx));
    assert!(visible(&mut h).contains(&ServerField::Name));
    type_into(&mut h, ServerField::Name, "web");
    assert!(!visible(&mut h).contains(&ServerField::Name));
}

#[gpui::test]
fn a_valid_form_saves_with_its_port(cx: &mut TestAppContext) {
    let mut h = open_form(cx);
    type_into(&mut h, ServerField::Name, "web");
    type_into(&mut h, ServerField::Host, "10.0.0.5");
    type_into(&mut h, ServerField::Port, "2222");
    type_into(&mut h, ServerField::User, "deploy");
    h.view.update(&mut h.cx, |app, cx| app.save_server_form(cx));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.remote.servers.servers.len(), 1);
        assert_eq!(app.remote.servers.servers[0].port, 2222);
        // The form closed.
        assert!(app.server_dialog.as_ref().unwrap().form.is_none());
    });
}
