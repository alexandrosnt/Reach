use super::*;

const KEY: &str = "-----BEGIN PGP PRIVATE KEY BLOCK-----\n\nxcLYBGZ...\n=abcd\n-----END PGP PRIVATE KEY BLOCK-----\n";

fn input(json: serde_json::Value) -> PasswordManagersConfigInput {
    serde_json::from_value(json).unwrap()
}

#[test]
fn server_address_must_be_https_without_credentials() {
    assert_eq!(validate_server_address("").unwrap(), "");
    assert_eq!(validate_server_address("  https://passbolt.example.com/  ").unwrap(), "https://passbolt.example.com");
    assert_eq!(validate_server_address("https://example.com:8443/passbolt").unwrap(), "https://example.com:8443/passbolt");
    // Stored as the URL parser reads it: lower-case host, no default port.
    assert_eq!(validate_server_address("HTTPS://PassBolt.Example.COM:443/").unwrap(), "https://passbolt.example.com");
    assert_eq!(validate_server_address("https://pb.example.com/a/../b/").unwrap(), "https://pb.example.com/b");

    for bad in [
        "http://passbolt.example.com",
        "HTTP://passbolt.example.com",
        "ftp://passbolt.example.com",
        "https://user:pw@passbolt.example.com",
        "https://user@passbolt.example.com",
        "passbolt.example.com",
        "--serverAddress=https://evil",
        "https://",
        "https://exa mple.com",
        "https://example.com\n--debug",
        "https://example.com/?x=1",
        "https://example.com/#frag",
        "javascript:alert(1)",
        "garbage",
    ] {
        assert!(validate_server_address(bad).is_err(), "accepted {bad:?}");
    }
    assert!(validate_server_address(&format!("https://{}.com", "a".repeat(3000))).is_err());
}

#[test]
fn private_keys_must_be_armored_openpgp_private_keys() {
    assert!(validate_private_key(KEY).is_ok());
    assert!(validate_private_key("-----BEGIN PGP PUBLIC KEY BLOCK-----\n...\n-----END PGP PUBLIC KEY BLOCK-----").is_err());
    assert!(validate_private_key("-----BEGIN OPENSSH PRIVATE KEY-----\n...\n-----END OPENSSH PRIVATE KEY-----").is_err());
    assert!(validate_private_key("").is_err());
    // The end marker before the start is not a key.
    assert!(validate_private_key("-----END PGP PRIVATE KEY BLOCK-----\n-----BEGIN PGP PRIVATE KEY BLOCK-----").is_err());
    assert!(validate_private_key(&format!("{KEY}\0")).is_err());
    let huge = format!("{KEY}{}", "A".repeat(MAX_PRIVATE_KEY_BYTES as usize));
    assert!(validate_private_key(&huge).is_err());
}

