//! Remote desktop over RDP, decoded in Rust and streamed to the webview.
//!
//! The protocol work is IronRDP's, end to end: connection sequence, CredSSP,
//! TLS through rustls, the bitmap codecs. All of it is pure Rust, which is why
//! this builds for Android as well as the three desktops with no native
//! toolchain involved. What lives here is the part IronRDP deliberately leaves
//! to the embedder — where the pixels go and where the input comes from.
//!
//! Pixels go over a Tauri [`Channel`] as raw bytes, not as events. A serial
//! console can afford `app.emit` because its payload is a few bytes of text;
//! a framebuffer cannot, because Tauri events are JSON and a single full
//! 1080p frame would be eleven megabytes of base64. `InvokeResponseBody::Raw`
//! skips serialisation entirely, and it works identically inside the Android
//! webview, which is what ruled out a loopback WebSocket — that would have
//! needed a listener, a CSP exception and a cleartext allowance on mobile.
//!
//! IronRDP is asked for [`RdpOutputEvent::DesktopUpdate`] rather than whole
//! frames. Those are tightly packed changed regions, so a blinking cursor is a
//! few hundred bytes and only a full-screen change costs a full screen. They
//! are not forwarded one by one, though. A video or a scroll arrives as
//! dozens of 64-pixel tiles per frame, and every message to the webview is a
//! script evaluation on its main thread plus a round trip to fetch the bytes;
//! forwarded as they came, the tiles would queue up faster than they could
//! be painted and the picture would fall further and further behind the
//! server. IronRDP will not drop a region either — a dropped diff is a hole
//! in the picture — so once the queue is full its session loop blocks, and
//! the server's own frames stack up behind it. That is the lag remote
//! desktop tools are known for, and it is a plumbing problem, not a protocol
//! one.
//!
//! So the pump keeps its own copy of the framebuffer, applies every update to
//! it the moment it arrives, and sends the webview only what has changed since
//! the last message it acknowledged painting, as one message. The webview is
//! never more than two messages behind, and a busy screen costs it exactly as
//! many messages as it can paint — under load the picture updates less often
//! but never late, which is how the reference clients behave. See [`Flow`].
//!
//! The clipboard is in — text and files, both ways — with a backend of our
//! own in [`clipboard`]. Not in this version, on purpose: sound and drive
//! redirection. Each pulls a native backend into the build, and neither is
//! needed to see a desktop and drive it. The server's certificate goes
//! through the same trust-on-first-use prompt and known-hosts file as an SSH
//! host key; see `build_config`.

pub mod clipboard;

use std::collections::HashMap;
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ironrdp_client::config::{ClipboardType, ConfigBuilder, Destination};
use ironrdp_client::output_channel::{output_channel, OutputEventReceiver};
use ironrdp_client::rdp::{RdpClient, RdpInputEvent, RdpInputSender, RdpOutputEvent};
use ironrdp_graphics::pointer::DecodedPointer;
use ironrdp_pdu::geometry::InclusiveRectangle;
use ironrdp_pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp_pdu::input::mouse::{MousePdu, PointerFlags};
use ironrdp_pdu::rdp::capability_sets::MajorPlatformType;
use serde::{Deserialize, Serialize};
use smallvec::smallvec;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;
use tokio::time::Instant;

/// What the webview sends to open a desktop.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RdpConnectParams {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub domain: String,
    /// A local folder to show inside the desktop as a drive.
    #[serde(default)]
    pub share_path: Option<String>,
    /// Advertise the graphics pipeline. Off unless the user turned it on.
    #[serde(default)]
    pub graphics_pipeline: bool,
    /// The size the panel can show, so the server renders at the size it is
    /// seen at rather than being scaled after the fact.
    pub width: u16,
    pub height: u16,
}

/// Lifecycle, sent as a Tauri event named `rdp-status-{id}`.
///
/// Frames are too large for events; these are too rare and too important for
/// the frame channel, where they would wait their turn behind pixels. Keeping
/// them apart means a "closed" is never late.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum RdpStatus {
    Connected,
    /// The server sent its login-complete notification: the desktop is usable.
    LoggedIn,
    Closed { reason: String },
    Error { message: String },
}

/// Frame kinds on the wire. Layout is little-endian, header first.
///
/// `Region` and `Full`: kind, x, y, w, h, fb_w, fb_h (u8 + 6×u16 = 13 bytes),
/// then `w × h × 4` bytes of RGBA. `Full` is a `Region` whose rectangle is the
/// whole framebuffer; it is a separate kind only so the receiver can tell "the
/// framebuffer was replaced" from "a region that happens to be everything".
///
/// `Pointer`: kind, x, y (5 bytes) — the server moved the cursor.
///
/// `PointerShape`: kind, w, h, hotspot x, hotspot y (9 bytes), then
/// `w × h × 4` bytes of straight (not premultiplied) RGBA — the cursor the
/// webview should show over the desktop from now on. `PointerHidden` and
/// `PointerDefault` are the kind byte alone. The cursor is drawn by the
/// webview, not by the server into the frame, so it moves at the speed of
/// the local mouse and not of the picture; that is also what mstsc does.
///
/// One message may hold several frames back to back — a cursor change, a
/// pointer position, then any number of regions. The receiver walks them in
/// order and acknowledges the message once, after the last one is painted.
mod kind {
    pub const REGION: u8 = 1;
    pub const FULL: u8 = 2;
    pub const POINTER: u8 = 3;
    pub const POINTER_SHAPE: u8 = 4;
    pub const POINTER_HIDDEN: u8 = 5;
    pub const POINTER_DEFAULT: u8 = 6;
}

const HEADER_LEN: usize = 13;

