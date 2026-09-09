//! What the server knows about the sessions the user has shared.
//!
//! Nothing is here unless the user shared it. Enabling the MCP server exposes
//! no sessions at all; each terminal tab is offered individually, and
//! `list_sessions` returns an empty array until then.
//!
//! # A correction on the echo-off lockout
//!
//! Two claims get conflated, and only one of them is a guarantee:
//!
//! * **A password typed at a prompt never enters the output stream.** True, and
//!   structural: the remote disables echo, so the bytes are never transmitted
//!   back. Nothing we do affects this and nothing can leak it.
//! * **Knowing that a prompt is currently waiting for a password**, so writes
//!   can be refused, is *inference*. For a local PTY the terminal state is in
//!   principle observable; over SSH we only have the byte stream, and the
//!   honest signal is that the remote just printed something that looks like a
//!   password prompt.
//!
//! So [`SharedSession::awaiting_secret`] is a heuristic, and is documented as
//! one. It is still worth having — it stops the common and genuinely bad case
//! of a model answering `sudo` with a guess — but it must not be described as
//! a guarantee, and the guard does not depend on it for the property that
//! matters.

use std::collections::{HashMap, HashSet, VecDeque};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::mcp::guard::RateWindow;

/// Bytes of scrollback retained per session. Enough for a model to see what a
/// command did without turning the server into a log store — history is a
/// separate, deliberate, vault-backed thing.
const BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    Ssh,
    Local,
}

/// A terminal the user has explicitly offered to the AI.
#[derive(Debug)]
pub struct SharedSession {
    pub id: String,
    pub kind: SessionKind,
    pub host: String,
    pub username: String,
    /// Set once we have observed enough to answer `describe_session` well.
    pub shell: Option<String>,
    /// Rolling scrollback. Bytes rather than text: terminal output is not
    /// guaranteed to be valid UTF-8 mid-stream, and slicing a multi-byte
    /// sequence in half to fit the cap would corrupt it.
    buffer: VecDeque<u8>,
    /// Increments on every write from the remote. A client's read is "current"
    /// only if it saw this value.
    pub seq: u64,
}

impl SharedSession {
    pub fn new(id: String, kind: SessionKind, host: String, username: String) -> Self {
        Self {
            id,
            kind,
            host,
            username,
            shell: None,
            buffer: VecDeque::with_capacity(BUFFER_BYTES.min(8 * 1024)),
            seq: 0,
        }
    }

    /// Feed output from the remote. Called from the SSH/PTY read loops.
    pub fn push_output(&mut self, bytes: &[u8]) {
        self.buffer.extend(bytes.iter().copied());
        while self.buffer.len() > BUFFER_BYTES {
            self.buffer.pop_front();
        }
        self.seq = self.seq.wrapping_add(1);
    }

    /// Scrollback as text. Invalid sequences become U+FFFD rather than an
    /// error: a model asking to read should get the readable part of a screen,
    /// not a failure because something emitted a stray byte.
    pub fn text(&self) -> String {
        let bytes: Vec<u8> = self.buffer.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The last `lines` lines of scrollback.
    pub fn tail(&self, lines: usize) -> String {
        let text = self.text();
        let mut collected: Vec<&str> = text.lines().rev().take(lines).collect();
        collected.reverse();
        collected.join("\n")
    }

    /// Best-effort: does the tail look like a prompt waiting for a secret?
    ///
    /// Heuristic, not a guarantee — see the module docs. Only the very end of
    /// the buffer is considered, because a `password:` from five commands ago
    /// says nothing about what the terminal wants now.
    pub fn awaiting_secret(&self) -> bool {
        let text = self.text();
        let tail = text
            .char_indices()
            .rev()
            .nth(200)
            .map_or(text.as_str(), |(i, _)| &text[i..]);
        // Trailing whitespace after a prompt is normal; the prompt itself must
        // be the last meaningful thing on screen.
        let tail = tail.trim_end();
        prompt_patterns().iter().any(|re| re.is_match(tail))
    }
}

fn prompt_patterns() -> &'static Vec<Regex> {
    static RULES: std::sync::OnceLock<Vec<Regex>> = std::sync::OnceLock::new();
    RULES.get_or_init(|| {
        [
            r"(?i)\[sudo\]\s+password\s+for\s+\S+:\s*$",
            r"(?i)\bpassword\s*:\s*$",
            r"(?i)\bpassword\s+for\s+\S+\s*:\s*$",
            r"(?i)\bpassphrase[^\n:]*:\s*$",
            r"(?i)\benter\s+(the\s+)?(pin|passphrase|password)[^\n:]*:\s*$",
            r"(?i)\bverification\s+code\s*:\s*$",
            r"(?i)\b\(current\)\s+unix\s+password\s*:\s*$",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("prompt pattern must compile"))
        .collect()
    })
}

/// Per-client bookkeeping. Preconditions are tracked per connected client, not
/// globally: one client having called `describe_session` must not satisfy the
/// precondition for another.
#[derive(Debug, Default)]
pub struct ClientState {
    described: HashSet<String>,
    last_read_seq: HashMap<String, u64>,
    pub rate: RateWindow,
}

