//! Passbolt, through its official command-line client, go-passbolt-cli (the
//! `passbolt` binary, v0.4.0 or later).
//!
//! The CLI keeps no session: every call logs in (GPGAuth or JWT), does one
//! thing and logs out. Reach runs it for two things only: listing entries
//! (metadata, nothing decrypted that is secret) and reading one entry's
//! password at connect time.
//!
//! How the CLI is run is the security boundary here, and every rule is in
//! [`build_command`]:
//!
//! - Secrets go in the child's environment, never on its command line (any
//!   local user can read a process's arguments) and never in a file. A
//!   passphrase that was asked for rather than stored is opened from its
//!   in-memory shield (`super::unlock`) only to build the command, and that
//!   plain copy is wiped once the command exists; the child then holds it in
//!   its environment until it exits.
//! - The child's environment starts empty. Only what a Go program needs to
//!   run and reach the server ([`INHERITED_ENV`]: PATH, the Windows system
//!   directories, temp and home directories, proxies) is copied from Reach's
//!   own, then the secrets are added. Nothing else gets through: not the
//!   CLI's own settings (it uses viper's automatic environment, with no
//!   prefix: `DEBUG`, `TLSSKIPVERIFY`, …, also removed by name), not
//!   `GODEBUG` (`http2debug=2` prints headers and cookies), not
//!   `SSL_CERT_FILE`/`SSL_CERT_DIR` (another set of trusted CAs).
//! - `--debug=false` always: debug mode prints raw HTTP bodies, the decrypted
//!   auth token and cookies to stdout. `--tlsSkipVerify=false` always: a
//!   flag set on the command line beats the environment and the CLI's
//!   configuration file, so a `tlsSkipVerify = true` in the user's own file
//!   (credentials from the CLI's configuration) is not honoured.
//! - With credentials stored by Reach, the user's own CLI configuration file
//!   is kept out by pointing `--config` at a file that does not exist (the
//!   CLI silently ignores a missing one). Reach never creates it, and refuses
//!   to run if something else has.
//! - No stdin: if the CLI wants to prompt (no passphrase, interactive MFA) it
//!   reads end-of-file and fails at once instead of hanging.
//! - Entry ids are UUIDs, parsed and re-formatted before they become an
//!   argument, so an id can never be read as an option.
//! - The binary must be called `passbolt` (`passbolt.exe`), be on this
//!   computer (no `\\server\share` path), and on Unix be owned by the user
//!   or root and writable by no one else, nor its folder. Whether it was set
//!   in Settings or found on PATH.
//! - A deadline, a kill when the handle drops, and a cap on what is read.
//! - At most two logins at once, across every caller
//!   (`process::cli_slot`).
//! - Errors are the CLI's last `Error:` line, cleaned up and shortened, or a
//!   plain explanation of the usual ones; never its environment or stdout.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
#[cfg(desktop)]
use std::time::Duration;

use serde::Deserialize;
use zeroize::Zeroizing;

use super::config::{CredentialsMode, MfaMode, PassboltConfig, PassboltSecrets};
use super::unlock::Passphrase;
use super::{sanitize_display, EntrySummary, Failure, PasswordManager, ProviderId, ProviderStatus};

const NAME: &str = "Passbolt";

/// Every key go-passbolt-cli reads through viper, as the environment
/// variable viper's `AutomaticEnv` (no prefix) looks for: the key upper-cased.
pub const VIPER_ENV_KEYS: &[&str] = &[
    "DEBUG",
    "TIMEOUT",
    "SERVERADDRESS",
    "USERPRIVATEKEY",
    "USERPASSWORD",
    "MFAMODE",
    "TOTPTOKEN",
    "MFATOTPTOKEN",
    "TOTPOFFSET",
    "MFATOTPOFFSET",
    "MFARETRYS",
    "MFADELAY",
    "TLSSKIPVERIFY",
    "TLSCLIENTCERT",
    "TLSCLIENTPRIVATEKEY",
    "WORKERS",
    "SERVERVERIFYTOKEN",
    "SERVERVERIFYENCTOKEN",
];