/// How many messages may be on their way to the webview at once. One is
/// being painted while the next is already in transit; a third would only be
/// latency. Two is also what keeps the picture at most one paint behind
/// under load.
const MAX_IN_FLIGHT: u32 = 2;

/// How long the picture must have been still before what changed is sent.
///
/// A frame reaches the pump as a burst of tiles, one event each, with
/// microseconds between them; the next frame is a whole network round trip
/// away. Waiting for this much quiet after the *last* tile means a message
/// holds whole frames, and the webview never paints half of one — which is
/// the mosaic of old and new tiles a video shows through clients that paint
/// tiles as they come. A keypress echo pays this once, which nobody sees.
const QUIET: Duration = Duration::from_millis(3);

/// The longest a change may be held back while the picture keeps changing.
/// Something that never goes quiet — a busy screen on a slow decode — is
/// still shown at least this often, torn if it must be, rather than never.
const MAX_HOLD: Duration = Duration::from_millis(50);

/// What the cursor should look like, as the server last said.
enum Cursor {
    Shape(Arc<DecodedPointer>),
    Hidden,
    Default,
}

/// The pacing agreement between the pump and the webview.
///
/// The webview acknowledges each message once it has painted it. The pump
/// counts what is unacknowledged and sends only while that is below
/// [`MAX_IN_FLIGHT`]; the rest accumulates in [`Screen`] until there is room.
/// This is what stops the queue growing when the screen is busier than the
/// webview can paint — the backlog lives in a framebuffer that only ever gets
/// newer, not in a queue that only ever gets longer.
pub struct Flow {
    in_flight: AtomicU32,
    notify: Notify,
    /// A new canvas is listening: send it the whole picture next.
    refresh: std::sync::atomic::AtomicBool,
}

