use super::config::PasswordManagersConfig;
use super::*;
use crate::state::{AuthMethod, ProxyConfig, SessionConfig};

fn passbolt_ref(id: &str) -> SecretRef {
    SecretRef { provider: "passbolt".into(), entry_id: id.into(), label: Some("prod db".into()), ..Default::default() }
}

const OLD_SESSION: &str = r#"{
    "id": "s1", "name": "web", "host": "example.com", "port": 22, "username": "root",
    "auth_method": { "type": "Password", "password": "hunter2" },
    "folder_id": null, "tags": [],
    "jump_chain": [
        { "host": "bastion", "port": 22, "username": "jump",
          "auth_method": { "type": "Key", "path": "/home/me/.ssh/id_ed25519", "passphrase": "pp" } }
    ],
    "proxy": { "proxy_type": "socks5", "host": "127.0.0.1", "port": 9050, "username": "u", "password": "p" }
}"#;

/// A session saved before password managers existed reads back unchanged,
/// with no references.
#[test]
fn old_sessions_read_without_refs() {
    let s: SessionConfig = serde_json::from_str(OLD_SESSION).unwrap();
    match &s.auth_method {
        AuthMethod::Password { password, password_ref } => {
            assert_eq!(password.as_deref(), Some("hunter2"));
            assert!(password_ref.is_none());
        }
        other => panic!("unexpected {other:?}"),
    }
    let hop = &s.jump_chain.as_ref().unwrap()[0];
    match &hop.auth_method {
        AuthMethod::Key { passphrase, passphrase_ref, .. } => {
            assert_eq!(passphrase.as_deref(), Some("pp"));
            assert!(passphrase_ref.is_none());
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(s.proxy.as_ref().unwrap().password_ref.is_none());
}

/// Writing a session without references emits no `*_ref` key, so an older
/// Reach reading the same vault sees exactly what it wrote.
#[test]
fn no_ref_keys_are_written_when_unset() {
    let s: SessionConfig = serde_json::from_str(OLD_SESSION).unwrap();
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("password_ref"), "{json}");
    assert!(!json.contains("passphrase_ref"), "{json}");

    let auth = serde_json::to_value(AuthMethod::Password { password: Some("x".into()), password_ref: None }).unwrap();
    assert_eq!(auth, serde_json::json!({ "type": "Password", "password": "x" }));
    let key = serde_json::to_value(AuthMethod::Key {
        path: Some("/k".into()),
        passphrase: None,
        key_content: None,
        key_id: None,
        passphrase_ref: None,
    })
    .unwrap();
    assert_eq!(key, serde_json::json!({ "type": "Key", "path": "/k" }));
    assert_eq!(serde_json::to_value(AuthMethod::Agent).unwrap(), serde_json::json!({ "type": "Agent" }));
}

#[test]
fn refs_round_trip() {
    let r = passbolt_ref("0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d");
    let auth = AuthMethod::Password { password: None, password_ref: Some(r.clone()) };
    let json = serde_json::to_value(&auth).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "type": "Password",
            "password_ref": { "provider": "passbolt", "entry_id": "0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d", "label": "prod db" }
        })
    );
    match serde_json::from_value::<AuthMethod>(json).unwrap() {
        AuthMethod::Password { password_ref, .. } => assert_eq!(password_ref, Some(r.clone())),
        other => panic!("unexpected {other:?}"),
    }

    let key = AuthMethod::Key { path: None, passphrase: None, key_content: None, key_id: Some("k1".into()), passphrase_ref: Some(r.clone()) };
    let back: AuthMethod = serde_json::from_str(&serde_json::to_string(&key).unwrap()).unwrap();
    assert!(matches!(back, AuthMethod::Key { passphrase_ref: Some(ref x), .. } if *x == r));

    let proxy = ProxyConfig {
        proxy_type: "socks5".into(),
        host: "h".into(),
        port: 1080,
        username: None,
        password: None,
        password_ref: Some(r.clone()),
    };
    let back: ProxyConfig = serde_json::from_str(&serde_json::to_string(&proxy).unwrap()).unwrap();
    assert_eq!(back.password_ref, Some(r));
}