#[test]
fn private_key_files_are_read_and_checked() {
    let dir = std::env::temp_dir().join(format!("reach-pwm-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let good = dir.join("kit.txt");
    std::fs::write(&good, KEY).unwrap();
    assert_eq!(read_private_key_file(good.to_str().unwrap()).unwrap().as_str(), KEY);

    let not_key = dir.join("id_ed25519");
    std::fs::write(&not_key, "-----BEGIN OPENSSH PRIVATE KEY-----\n").unwrap();
    assert!(read_private_key_file(not_key.to_str().unwrap()).is_err());

    let big = dir.join("big.asc");
    std::fs::write(&big, vec![b'A'; MAX_PRIVATE_KEY_BYTES as usize + 1]).unwrap();
    assert!(read_private_key_file(big.to_str().unwrap()).unwrap_err().contains("too large"));
    // A key at the cap is read (and, by a check in config.rs, fits in one
    // Windows environment variable).
    let at_cap = dir.join("at-cap.asc");
    let padded = format!("{KEY}{}", "A".repeat(MAX_PRIVATE_KEY_BYTES as usize - KEY.len()));
    std::fs::write(&at_cap, &padded).unwrap();
    assert_eq!(read_private_key_file(at_cap.to_str().unwrap()).unwrap().len(), MAX_PRIVATE_KEY_BYTES as usize);

    let binary = dir.join("bin.gpg");
    std::fs::write(&binary, [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
    assert!(read_private_key_file(binary.to_str().unwrap()).is_err());

    assert!(read_private_key_file(dir.to_str().unwrap()).is_err(), "a directory");
    assert!(read_private_key_file(dir.join("missing").to_str().unwrap()).is_err());
    assert!(read_private_key_file("kit.txt").unwrap_err().contains("full path"), "relative");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn private_key_files_on_network_shares_are_refused_unopened() {
    for unc in [r"\\attacker.example\share\kit.txt", r"\\?\UNC\attacker\share\kit.txt", "//attacker/share/kit.txt", r"  \\host\s\k"] {
        let e = read_private_key_file(unc).unwrap_err();
        assert!(e.contains("network share"), "{unc}: {e}");
    }
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "private_key_file": r"\\attacker\share\kit.txt" } }));
    assert!(validate_input(&mut i, None).is_err());
}

#[test]
fn a_pasted_key_has_the_same_cap() {
    let huge = format!("{KEY}{}", "A".repeat(MAX_PRIVATE_KEY_BYTES as usize));
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "private_key": huge } }));
    assert!(validate_input(&mut i, None).unwrap_err().contains("too large"));
}

/// An absolute path on this platform that has the CLI's name but no file.
fn missing_cli() -> String {
    std::env::temp_dir()
        .join(format!("reach-no-cli-{}", uuid::Uuid::new_v4()))
        .join(if cfg!(windows) { "passbolt.exe" } else { "passbolt" })
        .to_string_lossy()
        .into_owned()
}

#[test]
fn the_cli_path_must_name_the_passbolt_program() {
    let dir = std::env::temp_dir();
    let at = |name: &str| dir.join(name).to_string_lossy().into_owned();
    for ok in ["passbolt", "passbolt.exe", "PASSBOLT.EXE", "Passbolt"] {
        assert_eq!(validate_cli_path(Some(&at(ok)), false).unwrap(), Some(at(ok)), "{ok}");
    }
    for bad in ["passbolt.cmd", "passbolt.bat", "passbolt.ps1", "passbolt.vbs", "passbolt.js", "passbolt.sh", "evil.exe", "passbolt.exe.cmd", "cmd.exe"] {
        let e = validate_cli_path(Some(&at(bad)), false).unwrap_err();
        assert!(e.contains("passbolt program"), "{bad}: {e}");
    }
    for unc in [r"\\server\share\passbolt.exe", "//server/share/passbolt", r"\\?\UNC\server\share\passbolt.exe"] {
        let e = validate_cli_path(Some(unc), true).unwrap_err();
        assert!(e.contains("network share"), "{unc}: {e}");
    }
    assert_eq!(validate_cli_path(Some("   "), true).unwrap(), None, "empty: PATH");
    assert!(validate_cli_path(Some("passbolt"), false).is_err(), "relative");
}

#[test]
fn the_cli_file_is_checked_only_when_it_matters() {
    let missing = missing_cli();
    // Off: a CLI that was uninstalled does not stop the save.
    let mut i = input(serde_json::json!({ "enabled": false, "passbolt": { "cli_path": missing } }));
    assert_eq!(validate_input(&mut i, None).unwrap().config.passbolt.cli_path.as_deref(), Some(missing.as_str()));
    // On, unchanged: not looked at again.
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "cli_path": missing } }));
    assert!(validate_input(&mut i, Some(&missing)).is_ok());
    // On, new: it must be there.
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "cli_path": missing } }));
    assert!(validate_input(&mut i, None).unwrap_err().contains("no CLI"));
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "cli_path": missing } }));
    assert!(validate_input(&mut i, Some("/elsewhere/passbolt")).is_err());
    // The name rule holds even when the file is not looked at.
    let mut i = input(serde_json::json!({ "enabled": false, "passbolt": { "cli_path": missing.replace("passbolt", "passbolt.cmd") } }));
    assert!(validate_input(&mut i, None).is_err());
}

