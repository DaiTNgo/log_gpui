//! Writes a session with one Log Space open in a tab, to start the app on it.
//! `seed_session <session dir> <file>...` makes an Imported Log Space;
//! with `SEED_REMOTE_PORT` and `SEED_REMOTE_KEY` set, the files are paths on
//! a Server at 127.0.0.1 (also written to `servers.json`) and the space is a
//! Remote Log Space. `SEED_RAW` opens it in the Raw View. `SEED_COLUMNS`,
//! `SEED_HIDE` and `SEED_LABELS` set the Project's Columns section;
//! `SEED_RULES` adds Extraction Rules.

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
        // `SEED_REMOTE_COMMANDS="cmd one;;cmd two"` adds Remote Commands.
        if let Ok(commands) = std::env::var("SEED_REMOTE_COMMANDS") {
            let commands: Vec<_> = commands
                .split(";;")
                .map(|c| (id.clone(), "web1".to_string(), c.to_string(), None))
                .collect();
            space.add_remote_commands(&commands).unwrap();
        }
        space.id.clone()
    } else {
        let space = project.create_log_space("Live", LogSpaceKind::Imported).unwrap();
        space.add_imported_files(&files).unwrap();
        space.id.clone()
    };
    // `SEED_COLUMNS=user,region` turns Field Columns on, `SEED_HIDE=level`
    // hides fixed columns and `SEED_LABELS=module=Service` names columns.
    let list = |name: &str| -> Vec<String> {
        std::env::var(name).map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default()
    };
    for key in list("SEED_COLUMNS") {
        project.columns.set_shown(&key, true);
    }
    for key in list("SEED_HIDE") {
        project.columns.set_shown(&key, false);
    }
    for pair in list("SEED_LABELS") {
        if let Some((key, label)) = pair.split_once('=') {
            project.columns.labels.insert(key.to_string(), label.to_string());
        }
    }
    // `SEED_RULES="user=user=(\w+);;ms=took=(\d+)ms"` adds Extraction Rules.
    if let Ok(rules) = std::env::var("SEED_RULES") {
        for (name, pattern) in rules.split(";;").filter_map(|r| r.split_once('=')) {
            let id = log_gpui::extract::next_rule_id(&project.extraction_rules);
            project.extraction_rules.push(log_gpui::extract::ExtractionRule {
                id,
                name: name.into(),
                pattern: pattern.into(),
                scope: Default::default(),
                description: "custom regex".into(),
                enabled: true,
            });
            project.columns.set_shown(name, true);
        }
    }
    // `SEED_FILTER=level=warn` sets a Filter, which opens the Focus Pane.
    if let Some((column, value)) = std::env::var("SEED_FILTER").ok().as_deref().and_then(|f| f.split_once('=')) {
        if let Some(space) = project.log_space_mut(&space_id) {
            space.filter.toggle_value(column, value);
        }
    }
    // `SEED_SEARCH=error` types a Search Expression into the first Focus Tab.
    if let Ok(query) = std::env::var("SEED_SEARCH") {
        if let Some(space) = project.log_space_mut(&space_id) {
            space.focus.search_mut().query = query;
        }
    }
    let mut session = TabSessionState::new();
    if std::env::var("SEED_RAW").is_ok() {
        session.mode = log_gpui::workspace::working_set::ViewMode::Raw;
    }
    project.open_log_space(&space_id, session);
    SessionPersistence::new(base.join("session.json")).save(&ws).unwrap();
}