/// What the CLI inherits from Reach's environment, and nothing more. Names
/// are matched exactly on Unix and without regard to case on Windows, where
/// `Path` and `PATH` are the same variable.
pub const INHERITED_ENV: &[&str] = &[
    // Finding programs and the system. Go's networking on Windows needs
    // SystemRoot (DNS, the certificate store).
    "PATH",
    "SystemRoot",
    "SYSTEMROOT",
    "windir",
    "SystemDrive",
    // Temporary files, and where the CLI's own configuration file is found
    // (credentials from the CLI's configuration).
    "TEMP",
    "TMP",
    "TMPDIR",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "HOME",
    "XDG_CONFIG_HOME",
    // A proxy between this computer and the server.
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

/// Whether `name` is in [`INHERITED_ENV`].
pub fn is_inherited_env(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else { return false };
    if cfg!(windows) {
        INHERITED_ENV.iter().any(|k| k.eq_ignore_ascii_case(name))
    } else {
        INHERITED_ENV.contains(&name)
    }
}

/// The file names the CLI binary may have, compared without regard to case.
/// Anything else (`passbolt.cmd`, `passbolt.ps1`, another program) is
/// refused: a script would run through an interpreter, with the secrets.
pub const CLI_FILE_NAMES: &[&str] = &["passbolt", "passbolt.exe"];

/// The most a listing may print: tens of thousands of entries fit.
pub const STDOUT_CAP: usize = 32 * 1024 * 1024;
/// The most of stderr kept; only its last error line is used.
pub const STDERR_CAP: usize = 64 * 1024;
/// How long `passbolt --version` may take.
#[cfg(desktop)]
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
/// Allowed on top of the CLI's own `--timeout`, for start-up and logout.
#[cfg(desktop)]
const TIMEOUT_MARGIN: Duration = Duration::from_secs(10);
/// The longest error text shown.
const MAX_ERROR_CHARS: usize = 300;

pub const INSTALL_HINT: &str = "Install go-passbolt-cli (the `passbolt` command): `brew install go-passbolt-cli`, or a release zip/deb/rpm from https://github.com/passbolt/go-passbolt-cli/releases. Then put it on PATH or set its path here.";

/// What one CLI call does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Version,
    List,
    /// The UUID, already validated by [`validate_entry_id`].
    Get(String),
}

impl Op {
    fn args(&self) -> Vec<String> {
        match self {
            Op::Version => vec!["--version".into()],
            Op::List => [
                "list", "resource", "--json", "-c", "id", "-c", "name", "-c", "username", "-c", "uri", "-c",
                "folder_parent_id",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            Op::Get(id) => vec!["get".into(), "resource".into(), format!("--id={id}"), "--json".into()],
        }
    }
}

/// A Passbolt resource id: a UUID, returned in canonical lower-case form.
/// Anything else (an option such as `--debug`, an empty string) is refused.
pub fn validate_entry_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();
    // Uuid::parse_str also takes braced and URN forms; only the plain
    // hyphenated form is a Passbolt id.
    if trimmed.len() != 36 {
        return Err(format!("{NAME}: '{}' is not a Passbolt entry id", sanitize_display(trimmed, 40)));
    }
    uuid::Uuid::parse_str(trimmed)
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| format!("{NAME}: '{}' is not a Passbolt entry id", sanitize_display(trimmed, 40)))
}

/// Where the isolated (and never existing) CLI configuration file is.
pub fn isolated_config_path() -> PathBuf {
    crate::app_data_dir().join("pwmanager").join("passbolt-isolated.toml")
}

/// A path that names a network location (`\\server\share\…`, `//server/…`,
/// `\\?\UNC\…`). Opening one makes Windows sign in to that server with the
/// user's NTLM credentials, so such paths are refused before anything
/// touches them.
pub fn is_network_path(path: &str) -> bool {
    let p = path.trim_start();
    let mut chars = p.chars();
    matches!((chars.next(), chars.next()), (Some('\\' | '/'), Some('\\' | '/')))
}

/// The checks on a CLI path that need no file system: absolute, on this
/// computer, and named `passbolt` or `passbolt.exe`.
pub fn check_cli_path_shape(path: &Path) -> Result<(), String> {
    if is_network_path(&path.to_string_lossy()) {
        return Err(format!("{NAME}: the CLI must be on this computer, not on a network share"));
    }
    if !path.is_absolute() {
        return Err(format!("{NAME}: the CLI path must be absolute"));
    }
    let name = path.file_name().and_then(OsStr::to_str).unwrap_or("");
    if !CLI_FILE_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Err(format!(
            "{NAME}: the CLI must be the passbolt program itself (a file named passbolt or passbolt.exe), not '{}'",
            sanitize_display(name, 80)
        ));
    }
    Ok(())
}

