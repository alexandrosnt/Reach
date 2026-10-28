//! An X server for X11 forwarding on Windows, the way MobaXterm brings its
//! own: when nothing has set DISPLAY, Reach uses an X server already
//! running on this machine, or starts VcXsrv. VcXsrv is not in the
//! installer: the user turns it on in Settings, which downloads it once,
//! checks it against a SHA-256 pinned here and keeps it under the tools
//! folder. Nothing is downloaded without that.
//!
//! The server is started with a random MIT-MAGIC-COOKIE-1 of Reach's own
//! (`-auth`), never with access control off: nothing on this machine or
//! the network can draw on it without the cookie, and the cookie stays
//! with Reach (the SSH server gets a fake one, see `x11`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// The files of the official VcXsrv 21.1.16.1 release (unpacked from
/// vcxsrv-64.21.1.16.1.installer.noadmin.exe, SHA-256 dea6c7d6…d620,
/// without uninstall.exe and plink.exe), with its licence and a note on
/// where the source is.
pub const VERSION: &str = "21.1.16.1";
const URL: &str = "https://github.com/alexandrosnt/Reach/releases/download/tools-vcxsrv-21.1.16.1/vcxsrv-21.1.16.1-x64.zip";
const SHA256: &str = "2f272a234108595fbe91b0b537fad881c4d11ad52879e1634c844cb9c908ded8";
/// The zip's size, for the progress bar when the server does not say.
const ZIP_BYTES: u64 = 51267344;

/// A display Reach can forward to.
#[derive(Debug, Clone)]
pub struct Display {
    /// "127.0.0.1:N".
    pub name: String,
    /// The server's cookie, when Reach started it; `None` for a server
    /// someone else runs (its own access control applies).
    pub cookie: Option<Vec<u8>>,
    /// The authority file holding `cookie`, for xauth.
    pub auth_file: Option<PathBuf>,
    /// The xauth that comes with the server.
    pub xauth: Option<PathBuf>,
}

struct Running {
    display: Display,
    child: std::process::Child,
    /// Closing it ends the server: Reach's X server lives as long as Reach.
    _job: Job,
}

fn state() -> &'static tokio::sync::Mutex<Option<Running>> {
    static S: std::sync::OnceLock<tokio::sync::Mutex<Option<Running>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

fn root() -> PathBuf {
    crate::app_data_dir().join("tools").join("vcxsrv").join(VERSION)
}

/// Whether the X server is set up (turned on in Settings).
pub fn installed() -> bool {
    root().join("vcxsrv.exe").is_file()
}

fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)).is_ok()
}

/// The display to forward to: an X server already running here, or Reach's
/// own when it is turned on in Settings.
pub async fn ensure() -> Result<Display, String> {
    let mut st = state().lock().await;
    if let Some(r) = st.as_mut() {
        if r.child.try_wait().ok().flatten().is_none() {
            return Ok(r.display.clone());
        }
        *st = None;
    }
    // Someone else's X server on :0, only when what listens there is one
    // (MobaXterm, VcXsrv, X410, Xming, Cygwin/X): another program on that
    // port would see every window and keystroke.
    if port_open(6000) {
        match listener_owners(6000).as_deref() {
            Some([pid]) if process_image(*pid).as_deref().is_some_and(is_x_server) => {
                return Ok(Display { name: "127.0.0.1:0".into(), cookie: None, auth_file: None, xauth: None });
            }
            _ => tracing::warn!("X11: something that is not a known X server listens on display :0; Reach starts its own"),
        }
    }
    if !installed() {
        return Err("no X server on this computer: turn on the X11 server in Settings, General".into());
    }
    let running = start(&root()).await?;
    let display = running.display.clone();
    *st = Some(running);
    Ok(display)
}

/// Where an install is: bytes downloaded, then files unpacked.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: &'static str,
    pub done: u64,
    pub total: u64,
}