impl ClientState {
    pub fn mark_described(&mut self, session_id: &str) {
        self.described.insert(session_id.to_string());
    }

    pub fn has_described(&self, session_id: &str) -> bool {
        self.described.contains(session_id)
    }

    pub fn mark_read(&mut self, session_id: &str, seq: u64) {
        self.last_read_seq.insert(session_id.to_string(), seq);
    }

    /// True when this client has seen the session's current sequence.
    pub fn view_is_current(&self, session_id: &str, seq: u64) -> bool {
        self.last_read_seq.get(session_id) == Some(&seq)
    }

    /// Forget a session entirely — called when the user unshares it, so that
    /// re-sharing later does not silently reuse stale preconditions.
    pub fn forget(&mut self, session_id: &str) {
        self.described.remove(session_id);
        self.last_read_seq.remove(session_id);
    }
}

/// Summary returned by `list_sessions`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub kind: SessionKind,
    pub host: String,
    pub username: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SharedSession {
        SharedSession::new("s1".into(), SessionKind::Ssh, "db.internal".into(), "root".into())
    }

    #[test]
    fn output_advances_the_sequence() {
        let mut s = session();
        assert_eq!(s.seq, 0);
        s.push_output(b"hello");
        assert_eq!(s.seq, 1);
        s.push_output(b" world");
        assert_eq!(s.seq, 2);
        assert_eq!(s.text(), "hello world");
    }

    #[test]
    fn the_buffer_is_capped_and_keeps_the_newest() {
        let mut s = session();
        s.push_output(&vec![b'a'; BUFFER_BYTES]);
        s.push_output(b"NEWEST");
        assert!(s.text().ends_with("NEWEST"));
        assert!(s.text().len() <= BUFFER_BYTES);
    }

    /// Terminal output is not guaranteed to be valid UTF-8, and the cap can
    /// slice a multi-byte sequence. Reading must degrade, never fail.
    #[test]
    fn invalid_utf8_does_not_break_reading() {
        let mut s = session();
        s.push_output(&[0xff, 0xfe, b'o', b'k']);
        assert!(s.text().contains("ok"));
    }

    #[test]
    fn tail_returns_the_last_lines() {
        let mut s = session();
        s.push_output(b"one\ntwo\nthree\nfour\n");
        assert_eq!(s.tail(2), "three\nfour");
    }

    #[test]
    fn detects_the_prompts_that_actually_matter() {
        for prompt in [
            "[sudo] password for alex: ",
            "root@db's password: ",
            "Enter passphrase for key '/home/a/.ssh/id_ed25519': ",
            "Password:",
            "Verification code: ",
            "(current) UNIX password: ",
        ] {
            let mut s = session();
            s.push_output(prompt.as_bytes());
            assert!(s.awaiting_secret(), "missed: {prompt:?}");
        }
    }

    #[test]
    fn ordinary_output_is_not_mistaken_for_a_prompt() {
        for text in [
            "total 12\ndrwxr-xr-x 2 root root 4096 .",
            "root@db:~# ",
            "Reading package lists... Done",
            "PASSWORD_FILE=/etc/app/secret",
            "$ ",
        ] {
            let mut s = session();
            s.push_output(text.as_bytes());
            assert!(!s.awaiting_secret(), "false positive: {text:?}");
        }
    }

    /// A prompt from earlier in the scrollback says nothing about what the
    /// terminal wants right now.
    #[test]
    fn a_stale_prompt_does_not_latch() {
        let mut s = session();
        s.push_output(b"[sudo] password for alex: ");
        assert!(s.awaiting_secret());
        s.push_output(b"\nreading package lists... done\nroot@db:~# ");
        assert!(!s.awaiting_secret(), "the prompt was answered; the lock must clear");
    }

    #[test]
    fn view_currency_tracks_the_sequence() {
        let mut client = ClientState::default();
        let mut s = session();
        s.push_output(b"x");

        assert!(!client.view_is_current("s1", s.seq), "never read: not current");
        client.mark_read("s1", s.seq);
        assert!(client.view_is_current("s1", s.seq));

        s.push_output(b"more output");
        assert!(!client.view_is_current("s1", s.seq), "output arrived: no longer current");
    }

    /// One client satisfying a precondition must not satisfy it for another.
    #[test]
    fn preconditions_are_per_client() {
        let mut a = ClientState::default();
        let b = ClientState::default();
        a.mark_described("s1");
        assert!(a.has_described("s1"));
        assert!(!b.has_described("s1"));
    }

    /// Unsharing then re-sharing must not carry stale consent forward.
    #[test]
    fn forgetting_a_session_clears_its_preconditions() {
        let mut c = ClientState::default();
        c.mark_described("s1");
        c.mark_read("s1", 7);
        c.forget("s1");
        assert!(!c.has_described("s1"));
        assert!(!c.view_is_current("s1", 7));
    }
}
