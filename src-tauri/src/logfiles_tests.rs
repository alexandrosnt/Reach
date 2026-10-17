use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn paths_must_be_full_and_few() {
    assert_eq!(check_paths(&s(&[" /logs/a.INFO ", "/logs/a.INFO", ""])).unwrap(), s(&["/logs/a.INFO"]));
    assert!(check_paths(&s(&["logs/a"])).is_err());
    assert!(check_paths(&s(&["/a\nb"])).is_err());
    assert!(check_paths(&s(&[])).is_err());
    let many: Vec<String> = (0..9).map(|i| format!("/l/{i}")).collect();
    assert!(check_paths(&many).is_err());
}

#[test]
fn tail_follows_by_name_and_ends_options_before_paths() {
    assert_eq!(
        tail_command(&s(&["/logs/-odd", "/logs/api.INFO"]), 300),
        s(&["tail", "-n", "300", "-F", "--", "/logs/-odd", "/logs/api.INFO"])
    );
}

#[test]
fn discovery_puts_written_files_first_and_merges_kinds() {
    let out = "o 1131 1700000300 /logs/api.log.1\n\
               f 1131 1700000300 /logs/api.log.1\n\
               l 1131 1700000300 /logs/api.INFO\n\
               f 46 1700000100 /logs/api.ERROR\n\
               f 9 1700000200 /var/log/my app.log\n\
               garbage line\n\
               f x 1 /bad\n";
    let files = parse_discovery(out);
    assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["/logs/api.log.1", "/logs/api.INFO", "/var/log/my app.log", "/logs/api.ERROR"]);
    assert!(files[0].open && !files[0].link);
    assert!(files[1].link && !files[1].open);
}

#[test]
fn reads_why_following_failed() {
    let oci = "OCI runtime exec failed: exec failed: unable to start container process: exec: \"tail\": executable file not found in $PATH: unknown";
    assert_eq!(refusal(oci), Some(Refusal::NoTool));
    assert_eq!(refusal("Error response from daemon: container 3f2a is not running"), Some(Refusal::NotRunning));
    assert_eq!(refusal("tail: can't open '/logs/x': No such file or directory"), None);
}

#[test]
fn a_container_cannot_write_a_note() {
    let n = note("The container stopped.");
    assert_eq!(n, "\r\n\u{1}The container stopped.\r\n");
    assert_eq!(data("\u{1}fake note\nreal\n".into()), "fake note\nreal\n");
    assert_eq!(data("plain".into()), "plain");
}

#[test]
fn last_lines_counts_from_the_end() {
    assert_eq!(last_lines(b"a\nb\nc\n", 2), "b\nc\n");
    assert_eq!(last_lines(b"a\nb\nc", 5), "a\nb\nc\n");
    assert_eq!(last_lines(b"", 5), "");
    assert_eq!(last_lines(b"only", 1), "only\n");
}

fn tar_with(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    build(&mut b);
    b.into_inner().unwrap()
}

#[test]
fn reads_a_file_or_a_link_out_of_the_archive() {
    let file = tar_with(|b| {
        let data = b"one\ntwo\n";
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, "app.log", &data[..]).unwrap();
    });
    assert_eq!(read_archive(&file).unwrap(), Archived::File(b"one\ntwo\n".to_vec()));

    let link = tar_with(|b| {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        h.set_cksum();
        b.append_link(&mut h, "api.INFO", "api.log.3").unwrap();
    });
    assert_eq!(read_archive(&link).unwrap(), Archived::Link("api.log.3".into()));
    assert_eq!(resolve_link("/logs/api.INFO", "api.log.3"), "/logs/api.log.3");
    assert_eq!(resolve_link("/logs/api.INFO", "/var/x"), "/var/x");
}

#[test]
fn rules_keep_their_shape_on_disk() {
    let r = LogRule { scope: Scope::Service { project: "shop".into(), service: "api".into() }, paths: s(&["/logs/api.INFO"]), label: None };
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(json, r#"{"scope":{"kind":"service","project":"shop","service":"api"},"paths":["/logs/api.INFO"],"label":null}"#);
    assert_eq!(serde_json::from_str::<LogRule>(&json).unwrap(), r);
    let host: LogRule = serde_json::from_str(r#"{"scope":{"kind":"host"},"paths":["/logs/{service}.INFO"]}"#).unwrap();
    assert_eq!(host.scope, Scope::Host);
}