#[test]
fn the_cli_path_is_not_kept_in_the_synced_vault() {
    let config = PasswordManagersConfig {
        enabled: true,
        passbolt: PassboltConfig { cli_path: Some("/usr/bin/passbolt".into()), ..Default::default() },
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.contains("cli_path") && !json.contains("/usr/bin"), "{json}");
    // One written by an earlier build is ignored.
    let c = parse_config(r#"{"enabled":true,"passbolt":{"cli_path":"\\\\evil\\share\\passbolt.exe"}}"#).unwrap();
    assert!(c.enabled);
    assert_eq!(c.passbolt.cli_path, None);
}

#[test]
fn device_settings_round_trip() {
    let dir = std::env::temp_dir().join(format!("reach-pwm-device-{}", uuid::Uuid::new_v4()));
    let path = dir.join("pwmanager").join("device.json");
    assert_eq!(read_device_settings(&path), DeviceSettings::default(), "missing");
    let settings = DeviceSettings { passbolt_cli_path: Some(missing_cli()) };
    write_device_settings(&path, &settings).unwrap();
    assert_eq!(read_device_settings(&path), settings);
    write_device_settings(&path, &DeviceSettings::default()).unwrap();
    assert_eq!(read_device_settings(&path).passbolt_cli_path, None);
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(read_device_settings(&path), DeviceSettings::default(), "unreadable");
    assert!(!path.with_extension("json.tmp").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn totp_secrets_are_base32() {
    assert_eq!(normalize_totp_secret("jbsw y3dp ehpk 3pxp").unwrap().as_str(), "JBSWY3DPEHPK3PXP");
    assert_eq!(normalize_totp_secret("JBSWY3DPEHPK3PXP====").unwrap().as_str(), "JBSWY3DPEHPK3PXP====");
    assert!(normalize_totp_secret("123456").is_err(), "a code, not the seed");
    assert!(normalize_totp_secret("JBSWY3DPEHPK3PX1").is_err(), "1 is not base32");
    assert!(normalize_totp_secret("JBSWY3DPEHPK3PX!").is_err());
}

#[test]
fn timeouts_are_clamped() {
    assert_eq!(clamp_timeout(0), MIN_TIMEOUT_SECS);
    assert_eq!(clamp_timeout(60), 60);
    assert_eq!(clamp_timeout(100_000), MAX_TIMEOUT_SECS);
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "timeout_secs": 1 } }));
    assert_eq!(validate_input(&mut i, None).unwrap().config.passbolt.timeout_secs, MIN_TIMEOUT_SECS);
    // A hand-edited stored value is clamped on read.
    let c = parse_config(r#"{"enabled":true,"passbolt":{"timeout_secs":9999}}"#).unwrap();
    assert_eq!(c.passbolt.timeout_secs, MAX_TIMEOUT_SECS);
}

#[test]
fn stored_config_defaults() {
    // No Passbolt settings at all: a new setup, which asks for the
    // passphrase. (A configuration from before the passphrase mode always
    // has its Passbolt object; see the back-compat test below.)
    let c = parse_config("{}").unwrap();
    assert_eq!(c, PasswordManagersConfig::default());
    assert_eq!(c.passbolt.passphrase_mode, Some(PassphraseMode::Ask));
    assert!(!c.enabled);
    assert_eq!(c.passbolt.credentials, CredentialsMode::Reach);
    assert_eq!(c.passbolt.mfa_mode, MfaMode::None);
    assert_eq!(c.passbolt.timeout_secs, DEFAULT_TIMEOUT_SECS);
    assert!(parse_config("not json").is_none());

    let json = serde_json::to_value(PasswordManagersConfig {
        enabled: true,
        passbolt: PassboltConfig { credentials: CredentialsMode::CliConfig, mfa_mode: MfaMode::TotpSecret, ..Default::default() },
    })
    .unwrap();
    assert_eq!(json["passbolt"]["credentials"], "cli_config");
    assert_eq!(json["passbolt"]["mfa_mode"], "totp_secret");
}

