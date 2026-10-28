//! Tunnel and TunnelDevice, as ssh -w does it (ssh.c ssh_init_forwarding,
//! clientloop.c client_request_tun_fwd, misc.c tun_open and
//! openbsd-compat/port-net.c; PROTOCOL "tun@openssh.com"). A local tun (or
//! tap) device is joined to one on the server through a tun@openssh.com
//! channel, one packet per channel message.
//!
//! On the wire a layer 3 packet carries a 4-byte address family first,
//! with OpenBSD's numbers (2 for IPv4, 24 for IPv6). Linux has no such
//! header on its tun device, so it is added and removed here
//! (SSH_TUN_PREPEND_AF); macOS utun has one with its own numbers, which are
//! translated (SSH_TUN_COMPAT_AF). Layer 2 frames go as they are.
//!
//! Linux opens /dev/net/tun, macOS a utun control socket (point-to-point
//! only). Both need root (Linux: CAP_NET_ADMIN). Windows would need the
//! wintun driver, and a phone app has no tun device; there Tunnel says so
//! and the session goes on without it.

/// SSH_TUNMODE_POINTOPOINT and SSH_TUNMODE_ETHERNET.
pub const MODE_POINT_TO_POINT: u32 = 1;
pub const MODE_ETHERNET: u32 = 2;
/// SSH_TUNID_ANY: let the system (or the server) choose the unit.
pub const UNIT_ANY: u32 = 0x7fff_ffff;
/// SSH_TUNID_MAX.
const UNIT_MAX: u32 = UNIT_ANY - 2;

#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
/// OpenBSD's address family numbers, which the protocol uses.
const OPENBSD_AF_INET: u32 = 2;
#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
const OPENBSD_AF_INET6: u32 = 24;

/// Tunnel and TunnelDevice, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunConfig {
    pub mode: u32,
    /// The local unit (tunN / tapN / utunN), or UNIT_ANY.
    pub local: u32,
    /// The server's unit, or UNIT_ANY.
    pub remote: u32,
}

/// a2tun() for one side: "any" or a number up to SSH_TUNID_MAX.
fn unit(s: &str) -> Option<u32> {
    if s.eq_ignore_ascii_case("any") {
        return Some(UNIT_ANY);
    }
    s.parse::<u32>().ok().filter(|n| *n <= UNIT_MAX)
}

impl TunConfig {
    /// From the Tunnel value ("point-to-point", "ethernet", "no") and
    /// TunnelDevice ("local[:remote]", default "any:any").
    pub fn parse(tunnel: Option<&str>, device: Option<&str>) -> Option<TunConfig> {
        let mode = match tunnel? {
            "point-to-point" => MODE_POINT_TO_POINT,
            "ethernet" => MODE_ETHERNET,
            _ => return None,
        };
        let (local, remote) = match device {
            None => (UNIT_ANY, UNIT_ANY),
            Some(d) => match d.split_once(':') {
                Some((l, r)) => (unit(l)?, unit(r)?),
                None => (unit(d)?, UNIT_ANY),
            },
        };
        Some(TunConfig { mode, local, remote })
    }
}

#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
/// sys_tun_infilter: a packet read from the device, as it goes on the wire.
/// `None` drops it, as ssh does with a runt.
pub(crate) fn to_wire(mode: u32, packet: &[u8], device_has_af: bool) -> Option<Vec<u8>> {
    if mode != MODE_POINT_TO_POINT {
        return Some(packet.to_vec());
    }
    if device_has_af {
        // SSH_TUN_COMPAT_AF: translate the device's address family.
        if packet.len() <= 4 {
            return None;
        }
        let af = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
        let wire = if af == native_af_inet6() { OPENBSD_AF_INET6 } else { OPENBSD_AF_INET };
        let mut out = packet.to_vec();
        out[..4].copy_from_slice(&wire.to_be_bytes());
        return Some(out);
    }
    // SSH_TUN_PREPEND_AF: the family from the IP header's version.
    if packet.len() <= 20 {
        return None;
    }
    let af = if packet[0] >> 4 == 6 { OPENBSD_AF_INET6 } else { OPENBSD_AF_INET };
    let mut out = Vec::with_capacity(packet.len() + 4);
    out.extend_from_slice(&af.to_be_bytes());
    out.extend_from_slice(packet);
    Some(out)
}