/// The checks that look at the file: it exists and is a file, and on Unix
/// neither it nor its folder can be changed by anyone but the user or root.
/// Returns the path to run: on Unix the real file behind any symlink, so
/// what runs is what was checked.
pub fn check_cli_file(path: &Path) -> Result<PathBuf, String> {
    check_cli_path_shape(path)?;
    if !path.is_file() {
        return Err(format!("{NAME}: no CLI at {}. {INSTALL_HINT}", path.display()));
    }
    #[cfg(unix)]
    {
        let real = std::fs::canonicalize(path)
            .map_err(|e| format!("{NAME}: the CLI at {} cannot be read: {}", path.display(), e.kind()))?;
        check_unix_owner(&real, "the CLI")?;
        for dir in [real.parent(), path.parent()].into_iter().flatten() {
            check_unix_owner(dir, "the CLI's folder")?;
        }
        Ok(real)
    }
    #[cfg(not(unix))]
    Ok(path.to_path_buf())
}

/// Owned by the user running Reach or by root, and not writable by its group
/// or by others: otherwise someone else could swap in their own program and
/// be handed the private key and passphrase.
#[cfg(unix)]
pub(crate) fn check_unix_owner(path: &Path, what: &str) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("{NAME}: {what} at {} cannot be read: {}", path.display(), e.kind()))?;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let me = unsafe { libc::geteuid() };
    if meta.uid() != me && meta.uid() != 0 {
        return Err(format!("{NAME}: {what} at {} belongs to another user", path.display()));
    }
    if meta.mode() & 0o022 != 0 {
        return Err(format!(
            "{NAME}: {what} at {} can be changed by other users (group or world writable)",
            path.display()
        ));
    }
    Ok(())
}

/// The CLI binary: the configured path, or `passbolt` found on PATH. Always
/// an absolute path to an existing file that passed [`check_cli_file`].
pub fn resolve_binary(cli_path: Option<&str>) -> Result<PathBuf, String> {
    match cli_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => check_cli_file(Path::new(p)),
        None => {
            let found = which::which("passbolt").map_err(|_| format!("{NAME}: the passbolt CLI was not found. {INSTALL_HINT}"))?;
            let found = if found.is_absolute() { found } else { std::path::absolute(&found).unwrap_or(found) };
            // On Windows PATHEXT also finds passbolt.cmd or passbolt.bat, and
            // a PATH entry may be a network share: the same rules as a path
            // set in Settings.
            check_cli_file(&found)
        }
    }
}

/// Empty the child's environment, then copy back what [`INHERITED_ENV`]
/// allows from `parent`. The CLI's own keys are also removed by name, in case
/// one is ever added to the allowlist by mistake.
pub fn set_base_env(cmd: &mut tokio::process::Command, parent: impl IntoIterator<Item = (OsString, OsString)>) {
    cmd.env_clear();
    for (k, v) in parent {
        if is_inherited_env(&k) {
            cmd.env(k, v);
        }
    }
    for key in VIPER_ENV_KEYS {
        cmd.env_remove(key);
    }
}

/// The command for one CLI call, not yet started, with Reach's own
/// environment as the parent. See [`build_command_with_env`].
pub fn build_command(
    bin: &Path,
    cfg: &PassboltConfig,
    secrets: &PassboltSecrets,
    isolated_config: &Path,
    op: &Op,
) -> Result<tokio::process::Command, String> {
    build_command_with_env(bin, cfg, secrets, isolated_config, op, std::env::vars_os())
}

