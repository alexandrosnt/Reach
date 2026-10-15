use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::*;
use crate::pwmanager::config::{CredentialsMode, MfaMode, PassboltConfig, PassboltSecrets, PassphraseMode};

const UUID: &str = "0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d";
const KEY: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----\nKEYMATERIAL\n-----END PGP PRIVATE KEY BLOCK-----";
const PASS: &str = "correct horse battery staple";
const SEED: &str = "JBSWY3DPEHPK3PXP";

fn cfg(credentials: CredentialsMode, mfa_mode: MfaMode) -> PassboltConfig {
    PassboltConfig {
        cli_path: None,
        credentials,
        server_address: "https://passbolt.example.com".into(),
        mfa_mode,
        timeout_secs: 45,
        passphrase_mode: Some(PassphraseMode::Stored),
        passphrase_remember_secs: 0,
    }
}

fn secrets() -> PassboltSecrets {
    PassboltSecrets {
        private_key: Some(Zeroizing::new(KEY.into())),
        passphrase: Some(Zeroizing::new(PASS.into())),
        totp_secret: Some(Zeroizing::new(SEED.into())),
    }
}

fn isolated() -> &'static Path {
    Path::new("/reach-data/pwmanager/passbolt-isolated.toml")
}

/// Reach's environment as the tests pretend it is: what a Go program needs,
/// and things that must not reach the CLI.
const PARENT_ENV: &[(&str, &str)] = &[
    ("Path", "C:\\Windows\\system32"),
    ("PATH", "/usr/bin"),
    ("SystemRoot", "C:\\Windows"),
    ("HOME", "/home/me"),
    ("APPDATA", "C:\\Users\\me\\AppData\\Roaming"),
    ("https_proxy", "http://proxy.example:3128"),
    ("NO_PROXY", "localhost"),
    ("GODEBUG", "http2debug=2"),
    ("SSL_CERT_FILE", "/tmp/evil-ca.pem"),
    ("SSL_CERT_DIR", "/tmp/evil-ca"),
    ("RUST_LOG", "trace"),
    ("DEBUG", "1"),
    ("TLSSKIPVERIFY", "true"),
    ("tlsskipverify", "true"),
    ("USERPASSWORD", "inherited-password"),
    ("SERVERADDRESS", "https://inherited.example"),
];

fn parent_env() -> Vec<(OsString, OsString)> {
    PARENT_ENV.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))).collect()
}

fn build(c: &PassboltConfig, s: &PassboltSecrets, op: &Op) -> (Vec<String>, HashMap<String, Option<OsString>>) {
    let cmd = build_command_with_env(Path::new("/usr/bin/passbolt"), c, s, isolated(), op, parent_env()).unwrap();
    let std = cmd.as_std();
    let args = std.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    let envs = std
        .get_envs()
        .map(|(k, v)| (k.to_string_lossy().to_uppercase(), v.map(|v| v.to_os_string())))
        .collect();
    (args, envs)
}

fn assert_no_secret_in_args(args: &[String]) {
    for a in args {
        for secret in [KEY, "KEYMATERIAL", PASS, SEED] {
            assert!(!a.contains(secret), "secret in argument {a:?}");
        }
        let lower = a.to_lowercase();
        for flag in ["--userpassword", "--userprivatekey", "--mfatotptoken", "--totptoken", "--debug=true"] {
            assert!(!lower.starts_with(flag), "forbidden flag {a:?}");
        }
        if lower.starts_with("--tlsskipverify") {
            assert_eq!(a, "--tlsSkipVerify=false", "TLS verification may only be forced on");
        }
    }
}

