//! ICE servers for peer-to-peer session sharing.
//!
//! Two kinds, and the difference is the whole reason this is configurable:
//!
//! - **STUN** answers one tiny question — "what is my public address?" — so
//!   two peers behind NAT can find each other. It is stateless, costs nothing
//!   to run, and Google and Cloudflare operate public ones. Using those is
//!   normal and abuses nobody.
//! - **TURN** relays the actual traffic when hole-punching fails. That costs
//!   real bandwidth, so there is no such thing as a free public TURN server
//!   that is both trustworthy and still there next month.
//!
//! So the app ships STUN defaults and no TURN at all. Anyone who has a TURN
//! server — coturn on a VPS, Cloudflare Calls, whatever their team runs — adds
//! it here, and the ~15–30% of NAT pairs that cannot hole-punch start working
//! for them. Everyone else gets an honest failure message rather than their
//! terminal quietly routed through a stranger's box.
//!
//! The list lives in the settings vault, not localStorage, because a TURN
//! entry carries a username and credential. Same home as the OpenRouter key.

use serde::{Deserialize, Serialize};

/// One ICE server, in the shape `RTCPeerConnection` expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IceServer {
    /// One or more `stun:` / `stuns:` / `turn:` / `turns:` URLs.
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// Public STUN. Two operators, so one going away does not strand anyone.
pub fn default_servers() -> Vec<IceServer> {
    vec![
        IceServer {
            urls: vec!["stun:stun.l.google.com:19302".into()],
            username: None,
            credential: None,
        },
        IceServer {
            urls: vec!["stun:stun.cloudflare.com:3478".into()],
            username: None,
            credential: None,
        },
    ]
}

/// Why a server entry was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IceError {
    NoUrls,
    BadScheme(String),
    EmptyHost(String),
    TurnWithoutCredentials(String),
    StunWithCredentials(String),
    TooMany(usize),
}

impl std::fmt::Display for IceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IceError::NoUrls => write!(f, "An ICE server needs at least one URL"),
            IceError::BadScheme(u) => write!(
                f,
                "`{u}` must start with stun:, stuns:, turn: or turns:"
            ),
            IceError::EmptyHost(u) => write!(f, "`{u}` has no host"),
            IceError::TurnWithoutCredentials(u) => write!(
                f,
                "`{u}` is a TURN server, which needs a username and credential"
            ),
            IceError::StunWithCredentials(u) => write!(
                f,
                "`{u}` is a STUN server; STUN takes no credentials, so these would be sent for nothing"
            ),
            IceError::TooMany(n) => write!(f, "{n} servers is more than the cap of {MAX_SERVERS}"),
        }
    }
}

/// More than this and ICE gathering itself becomes the bottleneck: every
/// candidate is checked against every server before a connection can form.
pub const MAX_SERVERS: usize = 8;

fn is_turn(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("turn:") || lower.starts_with("turns:")
}

fn is_stun(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("stun:") || lower.starts_with("stuns:")
}

/// Validate one server entry.
///
/// The scheme rule matters more than it looks: a `https://` URL here does not
/// error in the browser, it just silently contributes no candidates, and the
/// user is left wondering why their carefully configured relay never helps.
pub fn validate(server: &IceServer) -> Result<(), IceError> {
    if server.urls.is_empty() {
        return Err(IceError::NoUrls);
    }

    let has_creds = server.username.as_deref().is_some_and(|u| !u.is_empty())
        && server.credential.as_deref().is_some_and(|c| !c.is_empty());

    for url in &server.urls {
        let url = url.trim();
        if !is_turn(url) && !is_stun(url) {
            return Err(IceError::BadScheme(url.to_string()));
        }
        // Everything after the scheme up to any `?transport=` is the host:port.
        let after_scheme = url.split_once(':').map(|(_, rest)| rest).unwrap_or("");
        let host = after_scheme.split('?').next().unwrap_or("").trim();
        if host.is_empty() {
            return Err(IceError::EmptyHost(url.to_string()));
        }
        if is_turn(url) && !has_creds {
            return Err(IceError::TurnWithoutCredentials(url.to_string()));
        }
        if is_stun(url) && has_creds {
            return Err(IceError::StunWithCredentials(url.to_string()));
        }
    }
    Ok(())
}

/// Validate a whole list.
pub fn validate_all(servers: &[IceServer]) -> Result<(), IceError> {
    if servers.len() > MAX_SERVERS {
        return Err(IceError::TooMany(servers.len()));
    }
    servers.iter().try_for_each(validate)
}

/// Serialise for the vault. One JSON blob under one key, so the whole list
/// is replaced atomically and there is no partial state to reconcile.
pub fn to_json(servers: &[IceServer]) -> Result<String, String> {
    serde_json::to_string(servers).map_err(|e| e.to_string())
}

