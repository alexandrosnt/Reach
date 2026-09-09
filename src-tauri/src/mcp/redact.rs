//! Best-effort scrubbing of secrets from terminal output before an AI sees it.
//!
//! **This is defence in depth, not a guarantee, and the UI must say so.**
//!
//! There are two very different things going on when we talk about "hiding
//! passwords from the AI", and conflating them is how people end up trusting
//! this more than they should:
//!
//! 1. **A password typed at a prompt is structurally safe.** When a program
//!    asks for a password it disables terminal echo, so the bytes never appear
//!    in the output stream at all. There is nothing to redact — see
//!    [`crate::mcp::guard`], which additionally refuses to *write* while echo
//!    is off. That is a real guarantee.
//!
//! 2. **`cat .env` is not preventable.** That is ordinary output that happens
//!    to contain a secret. The patterns below catch the shapes that show up
//!    most often — key material, tokens, connection strings — and they will
//!    miss things. A bespoke internal token format matches nothing here.
//!
//! So: this meaningfully reduces accidental exposure and must never be
//! described as making a session safe to share.
//!
//! Redaction is applied on the way *out* to the client and again when history
//! is read back, because a secret captured before a pattern existed would
//! otherwise leak from the log later.

use regex::Regex;
use std::sync::OnceLock;

/// What replaces a matched secret. Deliberately visible: a silent deletion
/// makes an AI think it read the whole file, and it will act on that belief.
const MASK: &str = "«redacted by Reach»";

struct Rule {
    re: Regex,
    /// Which capture group holds the secret. 0 means the whole match.
    group: usize,
}

fn rules() -> &'static Vec<Rule> {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let specs: &[(&str, usize)] = &[
            // PEM private key blocks, including the body. Matched first and
            // non-greedily so one key does not swallow the rest of the buffer.
            (r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----", 0),
            // OpenSSH private keys.
            (r"(?s)-----BEGIN OPENSSH PRIVATE KEY-----.*?-----END OPENSSH PRIVATE KEY-----", 0),
            // JWTs: three base64url segments.
            (r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b", 0),
            // Provider key formats with distinctive prefixes.
            (r"\bsk-[A-Za-z0-9_-]{16,}\b", 0),
            (r"\bghp_[A-Za-z0-9]{20,}\b", 0),
            (r"\bgithub_pat_[A-Za-z0-9_]{20,}\b", 0),
            (r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b", 0),
            (r"\bAKIA[0-9A-Z]{16}\b", 0),
            (r"\bAIza[0-9A-Za-z_-]{35}\b", 0),
            // KEY=value in env dumps and .env files. Group 2 is the value, so
            // the name stays visible — the AI should know PGPASSWORD is set.
            (
                r"(?im)^\s*([A-Z0-9_]*(?:PASSWORD|PASSWD|SECRET|TOKEN|APIKEY|API_KEY|PRIVATE_KEY|ACCESS_KEY)[A-Z0-9_]*)\s*=\s*(\S+)",
                2,
            ),
            // Inline flags: -p<secret>, --password=<secret>, --token <secret>.
            (r"(?i)--(?:password|token|api-key|secret)[= ]\s*(\S+)", 1),
            (r"(?i)\bmysql\b[^\n]*?\s-p(\S+)", 1),
            // Credentials embedded in URLs: scheme://user:secret@host
            (r"(?i)\b[a-z][a-z0-9+.-]*://[^\s:/@]+:([^\s@/]+)@", 1),
            // Authorization headers.
            (r"(?i)authorization:\s*(?:bearer|basic|token)\s+(\S+)", 1),
        ];
        specs
            .iter()
            .map(|(p, group)| Rule {
                // Every pattern here is a literal checked by the tests below;
                // a compile failure is a bug in this file, not user input.
                re: Regex::new(p).expect("built-in redaction pattern must compile"),
                group: *group,
            })
            .collect()
    })
}

