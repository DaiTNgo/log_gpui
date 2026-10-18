//! Writes a session with one Log Space open in a tab, to start the app on it.
//! `seed_session <session dir> <file>...` makes an Imported Log Space;
//! with `SEED_REMOTE_PORT` and `SEED_REMOTE_KEY` set, the files are paths on
//! a Server at 127.0.0.1 (also written to `servers.json`) and the space is a
//! Remote Log Space. `SEED_RAW` opens it in the Raw View.

use log_gpui::remote::{AuthMethod, Server, ServerStore};
use log_gpui::workspace::log_space::LogSpaceKind;
use log_gpui::workspace::project::ProjectColorAccent;
use log_gpui::workspace::session::SessionPersistence;
use log_gpui::workspace::state::WorkspaceState;
use log_gpui::workspace::working_set::TabSessionState;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("session dir");
    let files: Vec<String> = args.collect();
    let base = std::path::Path::new(&dir).join("log_gpui");
    let mut ws = WorkspaceState::new();
    ws.create_project("Live", ProjectColorAccent::Blue);
    let project = ws.active_project_mut().unwrap();
    let remote = std::env::var("SEED_REMOTE_PORT").ok().zip(std::env::var("SEED_REMOTE_KEY").ok());
    let space_id = if let Some((port, key)) = remote {
        let mut server = Server::new(
            "web1",
            "127.0.0.1",
            &std::env::var("USER").unwrap(),
            AuthMethod::Key { path: key },
        );
        server.port = port.parse().unwrap();
        let mut store = ServerStore::default();
        let id = store.save(server).unwrap();
        store.save_to(&base.join("servers.json")).unwrap();
        let space = project.create_log_space("Remote", LogSpaceKind::Remote).unwrap();
        let triples: Vec<_> = files.iter().map(|p| (id.clone(), "web1".to_string(), p.clone())).collect();
        space.add_remote_files(&triples).unwrap();
        space.id.clone()
    } else {
        let space = project.create_log_space("Live", LogSpaceKind::Imported).unwrap();
        space.add_imported_files(&files).unwrap();
        space.id.clone()
    };
    let mut session = TabSessionState::new();
    if std::env::var("SEED_RAW").is_ok() {
        session.mode = log_gpui::workspace::working_set::ViewMode::Raw;
    }
    project.open_log_space(&space_id, session);
    SessionPersistence::new(base.join("session.json")).save(&ws).unwrap();
}