/// Parse what the vault held. A corrupt value falls back to the defaults
/// rather than failing sharing outright — a broken setting should cost the
/// user their customisation, not the feature.
pub fn from_json(raw: &str) -> Vec<IceServer> {
    match serde_json::from_str::<Vec<IceServer>>(raw) {
        Ok(list) if validate_all(&list).is_ok() => list,
        Ok(_) => {
            tracing::warn!("Persisted ICE servers failed validation; using defaults");
            default_servers()
        }
        Err(e) => {
            tracing::warn!("Persisted ICE servers unreadable ({e}); using defaults");
            default_servers()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stun(url: &str) -> IceServer {
        IceServer { urls: vec![url.into()], username: None, credential: None }
    }

    fn turn(url: &str, user: &str, cred: &str) -> IceServer {
        IceServer {
            urls: vec![url.into()],
            username: Some(user.into()),
            credential: Some(cred.into()),
        }
    }

    #[test]
    fn the_defaults_are_valid() {
        // A default that fails validation would strand every fresh install.
        validate_all(&default_servers()).expect("defaults must validate");
        assert!(default_servers().len() >= 2, "one operator going away must not strand anyone");
    }

    #[test]
    fn the_defaults_carry_no_credentials() {
        // No TURN by default: there is no free public one worth trusting.
        for s in default_servers() {
            assert!(s.username.is_none() && s.credential.is_none());
            assert!(s.urls.iter().all(|u| is_stun(u)));
        }
    }

    #[test]
    fn accepts_every_ice_scheme() {
        for url in ["stun:a:3478", "stuns:a:5349", "STUN:a:3478"] {
            validate(&stun(url)).unwrap_or_else(|e| panic!("{url}: {e}"));
        }
        for url in ["turn:a:3478", "turns:a:5349", "turn:a:3478?transport=tcp"] {
            validate(&turn(url, "u", "p")).unwrap_or_else(|e| panic!("{url}: {e}"));
        }
    }

    #[test]
    fn rejects_a_url_the_browser_would_silently_ignore() {
        // https:// in an iceServers list does not throw; it contributes nothing
        // and the user never learns why their relay is not helping.
        for url in ["https://stun.example.com", "stun.example.com:3478", "ws://x", ""] {
            assert!(
                matches!(validate(&stun(url)), Err(IceError::BadScheme(_))),
                "{url:?} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_an_empty_host() {
        assert!(matches!(validate(&stun("stun:")), Err(IceError::EmptyHost(_))));
        assert!(matches!(validate(&stun("stun: ")), Err(IceError::EmptyHost(_))));
    }

    #[test]
    fn turn_requires_credentials() {
        // A TURN server without credentials is a TURN server that will refuse
        // every allocation, which is indistinguishable from not having one.
        assert!(matches!(
            validate(&stun("turn:relay.example.com:3478")),
            Err(IceError::TurnWithoutCredentials(_))
        ));
        let half = IceServer {
            urls: vec!["turn:relay:3478".into()],
            username: Some("u".into()),
            credential: None,
        };
        assert!(matches!(validate(&half), Err(IceError::TurnWithoutCredentials(_))));
        let blank = turn("turn:relay:3478", "", "");
        assert!(matches!(validate(&blank), Err(IceError::TurnWithoutCredentials(_))));
    }

    #[test]
    fn stun_refuses_credentials() {
        // STUN has no auth. Credentials on a STUN entry would be transmitted
        // to a public server for no reason, which is a leak, not a config.
        assert!(matches!(
            validate(&turn("stun:stun.example.com:3478", "u", "p")),
            Err(IceError::StunWithCredentials(_))
        ));
    }

    #[test]
    fn rejects_an_entry_with_no_urls() {
        let empty = IceServer { urls: vec![], username: None, credential: None };
        assert_eq!(validate(&empty), Err(IceError::NoUrls));
    }

    #[test]
    fn caps_the_list() {
        let many: Vec<_> = (0..=MAX_SERVERS).map(|i| stun(&format!("stun:s{i}:3478"))).collect();
        assert!(matches!(validate_all(&many), Err(IceError::TooMany(_))));
    }

    #[test]
    fn json_round_trips_with_credentials_intact() {
        let list = vec![stun("stun:a:3478"), turn("turn:b:3478", "user", "s3cret")];
        let raw = to_json(&list).unwrap();
        assert_eq!(from_json(&raw), list);
        assert!(raw.contains("s3cret"), "credentials must survive the vault round trip");
    }

    #[test]
    fn stun_entries_serialise_without_null_credential_fields() {
        // The browser accepts nulls, but an entry that says `username: null`
        // reads as a misconfigured TURN server to anyone inspecting it.
        let raw = to_json(&[stun("stun:a:3478")]).unwrap();
        assert!(!raw.contains("username"), "{raw}");
        assert!(!raw.contains("credential"), "{raw}");
    }

    #[test]
    fn a_corrupt_vault_value_falls_back_to_defaults() {
        assert_eq!(from_json("not json"), default_servers());
        assert_eq!(from_json("[{\"urls\":[\"https://nope\"]}]"), default_servers());
    }

    #[test]
    fn an_empty_persisted_list_is_honoured() {
        // Someone who removed every server on purpose — LAN-only sharing, no
        // outside contact — must not have the defaults quietly restored.
        assert_eq!(from_json("[]"), Vec::<IceServer>::new());
    }
}
