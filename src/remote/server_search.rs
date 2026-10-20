//! Finding Servers and listing them under their Server Group.

use super::server::Server;
use crate::unicode_text::fold;

/// Whether `server` matches the typed query: every word (split on
/// whitespace) must be contained, ignoring case the way Search does, in its
/// name, address, user, Server Group or one of its tags. An empty query
/// matches everything.
pub fn server_matches(server: &Server, query: &str) -> bool {
    let words: Vec<String> = query.split_whitespace().map(fold).collect();
    if words.is_empty() {
        return true;
    }
    let mut fields = vec![fold(&server.name), fold(&server.host), fold(&server.user)];
    if let Some(group) = &server.group {
        fields.push(fold(group));
    }
    fields.extend(server.tags.iter().map(|t| fold(t)));
    words.iter().all(|word| fields.iter().any(|field| field.contains(word.as_str())))
}

/// A heading and its Servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSection<'a> {
    /// `None` for Servers with no Server Group ("Ungrouped").
    pub group: Option<String>,
    pub servers: Vec<&'a Server>,
}

impl GroupSection<'_> {
    pub fn title(&self) -> &str {
        self.group.as_deref().unwrap_or("Ungrouped")
    }
}

/// The Servers that match `query`, under their Server Group: groups in
/// alphabetical order, Servers by name, "Ungrouped" last. A group with no
/// matching Server is left out.
pub fn grouped<'a>(servers: &'a [Server], query: &str) -> Vec<GroupSection<'a>> {
    let mut sections: Vec<GroupSection<'a>> = Vec::new();
    for server in servers.iter().filter(|s| server_matches(s, query)) {
        let group = server.group.as_deref().map(str::trim).filter(|g| !g.is_empty()).map(str::to_string);
        match sections.iter_mut().find(|s| s.group == group) {
            Some(section) => section.servers.push(server),
            None => sections.push(GroupSection { group, servers: vec![server] }),
        }
    }
    for section in &mut sections {
        section.servers.sort_by_cached_key(|s| (fold(&s.name), s.name.clone()));
    }
    sections.sort_by_cached_key(|s| (s.group.is_none(), s.group.as_deref().map(fold), s.group.clone()));
    sections
}

/// The Server Groups the Servers belong to, alphabetically, for suggestions.
pub fn group_names(servers: &[Server]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for group in servers.iter().filter_map(|s| s.group.as_deref()).map(str::trim).filter(|g| !g.is_empty()) {
        if !names.iter().any(|n| n == group) {
            names.push(group.to_string());
        }
    }
    names.sort_by_cached_key(|n| (fold(n), n.clone()));
    names
}

/// The first Server in the order the list shows them: what Enter picks.
pub fn first_match<'a>(servers: &'a [Server], query: &str) -> Option<&'a Server> {
    grouped(servers, query).into_iter().flat_map(|s| s.servers).next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::AuthMethod;

    fn server(name: &str, host: &str, group: Option<&str>, tags: &[&str]) -> Server {
        let mut s = Server::new(name, host, "deploy", AuthMethod::Password);
        s.group = group.map(str::to_string);
        s.tags = tags.iter().map(|t| t.to_string()).collect();
        s
    }

    fn all() -> Vec<Server> {
        vec![
            server("web2", "10.0.0.6", Some("prod"), &["eu"]),
            server("stg1", "10.0.9.1", Some("staging"), &[]),
            server("lab", "192.168.1.5", None, &["scratch"]),
            server("web1", "10.0.0.5", Some("prod"), &["eu", "web"]),
            server("db", "10.0.1.9", Some("prod"), &[]),
        ]
    }

    fn names(sections: &[GroupSection]) -> Vec<(String, Vec<String>)> {
        sections
            .iter()
            .map(|s| (s.title().to_string(), s.servers.iter().map(|x| x.name.clone()).collect()))
            .collect()
    }

    #[test]
    fn servers_are_listed_by_group_with_ungrouped_last() {
        let all = all();
        assert_eq!(
            names(&grouped(&all, "")),
            vec![
                ("prod".to_string(), vec!["db".to_string(), "web1".into(), "web2".into()]),
                ("staging".to_string(), vec!["stg1".to_string()]),
                ("Ungrouped".to_string(), vec!["lab".to_string()]),
            ]
        );
    }

    #[test]
    fn every_word_must_match_somewhere() {
        let all = all();
        assert_eq!(names(&grouped(&all, "prod web")), vec![("prod".to_string(), vec!["web1".to_string(), "web2".into()])]);
        assert_eq!(names(&grouped(&all, "prod eu 0.5")), vec![("prod".to_string(), vec!["web1".to_string()])]);
        assert!(grouped(&all, "prod stg").is_empty());
    }

    #[test]
    fn address_user_and_tag_are_searched() {
        let all = all();
        let found: Vec<String> = grouped(&all, "10.0.1").into_iter().flat_map(|s| s.servers).map(|s| s.name.clone()).collect();
        assert_eq!(found, vec!["db"]);
        assert_eq!(first_match(&all, "scratch").unwrap().name, "lab");
        assert_eq!(grouped(&all, "deploy").iter().map(|s| s.servers.len()).sum::<usize>(), 5);
    }

    #[test]
    fn case_and_composed_characters_are_folded_but_diacritics_are_kept() {
        let servers = [server("Máy chủ", "h", None, &["Sản xuất"]), server("may", "h2", None, &[])];
        assert!(server_matches(&servers[0], "MÁY"));
        // The same letter written decomposed.
        assert!(server_matches(&servers[0], "ma\u{301}y"));
        assert!(server_matches(&servers[0], "sản xuất"));
        // "may" does not match "máy": diacritics count.
        assert!(!server_matches(&servers[1], "máy"));
        assert!(!server_matches(&servers[0], "may chu"));
    }

    #[test]
    fn a_group_with_no_server_left_is_not_listed() {
        let mut all = all();
        for s in all.iter_mut().filter(|s| s.group.as_deref() == Some("staging")) {
            s.group = Some("prod".into());
        }
        assert!(grouped(&all, "").iter().all(|s| s.title() != "staging"));
        assert_eq!(group_names(&all), vec!["prod"]);
    }

    #[test]
    fn enter_picks_the_first_server_shown() {
        let all = all();
        assert_eq!(first_match(&all, "").unwrap().name, "db");
        assert!(first_match(&all, "nothing").is_none());
    }
}