#[test]
fn absent_secrets_are_kept_and_clears_win() {
    let mut i = input(serde_json::json!({
        "enabled": true,
        "passbolt": { "cli_path": "", "credentials": "reach", "server_address": "https://pb.example.com", "mfa_mode": "none", "timeout_secs": 60 }
    }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(v.private_key, SecretChange::Keep));
    assert!(matches!(v.passphrase, SecretChange::Keep));
    assert!(matches!(v.totp_secret, SecretChange::Keep));
    assert_eq!(v.config.passbolt.cli_path, None);

    let mut i = input(serde_json::json!({
        "enabled": true,
        "passbolt": { "server_address": "", "private_key": KEY, "passphrase": "secret", "totp_secret": "",
                      "clear_totp_secret": true }
    }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(&v.private_key, SecretChange::Set(k) if k.as_str() == KEY.trim()));
    assert!(matches!(&v.passphrase, SecretChange::Set(p) if p.as_str() == "secret"));
    assert!(matches!(v.totp_secret, SecretChange::Clear));
    // The input no longer holds what was taken from it.
    assert!(i.passbolt.passphrase.is_none() && i.passbolt.private_key.is_none());

    let mut i = input(serde_json::json!({ "enabled": false, "passbolt": { "passphrase": "", "clear_passphrase": true, "passphrase_unused": 1 } }));
    assert!(matches!(validate_input(&mut i, None).unwrap().passphrase, SecretChange::Clear));
}

#[test]
fn bad_input_is_refused_before_anything_is_stored() {
    for bad in [
        serde_json::json!({ "enabled": true, "passbolt": { "server_address": "http://pb.example.com" } }),
        serde_json::json!({ "enabled": true, "passbolt": { "private_key": "not a key" } }),
        serde_json::json!({ "enabled": true, "passbolt": { "totp_secret": "123456" } }),
        serde_json::json!({ "enabled": true, "passbolt": { "cli_path": "passbolt" } }),
        serde_json::json!({ "enabled": true, "passbolt": { "private_key_file": "Z:\\definitely\\missing\\key.asc" } }),
    ] {
        let mut i = input(bad.clone());
        assert!(validate_input(&mut i, None).is_err(), "accepted {bad}");
    }
}

#[test]
fn the_view_says_which_secrets_exist_and_never_what_they_are() {
    let loaded = Loaded {
        config: PasswordManagersConfig { enabled: true, passbolt: PassboltConfig::default() },
        passbolt_secrets: PassboltSecrets {
            private_key: Some(Zeroizing::new(KEY.into())),
            passphrase: Some(Zeroizing::new("hunter2-passphrase".into())),
            totp_secret: None,
        },
    };
    let json = serde_json::to_string(&PasswordManagersView::from(&loaded)).unwrap();
    assert!(json.contains(r#""has_private_key":true"#), "{json}");
    assert!(json.contains(r#""has_passphrase":true"#), "{json}");
    assert!(json.contains(r#""has_totp_secret":false"#), "{json}");
    assert!(!json.contains("hunter2-passphrase") && !json.contains("PGP"), "{json}");

    // Debug does not print them either.
    let debug = format!("{loaded:?}");
    assert!(!debug.contains("hunter2-passphrase") && !debug.contains("PGP"), "{debug}");
    let change = format!("{:?}", SecretChange::Set(Zeroizing::new("hunter2-passphrase".into())));
    assert!(!change.contains("hunter2"), "{change}");
}

#[test]
fn reserved_keys() {
    assert!(is_reserved_setting_key(CONFIG_KEY));
    assert!(is_reserved_setting_key(PASSBOLT_PASSPHRASE));
    assert!(is_reserved_setting_key("pwmanager.anything"));
    assert!(!is_reserved_setting_key("openrouter_api_key"));
}

#[test]
fn a_new_setup_asks_for_the_passphrase_and_keeps_it_fifteen_minutes() {
    let c = PassboltConfig::default();
    assert_eq!(c.passphrase_mode(), PassphraseMode::Ask);
    assert_eq!(c.remember_secs(), 900);
    assert!(c.asks_passphrase());
    let json = serde_json::to_value(PasswordManagersConfig::default()).unwrap();
    assert_eq!(json["passbolt"]["passphrase_mode"], "ask");
    assert_eq!(json["passbolt"]["passphrase_remember_secs"], 900);
    // With the CLI's own configuration the passphrase is not Reach's to ask.
    let cli = PassboltConfig { credentials: CredentialsMode::CliConfig, ..Default::default() };
    assert!(!cli.asks_passphrase());
}

#[test]
fn an_old_configuration_keeps_a_stored_passphrase_and_asks_otherwise() {
    // Saved before the setting existed: no passphrase_mode key.
    let old = r#"{"enabled":true,"passbolt":{"credentials":"reach","server_address":"https://pb.example.com","mfa_mode":"none","timeout_secs":60}}"#;
    let mut with_stored = parse_config(old).unwrap();
    assert_eq!(with_stored.passbolt.passphrase_mode, None);
    assert_eq!(with_stored.passbolt.passphrase_remember_secs, DEFAULT_REMEMBER_SECS);
    with_stored.passbolt.settle_passphrase_mode(true);
    assert_eq!(with_stored.passbolt.passphrase_mode, Some(PassphraseMode::Stored));
    assert!(!with_stored.passbolt.asks_passphrase());

    let mut without = parse_config(old).unwrap();
    without.passbolt.settle_passphrase_mode(false);
    assert_eq!(without.passbolt.passphrase_mode, Some(PassphraseMode::Ask));

    // A mode that was saved is never second-guessed.
    let mut asked = parse_config(r#"{"passbolt":{"passphrase_mode":"ask","passphrase_remember_secs":0}}"#).unwrap();
    asked.passbolt.settle_passphrase_mode(true);
    assert_eq!(asked.passbolt.passphrase_mode, Some(PassphraseMode::Ask));
    assert_eq!(asked.passbolt.remember_secs(), 0);
    let mut stored = parse_config(r#"{"passbolt":{"passphrase_mode":"stored"}}"#).unwrap();
    stored.passbolt.settle_passphrase_mode(false);
    assert_eq!(stored.passbolt.passphrase_mode, Some(PassphraseMode::Stored));
}

#[test]
fn remember_times_are_clamped() {
    assert_eq!(clamp_remember(0), 0);
    assert_eq!(clamp_remember(14_400), 14_400);
    assert_eq!(clamp_remember(u32::MAX), MAX_REMEMBER_SECS);
    let c = parse_config(r#"{"passbolt":{"passphrase_remember_secs":99999999}}"#).unwrap();
    assert_eq!(c.passbolt.passphrase_remember_secs, MAX_REMEMBER_SECS);
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "ask", "passphrase_remember_secs": 99999999 } }));
    assert_eq!(validate_input(&mut i, None).unwrap().config.passbolt.passphrase_remember_secs, MAX_REMEMBER_SECS);
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "ask", "passphrase_remember_secs": 0 } }));
    assert_eq!(validate_input(&mut i, None).unwrap().config.passbolt.passphrase_remember_secs, 0);
}

#[test]
fn asking_removes_a_stored_passphrase_and_never_stores_one() {
    let mut i = input(serde_json::json!({
        "enabled": true,
        "passbolt": { "server_address": "https://pb.example.com", "passphrase_mode": "ask", "passphrase_remember_secs": 300 }
    }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(v.passphrase, SecretChange::ClearIfWasStored), "{:?}", v.passphrase);
    assert_eq!(v.config.passbolt.passphrase_mode, Some(PassphraseMode::Ask));
    assert_eq!(v.config.passbolt.passphrase_remember_secs, 300);
    // Switching from stored to asked removes the stored passphrase.
    assert!(matches!(passphrase_effect(&v.passphrase, PassphraseMode::Stored), SecretChange::Clear));

    // A passphrase sent along anyway is dropped, not stored.
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "ask", "passphrase": "do not keep me" } }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(v.passphrase, SecretChange::ClearIfWasStored));
    assert!(matches!(passphrase_effect(&v.passphrase, PassphraseMode::Stored), SecretChange::Clear));
    assert!(matches!(passphrase_effect(&v.passphrase, PassphraseMode::Ask), SecretChange::Keep));
    assert!(i.passbolt.passphrase.is_none());

    // Stored: as before, set, kept or cleared as asked.
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "stored", "passphrase": "keep me" } }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(&v.passphrase, SecretChange::Set(p) if p.as_str() == "keep me"));
    assert_eq!(v.config.passbolt.passphrase_mode, Some(PassphraseMode::Stored));
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "stored" } }));
    assert!(matches!(validate_input(&mut i, None).unwrap().passphrase, SecretChange::Keep));
}