/// The command for one CLI call, inheriting from `parent_env` only what the
/// allowlist lets through. Pure apart from building the command, so tests
/// can look at its arguments and environment.
pub fn build_command_with_env(
    bin: &Path,
    cfg: &PassboltConfig,
    secrets: &PassboltSecrets,
    isolated_config: &Path,
    op: &Op,
    parent_env: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<tokio::process::Command, String> {
    #[cfg(desktop)]
    let mut cmd = super::process::silent_command(bin);
    #[cfg(not(desktop))]
    let mut cmd = tokio::process::Command::new(bin);

    set_base_env(&mut cmd, parent_env);
    cmd.args(op.args());
    cmd.arg("--debug=false");
    if *op == Op::Version {
        return Ok(cmd);
    }
    // Set explicitly, so it beats TLSSKIPVERIFY and the configuration file
    // (viper: a flag given on the command line comes first).
    cmd.arg("--tlsSkipVerify=false");
    cmd.arg(format!("--timeout={}s", cfg.timeout_secs()));

    match cfg.credentials {
        CredentialsMode::CliConfig => {}
        CredentialsMode::Reach => {
            if cfg.server_address.trim().is_empty() {
                return Err(format!("{NAME}: no server address is set. Set it in Settings → Password managers."));
            }
            let key = secrets
                .private_key
                .as_ref()
                .ok_or_else(|| format!("{NAME}: no private key is stored. Add it in Settings → Password managers."))?;
            let pass = secrets.passphrase.as_ref().ok_or_else(|| {
                if cfg.asks_passphrase() {
                    format!("{NAME}: the passphrase was not entered.")
                } else {
                    format!("{NAME}: no passphrase is stored. Add it in Settings → Password managers.")
                }
            })?;
            cmd.arg("--config").arg(isolated_config);
            cmd.env("SERVERADDRESS", cfg.server_address.trim());
            cmd.env("USERPRIVATEKEY", key.as_str());
            cmd.env("USERPASSWORD", pass.as_str());
            match cfg.mfa_mode {
                MfaMode::None => {
                    cmd.arg("--mfaMode=none");
                }
                MfaMode::TotpSecret => {
                    let seed = secrets.totp_secret.as_ref().ok_or_else(|| {
                        format!("{NAME}: MFA is set to TOTP but no TOTP secret is stored. Add it in Settings → Password managers.")
                    })?;
                    cmd.arg("--mfaMode=noninteractive-totp");
                    cmd.env("MFATOTPTOKEN", seed.as_str());
                }
            }
        }
    }
    Ok(cmd)
}

// ---------------------------------------------------------------------------
// Output

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default, alias = "ID")]
    id: Option<String>,
    #[serde(default, alias = "Name")]
    name: Option<String>,
    #[serde(default, alias = "Username")]
    username: Option<String>,
    #[serde(default, alias = "URI")]
    uri: Option<String>,
    #[serde(default, alias = "FolderParentID")]
    folder_parent_id: Option<String>,
}

fn clean_opt(s: Option<String>, max: usize) -> Option<String> {
    s.map(|v| sanitize_display(v.trim(), max)).filter(|v| !v.is_empty())
}

/// The entries in `passbolt list resource --json` output. Entries without a
/// valid id are skipped; empty fields become `None`.
pub fn parse_list(stdout: &[u8]) -> Result<Vec<EntrySummary>, String> {
    let raw: Vec<RawEntry> = serde_json::from_slice(stdout)
        .map_err(|_| format!("{NAME}: the CLI's list output was not understood (is go-passbolt-cli v0.4.0 or later installed?)"))?;
    let mut out: Vec<EntrySummary> = raw
        .into_iter()
        .filter_map(|e| {
            let id = validate_entry_id(e.id.as_deref()?).ok()?;
            Some(EntrySummary {
                id,
                name: clean_opt(e.name, 256).unwrap_or_default(),
                username: clean_opt(e.username, 256),
                uri: clean_opt(e.uri, 1024),
                folder_id: e.folder_parent_id.as_deref().and_then(|f| validate_entry_id(f).ok()),
            })
        })
        .collect();
    out.sort_by_cached_key(|e| e.name.to_lowercase());
    Ok(out)
}

#[derive(Deserialize)]
struct RawSecret {
    #[serde(default)]
    password: Option<String>,
}

/// The password in `passbolt get resource --json` output.
pub fn parse_get(stdout: &[u8]) -> Result<Zeroizing<String>, String> {
    // Only `password` is kept; serde skips every other field (description,
    // the secret and metadata maps) without copying it.
    let raw: RawSecret =
        serde_json::from_slice(stdout).map_err(|_| format!("{NAME}: the CLI's output for the entry was not understood"))?;
    let password = Zeroizing::new(raw.password.unwrap_or_default());
    if password.is_empty() {
        return Err(format!("{NAME}: the entry has no password"));
    }
    Ok(password)
}

/// The version from `passbolt --version`, e.g. `v0.5.2` from
/// `passbolt version v0.5.2 (Built on … from Git SHA …)`.
pub fn parse_version(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    let after = text.split_once(" version ").map(|(_, rest)| rest).unwrap_or(&text);
    after
        .split_whitespace()
        .next()
        .map(|v| sanitize_display(v, 40))
        .filter(|v| !v.is_empty())
}