impl Flow {
    fn new() -> Self {
        Self {
            in_flight: AtomicU32::new(0),
            notify: Notify::new(),
            refresh: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// A new webview attached: nothing it has not painted is in flight, and
    /// it needs the whole picture.
    fn reattached(&self) {
        self.in_flight.store(0, Ordering::Release);
        self.refresh.store(true, Ordering::Release);
        self.notify.notify_one();
    }

    fn take_refresh(&self) -> bool {
        self.refresh.swap(false, Ordering::AcqRel)
    }

    fn can_send(&self) -> bool {
        self.in_flight.load(Ordering::Acquire) < MAX_IN_FLIGHT
    }

    fn sent(&self) {
        self.in_flight.fetch_add(1, Ordering::AcqRel);
    }

    /// The webview painted one message. An extra acknowledgement — there is
    /// no path that produces one, but a wrapped counter would stop the
    /// picture for good — is ignored rather than counted.
    pub fn ack(&self) {
        let _ = self
            .in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
        self.notify.notify_one();
    }
}

/// One open desktop. Dropping the input sender does not end the session —
/// `request_close` does — so both halves are kept.
///
/// The session runs on a thread of its own, not on Tauri's runtime. The
/// connection sequence's future is not `Send` — CredSSP and the TLS handshake
/// hold state across awaits that the compiler cannot prove safe to move — so
/// it cannot be spawned onto a multi-threaded executor at all. A dedicated
/// thread with a single-threaded runtime is what IronRDP's own viewer does,
/// and it has a second benefit: a stall inside one desktop cannot starve
/// anything else in the app.
struct Open {
    input: RdpInputSender,
    session: std::thread::JoinHandle<()>,
    pump: tokio::task::JoinHandle<()>,
    flow: Arc<Flow>,
    /// Where frames go. Replaced when the webview's panel is re-created —
    /// a page navigation does that — so the session survives it.
    frames: Arc<std::sync::Mutex<Channel<InvokeResponseBody>>>,
    /// Dropped with the entry, which is what ends the clipboard thread.
    clipboard: clipboard::ClipboardHandle,
}

#[derive(Default)]
pub struct RdpManager {
    open: HashMap<String, Open>,
}

impl RdpManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a desktop and start streaming it to `frames`.
    ///
    /// Returns once the session task is running, not once the desktop is up:
    /// the connection sequence can take seconds against a slow host, and the
    /// webview learns the outcome through the status event either way.
    pub fn connect(
        &mut self,
        app: AppHandle,
        params: RdpConnectParams,
        frames: Channel<InvokeResponseBody>,
    ) -> Result<(), String> {
        let id = params.id.clone();

        // The panel for an open desktop was re-created — the page it lives on
        // was left and returned to. The session is still running; give it the
        // new channel and the whole picture, and say it is connected.
        if let Some(open) = self.open.get(&id) {
            if !open.session.is_finished() {
                tracing::info!("RDP {id}: panel re-attached to the running session");
                *open.frames.lock().unwrap_or_else(|e| e.into_inner()) = frames;
                open.flow.reattached();
                let _ = app.emit(&format!("rdp-status-{id}"), RdpStatus::Connected);
                return Ok(());
            }
        }

        // Otherwise a connect for an id that is already open replaces what is
        // left of it. The old session is asked to close rather than dropped.
        if let Some(previous) = self.open.remove(&id) {
            // A thread cannot be aborted; the close request ends the session
            // and the thread exits with it. Only the pump is cut short.
            previous.input.request_close();
            previous.pump.abort();
            let _ = previous.session;
        }

        tracing::info!(
            "RDP {id}: connecting to {}:{} as {} at {}x{}",
            params.host, params.port, params.username, params.width, params.height
        );
        let config = build_config(&params, app.clone()).map_err(|e| {
            tracing::warn!("RDP {id}: configuration rejected: {e}");
            e
        })?;

        // Sixteen control events may queue before the producer backpressures.
        // Frames are not counted against this; see the module docs.
        let (out_tx, out_rx) = output_channel(16);
        let client = RdpClient::new(config, out_tx).with_desktop_updates();
        let input = client.input_sender();

        // The clipboard has a thread of its own; see the clipboard module.
        let (clipboard, clip_factory) = clipboard::spawn(&id, input.clone(), app.clone())
            .map_err(|e| format!("could not start the RDP clipboard thread: {e}"))?;
        let mut client = client.with_cliprdr_backend_factory(Box::new(clip_factory));

        // A shared folder rides the device channel as one drive. The native
        // backends serve it: IronRDP's own on Windows, its Unix one on the
        // others. Android has no backend for it and says so.
        if let Some(path) = params.share_path.as_deref().filter(|p| !p.trim().is_empty()) {
            match drive_factory(path) {
                Ok(factory) => {
                    tracing::info!("RDP {id}: sharing {path} as a drive");
                    client = client.with_rdpdr_backend_factory(factory);
                }
                Err(e) => tracing::warn!("RDP {id}: not sharing {path}: {e}"),
            }
        }



        // See `Open` for why this is a thread and not a task.
        let thread_name = format!("rdp-{id}");
        let session = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("RDP: could not start a runtime for the session: {e}");
                        return;
                    }
                };
                runtime.block_on(client.run());
            })
            .map_err(|e| format!("could not start the RDP session thread: {e}"))?;

        let flow = Arc::new(Flow::new());
        let frames = Arc::new(std::sync::Mutex::new(frames));
        let pump = tokio::spawn(pump_output(app, id.clone(), out_rx, Arc::clone(&frames), Arc::clone(&flow)));

        self.open.insert(id, Open { input, session, pump, flow, frames, clipboard });
        Ok(())
    }

    /// The local clipboard may have changed: offer it to this desktop.
    pub fn clipboard_sync(&self, id: &str) -> Result<(), String> {
        self.open
            .get(id)
            .map(|open| open.clipboard.sync())
            .ok_or_else(|| format!("no RDP session {id}"))
    }

    /// Ask every open desktop to end. Used on the way out of the app, so the
    /// server sees a disconnect rather than a dropped socket, and no session
    /// thread is left holding a connection while the window goes away.
    pub fn disconnect_all(&mut self) {
        for (id, open) in self.open.drain() {
            tracing::info!("RDP {id}: closing on exit");
            open.input.request_close();
        }
    }

    /// The webview has painted one message; see [`Flow`].
    pub fn ack(&self, id: &str) -> Result<(), String> {
        self.open
            .get(id)
            .map(|open| open.flow.ack())
            .ok_or_else(|| format!("no RDP session {id}"))
    }

    /// Ask the session to end, then stop waiting for it.
    pub fn disconnect(&mut self, id: &str) -> Result<(), String> {
        let Some(open) = self.open.remove(id) else {
            return Err(format!("no RDP session {id}"));
        };
        open.input.request_close();
        // The session task ends on its own once the close goes through; the
        // pump ends when the output channel does. Neither needs aborting,
        // and aborting the session would skip the graceful disconnect PDU.
        let _ = open.pump;
        let _ = open.session;
        Ok(())
    }

    fn input(&self, id: &str) -> Result<&RdpInputSender, String> {
        self.open
            .get(id)
            .map(|open| &open.input)
            .ok_or_else(|| format!("no RDP session {id}"))
    }

    pub fn send_mouse(&self, id: &str, pdu: MousePdu) -> Result<(), String> {
        self.send_fast_path(id, FastPathInputEvent::MouseEvent(pdu))
    }

    pub fn send_key(&self, id: &str, scancode: u8, extended: bool, release: bool) -> Result<(), String> {
        let mut flags = KeyboardFlags::empty();
        if extended {
            flags |= KeyboardFlags::EXTENDED;
        }
        if release {
            flags |= KeyboardFlags::RELEASE;
        }
        self.send_fast_path(id, FastPathInputEvent::KeyboardEvent(flags, scancode))
    }

    /// For characters that have no scancode on this keyboard — an IME
    /// commit, or a character pasted rather than typed.
    pub fn send_unicode(&self, id: &str, code: u16, release: bool) -> Result<(), String> {
        let flags = if release { KeyboardFlags::RELEASE } else { KeyboardFlags::empty() };
        self.send_fast_path(id, FastPathInputEvent::UnicodeKeyboardEvent(flags, code))
    }

    pub fn resize(&self, id: &str, width: u16, height: u16) -> Result<(), String> {
        let (width, height) = clamp_desktop(width, height);
        tracing::info!("RDP {id}: resize requested to {width}x{height}");
        self.input(id)?
            .try_send(RdpInputEvent::Resize {
                width,
                height,
                scale_factor: 100,
                physical_size: None,
            })
            .map_err(|e| e.to_string())
    }

    fn send_fast_path(&self, id: &str, event: FastPathInputEvent) -> Result<(), String> {
        // `try_send` rather than a blocking send: the queue is bounded so a
        // stalled session cannot pile up input, and dropping a mouse move is
        // preferable to stalling the command that delivered it.
        self.input(id)?
            .try_send(RdpInputEvent::FastPath(smallvec![event]))
            .map_err(|e| e.to_string())
    }
}

