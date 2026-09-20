//! Turning a bare socket error into something a person can act on.
//!
//! Issue #47: on macOS, connecting to a host on the same switch failed with
//! "No route to host (os error 65)" while `ping` to the same address worked
//! from Terminal. Nothing was wrong with the route. macOS 15 puts every app's
//! access to the local network behind a permission, enforces it with a packet
//! filter rather than a prompt at the socket, and reports the dropped packets
//! to the app as EHOSTUNREACH — the same errno a genuinely missing route would
//! give. A build that is not signed with an Apple-issued certificate never
//! gets the permission prompt at all (Apple, TN3179), which is why the user
//! saw no dialog and had nothing to grant.
//!
//! The socket layer cannot tell those two cases apart, but it can tell whether
//! the address is on a local network, and on macOS that is enough to say what
//! is almost certainly going on and where the switch is.

use std::net::IpAddr;

/// EHOSTUNREACH on Darwin. Linux uses 113 for the same condition, and there
/// it really does mean a routing problem, so only Darwin's value is special.
#[cfg(target_os = "macos")]
const DARWIN_EHOSTUNREACH: i32 = 65;

/// Whether an address the user typed points at the local network: RFC 1918
/// private ranges, link-local, IPv6 unique-local and link-local, or a
/// `.local` name. A public address or an ordinary hostname is not — a
/// hostname may resolve to a LAN address, but after a failed connect there is
/// no resolution to look at, so the hint for those stays conditional.
pub fn is_local_network_target(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.to_ascii_lowercase().ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_link_local()
                // 100.64.0.0/10, carrier-grade NAT and Tailscale's range; on
                // a Mac this is still "the local network" for privacy purposes.
                || (o[0] == 100 && (64..=127).contains(&o[1]))
        }
        Ok(IpAddr::V6(v6)) => {
            let s = v6.segments();
            // fe80::/10 link-local, fc00::/7 unique-local.
            (s[0] & 0xffc0) == 0xfe80 || (s[0] & 0xfe00) == 0xfc00
        }
        Err(_) => false,
    }
}

/// The message for a failed connect. Every platform gets the plain error;
/// macOS gets the explanation when the error is the one local network privacy
/// produces.
pub fn describe_connect_error(host: &str, err: &russh::Error) -> String {
    #[cfg(target_os = "macos")]
    {
        if let russh::Error::IO(io) = err {
            if io.raw_os_error() == Some(DARWIN_EHOSTUNREACH) {
                return if is_local_network_target(host) {
                    format!(
                        "No route to host — but {} is on your local network, and macOS only \
                         lets an app reach the local network once you have allowed it. Open \
                         System Settings → Privacy & Security → Local Network and turn on \
                         Reach. If Reach is not in that list, macOS never offered the prompt: \
                         it only does so for apps signed with an Apple Developer certificate, \
                         and this build is not. The installation guide has the current \
                         workaround.",
                        host
                    )
                } else {
                    format!(
                        "No route to host ({}). If this host is on your local network, macOS \
                         needs Local Network permission for Reach: System Settings → Privacy & \
                         Security → Local Network.",
                        host
                    )
                };
            }
        }
    }
    let _ = host;
    format!("{}", err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc1918_and_link_local_are_local() {
        // The reporter's address, and the other private ranges.
        assert!(is_local_network_target("10.33.61.47"));
        assert!(is_local_network_target("192.168.8.180"));
        assert!(is_local_network_target("172.16.0.1"));
        assert!(is_local_network_target("172.31.255.254"));
        assert!(is_local_network_target("169.254.10.1"));
        assert!(is_local_network_target("100.100.1.2"));
    }

    #[test]
    fn ipv6_local_ranges_are_local() {
        assert!(is_local_network_target("fe80::1"));
        assert!(is_local_network_target("[fe80::c93:b30b:ab14:a041]"));
        assert!(is_local_network_target("fd12:3456::1"));
    }

    #[test]
    fn dot_local_names_are_local() {
        assert!(is_local_network_target("stevede-mac-mini.local"));
        assert!(is_local_network_target("NAS.LOCAL"));
    }

    #[test]
    fn public_addresses_and_plain_hostnames_are_not() {
        assert!(!is_local_network_target("51.75.64.176"));
        assert!(!is_local_network_target("8.8.8.8"));
        assert!(!is_local_network_target("172.32.0.1"));
        assert!(!is_local_network_target("2001:db8::1"));
        assert!(!is_local_network_target("example.com"));
        assert!(!is_local_network_target("bastion.corp.internal"));
        assert!(!is_local_network_target(""));
    }

    #[test]
    fn an_ordinary_error_is_left_alone() {
        let err = russh::Error::IO(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "Connection refused",
        ));
        assert_eq!(describe_connect_error("10.0.0.1", &err), format!("{}", err));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn host_unreachable_on_the_lan_names_the_permission() {
        let err = russh::Error::IO(std::io::Error::from_raw_os_error(DARWIN_EHOSTUNREACH));
        let msg = describe_connect_error("10.33.61.47", &err);
        assert!(msg.contains("Local Network"), "{}", msg);
        assert!(msg.contains("10.33.61.47"), "{}", msg);
        let msg = describe_connect_error("example.com", &err);
        assert!(msg.contains("If this host is on your local network"), "{}", msg);
    }
}