#[test]
fn passphrases_are_capped_in_bytes() {
    let fits = "é".repeat(MAX_PASSPHRASE_BYTES / 2);
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "stored", "passphrase": fits } }));
    assert!(validate_input(&mut i, None).is_ok());
    // Fewer characters than the cap, more bytes.
    let too_long = "é".repeat(MAX_PASSPHRASE_BYTES / 2 + 1);
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "stored", "passphrase": too_long } }));
    assert!(validate_input(&mut i, None).is_err());
}

#[test]
fn the_view_shows_the_passphrase_mode() {
    let loaded = Loaded {
        config: PasswordManagersConfig {
            enabled: true,
            passbolt: PassboltConfig { passphrase_mode: Some(PassphraseMode::Ask), passphrase_remember_secs: 3600, ..Default::default() },
        },
        passbolt_secrets: PassboltSecrets::default(),
    };
    let json = serde_json::to_value(PasswordManagersView::from(&loaded)).unwrap();
    assert_eq!(json["passbolt"]["passphrase_mode"], "ask");
    assert_eq!(json["passbolt"]["passphrase_remember_secs"], 3600);
    assert_eq!(json["passbolt"]["passphrase_cached"], false);
}

#[test]
fn a_save_that_was_already_asking_deletes_no_stored_passphrase() {
    // The stored configuration already asks: saving "ask" again (a form
    // filled from defaults, say) leaves the vault's passphrase entry alone.
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "ask" } }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(passphrase_effect(&v.passphrase, PassphraseMode::Ask), SecretChange::Keep));
    // Unless its removal is asked for, in either mode.
    let mut i = input(serde_json::json!({ "enabled": true, "passbolt": { "passphrase_mode": "ask", "clear_passphrase": true } }));
    let v = validate_input(&mut i, None).unwrap();
    assert!(matches!(passphrase_effect(&v.passphrase, PassphraseMode::Ask), SecretChange::Clear));
    assert!(matches!(passphrase_effect(&v.passphrase, PassphraseMode::Stored), SecretChange::Clear));
    // Every other change is what it was, whatever is stored.
    for stored in [PassphraseMode::Ask, PassphraseMode::Stored] {
        assert!(matches!(passphrase_effect(&SecretChange::Keep, stored), SecretChange::Keep));
        assert!(matches!(passphrase_effect(&SecretChange::Clear, stored), SecretChange::Clear));
        let set = SecretChange::Set(Zeroizing::new("p".into()));
        assert!(matches!(passphrase_effect(&set, stored), SecretChange::Set(_)));
    }
    assert_eq!(format!("{:?}", SecretChange::ClearIfWasStored), "ClearIfWasStored");
}