/// A label is optional, and an empty one is not written.
#[test]
fn secret_ref_label_is_optional() {
    let r: SecretRef = serde_json::from_str(r#"{"provider":"passbolt","entry_id":"x"}"#).unwrap();
    assert_eq!(r.label, None);
    assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"provider":"passbolt","entry_id":"x"}"#);
    let r: SecretRef = serde_json::from_str(r#"{"provider":"passbolt","entry_id":"x","label":null}"#).unwrap();
    assert_eq!(r.label, None);
}

#[test]
fn db_connection_ref_is_camel_case_and_accepts_snake_case() {
    use crate::db::types::{DbConnection, DbConnectionView};
    let old: DbConnection =
        serde_json::from_str(r#"{"id":"c","name":"pg","engine":"postgres","password":"pw"}"#).unwrap();
    assert!(old.password_ref.is_none());
    assert!(!serde_json::to_string(&old).unwrap().contains("passwordRef"));

    let with_ref: DbConnection = serde_json::from_str(
        r#"{"id":"c","name":"pg","engine":"postgres","passwordRef":{"provider":"passbolt","entry_id":"e"}}"#,
    )
    .unwrap();
    assert_eq!(with_ref.password_ref.as_ref().unwrap().entry_id, "e");
    let snake: DbConnection = serde_json::from_str(
        r#"{"id":"c","name":"pg","engine":"postgres","password_ref":{"provider":"passbolt","entry_id":"e"}}"#,
    )
    .unwrap();
    assert_eq!(snake.password_ref, with_ref.password_ref);

    // The list view shows the reference (not a secret) and never a password.
    let view = serde_json::to_value(DbConnectionView::from(&old)).unwrap();
    assert!(view.get("password").is_none_or(|p| p.is_null()));
    assert_eq!(view["hasPassword"], true);
    let view = serde_json::to_value(DbConnectionView::from(&with_ref)).unwrap();
    assert_eq!(view["passwordRef"]["entry_id"], "e");
}

#[test]
fn connect_params_take_refs() {
    let rdp: crate::rdp::RdpConnectParams = serde_json::from_str(
        r#"{"id":"r","host":"h","port":3389,"username":"u","width":800,"height":600,
            "passwordRef":{"provider":"passbolt","entry_id":"e"}}"#,
    )
    .unwrap();
    assert!(rdp.password.is_empty());
    assert_eq!(rdp.password_ref.unwrap().entry_id, "e");
    // Old callers still send a password and no reference.
    let rdp: crate::rdp::RdpConnectParams = serde_json::from_str(
        r#"{"id":"r","host":"h","port":3389,"username":"u","password":"pw","width":800,"height":600}"#,
    )
    .unwrap();
    assert_eq!(rdp.password, "pw");
    assert!(rdp.password_ref.is_none());

    let vnc: crate::vnc::VncConnectParams =
        serde_json::from_str(r#"{"id":"v","host":"h","port":5900,"password_ref":{"provider":"passbolt","entry_id":"e"}}"#)
            .unwrap();
    assert_eq!(vnc.password_ref.unwrap().entry_id, "e");

    let hop: crate::ipc::ssh_commands::JumpHostConnectParams = serde_json::from_str(
        r#"{"host":"b","port":22,"username":"u","authMethod":"password",
            "passwordRef":{"provider":"passbolt","entry_id":"e1"},
            "keyPassphraseRef":{"provider":"passbolt","entry_id":"e2"}}"#,
    )
    .unwrap();
    assert_eq!(hop.password_ref.unwrap().entry_id, "e1");
    assert_eq!(hop.key_passphrase_ref.unwrap().entry_id, "e2");
    let hop: crate::ipc::ssh_commands::JumpHostConnectParams = serde_json::from_str(
        r#"{"host":"b","port":22,"username":"u","authMethod":"key","password_ref":{"provider":"passbolt","entry_id":"e1"},
            "key_passphrase_ref":{"provider":"passbolt","entry_id":"e2"}}"#,
    )
    .unwrap();
    assert!(hop.password_ref.is_some() && hop.key_passphrase_ref.is_some());
}

#[test]
fn provider_ids() {
    assert_eq!(ProviderId::parse("passbolt").unwrap(), ProviderId::Passbolt);
    assert_eq!(ProviderId::Passbolt.as_str(), "passbolt");
    assert!(ProviderId::parse("Passbolt").is_err());
    let err = ProviderId::parse("bit\u{1b}[31mwarden").unwrap_err();
    assert!(!err.contains('\u{1b}'), "{err}");
}

#[test]
fn the_gate_refuses_while_off() {
    let mut cfg = PasswordManagersConfig::default();
    assert!(!cfg.enabled, "off as shipped");
    assert_eq!(require(&cfg).unwrap_err(), DISABLED_MESSAGE);
    cfg.enabled = true;
    assert!(require(&cfg).is_ok());
}

fn entry(i: usize, name: &str, user: Option<&str>, uri: Option<&str>) -> EntrySummary {
    EntrySummary {
        id: format!("00000000-0000-4000-8000-{i:012}"),
        name: name.into(),
        username: user.map(Into::into),
        uri: uri.map(Into::into),
        folder_id: None,
    }
}

#[test]
fn filtering_matches_name_username_and_uri_case_insensitively() {
    let all = vec![
        entry(1, "Prod Database", Some("postgres"), None),
        entry(2, "Router", Some("admin"), Some("https://192.168.1.1")),
        entry(3, "Mail", None, Some("https://MAIL.example.com")),
    ];
    let names = |q: Option<&str>| filter_entries(&all, q).into_iter().map(|e| e.name).collect::<Vec<_>>();
    assert_eq!(names(None), vec!["Prod Database", "Router", "Mail"]);
    assert_eq!(names(Some("   ")), vec!["Prod Database", "Router", "Mail"]);
    assert_eq!(names(Some("prod")), vec!["Prod Database"]);
    assert_eq!(names(Some("ADMIN")), vec!["Router"]);
    assert_eq!(names(Some("mail.example")), vec!["Mail"]);
    assert!(names(Some("nothing")).is_empty());
}

#[test]
fn filtering_caps_results() {
    let all: Vec<EntrySummary> = (0..MAX_LIST_RESULTS + 250).map(|i| entry(i, "server", None, None)).collect();
    assert_eq!(filter_entries(&all, None).len(), MAX_LIST_RESULTS);
    assert_eq!(filter_entries(&all, Some("serv")).len(), MAX_LIST_RESULTS);
}

#[test]
fn display_text_is_cleaned_and_capped() {
    assert_eq!(sanitize_display("a\u{7}b\nc", 10), "abc");
    assert_eq!(sanitize_display("abcdef", 3), "abc…");
}