#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
/// sys_tun_outfilter: channel data, as it is written to the device.
pub(crate) fn from_wire(mode: u32, data: &[u8], device_has_af: bool) -> Option<Vec<u8>> {
    if mode != MODE_POINT_TO_POINT {
        return Some(data.to_vec());
    }
    if data.len() < 4 {
        return None;
    }
    if device_has_af {
        let wire = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let af = if wire == OPENBSD_AF_INET6 { native_af_inet6() } else { 2 };
        let mut out = data.to_vec();
        out[..4].copy_from_slice(&af.to_be_bytes());
        return Some(out);
    }
    Some(data[4..].to_vec())
}

#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
/// AF_INET6 where the device's header uses native numbers (30 on macOS).
fn native_af_inet6() -> u32 {
    #[cfg(unix)]
    {
        libc::AF_INET6 as u32
    }
    #[cfg(not(unix))]
    {
        23
    }
}

/// A running tunnel; dropping it stops the relay.
pub struct TunTask {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TunTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod sys {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    use tokio::io::unix::AsyncFd;

    pub struct Device {
        fd: AsyncFd<OwnedFd>,
        pub name: String,
        /// The device reads and writes a 4-byte address family (utun).
        pub has_af: bool,
    }

    fn cvt(r: libc::c_int) -> io::Result<libc::c_int> {
        if r < 0 { Err(io::Error::last_os_error()) } else { Ok(r) }
    }

    impl Device {
        pub async fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
            loop {
                let mut guard = self.fd.readable().await?;
                // SAFETY: reads into a buffer we own, of its length.
                match guard.try_io(|fd| {
                    let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
                    if n < 0 { Err(io::Error::last_os_error()) } else { Ok(n as usize) }
                }) {
                    Ok(r) => return r,
                    Err(_would_block) => continue,
                }
            }
        }

        pub async fn write(&self, buf: &[u8]) -> io::Result<usize> {
            loop {
                let mut guard = self.fd.writable().await?;
                // SAFETY: writes from a buffer we own, of its length.
                match guard.try_io(|fd| {
                    let n = unsafe { libc::write(fd.as_raw_fd(), buf.as_ptr().cast(), buf.len()) };
                    if n < 0 { Err(io::Error::last_os_error()) } else { Ok(n as usize) }
                }) {
                    Ok(r) => return r,
                    Err(_would_block) => continue,
                }
            }
        }
    }

    /// TUNSETIFF, _IOW('T', 202, int).
    #[cfg(target_os = "linux")]
    #[cfg(any(target_arch = "mips", target_arch = "mips64", target_arch = "powerpc", target_arch = "powerpc64", target_arch = "sparc64"))]
    const TUNSETIFF: libc::c_ulong = 0x8004_54ca;
    #[cfg(target_os = "linux")]
    #[cfg(not(any(target_arch = "mips", target_arch = "mips64", target_arch = "powerpc", target_arch = "powerpc64", target_arch = "sparc64")))]
    const TUNSETIFF: libc::c_ulong = 0x4004_54ca;

    /// struct ifreq as TUNSETIFF reads it: the name, then the flags.
    #[cfg(target_os = "linux")]
    #[repr(C)]
    struct IfReq {
        name: [libc::c_char; libc::IFNAMSIZ],
        flags: libc::c_short,
        _pad: [u8; 22],
    }

    /// sys_tun_open (SSH_TUN_LINUX): /dev/net/tun, IFF_TUN or IFF_TAP with
    /// IFF_NO_PI, named tunN/tapN when a unit is given.
    #[cfg(target_os = "linux")]
    pub fn open(mode: u32, unit: u32) -> io::Result<Device> {
        // SAFETY: a plain open of a NUL-terminated path.
        let raw = cvt(unsafe { libc::open(c"/dev/net/tun".as_ptr(), libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC) })?;
        // SAFETY: raw was just opened and is owned here alone.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let mut ifr = IfReq { name: [0; libc::IFNAMSIZ], flags: 0, _pad: [0; 22] };
        let (flag, base) = if mode == super::MODE_ETHERNET { (libc::IFF_TAP, "tap") } else { (libc::IFF_TUN, "tun") };
        ifr.flags = (flag | libc::IFF_NO_PI) as libc::c_short;
        if unit != super::UNIT_ANY {
            let name = format!("{base}{unit}");
            for (d, s) in ifr.name.iter_mut().zip(name.bytes().take(libc::IFNAMSIZ - 1)) {
                *d = s as libc::c_char;
            }
        }
        // SAFETY: TUNSETIFF reads and fills the ifreq we pass.
        cvt(unsafe { libc::ioctl(fd.as_raw_fd(), TUNSETIFF as _, &mut ifr) })?;
        let name: String = ifr.name.iter().take_while(|c| **c != 0).map(|c| *c as u8 as char).collect();
        // SAFETY: the OwnedFd moves into the AsyncFd, so the descriptor stays
        // open and the same until the Device is dropped.
        Ok(Device { fd: unsafe { AsyncFd::register(fd)? }, name, has_af: false })
    }