/// Build the mouse PDU from what the webview knows: where, and what changed.
///
/// RDP encodes a button release as the button flag *without* `DOWN`, and a
/// move as `MOVE` alone; the wheel is a magnitude plus a sign flag. This is
/// the one place that mapping lives.
pub fn mouse_pdu(x: u16, y: u16, action: &str, button: u8, delta: i16) -> Result<MousePdu, String> {
    let button_flag = match button {
        0 => PointerFlags::LEFT_BUTTON,
        1 => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
        2 => PointerFlags::RIGHT_BUTTON,
        other => return Err(format!("unknown mouse button {other}")),
    };

    let mut flags = PointerFlags::empty();
    let mut rotation: i16 = 0;

    match action {
        "move" => flags |= PointerFlags::MOVE,
        "down" => flags |= button_flag | PointerFlags::DOWN,
        "up" => flags |= button_flag,
        "wheel" => {
            flags |= PointerFlags::VERTICAL_WHEEL;
            if delta < 0 {
                flags |= PointerFlags::WHEEL_NEGATIVE;
            }
            // The wire field is a magnitude; the sign travels as a flag.
            rotation = delta.unsigned_abs().min(255) as i16;
        }
        other => return Err(format!("unknown mouse action {other}")),
    }

    Ok(MousePdu {
        flags,
        number_of_wheel_rotation_units: rotation,
        x_position: x,
        y_position: y,
    })
}

fn build_config(p: &RdpConnectParams, app: AppHandle) -> Result<ironrdp_client::config::Config, String> {
    let destination = Destination::new(format!("{}:{}", p.host, p.port)).map_err(|e| e.to_string())?;

    // Certificates are checked strictly first; one the platform trusts passes
    // without a word. Anything else — which is nearly every RDP server, since
    // they self-sign — goes through the same trust-on-first-use prompt and
    // known-hosts file as an SSH host key. The callback is synchronous and
    // runs on the session thread, so the prompt is run on the app's runtime
    // and waited for here; the session has nothing else to do meanwhile.
    let (host, port) = (p.host.clone(), p.port);
    let verify: ironrdp_tls::CertificateValidationCallback = Arc::new(move |der: &[u8], _name: &str, reason: &str| {
        use base64::Engine as _;
        use sha2::Digest as _;
        tracing::info!("RDP {host}:{port}: certificate not trusted by the platform ({reason}); asking");
        let fingerprint = format!(
            "SHA256:{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(sha2::Sha256::digest(der))
        );
        let (tx, rx) = std::sync::mpsc::channel();
        let (app, host) = (app.clone(), host.clone());
        tauri::async_runtime::spawn(async move {
            let host_id = format!("rdp:{host}:{port}");
            let ok = crate::ssh::client::verify_host_identity(Some(app), &host, port, &host_id, &fingerprint, "x509 certificate").await;
            let _ = tx.send(ok);
        });
        // The prompt itself times out at 120 s; this only guards against
        // the task never reporting back.
        rx.recv_timeout(Duration::from_secs(130)).unwrap_or(false)
    });
    let (width, height) = clamp_desktop(p.width, p.height);

    // Three fields the builder insists on and the docs do not mention. The
    // build number is what mstsc reports about itself and servers only log
    // it; the client dir is the module path mstsc sends, which some servers
    // have been seen to inspect, so the reference viewer sends the same
    // string and so does this. The platform is what the server uses to
    // pick a keyboard model and a few capability defaults.
    let platform = if cfg!(target_os = "windows") {
        MajorPlatformType::WINDOWS
    } else if cfg!(target_os = "macos") {
        MajorPlatformType::MACINTOSH
    } else if cfg!(target_os = "android") {
        MajorPlatformType::ANDROID
    } else if cfg!(target_os = "ios") {
        MajorPlatformType::IOS
    } else {
        MajorPlatformType::UNIX
    };

    let mut builder = ConfigBuilder::new()
        .with_destination(destination)
        .with_username(&p.username)
        .with_password(&p.password)
        .with_client_name("Reach")
        .with_client_build(client_build())
        .with_client_dir("C:\\Windows\\System32\\mstscax.dll")
        .with_platform(platform)
        .with_desktop_width(width)
        .with_desktop_height(height)
        // Windows requires Network Level Authentication by default, and
        // CredSSP is how that is done. Both halves are pure Rust in IronRDP.
        .with_credssp(true)
        .with_tls(true)
        // "Server pointer" is IronRDP's name for taking the cursor shapes the
        // server sends; without it the server draws the cursor into the frame
        // and it moves at the frame rate. With it, and software rendering off,
        // the shapes come out as events and the webview draws the cursor
        // itself — the local mouse never waits for the picture.
        .with_server_pointer(true)
        .with_pointer_software_rendering(false)
        // Our own backend, handed to the client in `connect`.
        .with_clipboard(ClipboardType::Enable)
        .with_rdpdr(p.share_path.as_deref().is_some_and(|s| !s.trim().is_empty()))
        .with_graphics_pipeline(p.graphics_pipeline)
        // H.264 for the graphics pipeline, one decoder per connection. With
        // it the client advertises AVC420 and a Windows server sends video as
        // video rather than as tiles. Made here rather than once, because a
        // reconnect builds the pipeline again.
        .with_h264_decoder_factory(Arc::new(|| match ironrdp_egfx::decode::OpenH264Decoder::new() {
            Ok(decoder) => Some(Box::new(decoder) as Box<dyn ironrdp_egfx::decode::H264Decoder>),
            Err(e) => {
                tracing::warn!("RDP: no H.264 decoder, video will come as tiles: {e}");
                None
            }
        }))
        .with_certificate_validation(ironrdp_tls::CertificateValidation::Strict)
        .with_certificate_validation_callback(verify);

    if !p.domain.is_empty() {
        builder = builder.with_domain(&p.domain);
    }

    // Remote audio, played through the default output device. IronRDP owns
    // the playback backend; only the desktops build it (see Cargo.toml).
    #[cfg(not(target_os = "android"))]
    {
        builder = builder.with_sound(true);
    }

    builder.build().map_err(|e| e.to_string())
}

