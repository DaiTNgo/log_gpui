use log_gpui::remote::command::{DEFAULT_PRESET_LINES, unedited_preset};
use log_gpui::remote::*;

fn refused(command: &str) -> bool {
    check_elevation(command).is_err()
}

#[test]
fn elevation_is_refused_as_the_first_word_of_a_part() {
    for command in [
        "sudo journalctl -f",
        "sudo -n journalctl -f",
        "FOO=1 sudo x",
        "/usr/bin/sudo x",
        "a | sudo tee /x",
        "a; su -c x",
        "a && doas x",
        "a || pkexec x",
        "run0 x",
        "env sudo x",
        "env -i FOO=1 sudo x",
        "nohup sudo x",
        "command sudo x",
        "exec sudo x",
        "time sudo x",
        "FOO=1 BAR=2 env nohup sudo x",
        "(sudo x)",
        "echo hi; (cd /; sudo x)",
        "echo $(sudo cat /x)",
        "echo `sudo cat /x`",
        "a\nsudo x",
        "a & sudo x",
        "'sudo' x",
        "\"sudo\" x",
    ] {
        assert!(refused(command), "{command:?} should be refused");
    }
}

#[test]
fn the_refusal_names_the_word_found() {
    assert_eq!(check_elevation("FOO=1 /usr/bin/sudo x").unwrap_err().word, "/usr/bin/sudo");
}

#[test]
fn text_that_only_mentions_elevation_is_allowed() {
    for command in [
        "journalctl -f -u nginx",
        "echo sudo",
        "echo 'a | sudo x'",
        "grep -r sudo /var/log",
        "tail -F /var/log/auth.log | grep sudo",
        "docker logs -f sudoku",
        "cat /etc/sudoers.d/x",
        "echo \"; sudo x\"",
        "ls /usr/bin/su*",
        "FOO=sudo env",
    ] {
        assert!(!refused(command), "{command:?} should be allowed");
    }
}

#[test]
fn journalctl_preset_writes_the_flags_a_log_viewer_needs() {
    let fields = PresetFields::journal("nginx");
    assert_eq!(fields.command(), "journalctl --no-pager -o short-iso-precise -f -n 1000 -u 'nginx'");
    assert_eq!(
        render(&fields, &Start::Since("@1760842923".into())),
        "journalctl --no-pager -o short-iso-precise -f --since @1760842923 -u 'nginx'"
    );
    // No unit: the whole journal.
    assert_eq!(
        PresetFields::journal("").command(),
        "journalctl --no-pager -o short-iso-precise -f -n 1000"
    );
}

#[test]
fn docker_preset_writes_timestamps_and_resumes_from_one() {
    let fields = PresetFields::docker("api");
    assert_eq!(fields.command(), "docker logs -f --timestamps --tail 1000 'api'");
    assert_eq!(
        render(&fields, &Start::Since("2026-10-19T03:02:03.123456789Z".into())),
        "docker logs -f --timestamps --since 2026-10-19T03:02:03.123456789Z 'api'"
    );
}

#[test]
fn kubernetes_preset_orders_its_optional_flags() {
    let fields = PresetFields::kube("api-7d9", "prod", "app", "eu");
    assert_eq!(
        fields.command(),
        "kubectl logs -f --timestamps --tail=1000 --context 'eu' -n 'prod' -c 'app' 'api-7d9'"
    );
    assert_eq!(
        render(&fields, &Start::Since("2026-10-19T03:02:03.123456789Z".into())),
        "kubectl logs -f --timestamps --since-time=2026-10-19T03:02:03.123456789Z --context 'eu' -n 'prod' -c 'app' 'api-7d9'"
    );
    assert_eq!(
        PresetFields::kube("p", "", "", "").command(),
        "kubectl logs -f --timestamps --tail=1000 'p'"
    );
}

#[test]
fn values_are_quoted_for_the_shell() {
    let fields = PresetFields::docker("a'b; rm -rf /");
    assert_eq!(fields.command(), "docker logs -f --timestamps --tail 1000 'a'\\''b; rm -rf /'");
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
}

#[test]
fn a_since_value_with_unsafe_characters_is_quoted() {
    let fields = PresetFields::docker("api");
    assert_eq!(
        render(&fields, &Start::Since("x; reboot".into())),
        "docker logs -f --timestamps --since 'x; reboot' 'api'"
    );
}

#[test]
fn a_preset_is_kept_only_while_the_command_is_what_it_writes() {
    let fields = PresetFields::journal("nginx");
    let command = fields.command();
    assert_eq!(unedited_preset(&command, Some(&fields)), Some(fields.clone()));
    let edited = command.replace("-n 1000", "-n 50");
    assert_eq!(unedited_preset(&edited, Some(&fields)), None);
    assert_eq!(unedited_preset(&command, None), None);
    assert_eq!(fields.lines(), DEFAULT_PRESET_LINES);
}