    /// A utun interface through the kernel control socket; unit N is utunN.
    #[cfg(target_os = "macos")]
    pub fn open(mode: u32, unit: u32) -> io::Result<Device> {
        if mode == super::MODE_ETHERNET {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "macOS utun devices are point-to-point only; Tunnel ethernet needs a tap device"));
        }
        // SAFETY: socket(2) with constant arguments.
        let raw = cvt(unsafe { libc::socket(libc::PF_SYSTEM, libc::SOCK_DGRAM, libc::SYSPROTO_CONTROL) })?;
        // SAFETY: raw was just created and is owned here alone.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: ctl_info is plain data; zeroed is a valid value.
        let mut info: libc::ctl_info = unsafe { std::mem::zeroed() };
        for (d, s) in info.ctl_name.iter_mut().zip(b"com.apple.net.utun_control".iter()) {
            *d = *s as libc::c_char;
        }
        // SAFETY: CTLIOCGINFO fills the ctl_info we pass.
        cvt(unsafe { libc::ioctl(fd.as_raw_fd(), libc::CTLIOCGINFO, &mut info) })?;
        // SAFETY: sockaddr_ctl is plain data; zeroed is a valid value.
        let mut addr: libc::sockaddr_ctl = unsafe { std::mem::zeroed() };
        addr.sc_len = std::mem::size_of::<libc::sockaddr_ctl>() as u8;
        addr.sc_family = libc::AF_SYSTEM as u8;
        addr.ss_sysaddr = libc::AF_SYS_CONTROL as u16;
        addr.sc_id = info.ctl_id;
        addr.sc_unit = if unit == super::UNIT_ANY { 0 } else { unit + 1 };
        // SAFETY: connect with a sockaddr_ctl of the stated size.
        cvt(unsafe { libc::connect(fd.as_raw_fd(), (&addr as *const libc::sockaddr_ctl).cast(), std::mem::size_of::<libc::sockaddr_ctl>() as libc::socklen_t) })?;
        let mut name = [0u8; libc::IFNAMSIZ];
        let mut len = name.len() as libc::socklen_t;
        // SAFETY: UTUN_OPT_IFNAME writes at most `len` bytes into `name`.
        cvt(unsafe { libc::getsockopt(fd.as_raw_fd(), libc::SYSPROTO_CONTROL, libc::UTUN_OPT_IFNAME, name.as_mut_ptr().cast(), &mut len) })?;
        let name: String = name.iter().take_while(|c| **c != 0).map(|c| *c as char).collect();
        // SAFETY: fcntl on our own descriptor.
        let flags = cvt(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) })?;
        cvt(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) })?;
        cvt(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) })?;
        // SAFETY: the OwnedFd moves into the AsyncFd, so the descriptor stays
        // open and the same until the Device is dropped.
        Ok(Device { fd: unsafe { AsyncFd::register(fd)? }, name, has_af: true })
    }
}

/// Why the device could not be opened, in words.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_error(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => format!(
            "opening a tun device needs administrator rights (root{}): {e}",
            if cfg!(target_os = "linux") { " or CAP_NET_ADMIN" } else { "" }
        ),
        Some(libc::ENOENT) | Some(libc::ENODEV) => format!("there is no tun device on this computer (is the tun module loaded?): {e}"),
        Some(libc::EBUSY) => format!("the tun unit is already in use: {e}"),
        _ => format!("Tunnel device open failed: {e}"),
    }
}