/// Scrub `input`, returning the text and how many secrets were masked.
///
/// The count is surfaced to the client so the model knows the text is
/// incomplete rather than silently reasoning about a truncated file.
pub fn redact(input: &str) -> (String, usize) {
    let mut out = input.to_string();
    let mut hits = 0usize;

    for rule in rules() {
        let mut count = 0usize;
        // One pass per rule via `replace_all`, deliberately *not* a
        // match-replace-rescan loop: the mask is itself non-whitespace, so a
        // rule like `KEY=(\S+)` would match its own output and keep replacing
        // until the safety valve tripped.
        let replaced = rule.re.replace_all(&out, |caps: &regex::Captures| {
            let Some(whole) = caps.get(0) else {
                return String::new();
            };
            let Some(target) = caps.get(rule.group) else {
                // Optional group did not participate; leave the text alone.
                return whole.as_str().to_string();
            };
            count += 1;
            let text = whole.as_str();
            let start = target.start() - whole.start();
            let end = target.end() - whole.start();
            format!("{}{}{}", &text[..start], MASK, &text[end..])
        });
        hits += count;
        out = replaced.into_owned();
    }

    (out, hits)
}

/// True if `input` contains anything this module would mask. Used by the guard
/// to refuse *sending* a command that carries a secret in the clear, which
/// would otherwise be written into history verbatim.
pub fn contains_secret(input: &str) -> bool {
    rules().iter().any(|r| r.re.is_match(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_a_pem_private_key() {
        let text = "before\n-----BEGIN RSA PRIVATE KEY-----\nMIIEow==\n-----END RSA PRIVATE KEY-----\nafter";
        let (out, hits) = redact(text);
        assert_eq!(hits, 1);
        assert!(!out.contains("MIIEow"));
        assert!(out.starts_with("before"), "context is kept: {out}");
        assert!(out.ends_with("after"), "context is kept: {out}");
    }

    #[test]
    fn keeps_the_variable_name_but_masks_the_value() {
        let (out, hits) = redact("PGPASSWORD=hunter2\nPATH=/usr/bin");
        assert_eq!(hits, 1);
        // The AI should still be able to see *that* a password is configured.
        assert!(out.contains("PGPASSWORD="), "{out}");
        assert!(!out.contains("hunter2"), "{out}");
        assert!(out.contains("PATH=/usr/bin"), "unrelated vars survive: {out}");
    }

    #[test]
    fn masks_credentials_inside_a_url() {
        let (out, hits) = redact("postgres://admin:s3cr3t@db.internal:5432/app");
        assert_eq!(hits, 1);
        assert!(!out.contains("s3cr3t"), "{out}");
        // Host and user stay: the model needs them to reason about the target.
        assert!(out.contains("db.internal"), "{out}");
        assert!(out.contains("admin"), "{out}");
    }

    #[test]
    fn masks_several_secrets_in_one_buffer() {
        let (out, hits) = redact("A=sk-abcdefghijklmnopqrst\nB=ghp_ABCDEFGHIJKLMNOPQRSTU");
        assert!(hits >= 2, "expected both, got {hits}: {out}");
        assert!(!out.contains("sk-abcdefghij"), "{out}");
        assert!(!out.contains("ghp_ABCDEFGHIJ"), "{out}");
    }

    #[test]
    fn leaves_ordinary_output_alone() {
        let text = "total 12\ndrwxr-xr-x 2 root root 4096 Sep  9 21:00 .\n-rw-r--r-- 1 root root 220 Sep 9 nginx.conf";
        let (out, hits) = redact(text);
        assert_eq!(hits, 0, "no false positives on an ls listing");
        assert_eq!(out, text);
    }

    #[test]
    fn detects_a_secret_in_a_command_about_to_be_sent() {
        assert!(contains_secret("mysql -uroot -pHunter2 mydb"));
        assert!(contains_secret("curl -H 'Authorization: Bearer eyJhbGciOi.AAAAAAAA.BBBBBBBB'"));
        assert!(!contains_secret("systemctl restart nginx"));
    }

    /// The mask has to be visible. A silent deletion leaves the model believing
    /// it read a complete file, and it will act on that belief.
    #[test]
    fn the_mask_is_self_describing() {
        let (out, _) = redact("TOKEN=abcdefghijklmnop");
        assert!(out.contains("redacted"), "{out}");
        assert!(out.contains("Reach"), "attributed so it is not mistaken for file content: {out}");
    }
}