/// Drop ANSI escape sequences and every control character but newlines.
pub fn strip_control(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ params… final byte (0x40–0x7e). Other escapes: ESC
            // and the next character.
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c == '\n' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

/// What kind of failure a CLI call's stderr reports, as far as Reach acts on
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliErrorKind {
    /// The private key could not be unlocked: the passphrase is wrong (or
    /// the key is not a Passbolt key). Asking for the passphrase again may
    /// help.
    WrongPassphrase,
    Other,
}

/// A user-facing message for a failed CLI call, from its stderr. Known
/// failures get a plain explanation; anything else is the CLI's last error
/// line, cleaned and shortened.
pub fn friendly_error(stderr: &[u8]) -> String {
    classify_error(stderr).1
}

/// The call's error as a [`Failure`]: a rejected passphrase is told apart,
/// so it can be asked for again.
pub fn failure(stderr: &[u8]) -> Failure {
    match classify_error(stderr) {
        (CliErrorKind::WrongPassphrase, message) => Failure::WrongPassphrase(message),
        (CliErrorKind::Other, message) => Failure::Other(message),
    }
}

/// [`friendly_error`], with its kind.
pub fn classify_error(stderr: &[u8]) -> (CliErrorKind, String) {
    let text = strip_control(&String::from_utf8_lossy(stderr));
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let detail = lines
        .iter()
        .rev()
        .find_map(|l| l.split_once("Error:").map(|(_, rest)| rest.trim()))
        .or_else(|| lines.last().copied())
        .unwrap_or("")
        .to_string();
    let lower = text.to_lowercase();
    let has = |needle: &str| lower.contains(&needle.to_lowercase());

    let mut kind = CliErrorKind::Other;
    let friendly = if has("serverAddress is not defined") {
        "no server address is configured. Set it in Settings → Password managers.".to_string()
    } else if has("userPrivateKey is not defined") {
        "no private key is configured. Add it in Settings → Password managers.".to_string()
    } else if has("reading Password") {
        "no passphrase is configured (the CLI asked for one). Add it in Settings → Password managers.".to_string()
    } else if has("unlock Key") || has("get Private Key") {
        kind = CliErrorKind::WrongPassphrase;
        "could not unlock the private key: wrong passphrase, or not a valid Passbolt key.".to_string()
    } else if has("MFA callback is not defined") || has("reading TOTP") || has("MFA challenge") {
        "the server requires MFA. Choose TOTP in Settings → Password managers and store the TOTP secret.".to_string()
    } else if has("(code 401)") || has("authentication failed") {
        "authentication failed. Check the private key, passphrase and server address.".to_string()
    } else if has("(code 404)") {
        "entry not found. It may have been deleted, or it is no longer shared with you.".to_string()
    } else if has("(code 403)") {
        "access denied by the server.".to_string()
    } else if has("x509") || has("certificate") {
        "the server's TLS certificate is not trusted.".to_string()
    } else if has("no such host") || has("connection refused") || has("dial tcp") || has("i/o timeout") {
        "could not reach the server. Check the server address and the network.".to_string()
    } else if has("context deadline exceeded") {
        "the server did not answer in time.".to_string()
    } else if detail.is_empty() {
        "the CLI failed without saying why.".to_string()
    } else {
        truncate_chars(&detail, MAX_ERROR_CHARS)
    };
    (kind, format!("{NAME}: {friendly}"))
}

// ---------------------------------------------------------------------------
// The provider

/// Passbolt, with the configuration and secrets for one call.
pub(crate) struct Passbolt {
    cfg: PassboltConfig,
    secrets: PassboltSecrets,
    enabled: bool,
}

impl Passbolt {
    pub(crate) fn new(cfg: PassboltConfig, secrets: PassboltSecrets, enabled: bool) -> Self {
        Self { cfg, secrets, enabled }
    }

    /// Enough is set up to try a login. A passphrase that is asked for when
    /// needed counts as there.
    fn configured(&self) -> bool {
        match self.cfg.credentials {
            CredentialsMode::CliConfig => true,
            CredentialsMode::Reach => {
                !self.cfg.server_address.trim().is_empty()
                    && self.secrets.private_key.is_some()
                    && (self.cfg.asks_passphrase() || self.secrets.passphrase.is_some())
                    && (self.cfg.mfa_mode != MfaMode::TotpSecret || self.secrets.totp_secret.is_some())
            }
        }
    }

    /// The secrets for one call: with a passphrase that is asked for, the
    /// typed one, opened from its shield here, and never a stored one (there
    /// should be none). `None`: the stored secrets as they are. Dropped (and
    /// wiped) as soon as the command is built.
    pub(crate) fn call_secrets(&self, typed: Option<&Passphrase>) -> Option<PassboltSecrets> {
        self.cfg.asks_passphrase().then(|| PassboltSecrets {
            passphrase: typed.map(Passphrase::reveal),
            ..self.secrets.clone()
        })
    }

    /// Run one call and return its stdout, or a user-facing error.
    #[cfg(desktop)]
    async fn run(&self, op: &Op, typed: Option<&Passphrase>) -> Result<Zeroizing<Vec<u8>>, Failure> {
        use super::process::{run, RunError};

        let bin = resolve_binary(self.cfg.cli_path.as_deref())?;
        let isolated = isolated_config_path();
        if self.cfg.credentials == CredentialsMode::Reach && std::fs::symlink_metadata(&isolated).is_ok() {
            return Err(Failure::Other(format!(
                "{NAME}: {} exists, but Reach keeps that path empty so the CLI reads no configuration file. Remove it and try again.",
                isolated.display()
            )));
        }
        let with_typed = match op {
            Op::Version => None,
            _ => self.call_secrets(typed),
        };
        let cmd = build_command(&bin, &self.cfg, with_typed.as_ref().unwrap_or(&self.secrets), &isolated, op)?;
        drop(with_typed);
        let timeout = match op {
            Op::Version => VERSION_TIMEOUT,
            _ => Duration::from_secs(u64::from(self.cfg.timeout_secs())) + TIMEOUT_MARGIN,
        };
        // Every call but `--version` is a full login: take a slot first and
        // keep it until the child is gone. The wait is not counted against
        // the timeout.
        let _slot = match op {
            Op::Version => None,
            _ => Some(super::process::cli_slot().await.map_err(|e| format!("{NAME}: {e}"))?),
        };
        let out = run(cmd, timeout, STDOUT_CAP, STDERR_CAP).await.map_err(|e| match e {
            RunError::Spawn(e) if e.kind() == std::io::ErrorKind::NotFound => {
                format!("{NAME}: the passbolt CLI was not found. {INSTALL_HINT}")
            }
            RunError::Spawn(e) => format!("{NAME}: could not start the CLI: {}", e.kind()),
            RunError::Timeout => format!("{NAME}: no answer within {} seconds.", timeout.as_secs()),
            RunError::TooLarge => format!("{NAME}: the CLI printed more than Reach accepts."),
            RunError::Io(e) => format!("{NAME}: reading the CLI's output failed: {}", e.kind()),
        })?;
        if !out.status.success() {
            return Err(failure(&out.stderr));
        }
        Ok(out.stdout)
    }

    #[cfg(not(desktop))]
    async fn run(&self, _op: &Op, _typed: Option<&Passphrase>) -> Result<Zeroizing<Vec<u8>>, Failure> {
        Err(Failure::Other(format!("{NAME}: available on desktop only")))
    }
}

impl PasswordManager for Passbolt {
    async fn status(&self) -> ProviderStatus {
        let mut status = ProviderStatus {
            id: ProviderId::Passbolt.as_str().into(),
            name: NAME.into(),
            enabled: self.enabled,
            available: false,
            configured: self.configured(),
            version: None,
            detail: None,
        };
        if !cfg!(desktop) {
            status.detail = Some("Available on desktop only".into());
            return status;
        }
        match resolve_binary(self.cfg.cli_path.as_deref()) {
            Ok(bin) => {
                status.available = true;
                status.detail = Some(bin.display().to_string());
                // Running the binary waits until the feature is on: off
                // means nothing is started.
                if self.enabled {
                    if let Ok(out) = self.run(&Op::Version, None).await {
                        status.version = parse_version(&out);
                    }
                }
            }
            Err(e) => status.detail = Some(e),
        }
        status
    }

    fn passphrase_prompt(&self) -> Option<u32> {
        self.cfg.asks_passphrase().then(|| self.cfg.remember_secs())
    }

    async fn list(&self, passphrase: Option<&Passphrase>) -> Result<Vec<EntrySummary>, Failure> {
        let out = self.run(&Op::List, passphrase).await?;
        Ok(parse_list(&out)?)
    }

    async fn resolve(&self, entry_id: &str, passphrase: Option<&Passphrase>) -> Result<Zeroizing<String>, Failure> {
        let id = validate_entry_id(entry_id)?;
        let out = self.run(&Op::Get(id), passphrase).await?;
        Ok(parse_get(&out)?)
    }
}

#[cfg(test)]
#[path = "passbolt_tests.rs"]
mod tests;