/// Open the local device and the channel, then relay until either ends.
/// Returns the running tunnel and the note ssh logs ("Tunnel forwarding
/// using interface tun0").
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub async fn start(handle: &super::client::SharedHandle, cfg: &TunConfig) -> Result<(TunTask, String), String> {
    let dev = sys::open(cfg.mode, cfg.local).map_err(|e| open_error(&e))?;
    tracing::debug!("Requesting tun unit {} in mode {}", cfg.local, cfg.mode);
    let channel = handle
        .lock()
        .await
        .channel_open_tun(cfg.mode, cfg.remote)
        .await
        .map_err(|e| format!("the server refused the tunnel (its PermitTunnel, or the unit): {e}"))?;
    let note = format!("Tunnel forwarding using interface {}", dev.name);
    let mode = cfg.mode;
    let task = tokio::spawn(async move {
        let (mut rd, wr) = channel.split();
        let mut buf = vec![0u8; 65536];
        loop {
            tokio::select! {
                r = dev.read(&mut buf) => match r {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Some(pkt) = to_wire(mode, &buf[..n], dev.has_af) {
                            if wr.datagram(pkt).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Tunnel device {}: {e}", dev.name);
                        break;
                    }
                },
                msg = rd.wait() => match msg {
                    Some(russh::ChannelMsg::Data { data }) => {
                        if let Some(pkt) = from_wire(mode, &data, dev.has_af) {
                            // A packet the device refuses is lost, as in ssh.
                            if let Err(e) = dev.write(&pkt).await {
                                tracing::debug!("Tunnel device {}: write: {e}", dev.name);
                            }
                        }
                    }
                    Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) | None => break,
                    Some(_) => {}
                },
            }
        }
        let _ = wr.close().await;
        tracing::info!("Tunnel on {} closed", dev.name);
    });
    Ok((TunTask { task }, note))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub async fn start(_handle: &super::client::SharedHandle, _cfg: &TunConfig) -> Result<(TunTask, String), String> {
    Err(if cfg!(windows) {
        "Tunnel needs a tun device; not available on this platform (Windows would need the wintun driver)".to_string()
    } else {
        "Tunnel needs a tun device; not available on this platform".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_device_forms() {
        let pp = |d| TunConfig::parse(Some("point-to-point"), d);
        assert_eq!(pp(None), Some(TunConfig { mode: 1, local: UNIT_ANY, remote: UNIT_ANY }));
        assert_eq!(pp(Some("0:1")), Some(TunConfig { mode: 1, local: 0, remote: 1 }));
        assert_eq!(pp(Some("5")), Some(TunConfig { mode: 1, local: 5, remote: UNIT_ANY }));
        assert_eq!(pp(Some("any:3")), Some(TunConfig { mode: 1, local: UNIT_ANY, remote: 3 }));
        assert_eq!(TunConfig::parse(Some("ethernet"), Some("2:any")).map(|c| c.mode), Some(MODE_ETHERNET));
        assert_eq!(TunConfig::parse(Some("no"), None), None);
        assert_eq!(TunConfig::parse(None, Some("1")), None);
    }

    #[test]
    fn address_family_on_the_wire() {
        let mut v4 = vec![0x45u8; 28];
        v4[0] = 0x45;
        let w = to_wire(MODE_POINT_TO_POINT, &v4, false).unwrap();
        assert_eq!(&w[..4], &[0, 0, 0, 2]);
        assert_eq!(&w[4..], &v4[..]);
        assert_eq!(from_wire(MODE_POINT_TO_POINT, &w, false).unwrap(), v4);
        let mut v6 = vec![0u8; 48];
        v6[0] = 0x60;
        assert_eq!(&to_wire(MODE_POINT_TO_POINT, &v6, false).unwrap()[..4], &[0, 0, 0, 24]);
        // Runts are dropped, as sys_tun_infilter does.
        assert!(to_wire(MODE_POINT_TO_POINT, &v4[..20], false).is_none());
        assert!(from_wire(MODE_POINT_TO_POINT, &[0, 0, 2], false).is_none());
        // A device with its own header (utun): translated both ways.
        let mut dev6 = native_af_inet6().to_be_bytes().to_vec();
        dev6.extend_from_slice(&v6);
        let w = to_wire(MODE_POINT_TO_POINT, &dev6, true).unwrap();
        assert_eq!(&w[..4], &[0, 0, 0, 24]);
        assert_eq!(from_wire(MODE_POINT_TO_POINT, &w, true).unwrap(), dev6);
        // Ethernet frames pass untouched.
        assert_eq!(to_wire(MODE_ETHERNET, &[1, 2, 3], false).unwrap(), vec![1, 2, 3]);
    }
}