/// Downloads, checks and unpacks the X server. `cancel` stops it at the
/// next chunk or file; a stopped or failed install leaves nothing behind.
pub async fn install(cancel: &AtomicBool, progress: impl Fn(Progress)) -> Result<(), String> {
    let dir = root();
    let bytes = fetch(cancel, &progress).await?;
    progress(Progress { stage: "verify", done: 0, total: 1 });
    let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if got != SHA256 {
        return Err(format!("the download does not match the X server's pinned SHA-256 (got {got}); nothing was installed"));
    }
    let staging = dir.with_extension("partial");
    let _ = std::fs::remove_dir_all(&staging);
    // Thousands of small files: blocking work, off the async workers.
    let result = tokio::task::block_in_place(|| unpack(&bytes, &staging, cancel, &progress));
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&dir);
    // Only a complete, verified tree takes the final name.
    std::fs::rename(staging.join("vcxsrv"), &dir).map_err(|e| format!("X server: {e}"))?;
    let _ = std::fs::remove_dir_all(&staging);
    tracing::info!("X11: VcXsrv {VERSION} installed");
    Ok(())
}

/// Stops Reach's X server and removes its files.
pub async fn remove() -> Result<(), String> {
    if let Some(mut r) = state().lock().await.take() {
        let _ = r.child.kill();
    }
    let dir = root();
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("could not remove the X server: {e}"))?;
    }
    Ok(())
}

const CANCELLED: &str = "cancelled";

async fn fetch(cancel: &AtomicBool, progress: &impl Fn(Progress)) -> Result<Vec<u8>, String> {
    // A development build can use a local copy of the zip.
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("REACH_DEV_VCXSRV_ZIP") {
        let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
        let total = bytes.len() as u64;
        // Paced, so the progress bar and Cancel can be tried.
        for step in 1..=20u64 {
            if cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.into());
            }
            progress(Progress { stage: "download", done: total * step / 20, total });
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        return Ok(bytes);
    }
    use futures_util::StreamExt;
    let client = crate::http::client_builder()
        .user_agent("Reach (https://github.com/alexandrosnt/Reach)")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(URL).send().await.map_err(|e| format!("X server download failed: {e}"))?;
    let resp = resp.error_for_status().map_err(|e| format!("X server download failed: {e}"))?;
    let total = resp.content_length().unwrap_or(ZIP_BYTES);
    let mut bytes = Vec::with_capacity(total as usize);
    let mut stream = resp.bytes_stream();
    let mut last = 0u64;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let chunk = chunk.map_err(|e| format!("X server download failed: {e}"))?;
        bytes.extend_from_slice(&chunk);
        // A size well past the expected one is not the X server.
        if bytes.len() as u64 > ZIP_BYTES * 2 {
            return Err("the X server download is larger than it should be; stopped".into());
        }
        let done = bytes.len() as u64;
        if done - last >= 256 * 1024 || done == total {
            last = done;
            progress(Progress { stage: "download", done, total });
        }
    }
    Ok(bytes)
}

/// Unpacks the zip under `into`, refusing any entry that would land
/// outside it.
fn unpack(bytes: &[u8], into: &Path, cancel: &AtomicBool, progress: &impl Fn(Progress)) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { return Err(format!("unsafe path in the X server zip: {}", String::from_utf8_lossy(f.name_raw()))) };
        let out = into.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
        }
        if i % 50 == 0 || i as u64 + 1 == total {
            progress(Progress { stage: "unpack", done: i as u64 + 1, total });
        }
    }
    Ok(())
}

/// An Xauthority entry for display `n` (any address: the server checks
/// the cookie, not where it is presented from).
fn authority(n: u16, cookie: &[u8]) -> Vec<u8> {
    // FamilyWild (0xffff) with an empty address matches any address.
    let mut out = vec![0xff, 0xff, 0, 0];
    for field in [n.to_string().as_bytes(), b"MIT-MAGIC-COOKIE-1".as_slice(), cookie] {
        out.extend((field.len() as u16).to_be_bytes());
        out.extend(field);
    }
    out
}