/// The drive backend for one shared folder, named after the folder.
#[cfg(windows)]
fn drive_factory(path: &str) -> Result<Box<dyn ironrdp_rdpdr::backend::RdpdrBackendFactory + Send>, String> {
    use ironrdp_rdpdr_native::{RedirectedDrive, WindowsRdpdrBackendFactory};
    let drive = RedirectedDrive::new(1, drive_name(path), path, false).map_err(|e| e.to_string())?;
    Ok(Box::new(WindowsRdpdrBackendFactory::new(drive)))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn drive_factory(path: &str) -> Result<Box<dyn ironrdp_rdpdr::backend::RdpdrBackendFactory + Send>, String> {
    struct Factory {
        path: String,
        name: String,
    }
    impl ironrdp_rdpdr::backend::RdpdrBackendFactory for Factory {
        fn build_rdpdr_backend(&self) -> ironrdp_rdpdr::backend::RdpdrBackendFactoryResult<ironrdp_rdpdr::backend::RdpdrBackendProduct> {
            let backend = ironrdp_rdpdr_native::backend::NixRdpdrBackend::new(self.path.clone());
            Ok(ironrdp_rdpdr::backend::RdpdrBackendProduct::new(
                Box::new(backend),
                vec![ironrdp_rdpdr::backend::RdpdrDrive::new(1, self.name.clone())],
            ))
        }
    }
    if !std::path::Path::new(path).is_dir() {
        return Err("not a directory".into());
    }
    Ok(Box::new(Factory { path: path.to_string(), name: drive_name(path) }))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn drive_factory(_path: &str) -> Result<Box<dyn ironrdp_rdpdr::backend::RdpdrBackendFactory + Send>, String> {
    Err("drive redirection is not available on this platform".into())
}

/// What the drive is called on the remote: the folder's own name, or the
/// whole path when it has none (a volume root).
fn drive_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| path.trim_end_matches(['\\', '/']).to_string())
}

/// The build number this client reports about itself: Reach's own version,
/// folded into one integer the way the reference viewer does it. Servers log
/// it and nothing more, so what matters is only that it is stable and ours.
fn client_build() -> u32 {
    let mut parts = env!("CARGO_PKG_VERSION").split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    major * 100 + minor * 10 + patch
}

/// RDP wants even dimensions and has limits both ways. A panel can report
/// zero during layout; that is turned into something a server will accept.
fn clamp_desktop(width: u16, height: u16) -> (u16, u16) {
    let even = |v: u16| v & !1;
    (even(width.clamp(200, 8192)), even(height.clamp(200, 8192)))
}

/// Move output from IronRDP to the webview until the session ends.
///
/// Nothing is forwarded as it arrives. Every change lands in [`Screen`]'s copy
/// of the framebuffer first, and what goes over the channel is whatever has
/// changed since the last message the webview acknowledged, as one message.
/// See [`Flow`] for why.
async fn pump_output(
    app: AppHandle,
    id: String,
    mut rx: OutputEventReceiver,
    frames: Arc<std::sync::Mutex<Channel<InvokeResponseBody>>>,
    flow: Arc<Flow>,
) {
    let status = |s: RdpStatus| {
        let _ = app.emit(&format!("rdp-status-{id}"), s);
    };

    let mut screen = Screen::default();

    loop {
        let pending = screen.pending();
        let settled = screen.settled();
        let deadline = screen.deadline().unwrap_or_else(Instant::now);

        // Each arm's guard keeps this from spinning: the sleep is armed only
        // while the quiet window is open, and the notify only while the
        // webview owes an acknowledgement. After any arm, the flush below
        // decides — so a burst of events on the first arm still gets flushed
        // as soon as the picture goes quiet and there is room.
        tokio::select! {
            biased;
            event = rx.recv() => {
                let Some(event) = event else { break };
                if !screen.receive(&id, event, &status) {
                    break;
                }
            }
            _ = flow.notify.notified(), if pending && !flow.can_send() => {}
            _ = tokio::time::sleep_until(deadline), if pending && !settled => {}
        }

        if flow.take_refresh() {
            screen.replaced = true;
            screen.touch();
        }

        if screen.pending() && screen.settled() && flow.can_send() {
            flow.sent();
            let sent = frames
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .send(InvokeResponseBody::Raw(screen.take()));
            if sent.is_err() {
                // The webview is gone; nothing left to draw for.
                break;
            }
        }
    }
}

/// The pump's own picture of the desktop, plus what has changed on it since
/// the webview last saw it.
#[derive(Default)]
struct Screen {
    /// RGBA, row-major, `width × height × 4` bytes.
    fb: Vec<u8>,
    width: u16,
    height: u16,
    /// Changed rectangles not yet sent. Empty while `replaced` is set: a full
    /// frame covers them all.
    damage: Vec<InclusiveRectangle>,
    /// The framebuffer changed size, or the server sent the whole thing.
    replaced: bool,
    pointer: Option<(u16, u16)>,
    cursor: Option<Cursor>,
    /// When the oldest unsent change arrived, and when the newest did. The
    /// quiet window is measured from the newest, the hold limit from the
    /// oldest; see [`QUIET`] and [`MAX_HOLD`].
    since: Option<Instant>,
    last: Option<Instant>,
}

impl Screen {
    fn pending(&self) -> bool {
        self.replaced || !self.damage.is_empty() || self.pointer.is_some() || self.cursor.is_some()
    }

    /// What is pending may be sent: the picture has been still for
    /// [`QUIET`], or has been held for [`MAX_HOLD`].
    fn settled(&self) -> bool {
        self.deadline().is_none_or(|t| t <= Instant::now())
    }

