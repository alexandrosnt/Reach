//! Peer-to-peer session sharing: configuration only.
//!
//! The transport itself is not here. WebRTC lives in the webview, which
//! already ships a complete stack, so the Rust side owns nothing but the
//! settings — and owns those because a TURN entry carries a credential,
//! which belongs in the vault next to every other secret.
//!
//! Two rules, both borrowed from the MCP server:
//!
//! 1. **Off by default, and off means off.** Not hidden in the UI: the
//!    frontend does not import the sharing module at all until this is
//!    enabled and a share is started. No `RTCPeerConnection` is constructed,
//!    no STUN server is contacted, no listener exists. Someone who never
//!    wants this runs none of its code.
//! 2. **Nothing happens on its own.** Enabled is not the same as active.
//!    There is no pre-warming, no connectivity probe at launch, no keepalive
//!    to a relay. The first packet this feature sends is in response to the
//!    user clicking Share.

pub mod ice;

use serde::{Deserialize, Serialize};

pub use ice::{default_servers, IceServer};

/// Everything the frontend needs to decide whether, and how, to connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareConfig {
    /// Master switch. `false` is the shipped default.
    pub enabled: bool,
    /// ICE servers in the order they will be offered to the browser.
    pub ice_servers: Vec<IceServer>,
}

impl Default for ShareConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ice_servers: default_servers(),
        }
    }
}

/// Settings-vault keys. Two rather than one so flipping the switch does not
/// rewrite the server list, and editing the list does not touch the switch.
pub const ENABLED_KEY: &str = "share_enabled";
pub const ICE_KEY: &str = "share_ice_servers";