#[test]
fn entry_ids_must_be_uuids() {
    assert_eq!(validate_entry_id(UUID).unwrap(), UUID);
    assert_eq!(validate_entry_id(&UUID.to_uppercase()).unwrap(), UUID, "re-formatted lower case");
    assert_eq!(validate_entry_id(&format!("  {UUID} ")).unwrap(), UUID);
    for bad in [
        "",
        "--debug",
        "-x",
        "--id=0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d",
        "not-a-uuid",
        "0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9",
        "0b9f3c478a354b7e9d2f1c5e6a7b8c9d",
        "{0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d}",
        "urn:uuid:0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d",
        "0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d --debug",
        "0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9z",
    ] {
        assert!(validate_entry_id(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn reach_mode_puts_secrets_in_the_environment_only() {
    let (args, envs) = build(&cfg(CredentialsMode::Reach, MfaMode::None), &secrets(), &Op::Get(UUID.into()));
    assert_no_secret_in_args(&args);
    assert_eq!(&args[..4], ["get", "resource", &format!("--id={UUID}"), "--json"]);
    assert!(args.contains(&"--debug=false".to_string()), "{args:?}");
    assert!(args.contains(&"--tlsSkipVerify=false".to_string()), "{args:?}");
    assert!(args.contains(&"--timeout=45s".to_string()), "{args:?}");
    assert!(args.contains(&"--mfaMode=none".to_string()), "{args:?}");
    let at = args.iter().position(|a| a == "--config").expect("--config");
    assert_eq!(Path::new(&args[at + 1]), isolated());

    // The environment starts empty, so a key the CLI reads is either set by
    // Reach or not there at all (not inherited).
    let get = |k: &str| envs.get(k).cloned().flatten();
    assert_eq!(get("SERVERADDRESS"), Some("https://passbolt.example.com".into()));
    assert_eq!(get("USERPRIVATEKEY"), Some(KEY.into()));
    assert_eq!(get("USERPASSWORD"), Some(PASS.into()));
    // Every other key the CLI reads is absent, not inherited.
    for key in VIPER_ENV_KEYS {
        if !["SERVERADDRESS", "USERPRIVATEKEY", "USERPASSWORD"].contains(key) {
            assert_eq!(get(key), None, "{key} must be removed");
        }
    }
}

#[test]
fn totp_mode_passes_the_seed_in_the_environment() {
    let (args, envs) = build(&cfg(CredentialsMode::Reach, MfaMode::TotpSecret), &secrets(), &Op::List);
    assert_no_secret_in_args(&args);
    assert!(args.contains(&"--mfaMode=noninteractive-totp".to_string()), "{args:?}");
    let get = |k: &str| envs.get(k).cloned().flatten();
    assert_eq!(get("MFATOTPTOKEN"), Some(SEED.into()));
    assert_eq!(get("TOTPTOKEN"), None);
    assert_eq!(get("DEBUG"), None, "inherited DEBUG=1 does not reach it");
    assert_eq!(get("TLSSKIPVERIFY"), None, "inherited TLSSKIPVERIFY=true does not reach it");

    let mut no_seed = secrets();
    no_seed.totp_secret = None;
    let c = cfg(CredentialsMode::Reach, MfaMode::TotpSecret);
    assert!(build_command(Path::new("/p"), &c, &no_seed, isolated(), &Op::List).is_err());
}

#[test]
fn listing_asks_for_metadata_columns_only() {
    let (args, _) = build(&cfg(CredentialsMode::Reach, MfaMode::None), &secrets(), &Op::List);
    assert_eq!(&args[..3], ["list", "resource", "--json"]);
    let columns: Vec<&str> = args.windows(2).filter(|w| w[0] == "-c").map(|w| w[1].as_str()).collect();
    assert_eq!(columns, ["id", "name", "username", "uri", "folder_parent_id"]);
    assert!(!args.iter().any(|a| a.contains("password") || a.contains("description")), "{args:?}");
}

#[test]
fn cli_config_mode_passes_no_credentials_and_no_config() {
    let (args, envs) = build(&cfg(CredentialsMode::CliConfig, MfaMode::None), &secrets(), &Op::List);
    assert_no_secret_in_args(&args);
    assert!(!args.iter().any(|a| a.starts_with("--config")), "{args:?}");
    assert!(!args.iter().any(|a| a.starts_with("--mfaMode")), "{args:?}");
    assert!(args.contains(&"--debug=false".to_string()));
    // The user's own configuration file may say tlsSkipVerify = true; the
    // flag on the command line wins over it.
    assert!(args.contains(&"--tlsSkipVerify=false".to_string()), "{args:?}");
    for key in VIPER_ENV_KEYS {
        assert_eq!(envs.get(*key).cloned().flatten(), None, "{key} must not be set");
    }
    // Nothing from the stored secrets either: only what is inherited.
    for (k, v) in &envs {
        if v.is_some() {
            assert!(is_inherited_env(OsStr::new(k)), "{k} set");
        }
    }
}

#[test]
fn only_the_allowlisted_environment_is_inherited() {
    for c in [cfg(CredentialsMode::Reach, MfaMode::None), cfg(CredentialsMode::CliConfig, MfaMode::None)] {
        for op in [Op::List, Op::Get(UUID.into()), Op::Version] {
            let (_, envs) = build(&c, &secrets(), &op);
            let get = |k: &str| envs.get(k).cloned().flatten().map(|v| v.to_string_lossy().into_owned());
            assert_eq!(get("HOME").as_deref(), Some("/home/me"));
            assert!(get("APPDATA").is_some());
            assert_eq!(get("NO_PROXY").as_deref(), Some("localhost"));
            if cfg!(windows) {
                // One variable whatever its case; the last one wins.
                assert!(get("PATH").is_some());
                assert_eq!(get("SYSTEMROOT").as_deref(), Some("C:\\Windows"));
                assert_eq!(get("HTTPS_PROXY").as_deref(), Some("http://proxy.example:3128"));
            } else {
                assert_eq!(get("PATH").as_deref(), Some("/usr/bin"));
                // Both spellings of the proxy variables are allowed on Unix.
                assert!(envs.iter().any(|(k, v)| k == "HTTPS_PROXY" && v.is_some()));
            }
            for leak in ["GODEBUG", "SSL_CERT_FILE", "SSL_CERT_DIR", "RUST_LOG"] {
                assert!(!envs.contains_key(leak), "{leak} inherited");
            }
            assert!(get("DEBUG").is_none() && get("TLSSKIPVERIFY").is_none());
            if c.credentials == CredentialsMode::Reach && op != Op::Version {
                assert_eq!(get("USERPASSWORD").as_deref(), Some(PASS), "ours, not the inherited one");
                assert_eq!(get("SERVERADDRESS").as_deref(), Some("https://passbolt.example.com"));
            } else {
                assert!(get("USERPASSWORD").is_none() && get("SERVERADDRESS").is_none());
            }
        }
    }
}

#[test]
fn allowlist_matching_follows_the_platform() {
    assert!(is_inherited_env(OsStr::new("PATH")));
    assert!(is_inherited_env(OsStr::new("http_proxy")));
    assert!(is_inherited_env(OsStr::new("HTTP_PROXY")));
    assert_eq!(is_inherited_env(OsStr::new("Path")), cfg!(windows));
    assert_eq!(is_inherited_env(OsStr::new("systemroot")), cfg!(windows));
    assert_eq!(is_inherited_env(OsStr::new("Http_Proxy")), cfg!(windows));
    for no in ["GODEBUG", "SSL_CERT_FILE", "DEBUG", "TLSSKIPVERIFY", "USERPASSWORD", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES", "COMSPEC", ""] {
        assert!(!is_inherited_env(OsStr::new(no)), "{no}");
    }
    for key in VIPER_ENV_KEYS {
        assert!(!is_inherited_env(OsStr::new(key)), "{key}");
    }
}

/// Not a check of its own. Run as a child by
/// `the_child_really_gets_only_the_allowlisted_environment`, it prints the
/// names of its environment variables.
#[test]
fn child_env_dump() {
    if std::env::var_os(ENV_DUMP_MARKER).is_none() {
        return;
    }
    for (k, _) in std::env::vars_os() {
        println!("ENV:{}", k.to_string_lossy());
    }
}

const ENV_DUMP_MARKER: &str = "REACH_TEST_PWM_ENV_DUMP";

/// The environment really is emptied: a child (this test binary, running
/// `child_env_dump`) sees only what the allowlist lets through, although
/// its parent had more (Reach's real environment, plus GODEBUG and friends).
#[cfg(desktop)]
#[tokio::test]
async fn the_child_really_gets_only_the_allowlisted_environment() {
    let me = std::env::current_exe().unwrap();
    let mut cmd = crate::pwmanager::process::silent_command(&me);
    let parent = std::env::vars_os().chain(parent_env().into_iter().filter(|(k, _)| {
        // Keep the real PATH and SystemRoot: the child must start.
        !k.eq_ignore_ascii_case("PATH") && !k.eq_ignore_ascii_case("SystemRoot")
    }));
    set_base_env(&mut cmd, parent);
    cmd.env(ENV_DUMP_MARKER, "1");
    cmd.args(["--exact", "pwmanager::passbolt::tests::child_env_dump", "--nocapture", "--test-threads=1"]);
    let out = crate::pwmanager::process::run(cmd, std::time::Duration::from_secs(60), 1024 * 1024, 64 * 1024)
        .await
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    let names: Vec<&str> = stdout.lines().filter_map(|l| l.strip_prefix("ENV:")).collect();
    assert!(names.contains(&ENV_DUMP_MARKER), "the dump did not run: {stdout}");
    for name in names {
        assert!(name == ENV_DUMP_MARKER || is_inherited_env(OsStr::new(name)), "{name} reached the child");
    }
}

#[test]
fn reach_mode_needs_what_it_logs_in_with() {
    let c = cfg(CredentialsMode::Reach, MfaMode::None);
    let mut s = secrets();
    s.passphrase = None;
    let e = build_command(Path::new("/p"), &c, &s, isolated(), &Op::List).unwrap_err();
    assert!(e.starts_with("Passbolt: no passphrase"), "{e}");
    let mut s = secrets();
    s.private_key = None;
    assert!(build_command(Path::new("/p"), &c, &s, isolated(), &Op::List).is_err());
    let mut c = cfg(CredentialsMode::Reach, MfaMode::None);
    c.server_address.clear();
    assert!(build_command(Path::new("/p"), &c, &secrets(), isolated(), &Op::List).is_err());
}

#[test]
fn the_version_call_logs_in_to_nothing() {
    let (args, envs) = build(&cfg(CredentialsMode::Reach, MfaMode::None), &secrets(), &Op::Version);
    assert_eq!(args, ["--version", "--debug=false"]);
    for (k, v) in &envs {
        if v.is_some() {
            assert!(is_inherited_env(OsStr::new(k)), "{k}: no credentials for --version");
        }
    }
}

#[test]
fn the_list_output_is_parsed() {
    // As go-passbolt-cli v0.5 prints it for v5 resources, with gaps.
    let stdout = br#"[
      { "id": "0b9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d", "name": "Prod DB", "username": "postgres",
        "uri": "db.example.com", "folder_parent_id": "1a2b3c4d-1111-4222-8333-444455556666" },
      { "id": "1c9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d", "name": "router\u001b[31m", "username": "",
        "uri": null, "folder_parent_id": "" },
      { "id": "2d9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d", "name": "alpha" },
      { "name": "no id" },
      { "id": "--debug", "name": "bad id" },
      { "ID": "3e9f3c47-8a35-4b7e-9d2f-1c5e6a7b8c9d", "Name": "Legacy keys", "Username": "u", "URI": "https://x" }
    ]"#;
    let entries = parse_list(stdout).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["alpha", "Legacy keys", "Prod DB", "router[31m"], "sorted, bad ids skipped, escapes stripped");
    let prod = entries.iter().find(|e| e.name == "Prod DB").unwrap();
    assert_eq!(prod.username.as_deref(), Some("postgres"));
    assert_eq!(prod.uri.as_deref(), Some("db.example.com"));
    assert_eq!(prod.folder_id.as_deref(), Some("1a2b3c4d-1111-4222-8333-444455556666"));
    let router = entries.iter().find(|e| e.name.starts_with("router")).unwrap();
    assert_eq!((router.username.as_deref(), router.uri.as_deref(), router.folder_id.as_deref()), (None, None, None));
    let alpha = entries.iter().find(|e| e.name == "alpha").unwrap();
    assert_eq!(alpha.username, None);

    assert!(parse_list(b"[]").unwrap().is_empty());
    assert!(parse_list(b"").is_err());
    assert!(parse_list(b"ID  Name\n1 x").is_err(), "table output is not JSON");
}

#[test]
fn the_get_output_is_parsed() {
    let stdout = br#"{
      "folder_parent_id": "", "name": "Prod DB", "username": "postgres", "uri": "db.example.com",
      "password": "s3cr\"et", "description": "the main one",
      "metadata": { "object_type": "PASSBOLT_RESOURCE_METADATA", "name": "Prod DB" },
      "secret": { "object_type": "PASSBOLT_SECRET_DATA", "password": "s3cr\"et" }
    }"#;
    assert_eq!(parse_get(stdout).unwrap().as_str(), "s3cr\"et");
    assert_eq!(parse_get(br#"{"name":"x","password":""}"#).unwrap_err(), "Passbolt: the entry has no password");
    assert!(parse_get(br#"{"name":"x"}"#).is_err());
    let e = parse_get(b"Password: hunter2").unwrap_err();
    assert!(!e.contains("hunter2"), "{e}");
}

#[test]
fn versions_are_read() {
    assert_eq!(
        parse_version(b"passbolt version v0.5.2 (Built on 2025-01-01 from Git SHA abc)\n").as_deref(),
        Some("v0.5.2")
    );
    assert_eq!(parse_version(b"v0.4.0\n").as_deref(), Some("v0.4.0"));
    assert_eq!(parse_version(b"").as_deref(), None);
}

#[test]
fn errors_are_explained() {
    let cases: &[(&str, &str)] = &[
        ("Error: serverAddress is not defined\n", "no server address"),
        ("Error: reading Password: EOF\n", "no passphrase"),
        ("Error: creating Client: get Private Key: unlock Key: gopenpgp: error in unlocking key\n", "wrong passphrase"),
        ("Error: logging in: MFA callback is not defined\n", "requires MFA"),
        ("Error: logging in: reading TOTP: EOF\n", "requires MFA"),
        (
            "Error: logging in: authentication failed, check your private key and password: API error (code 401): nope\n",
            "authentication failed",
        ),
        (
            "Error: getting resource: API error (code 404): The resource does not exist.\n",
            "entry not found",
        ),
        ("Error: logging in: Post \"https://pb/auth\": dial tcp: lookup pb: no such host\n", "could not reach"),
        ("Error: logging in: tls: failed to verify certificate: x509: certificate signed by unknown authority\n", "not trusted"),
    ];
    for (stderr, expect) in cases {
        let msg = friendly_error(stderr.as_bytes());
        assert!(msg.starts_with("Passbolt: "), "{msg}");
        assert!(msg.contains(expect), "{stderr:?} -> {msg}");
    }
}

#[test]
fn unknown_errors_are_cleaned_and_shortened() {
    let msg = friendly_error(b"Using something\n\x1b[31mError: \x1b[0msomething odd\x07 happened\r\n");
    assert_eq!(msg, "Passbolt: something odd happened");
    // The last error line wins.
    let msg = friendly_error(b"Error: first\nWarning: x\nError: second\n");
    assert_eq!(msg, "Passbolt: second");
    // No Error: line: the last line.
    assert_eq!(friendly_error(b"panic: boom\n"), "Passbolt: panic: boom");
    assert_eq!(friendly_error(b""), "Passbolt: the CLI failed without saying why.");
    let long = format!("Error: {}\n", "x".repeat(2000));
    let msg = friendly_error(long.as_bytes());
    assert!(msg.chars().count() <= "Passbolt: ".len() + 301, "{}", msg.len());
    assert!(msg.ends_with('…'));
    // Multi-byte text is cut on a character boundary.
    let wide = format!("Error: {}\n", "é".repeat(1000));
    assert!(friendly_error(wide.as_bytes()).ends_with('…'));
}

#[test]
fn control_characters_are_stripped() {
    assert_eq!(strip_control("a\x1b[1;31mb\x1b[0m\tc\u{7}d\ne"), "abcd\ne");
    assert_eq!(strip_control("\x1b]0;title\x07x"), "0;titlex", "only the ESC and the next char of non-CSI escapes go");
}

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("reach-pwm-cli-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

#[test]
fn a_configured_cli_path_must_be_an_absolute_file() {
    assert!(resolve_binary(Some("passbolt")).is_err(), "relative");
    let missing = std::env::temp_dir().join(format!("no-passbolt-{}", uuid::Uuid::new_v4())).join("passbolt");
    assert!(resolve_binary(Some(missing.to_str().unwrap())).unwrap_err().contains("go-passbolt-cli"));
    let dir = temp_dir();
    let as_dir = dir.join("passbolt");
    std::fs::create_dir_all(&as_dir).unwrap();
    assert!(resolve_binary(Some(as_dir.to_str().unwrap())).is_err(), "a directory");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_must_be_called_passbolt() {
    let dir = temp_dir();
    for name in ["passbolt", "passbolt.exe", "PassBolt.EXE"] {
        let path = dir.join(name);
        std::fs::write(&path, b"binary").unwrap();
        let found = resolve_binary(Some(path.to_str().unwrap())).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(found.file_name().unwrap().eq_ignore_ascii_case(name), "{found:?}");
        let _ = std::fs::remove_file(&path);
    }
    // Scripts and other programs are refused even though the file exists.
    for name in ["passbolt.cmd", "passbolt.bat", "passbolt.ps1", "passbolt.vbs", "passbolt.js", "passbolt.sh", "cmd.exe", "passbolt.exe.bat"] {
        let path = dir.join(name);
        std::fs::write(&path, b"@echo off").unwrap();
        let e = resolve_binary(Some(path.to_str().unwrap())).unwrap_err();
        assert!(e.contains("passbolt program"), "{name}: {e}");
    }
    let me = std::env::current_exe().unwrap();
    assert!(resolve_binary(Some(me.to_str().unwrap())).is_err(), "any other program");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cli_on_a_network_share_is_refused_unopened() {
    for unc in [
        r"\\server\share\passbolt.exe",
        r"\\?\UNC\server\share\passbolt.exe",
        r"\\.\pipe\passbolt",
        "//server/share/passbolt",
    ] {
        let e = resolve_binary(Some(unc)).unwrap_err();
        assert!(e.contains("network share"), "{unc}: {e}");
        assert!(check_cli_path_shape(Path::new(unc)).is_err());
    }
    assert!(is_network_path(r"\\h\s"));
    assert!(is_network_path("  //h/s"));
    assert!(!is_network_path(r"C:\Tools\passbolt.exe"));
    assert!(!is_network_path("/usr/bin/passbolt"));
}

#[cfg(unix)]
#[test]
fn a_cli_others_can_change_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir();
    let bin = dir.join("passbolt");
    std::fs::write(&bin, b"binary").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(resolve_binary(Some(bin.to_str().unwrap())).is_ok());

    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o775)).unwrap();
    assert!(resolve_binary(Some(bin.to_str().unwrap())).unwrap_err().contains("other users"), "group writable");
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o757)).unwrap();
    assert!(resolve_binary(Some(bin.to_str().unwrap())).is_err(), "world writable");
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(resolve_binary(Some(bin.to_str().unwrap())).unwrap_err().contains("folder"), "its folder");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    // A symlink is followed: what runs is the file that was checked.
    let link_dir = temp_dir();
    let link = link_dir.join("passbolt");
    std::os::unix::fs::symlink(&bin, &link).unwrap();
    assert_eq!(resolve_binary(Some(link.to_str().unwrap())).unwrap(), std::fs::canonicalize(&bin).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&link_dir);
}

#[test]
fn configured_needs_the_stored_secrets() {
    let p = Passbolt::new(cfg(CredentialsMode::Reach, MfaMode::None), secrets(), true);
    assert!(p.configured());
    let mut s = secrets();
    s.passphrase = None;
    assert!(!Passbolt::new(cfg(CredentialsMode::Reach, MfaMode::None), s, true).configured());
    let mut s = secrets();
    s.totp_secret = None;
    assert!(!Passbolt::new(cfg(CredentialsMode::Reach, MfaMode::TotpSecret), s, true).configured());
    assert!(Passbolt::new(cfg(CredentialsMode::CliConfig, MfaMode::None), PassboltSecrets::default(), true).configured());
}

/// A real Passbolt server. Run with
/// `cargo test --lib passbolt_live -- --ignored --nocapture` and
/// REACH_TEST_PASSBOLT_SERVER, REACH_TEST_PASSBOLT_KEY_FILE,
/// REACH_TEST_PASSBOLT_PASSPHRASE, REACH_TEST_PASSBOLT_ENTRY_ID (and, if the
/// CLI is not on PATH, REACH_TEST_PASSBOLT_CLI) set.
#[cfg(desktop)]
#[tokio::test]
#[ignore = "needs a Passbolt server and go-passbolt-cli"]
async fn passbolt_live() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let key = crate::pwmanager::config::read_private_key_file(&var("REACH_TEST_PASSBOLT_KEY_FILE")).unwrap();
    let cfg = PassboltConfig {
        cli_path: std::env::var("REACH_TEST_PASSBOLT_CLI").ok(),
        credentials: CredentialsMode::Reach,
        server_address: crate::pwmanager::config::validate_server_address(&var("REACH_TEST_PASSBOLT_SERVER")).unwrap(),
        mfa_mode: MfaMode::None,
        timeout_secs: 60,
        passphrase_mode: Some(PassphraseMode::Stored),
        passphrase_remember_secs: 0,
    };
    let secrets = PassboltSecrets {
        private_key: Some(key),
        passphrase: Some(Zeroizing::new(var("REACH_TEST_PASSBOLT_PASSPHRASE"))),
        totp_secret: None,
    };
    let p = Passbolt::new(cfg, secrets, true);
    let status = p.status().await;
    println!("status: available={} version={:?} detail={:?}", status.available, status.version, status.detail);
    assert!(status.available);

    let entries = p.list(None).await.unwrap();
    println!("{} entries", entries.len());
    let id = var("REACH_TEST_PASSBOLT_ENTRY_ID");
    assert!(entries.iter().any(|e| e.id == validate_entry_id(&id).unwrap()), "the entry is listed");
    let secret = p.resolve(&id, None).await.unwrap();
    assert!(!secret.is_empty());
    println!("resolved a password of {} characters", secret.chars().count());

    let missing = p.resolve("00000000-0000-4000-8000-000000000000", None).await.unwrap_err().into_message();
    println!("missing entry: {missing}");
    assert!(missing.starts_with("Passbolt: "));
}

fn asking(remember_secs: u32) -> PassboltConfig {
    PassboltConfig {
        passphrase_mode: Some(PassphraseMode::Ask),
        passphrase_remember_secs: remember_secs,
        ..cfg(CredentialsMode::Reach, MfaMode::None)
    }
}

#[test]
fn a_typed_passphrase_goes_in_the_environment_only() {
    const TYPED: &str = "typed in the window, not stored";
    // A stored passphrase left over is not what an asking setup uses.
    let p = Passbolt::new(asking(900), secrets(), true);
    let typed = crate::pwmanager::unlock::Passphrase::new(TYPED.into()).unwrap();
    let call = p.call_secrets(Some(&typed)).expect("asking mode builds its own secrets");
    let (args, envs) = build(&p.cfg, &call, &Op::Get(UUID.into()));
    assert_no_secret_in_args(&args);
    for a in &args {
        assert!(!a.contains(TYPED), "typed passphrase in argument {a:?}");
    }
    let get = |k: &str| envs.get(k).cloned().flatten();
    assert_eq!(get("USERPASSWORD"), Some(TYPED.into()));
    assert_eq!(get("USERPRIVATEKEY"), Some(KEY.into()));

    // Nothing typed: no passphrase at all (not the stored one), and the
    // error does not send the user to Settings to store one.
    let none = p.call_secrets(None).unwrap();
    assert!(none.passphrase.is_none());
    let e = build_command(Path::new("/p"), &p.cfg, &none, isolated(), &Op::List).unwrap_err();
    assert_eq!(e, "Passbolt: the passphrase was not entered.");
    assert!(!e.contains("Settings"), "{e}");

    // A stored passphrase is used as it is.
    let stored = Passbolt::new(cfg(CredentialsMode::Reach, MfaMode::None), secrets(), true);
    assert!(stored.call_secrets(Some(&typed)).is_none());
    assert_eq!(stored.passphrase_prompt(), None);
}

#[test]
fn asking_counts_as_configured_and_says_how_long_to_keep_it() {
    let mut s = secrets();
    s.passphrase = None;
    let p = Passbolt::new(asking(300), s.clone(), true);
    assert!(p.configured(), "no stored passphrase is needed when it is asked for");
    assert_eq!(p.passphrase_prompt(), Some(300));
    // A hand-edited time is cut to the most Reach keeps it.
    assert_eq!(Passbolt::new(asking(999_999), s.clone(), true).passphrase_prompt(), Some(crate::pwmanager::config::MAX_REMEMBER_SECS));
    // The private key is still needed.
    let mut no_key = s;
    no_key.private_key = None;
    assert!(!Passbolt::new(asking(300), no_key, true).configured());
    // With the CLI's own configuration, Reach asks nothing.
    let mut cli = asking(300);
    cli.credentials = CredentialsMode::CliConfig;
    assert_eq!(Passbolt::new(cli, PassboltSecrets::default(), true).passphrase_prompt(), None);
}

#[test]
fn a_rejected_passphrase_is_told_apart() {
    let wrong = b"Error: creating Client: get Private Key: unlock Key: gopenpgp: error in unlocking key\n";
    assert_eq!(classify_error(wrong).0, CliErrorKind::WrongPassphrase);
    assert!(matches!(failure(wrong), Failure::WrongPassphrase(m) if m.contains("wrong passphrase")));
    for other in [
        &b"Error: getting resource: API error (code 404): The resource does not exist.\n"[..],
        b"Error: logging in: MFA callback is not defined\n",
        b"Error: reading Password: EOF\n",
        b"",
    ] {
        assert_eq!(classify_error(other).0, CliErrorKind::Other, "{}", String::from_utf8_lossy(other));
        assert!(matches!(failure(other), Failure::Other(_)));
    }
}

#[test]
fn a_wrong_passphrase_is_recognised_only_from_the_clis_own_error_line() {
    let wrong = |s: &str| classify_error(s.as_bytes()).0 == CliErrorKind::WrongPassphrase;
    // The CLI's own chain, as cobra prints it, in any case.
    assert!(wrong("Error: creating Client: get Private Key: unlock Key: gopenpgp: error in unlocking key\n"));
    assert!(wrong("Error: Creating client: get private key: unlock key: openpgp: invalid data\n"));
    assert!(wrong("\x1b[31mError:\x1b[0m creating Client: get Private Key: unlock Key: x\n"));
    // The same words anywhere else are not it: in what a server sent, in
    // an earlier line, after another prefix, or for another key.
    assert!(!wrong("Error: getting resource: API error (code 500): unlock Key: get Private Key\n"));
    assert!(!wrong("note: creating Client: get Private Key: unlock Key: x\nError: logging in: API error (code 401): nope\n"));
    assert!(!wrong("Error: creating Client: get Private Key: unlock Key: x\nError: logging out: EOF\n"));
    assert!(!wrong("Warning Error: creating Client: get Private Key: unlock Key: x\n"));
    assert!(!wrong("Error: getting resource: get Metadata Private Key: unlock Key: gopenpgp: error in unlocking key\n"));
    assert!(!wrong("creating Client: get Private Key: unlock Key: x\n"), "no Error: line at all");
    // A key that cannot be read is not a passphrase to ask for again.
    let (kind, msg) = classify_error(b"Error: creating Client: get Private Key: key From Armored: openpgp: invalid argument\n");
    assert_eq!(kind, CliErrorKind::Other);
    assert!(msg.contains("could not be read"), "{msg}");
}

#[tokio::test]
async fn the_command_is_built_only_once_a_cli_slot_is_free() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let built = std::sync::Arc::new(AtomicBool::new(false));
    let (free, slot) = tokio::sync::oneshot::channel::<()>();
    let flag = built.clone();
    let task = tokio::spawn(build_when_ready(
        async move {
            slot.await.map_err(|_| "no slot".to_string())?;
            Ok::<_, String>("permit")
        },
        move || {
            flag.store(true, Ordering::SeqCst);
            Ok("command")
        },
    ));
    // Waiting for its turn: nothing built, so no secret is copied anywhere.
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!built.load(Ordering::SeqCst));
    free.send(()).unwrap();
    assert_eq!(task.await.unwrap().unwrap(), ("permit", "command"));
    assert!(built.load(Ordering::SeqCst));
    // No slot: nothing is ever built.
    let never = std::sync::Arc::new(AtomicBool::new(false));
    let flag = never.clone();
    let e = build_when_ready(async { Err::<(), _>("Passbolt: shutting down".to_string()) }, move || {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap_err();
    assert_eq!(e, "Passbolt: shutting down");
    assert!(!never.load(Ordering::SeqCst));
}

#[test]
fn the_passphrase_question_shows_the_server_host_only() {
    let mut c = asking(300);
    c.server_address = "https://pb.example.com:8443/team".into();
    assert_eq!(Passbolt::new(c.clone(), secrets(), true).server_host().as_deref(), Some("pb.example.com"));
    c.server_address = String::new();
    assert_eq!(Passbolt::new(c, secrets(), true).server_host(), None);
    // With the CLI's own configuration Reach does not know the server.
    let mut cli = asking(300);
    cli.credentials = CredentialsMode::CliConfig;
    assert_eq!(Passbolt::new(cli, PassboltSecrets::default(), true).server_host(), None);
}