    /// When what is pending becomes sendable, if anything is pending.
    fn deadline(&self) -> Option<Instant> {
        let (since, last) = (self.since?, self.last?);
        Some((last + QUIET).min(since + MAX_HOLD))
    }

    fn touch(&mut self) {
        let now = Instant::now();
        if self.since.is_none() {
            self.since = Some(now);
        }
        self.last = Some(now);
    }

    /// Absorb one event. Returns false when the session is over.
    fn receive(&mut self, id: &str, event: RdpOutputEvent, status: &dyn Fn(RdpStatus)) -> bool {
        match event {
            RdpOutputEvent::Connected => {
                tracing::info!("RDP {id}: connected");
                status(RdpStatus::Connected);
            }
            RdpOutputEvent::LoginComplete => {
                tracing::info!("RDP {id}: login complete");
                status(RdpStatus::LoggedIn);
            }

            RdpOutputEvent::DesktopUpdate(update) => {
                let (buffer, width, height, region) = update.into_parts();
                self.apply(&buffer, width, height, region);
            }

            RdpOutputEvent::Image { buffer, width, height } => {
                let all = InclusiveRectangle {
                    left: 0,
                    top: 0,
                    right: width.get() - 1,
                    bottom: height.get() - 1,
                };
                self.apply(&buffer, width, height, all);
                self.replaced = true;
                self.damage.clear();
            }

            RdpOutputEvent::PointerPosition { x, y } => {
                self.pointer = Some((x, y));
                self.touch();
            }

            // The cursor is the webview's to draw; only the latest word on
            // its shape matters, so a newer one replaces an unsent older one.
            RdpOutputEvent::PointerBitmap(shape) => {
                self.cursor = Some(Cursor::Shape(shape));
                self.touch();
            }
            RdpOutputEvent::PointerHidden => {
                self.cursor = Some(Cursor::Hidden);
                self.touch();
            }
            RdpOutputEvent::PointerDefault => {
                self.cursor = Some(Cursor::Default);
                self.touch();
            }

            RdpOutputEvent::ConnectionFailure(err) => {
                // Logged as well as shown: the panel's overlay is the only
                // other place this goes, and "could not connect" relayed by
                // a person is not a diagnosis.
                tracing::warn!("RDP {id}: connection failed: {err:?}");
                status(RdpStatus::Error { message: err.to_string() });
                return false;
            }

            RdpOutputEvent::Terminated(result) => {
                let reason = match result {
                    Ok(graceful) => format!("{graceful:?}"),
                    Err(err) => {
                        tracing::warn!("RDP {id}: session ended with an error: {err:?}");
                        err.to_string()
                    }
                };
                tracing::info!("RDP {id}: closed ({reason})");
                status(RdpStatus::Closed { reason });
                return false;
            }

            // RAIL, monitor layouts, reconnect notices: not used yet. Listed
            // rather than wildcarded so a new variant in an IronRDP upgrade
            // is a compile error here, not a silent drop.
            RdpOutputEvent::MonitorLayout(_)
            | RdpOutputEvent::WindowingOrders(_)
            | RdpOutputEvent::RailHandshake { .. }
            | RdpOutputEvent::RailDesktopSynchronized { .. }
            | RdpOutputEvent::RailPostHandshakeQueueReleased { .. }
            | RdpOutputEvent::RailExecuteResult(_)
            | RdpOutputEvent::RailExecuteFailed { .. }
            | RdpOutputEvent::RailApplicationId { .. }
            | RdpOutputEvent::RailControl(_)
            | RdpOutputEvent::PostLogonDisplayRedraw
            | RdpOutputEvent::MalformedBitmapDisplayRedraw
            | RdpOutputEvent::AutoReconnected => {}
            RdpOutputEvent::DisplayResizeFallback(size) => {
                tracing::info!("RDP {id}: server has no display control; resize to {size:?} means a reconnect");
            }
            RdpOutputEvent::AutoReconnecting { .. } => {
                tracing::info!("RDP {id}: reconnecting");
            }
        }
        true
    }

    /// Write one region of IronRDP's `0x00RRGGBB` pixels into the framebuffer
    /// and remember that it changed.
    fn apply(&mut self, buffer: &[u32], width: NonZeroU16, height: NonZeroU16, region: InclusiveRectangle) {
        if self.width != width.get() || self.height != height.get() {
            tracing::info!("RDP: desktop is now {}x{}", width.get(), height.get());
            self.width = width.get();
            self.height = height.get();
            // Opaque black until the server has painted it; the canvas on
            // the other side is opaque too, so alpha is only ever 0xff.
            self.fb = vec![0u8; usize::from(self.width) * usize::from(self.height) * 4];
            for px in self.fb.chunks_exact_mut(4) {
                px[3] = 0xff;
            }
            self.replaced = true;
            self.damage.clear();
        }

        if region.right >= self.width || region.bottom >= self.height || region.left > region.right || region.top > region.bottom {
            // IronRDP validates its own updates against the extent it
            // reports, so this is not expected; skipping a bad one beats
            // reading out of bounds for it.
            tracing::debug!("RDP: update region {region:?} outside a {}x{} desktop", self.width, self.height);
            return;
        }

        let w = usize::from(region.right - region.left) + 1;
        let stride = usize::from(self.width) * 4;
        // A short buffer stops early and a long one is cut; neither should
        // happen, and neither is worth a panic.
        for (row, src) in (region.top..=region.bottom).zip(buffer.chunks(w)) {
            let start = usize::from(row) * stride + usize::from(region.left) * 4;
            for (dst, &px) in self.fb[start..start + w * 4].chunks_exact_mut(4).zip(src) {
                dst[0] = (px >> 16) as u8;
                dst[1] = (px >> 8) as u8;
                dst[2] = px as u8;
                dst[3] = 0xff;
            }
        }

        if !self.replaced {
            self.damage.push(region);
        }
        self.touch();
    }

