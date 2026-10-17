use super::*;

fn p(text: &str, label: &str) -> Private {
    Private { text: text.into(), label: label.into() }
}

#[test]
fn takes_out_addresses_but_keeps_times() {
    let out = redact(
        "2026-10-17T12:34:56.789Z  INFO connect 192.168.30.214:22 and fe80::1ff:fe23:4567:890a then 2001:db8::1",
        None,
        &[],
    );
    assert!(out.contains("12:34:56.789Z"), "{out}");
    assert!(!out.contains("192.168"), "{out}");
    assert!(!out.contains("fe80"), "{out}");
    assert!(!out.contains("2001:db8"), "{out}");
}

#[test]
fn keeps_rust_paths_which_look_like_ipv6() {
    let line = "INFO reach_lib::monitoring::collector: up, then [::1]:22 and ::1 refused";
    assert_eq!(redact(line, None, &[]), "INFO reach_lib::monitoring::collector: up, then [[ip]]:22 and [ip] refused");
}

#[test]
fn takes_out_the_users_own_hosts_accounts_and_names() {
    let out = redact(
        "WARN auth failed for soap on Build-Server.lan (session \"my-netone\")",
        None,
        &[p("build-server.lan", "host"), p("soap", "user"), p("my-netone", "session")],
    );
    assert_eq!(out, "WARN auth failed for [user] on [host] (session \"[session]\")");
}

#[test]
fn takes_out_user_at_host_and_the_home_folder() {
    let out = redact(
        r"ERROR cannot read C:\Users\Alice\.ssh\id_ed25519 for alice@example.org, /home/alice/x",
        Some(r"C:\Users\Alice"),
        &[],
    );
    assert!(out.contains(r"~\.ssh\id_ed25519"), "{out}");
    assert!(out.contains("[user]@[host]"), "{out}");
    assert!(!out.to_lowercase().contains("users\\alice"), "{out}");
}

#[test]
fn takes_out_the_home_folder_as_debug_formatting_writes_it() {
    let out = redact(r#"INFO App data dir: "C:\\Users\\Alice\\AppData\\Roaming""#, Some(r"C:\Users\Alice"), &[]);
    assert_eq!(out, r#"INFO App data dir: "~\\AppData\\Roaming""#);
}

#[test]
fn a_longer_name_is_replaced_before_one_inside_it() {
    let out = redact("db.prod.example.net", None, &[p("prod", "session"), p("db.prod.example.net", "host")]);
    assert_eq!(out, "[host]");
}

#[test]
fn replaces_whole_names_only() {
    let out = redact("dev on devops-1, folder C--Users-dev-x, user dev!", None, &[p("dev", "user"), p("box!", "session")]);
    assert_eq!(out, "[user] on devops-1, folder C--Users-[user]-x, user [user]!");
    assert_eq!(redact("open box! now", None, &[p("box!", "session")]), "open [session] now");
}

#[test]
fn ignores_names_too_short_to_be_safe_to_replace() {
    assert_eq!(redact("a WARN b", None, &[p("a", "user")]), "a WARN b");
}

#[test]
fn keeps_only_the_newest_part_starting_on_a_whole_line() {
    let line = "x".repeat(99) + "\n";
    let log = line.repeat(KEEP_BYTES / 100 + 50);
    let kept = newest(&log);
    assert!(kept.len() <= KEEP_BYTES);
    assert!(kept.starts_with('x') && kept.lines().all(|l| l.len() == 99));
}

#[test]
fn quotes_the_newest_warnings_and_errors_in_order() {
    let log = (0..30).map(|i| format!("t  WARN w{i}\nt  INFO i{i}\n")).collect::<String>();
    let got = problems(&log);
    assert_eq!(got.len(), QUOTED_LINES);
    assert!(got.first().unwrap().ends_with("w15") && got.last().unwrap().ends_with("w29"), "{got:?}");
}
