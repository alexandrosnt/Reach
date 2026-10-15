//! A session's approvals (commands that may run, weaker settings that may
//! apply) are the user's own decisions, but the session record syncs and
//! may sit in a vault other people can write to. So each approval is
//! stored signed: who gave it, which list it is for, what was approved, and
//! an HMAC-SHA256 under a key only that identity has
//! (`VaultManager::approval_key`) over all of that and the session's
//! context: its id and everything that decides where and how it connects
//! (see `session_commands::approval_context`). Nobody else can write one
//! that passes, and one does not survive a change of host, port, user,
//! route or ssh_config: an approval given for one server is never replayed
//! against another.

use base64::Engine;

/// Which list an approval belongs to; part of what is signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Command,
    Weakening,
    /// A password manager entry a session may send its secret from (see
    /// `session_commands::ref_slots`). Its own tag, so a signature given for
    /// one list never counts for another.
    SecretRef,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Command => "command",
            Kind::Weakening => "weakening",
            Kind::SecretRef => "secret_ref",
        }
    }
}

const SEP: char = '\t';

/// HMAC-SHA256: HKDF's extract step is exactly HMAC(salt, ikm).
fn mac(key: &[u8; 32], who: &str, kind: Kind, context: &str, what: &str) -> [u8; 32] {
    let mut msg = Vec::new();
    for part in [who, kind.tag(), context, what] {
        msg.extend((part.len() as u32).to_be_bytes());
        msg.extend(part.as_bytes());
    }
    let (prk, _) = hkdf::Hkdf::<sha2::Sha256>::extract(Some(key), &msg);
    prk.into()
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD_NO_PAD
}

/// The stored form of one approval.
pub fn sign(key: &[u8; 32], who: &str, kind: Kind, context: &str, what: &str) -> String {
    format!("{who}{SEP}{}{SEP}{}", b64().encode(what.as_bytes()), b64().encode(mac(key, who, kind, context, what)))
}

/// Who an entry claims to be from.
fn owner(entry: &str) -> Option<&str> {
    entry.split(SEP).next()
}

/// The approvals in `stored` that `who` really gave for this list and this
/// context. Anything else, forged, copied or given before a change, is not
/// among them.
pub fn verified(stored: &[String], key: &[u8; 32], who: &str, kind: Kind, context: &str) -> Vec<String> {
    stored
        .iter()
        .filter_map(|e| {
            let mut parts = e.split(SEP);
            let (owner, what, tag) = (parts.next()?, parts.next()?, parts.next()?);
            if owner != who || parts.next().is_some() {
                return None;
            }
            let what = String::from_utf8(b64().decode(what).ok()?).ok()?;
            let tag = b64().decode(tag).ok()?;
            let want = mac(key, who, kind, context, &what);
            // Constant time.
            let same = tag.len() == want.len() && tag.iter().zip(want.iter()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0;
            same.then_some(what)
        })
        .collect()
}

/// `stored` with `who`'s approvals replaced by `mine`, signed; other
/// people's entries stay for them.
pub fn replace_mine(stored: &[String], mine: &[String], key: &[u8; 32], who: &str, kind: Kind, context: &str) -> Vec<String> {
    let mut out: Vec<String> = stored.iter().filter(|e| owner(e) != Some(who)).cloned().collect();
    let mut seen = std::collections::HashSet::new();
    for m in mine {
        if seen.insert(m.as_str()) {
            out.push(sign(key, who, kind, context, m));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: [u8; 32] = [1; 32];
    const MALLORY: [u8; 32] = [2; 32];

    #[test]
    fn only_the_owner_can_approve() {
        let mine = sign(&ALICE, "alice", Kind::Command, "s1", "nc %h %p");
        assert_eq!(verified(std::slice::from_ref(&mine), &ALICE, "alice", Kind::Command, "s1"), vec!["nc %h %p"]);
        // Mallory writes an entry in alice's name with her own key.
        let forged = sign(&MALLORY, "alice", Kind::Command, "s1", "evil");
        assert!(verified(&[forged], &ALICE, "alice", Kind::Command, "s1").is_empty());
        // Copied to another session or list, alice's entry counts for nothing.
        assert!(verified(std::slice::from_ref(&mine), &ALICE, "alice", Kind::Command, "s2").is_empty());
        assert!(verified(std::slice::from_ref(&mine), &ALICE, "alice", Kind::Weakening, "s1").is_empty());
        assert!(verified(std::slice::from_ref(&mine), &ALICE, "alice", Kind::SecretRef, "s1").is_empty());
        // A changed approved text breaks the tag.
        let tampered = mine.replacen(&b64().encode("nc %h %p"), &b64().encode("evil"), 1);
        assert!(verified(&[tampered], &ALICE, "alice", Kind::Command, "s1").is_empty());
    }

    #[test]
    fn replacing_keeps_other_peoples_entries() {
        let bobs = sign(&MALLORY, "bob", Kind::Weakening, "s1", "ForwardAgent yes");
        let old = sign(&ALICE, "alice", Kind::Weakening, "s1", "MACs +hmac-sha1");
        let out = replace_mine(&[bobs.clone(), old], &["StrictHostKeyChecking no".into()], &ALICE, "alice", Kind::Weakening, "s1");
        assert!(out.contains(&bobs));
        assert_eq!(verified(&out, &ALICE, "alice", Kind::Weakening, "s1"), vec!["StrictHostKeyChecking no"]);
    }
}