    /// Everything pending, as one message, and nothing pending afterwards.
    fn take(&mut self) -> Vec<u8> {
        let mut out = Vec::new();

        match self.cursor.take() {
            Some(Cursor::Shape(shape)) => {
                let bytes = usize::from(shape.width) * usize::from(shape.height) * 4;
                out.reserve(9 + bytes);
                out.push(kind::POINTER_SHAPE);
                for v in [shape.width, shape.height, shape.hotspot_x, shape.hotspot_y] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                // IronRDP sizes the bitmap to width × height × 4; guarded
                // all the same, as the region path is.
                out.extend_from_slice(&shape.bitmap_data[..bytes.min(shape.bitmap_data.len())]);
                out.resize(9 + bytes, 0);
            }
            Some(Cursor::Hidden) => out.push(kind::POINTER_HIDDEN),
            Some(Cursor::Default) => out.push(kind::POINTER_DEFAULT),
            None => {}
        }

        if let Some((x, y)) = self.pointer.take() {
            out.push(kind::POINTER);
            out.extend_from_slice(&x.to_le_bytes());
            out.extend_from_slice(&y.to_le_bytes());
        }

        if self.replaced {
            self.replaced = false;
            self.damage.clear();
            if self.width > 0 && self.height > 0 {
                let all = InclusiveRectangle {
                    left: 0,
                    top: 0,
                    right: self.width - 1,
                    bottom: self.height - 1,
                };
                self.encode(kind::FULL, &all, &mut out);
            }
        } else {
            for rect in merge_damage(&self.damage) {
                self.encode(kind::REGION, &rect, &mut out);
            }
            self.damage.clear();
        }

        self.since = None;
        out
    }

    /// Append one region of the framebuffer in the wire format.
    fn encode(&self, kind: u8, region: &InclusiveRectangle, out: &mut Vec<u8>) {
        let w = usize::from(region.right - region.left) + 1;
        let h = usize::from(region.bottom - region.top) + 1;
        out.reserve(HEADER_LEN + w * h * 4);
        out.push(kind);
        for v in [region.left, region.top, w as u16, h as u16, self.width, self.height] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let stride = usize::from(self.width) * 4;
        for row in region.top..=region.bottom {
            let start = usize::from(row) * stride + usize::from(region.left) * 4;
            out.extend_from_slice(&self.fb[start..start + w * 4]);
        }
    }
}

