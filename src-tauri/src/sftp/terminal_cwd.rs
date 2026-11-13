//! The folder the terminal is in, for the file explorer to follow (#162).
//!
//! Nothing is typed into the user's shell and no prompt hook is installed:
//! tools that do it that way (MobaXterm's "Follow SSH path") break whenever
//! the shell prints something of its own. Instead, a separate command on the
//! same connection looks the shell up. On Linux every channel of one SSH
//! connection is a child of the same sshd process (`sshd-session` on
//! OpenSSH 9.8 and later), so the command's own parent leads to the
//! terminal's shell: the sibling that has a terminal. Its terminal's
//! foreground job (a `sleep` started in /etc, a nested shell) says where the
//! user is now; if that one cannot be read (a root shell under sudo), the
//! login shell's own folder is the answer.
//!
//! It needs `/proc`, so a server without it (BSD, macOS) answers nothing,
//! and the explorer says so.

use crate::ssh::client::{exec_on_connection_with_exit_code, SharedHandle, SshError};

/// POSIX sh, run under `sh` whatever the login shell is (fish included).
/// No single quotes in it: it goes inside one pair. Each process's stat
/// line is read with the `read` builtin, so the scan forks nothing per
/// process. In /proc/PID/stat everything after the `) ` that closes the
/// command name is space-separated: state, ppid, pgrp, session, tty_nr,
/// tpgid. A login shell that forks the command instead of replacing itself
/// puts one more process in between, hence the grandparent as well.
const PROBE: &str = r#"read -r s < /proc/$$/stat; s=${s##*") "}; set -- $s; p=$2
read -r s < /proc/$p/stat; s=${s##*") "}; set -- $s; q=$2; best=
for d in /proc/[0-9]*; do
 read -r s < $d/stat 2>/dev/null || continue; s=${s##*") "}; set -- $s
 [ "$5" != 0 ] || continue
 [ "$2" = "$p" ] || [ "$2" = "$q" ] && best="${d#/proc/} $6"
done
[ -n "$best" ] || exit 3
set -- $best; readlink /proc/$2/cwd 2>/dev/null || readlink /proc/$1/cwd"#;

fn command() -> String {
    format!("exec sh -c '{PROBE}'")
}

/// The terminal's folder on this connection, or `None` when the server
/// cannot say.
pub async fn terminal_cwd(handle: &SharedHandle) -> Result<Option<String>, SshError> {
    let (out, _, code) = exec_on_connection_with_exit_code(handle, &command()).await?;
    Ok(parse(code, &out))
}

fn parse(code: i32, out: &str) -> Option<String> {
    if code != 0 {
        return None;
    }
    let path = out.lines().next()?.trim_end_matches('\r');
    path.starts_with('/').then(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_fits_inside_single_quotes() {
        assert!(!PROBE.contains('\''));
        assert!(command().starts_with("exec sh -c '") && command().ends_with('\''));
    }

    #[test]
    fn only_an_absolute_path_is_an_answer() {
        assert_eq!(parse(0, "/usr/share\n"), Some("/usr/share".into()));
        assert_eq!(parse(0, "/home/a b\r\n"), Some("/home/a b".into()));
        assert_eq!(parse(3, ""), None);
        assert_eq!(parse(0, ""), None);
        assert_eq!(parse(0, "readlink: x\n"), None);
        assert_eq!(parse(1, "/tmp\n"), None);
    }
}

// Against a real server: sftp::transfer::live::live_terminal_cwd.