async fn start(dir: &Path) -> Result<Running, String> {
    // The first free display from 10 up: below that MobaXterm and the like
    // usually sit. A display with an X<n>.hosts file is skipped: hosts
    // listed there get in without the cookie.
    let n = (10u16..64)
        .find(|n| !port_open(6000 + n) && !dir.join(format!("X{n}.hosts")).exists())
        .ok_or("no free X display number")?;
    let cookie: [u8; 16] = rand::random();
    // VcXsrv takes relative paths from its own folder and refuses any
    // argument over 128 characters, so the files live there under short
    // names (the folder is the user's own, under the app data).
    let auth_name = format!("reach-auth.{n}");
    let auth_file = dir.join(&auth_name);
    std::fs::write(&auth_file, authority(n, &cookie)).map_err(|e| format!("X server: {e}"))?;
    let log = dir.join("reach-vcxsrv.log");

    let mut cmd = std::process::Command::new(dir.join("vcxsrv.exe"));
    cmd.arg(format!(":{n}"))
        .args(["-multiwindow", "-clipboard", "-wgl", "-silent-dup-error", "-notrayicon", "-auth", &auth_name, "-logfile", "reach-vcxsrv.log"])
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("could not start the X server: {e}"))?;
    let job = Job::new().map_err(|e| format!("X server: {e}"))?;
    job.assign(&child);

    // OpenGL setup alone takes seconds on a first start.
    for _ in 0..300 {
        if port_open(6000 + n) {
            // The port must be the server's alone: a program that took it
            // first (or listens on 127.0.0.1 beside it) would receive the
            // real cookie.
            if listener_owners(6000 + n).as_deref() != Some(&[child.id()][..]) {
                let _ = child.kill();
                return Err(format!("another program listens on X display :{n}; not forwarding X11 to it"));
            }
            // A server that lets a client in without the cookie (it could
            // not read the file) is not one to forward to.
            if !refuses_without_cookie(6000 + n) {
                let _ = child.kill();
                return Err(format!("the X server accepts connections without its cookie; not using it (see {})", log.display()));
            }
            let display = Display {
                // By address: "localhost" could reach a listener on ::1.
                name: format!("127.0.0.1:{n}"),
                cookie: Some(cookie.to_vec()),
                auth_file: Some(auth_file),
                xauth: Some(dir.join("xauth.exe")).filter(|p| p.is_file()),
            };
            tracing::info!("X11: started VcXsrv {VERSION} on display :{n}");
            return Ok(Running { display, child, _job: job });
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("the X server exited ({status}); see {}", log.display()));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = child.kill();
    Err(format!("the X server did not start listening; see {}", log.display()))
}