/// Fewer, larger rectangles when that costs little. Video and scrolling
/// arrive as dozens of tiles that together are one rectangle anyway; a cursor
/// blink and a clock tick are two small ones far apart and stay two. The
/// bounding box wins when it is at most half again the area of the pieces,
/// or when there are more pieces than are worth painting one by one.
fn merge_damage(rects: &[InclusiveRectangle]) -> Vec<InclusiveRectangle> {
    if rects.len() <= 1 {
        return rects.to_vec();
    }
    let area = |r: &InclusiveRectangle| u64::from(r.right - r.left + 1) * u64::from(r.bottom - r.top + 1);
    let bbox = rects.iter().skip(1).fold(rects[0].clone(), |b, r| InclusiveRectangle {
        left: b.left.min(r.left),
        top: b.top.min(r.top),
        right: b.right.max(r.right),
        bottom: b.bottom.max(r.bottom),
    });
    let pieces: u64 = rects.iter().map(area).sum();
    if rects.len() > 8 || area(&bbox) * 2 <= pieces * 3 {
        vec![bbox]
    } else {
        rects.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nz(v: u16) -> NonZeroU16 {
        NonZeroU16::new(v).unwrap()
    }

    fn header(bytes: &[u8], at: usize) -> (u8, u16, u16, u16, u16, u16, u16) {
        let u = |i: usize| u16::from_le_bytes([bytes[at + i], bytes[at + i + 1]]);
        (bytes[at], u(1), u(3), u(5), u(7), u(9), u(11))
    }

    #[test]
    fn first_update_is_a_full_frame_and_later_ones_are_regions() {
        let mut screen = Screen::default();
        let region = InclusiveRectangle { left: 2, top: 3, right: 3, bottom: 3 }; // 2×1
        screen.apply(&[0x00112233, 0x00AABBCC], nz(100), nz(50), region.clone());
        assert!(screen.pending());

        let bytes = screen.take();
        assert_eq!(bytes.len(), HEADER_LEN + 100 * 50 * 4);
        assert_eq!(header(&bytes, 0), (kind::FULL, 0, 0, 100, 50, 100, 50));
        let at = HEADER_LEN + (3 * 100 + 2) * 4;
        assert_eq!(&bytes[at..at + 8], &[0x11, 0x22, 0x33, 0xff, 0xAA, 0xBB, 0xCC, 0xff]);
        assert!(!screen.pending());

        screen.apply(&[0x00FF0000, 0x0000FF00], nz(100), nz(50), region);
        let bytes = screen.take();
        assert_eq!(bytes.len(), HEADER_LEN + 2 * 4);
        assert_eq!(header(&bytes, 0), (kind::REGION, 2, 3, 2, 1, 100, 50));
        assert_eq!(&bytes[HEADER_LEN..], &[0xFF, 0, 0, 0xff, 0, 0xFF, 0, 0xff]);
    }

    #[test]
    fn short_buffer_leaves_the_rest_untouched() {
        let mut screen = Screen::default();
        let region = InclusiveRectangle { left: 0, top: 0, right: 1, bottom: 0 }; // 2 px
        screen.apply(&[0x00FFFFFF], nz(2), nz(1), region);
        let bytes = screen.take();
        assert_eq!(bytes.len(), HEADER_LEN + 8);
        assert_eq!(&bytes[HEADER_LEN..HEADER_LEN + 4], &[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(&bytes[HEADER_LEN + 4..], &[0, 0, 0, 0xff]);
    }

    #[test]
    fn pointer_and_pixels_share_one_message() {
        let mut screen = Screen::default();
        screen.apply(&[0; 4], nz(2), nz(2), InclusiveRectangle { left: 0, top: 0, right: 1, bottom: 1 });
        screen.take();
        screen.pointer = Some((7, 9));
        screen.apply(&[0x00010203], nz(2), nz(2), InclusiveRectangle { left: 1, top: 1, right: 1, bottom: 1 });
        let bytes = screen.take();
        assert_eq!(bytes[0], kind::POINTER);
        assert_eq!(u16::from_le_bytes([bytes[1], bytes[2]]), 7);
        assert_eq!(header(&bytes, 5), (kind::REGION, 1, 1, 1, 1, 2, 2));
        assert_eq!(bytes.len(), 5 + HEADER_LEN + 4);
    }

    #[test]
    fn many_tiles_become_one_rectangle_but_two_far_apart_stay_two() {
        let tile = |x: u16, y: u16| InclusiveRectangle { left: x, top: y, right: x + 63, bottom: y + 63 };
        let tiles: Vec<_> = (0..3).flat_map(|r| (0..3).map(move |c| tile(c * 64, r * 64))).collect();
        assert_eq!(merge_damage(&tiles), vec![InclusiveRectangle { left: 0, top: 0, right: 191, bottom: 191 }]);

        let far = [tile(0, 0), tile(1000, 1000)];
        assert_eq!(merge_damage(&far).len(), 2);

        let two_adjacent = [tile(0, 0), tile(64, 0)];
        assert_eq!(merge_damage(&two_adjacent), vec![InclusiveRectangle { left: 0, top: 0, right: 127, bottom: 63 }]);
    }

    #[test]
    fn cursor_goes_first_and_only_the_latest_shape_is_sent() {
        let mut screen = Screen::default();
        screen.cursor = Some(Cursor::Hidden);
        screen.cursor = Some(Cursor::Shape(Arc::new(DecodedPointer {
            width: 2,
            height: 1,
            hotspot_x: 1,
            hotspot_y: 0,
            bitmap_data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        })));
        screen.touch();
        screen.pointer = Some((3, 4));
        let bytes = screen.take();
        assert_eq!(bytes[0], kind::POINTER_SHAPE);
        assert_eq!(&bytes[1..9], &[2, 0, 1, 0, 1, 0, 0, 0]);
        assert_eq!(&bytes[9..17], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(bytes[17], kind::POINTER);
        assert_eq!(bytes.len(), 17 + 5);
        assert!(!screen.pending());

        screen.cursor = Some(Cursor::Default);
        assert_eq!(screen.take(), vec![kind::POINTER_DEFAULT]);
    }

    #[test]
    fn a_change_waits_for_quiet_but_not_forever() {
        let mut screen = Screen::default();
        assert!(screen.settled(), "nothing pending is trivially settled");
        screen.touch();
        assert!(!screen.settled(), "just touched: the quiet window is open");
        let deadline = screen.deadline().unwrap();
        assert!(deadline <= screen.since.unwrap() + MAX_HOLD);
        assert!(deadline >= screen.last.unwrap() + QUIET - Duration::from_micros(1));
        // A touch long ago with a recent one: the hold limit wins.
        screen.since = Some(Instant::now() - MAX_HOLD * 2);
        screen.last = Some(Instant::now());
        assert!(screen.settled());
    }

    #[test]
    fn flow_admits_two_and_then_waits_for_acks() {
        let flow = Flow::new();
        assert!(flow.can_send());
        flow.sent();
        flow.sent();
        assert!(!flow.can_send());
        flow.ack();
        assert!(flow.can_send());
        flow.ack();
        flow.ack(); // one more than was sent: must not wrap around
        assert!(flow.can_send());
        flow.sent();
        flow.sent();
        assert!(!flow.can_send());
    }

    #[test]
    fn release_is_button_without_down() {
        let down = mouse_pdu(1, 2, "down", 0, 0).unwrap();
        let up = mouse_pdu(1, 2, "up", 0, 0).unwrap();
        assert!(down.flags.contains(PointerFlags::LEFT_BUTTON | PointerFlags::DOWN));
        assert!(up.flags.contains(PointerFlags::LEFT_BUTTON));
        assert!(!up.flags.contains(PointerFlags::DOWN));
    }

    #[test]
    fn wheel_sign_travels_as_a_flag() {
        let back = mouse_pdu(0, 0, "wheel", 0, -120).unwrap();
        assert!(back.flags.contains(PointerFlags::VERTICAL_WHEEL | PointerFlags::WHEEL_NEGATIVE));
        assert_eq!(back.number_of_wheel_rotation_units, 120);
        let fwd = mouse_pdu(0, 0, "wheel", 0, 120).unwrap();
        assert!(!fwd.flags.contains(PointerFlags::WHEEL_NEGATIVE));
    }

    #[test]
    fn desktop_size_is_even_and_bounded() {
        assert_eq!(clamp_desktop(0, 0), (200, 200));
        assert_eq!(clamp_desktop(1921, 1081), (1920, 1080));
        assert_eq!(clamp_desktop(9000, 9000), (8192, 8192));
    }
}