/// The processes listening for IPv4 connections to 127.0.0.1 on `port`
/// (bound to it or to every address), each once; `None` if the table
/// cannot be read.
fn listener_owners(port: u16) -> Option<Vec<u32>> {
    use windows::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER};
    use windows::Win32::Networking::WinSock::AF_INET;
    let mut size = 0u32;
    // SAFETY: a size query with no buffer.
    unsafe { GetExtendedTcpTable(None, &mut size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    // u32s, so the table is aligned as Windows lays it out.
    let mut buf = vec![0u32; (size as usize).div_ceil(4) + 64];
    size = (buf.len() * 4) as u32;
    // SAFETY: `buf` holds `size` bytes.
    let r = unsafe { GetExtendedTcpTable(Some(buf.as_mut_ptr().cast()), &mut size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    if r != 0 {
        return None;
    }
    let table = buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
    // SAFETY: Windows filled a MIB_TCPTABLE_OWNER_PID with dwNumEntries rows.
    let rows = unsafe { std::slice::from_raw_parts((*table).table.as_ptr(), (*table).dwNumEntries as usize) };
    let loopback = u32::from_ne_bytes([127, 0, 0, 1]);
    let mut pids: Vec<u32> = rows
        .iter()
        .filter(|r| u16::from_be(r.dwLocalPort as u16) == port && (r.dwLocalAddr == 0 || r.dwLocalAddr == loopback))
        .map(|r| r.dwOwningPid)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    Some(pids)
}

/// The executable a process runs.
fn process_image(pid: u32) -> Option<String> {
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: the handle is closed below; `buf` holds `len` UTF-16 units.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = windows::Win32::Foundation::CloseHandle(h);
        r.ok()?;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

/// The X servers people run on Windows.
fn is_x_server(image: &str) -> bool {
    let name = image.rsplit(['\\', '/']).next().unwrap_or(image).to_ascii_lowercase();
    matches!(name.as_str(), "vcxsrv.exe" | "xwin.exe" | "xwin_mobax.exe" | "mobaxterm.exe" | "x410.exe" | "xming.exe")
}

/// Connects with no authorization and reads the answer: 0 is "Failed".
fn refuses_without_cookie(port: u16) -> bool {
    use std::io::{Read, Write};
    let Ok(mut s) = std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    // Little-endian setup, protocol 11.0, no authorization.
    if s.write_all(&[0x6c, 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_err() {
        return false;
    }
    let mut first = [0u8; 1];
    matches!(s.read_exact(&mut first), Ok(()) if first[0] == 0)
}

/// A Windows job that kills its processes when Reach closes it or exits.
struct Job(windows::Win32::Foundation::HANDLE);

// SAFETY: a job handle may be used from any thread.
unsafe impl Send for Job {}

impl Job {
    fn new() -> windows::core::Result<Self> {
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        // SAFETY: plain Win32 calls with valid arguments; the handle is
        // owned by the returned Job and closed in Drop.
        unsafe {
            let h = CreateJobObjectW(None, None)?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )?;
            Ok(Job(h))
        }
    }

    fn assign(&self, child: &std::process::Child) {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        let ph = windows::Win32::Foundation::HANDLE(child.as_raw_handle());
        // SAFETY: both handles are valid for the call.
        if let Err(e) = unsafe { AssignProcessToJobObject(self.0, ph) } {
            tracing::warn!("X11: the X server is not tied to Reach's lifetime: {e}");
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateJobObjectW and is closed once.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_entry_is_xauth_format() {
        let a = authority(12, &[7; 16]);
        assert_eq!(&a[..4], &[0xff, 0xff, 0, 0]);
        assert_eq!(&a[4..6], &[0, 2]);
        assert_eq!(&a[6..8], b"12");
        assert_eq!(&a[8..10], &[0, 18]);
        assert_eq!(&a[10..28], b"MIT-MAGIC-COOKIE-1");
        assert_eq!(&a[28..30], &[0, 16]);
        assert_eq!(&a[30..], &[7; 16]);
    }

    #[test]
    fn known_x_servers() {
        assert!(is_x_server(r"C:\Program Files\VcXsrv\vcxsrv.exe"));
        assert!(is_x_server(r"C:\Users\u\AppData\Local\Temp\Mxt\bin\XWin_MobaX.exe"));
        assert!(!is_x_server(r"C:\evil\listener.exe"));
    }

    #[test]
    fn finds_who_listens() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert_eq!(listener_owners(port), Some(vec![std::process::id()]));
        let me = process_image(std::process::id()).unwrap();
        assert!(me.to_ascii_lowercase().ends_with(".exe"), "{me}");
    }

    #[test]
    fn unpack_refuses_paths_outside() {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            w.start_file("../evil.txt", zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, b"x").unwrap();
            w.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("reach-xs-{}", std::process::id()));
        assert!(unpack(&buf, &dir, &AtomicBool::new(false), &|_| {}).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
