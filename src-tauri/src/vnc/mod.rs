//! VNC (RFB) desktops.
//!
//! The protocol is vnc-rs's; what reaches the webview is the RDP backend's.
//! Every update the server sends is folded into the same [`Screen`] the RDP
//! pump keeps, and leaves it in the same wire format under the same pacing
//! ([`Flow`]), so the desktop panel that draws RDP draws VNC unchanged:
//! scaling, the cursor, full screen and the phone key bar come with it.
//!
//! RFB is a pull protocol: the server sends an update only when asked. The
//! pump asks every [`PULL_EVERY`] while the webview has room for more and
//! stops asking while it does not, which is how a slow canvas slows the
//! server down instead of growing a backlog.
//!
//! Keys travel as X11 keysyms, worked out by the webview from what was typed
//! (see `src/lib/vnc/keysym.ts`): a VNC server types the character it is
//! sent, so the local keyboard layout is the one that counts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ironrdp_graphics::pointer::DecodedPointer;
use serde::Deserialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Instant, MissedTickBehavior};
use vnc::{PixelFormat, VncClient, VncConnector, VncEncoding, VncEvent, X11Event};

use crate::db::conn::Forward;
use crate::rdp::{Cursor, Flow, RdpStatus, Screen};

/// What the webview sends to open a desktop.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VncConnectParams {
    pub id: String,
    pub host: String,
    pub port: u16,
    /// The VNC password; empty for a server that asks for none.
    #[serde(default)]
    pub password: String,
    /// Use the saved session's password manager entry, fetched by
    /// `vnc_connect` from the stored session (not from this value); wins over
    /// `password`. JSON `passwordRef` (`password_ref` is accepted).
    #[serde(default, alias = "password_ref")]
    pub password_ref: Option<crate::pwmanager::SecretRef>,
    /// A saved SSH session to reach the server through. VNC itself is not
    /// encrypted; through SSH it is.
    #[serde(default)]
    pub via_session_id: Option<String>,
}

/// How often to ask the server for what changed: sixty times a second, the
/// same ceiling the RDP pump holds to. A server with nothing new says
/// nothing, so an idle desktop costs a few bytes a tick.
const PULL_EVERY: Duration = Duration::from_millis(16);

/// How long the connection and the login together may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Mouse and keyboard events that may wait for the session. A stalled
/// session drops input past this rather than piling it up.
const INPUT_QUEUE: usize = 256;

/// RFB button bits: left, middle, right, then the wheel as buttons 4 and 5.
const WHEEL_UP: u8 = 1 << 3;
const WHEEL_DOWN: u8 = 1 << 4;

pub enum Input {
    Key { keysym: u32, down: bool },
    Mouse { x: u16, y: u16, action: MouseAction, button: u8, delta: i16 },
    Clipboard(String),
    Resize { width: u16, height: u16 },
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Move,
    Down,
    Up,
    Wheel,
}

impl MouseAction {
    pub fn parse(action: &str) -> Result<Self, String> {
        Ok(match action {
            "move" => Self::Move,
            "down" => Self::Down,
            "up" => Self::Up,
            "wheel" => Self::Wheel,
            other => return Err(format!("unknown mouse action {other}")),
        })
    }
}

/// One open desktop.
struct Open {
    input: mpsc::Sender<Input>,
    task: tokio::task::JoinHandle<()>,
    flow: Arc<Flow>,
    /// Replaced when the panel is re-created, as for RDP.
    frames: Arc<std::sync::Mutex<Channel<InvokeResponseBody>>>,
}

#[derive(Default)]
pub struct VncManager {
    open: HashMap<String, Open>,
}

impl VncManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `id` is a session still running, to which a re-created panel
    /// can re-attach rather than connect again.
    pub fn running(&self, id: &str) -> bool {
        self.open.get(id).is_some_and(|o| !o.task.is_finished())
    }

    /// Open a desktop and start streaming it to `frames`. Returns at once;
    /// the outcome arrives as `vnc-status-{id}` events. `forward` is the SSH
    /// leg when there is one, kept alive for as long as the session is.
    pub fn connect(
        &mut self,
        app: AppHandle,
        params: VncConnectParams,
        forward: Option<Forward>,
        frames: Channel<InvokeResponseBody>,
    ) {
        let id = params.id.clone();

        if let Some(open) = self.open.get(&id) {
            if !open.task.is_finished() {
                tracing::info!("VNC {id}: panel re-attached to the running session");
                *open.frames.lock().unwrap_or_else(|e| e.into_inner()) = frames;
                open.flow.reattached();
                let _ = app.emit(&format!("vnc-status-{id}"), RdpStatus::Connected);
                return;
            }
        }
        if let Some(previous) = self.open.remove(&id) {
            let _ = previous.input.try_send(Input::Close);
        }

        let (input, rx) = mpsc::channel(INPUT_QUEUE);
        let flow = Arc::new(Flow::new());
        let frames = Arc::new(std::sync::Mutex::new(frames));
        let task = tokio::spawn(session(app, params, forward, rx, Arc::clone(&frames), Arc::clone(&flow)));
        self.open.insert(id, Open { input, task, flow, frames });
    }

    pub fn send(&self, id: &str, event: Input) -> Result<(), String> {
        let open = self.open.get(id).ok_or_else(|| format!("no VNC session {id}"))?;
        open.input.try_send(event).map_err(|e| e.to_string())
    }

    pub fn ack(&self, id: &str) -> Result<(), String> {
        self.open
            .get(id)
            .map(|open| open.flow.ack())
            .ok_or_else(|| format!("no VNC session {id}"))
    }

    pub fn disconnect(&mut self, id: &str) -> Result<(), String> {
        let open = self.open.remove(id).ok_or_else(|| format!("no VNC session {id}"))?;
        // The session closes the connection and ends on its own; should it
        // be stuck on the network, aborting it is what frees the socket.
        if open.input.try_send(Input::Close).is_err() {
            open.task.abort();
        }
        Ok(())
    }

    pub fn disconnect_all(&mut self) {
        for (id, open) in self.open.drain() {
            tracing::info!("VNC {id}: closing on exit");
            if open.input.try_send(Input::Close).is_err() {
                open.task.abort();
            }
        }
    }
}

/// Connect, log in, then run the pump until either side ends it.
async fn session(
    app: AppHandle,
    params: VncConnectParams,
    forward: Option<Forward>,
    input: mpsc::Receiver<Input>,
    frames: Arc<std::sync::Mutex<Channel<InvokeResponseBody>>>,
    flow: Arc<Flow>,
) {
    let id = params.id.clone();
    let status = |s: RdpStatus| {
        let _ = app.emit(&format!("vnc-status-{id}"), s);
    };

    let (host, port) = match &forward {
        Some(f) => ("127.0.0.1".to_string(), f.port),
        None => (params.host.clone(), params.port),
    };
    tracing::info!("VNC {id}: connecting to {}:{}{}", params.host, params.port, if forward.is_some() { " through SSH" } else { "" });

    let client = match tokio::time::timeout(CONNECT_TIMEOUT, open(&host, port, params.password.clone())).await {
        Ok(Ok(client)) => client,
        Ok(Err(e)) => {
            tracing::warn!("VNC {id}: connection failed: {e}");
            status(RdpStatus::Error { message: e });
            return;
        }
        Err(_) => {
            tracing::warn!("VNC {id}: connection timed out");
            status(RdpStatus::Error { message: format!("No answer from {}:{} within {} seconds", params.host, params.port, CONNECT_TIMEOUT.as_secs()) });
            return;
        }
    };
    tracing::info!("VNC {id}: connected");
    status(RdpStatus::Connected);

    let reason = Pump { app: &app, id: &id, client: &client, flow: &flow, frames: &frames }.run(input).await;
    let _ = client.close().await;
    tracing::info!("VNC {id}: closed ({reason})");
    status(RdpStatus::Closed { reason });
    // The SSH leg goes last, once nothing is using it.
    drop(forward);
}

/// TCP, the RFB handshake and the login.
async fn open(host: &str, port: u16, password: String) -> Result<VncClient, String> {
    let tcp = TcpStream::connect((host, port)).await.map_err(|e| format!("Could not reach {host}:{port}: {e}"))?;
    // Input is many small messages; Nagle would hold each one back.
    let _ = tcp.set_nodelay(true);
    VncConnector::new(tcp)
        .set_auth_method(std::future::ready(Ok(password)))
        // In order of preference. Tight is the smallest on the wire, ZRLE
        // is lossless; CopyRect makes scrolling and dragging nearly free.
        .add_encoding(VncEncoding::Tight)
        .add_encoding(VncEncoding::Zrle)
        .add_encoding(VncEncoding::Trle)
        .add_encoding(VncEncoding::CopyRect)
        .add_encoding(VncEncoding::Raw)
        // The cursor is drawn by the webview, as for RDP.
        .add_encoding(VncEncoding::CursorPseudo)
        .add_encoding(VncEncoding::DesktopSizePseudo)
        .add_encoding(VncEncoding::ExtendedDesktopSizePseudo)
        .add_encoding(VncEncoding::LastRectPseudo)
        .allow_shared(true)
        // Bytes in R, G, B, X order: what the canvas takes, so a raw update
        // is copied without being converted.
        .set_pixel_format(PixelFormat::rgba())
        .build()
        .map_err(login_error)?
        .try_start()
        .await
        .map_err(login_error)?
        .finish()
        .map_err(login_error)
}

fn login_error(e: vnc::VncError) -> String {
    match e {
        vnc::VncError::WrongPassword => "Wrong VNC password".to_string(),
        vnc::VncError::NoPassword => "This server asks for a VNC password".to_string(),
        // Apple's screen sharing, RealVNC's encryption, VeNCrypt.
        vnc::VncError::General(m) if m.contains("not been implemented") => {
            "This server only offers a login Reach does not support yet (only the VNC password is supported)".to_string()
        }
        other => other.to_string(),
    }
}

struct Pump<'a> {
    app: &'a AppHandle,
    id: &'a str,
    client: &'a VncClient,
    flow: &'a Flow,
    frames: &'a std::sync::Mutex<Channel<InvokeResponseBody>>,
}

impl Pump<'_> {
    /// Runs until the session ends; returns why.
    async fn run(self, mut input: mpsc::Receiver<Input>) -> String {
        let mut screen = Screen::default();
        let mut buttons: u8 = 0;
        // The last text either side put on the clipboard, so a copy is not
        // echoed back to where it came from.
        let mut clip: Option<String> = None;
        let mut pull = tokio::time::interval(PULL_EVERY);
        pull.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            let pending = screen.pending();
            let settled = screen.settled();
            let deadline = screen.deadline().unwrap_or_else(Instant::now);

            // `recv_event` holds the client's lock while it waits; select
            // drops it before any arm runs, so `input` below never waits on
            // it. It is cancel-safe: an mpsc receive underneath.
            tokio::select! {
                biased;
                cmd = input.recv() => {
                    let Some(cmd) = cmd else { return "closed".into() };
                    if let Err(reason) = self.input(cmd, &mut buttons, &mut clip).await {
                        return reason;
                    }
                }
                event = self.client.recv_event() => match event {
                    Ok(event) => {
                        if let Err(reason) = self.absorb(event, &mut screen, &mut clip) {
                            return reason;
                        }
                    }
                    Err(e) => return format!("connection ended: {e}"),
                },
                _ = pull.tick(), if self.flow.can_send() => {
                    if let Err(e) = self.client.input(X11Event::Refresh).await {
                        return format!("connection ended: {e}");
                    }
                }
                _ = self.flow.notify.notified(), if pending && !self.flow.can_send() => {}
                _ = tokio::time::sleep_until(deadline), if pending && !settled => {}
            }

            if self.flow.take_refresh() {
                screen.replaced = true;
                screen.touch();
            }

            if screen.pending() && screen.settled() && self.flow.can_send() {
                self.flow.sent();
                let sent = self
                    .frames
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .send(InvokeResponseBody::Raw(screen.take()));
                if sent.is_err() {
                    return "the panel is gone".into();
                }
            }
        }
    }

    async fn input(&self, cmd: Input, buttons: &mut u8, clip: &mut Option<String>) -> Result<(), String> {
        let event = match cmd {
            Input::Close => return Err("closed".into()),
            Input::Key { keysym, down } => X11Event::KeyEvent((keysym, down).into()),
            Input::Mouse { x, y, action, button, delta } => {
                let bit = 1u8 << button.min(2);
                match action {
                    MouseAction::Move => {}
                    MouseAction::Down => *buttons |= bit,
                    MouseAction::Up => *buttons &= !bit,
                    MouseAction::Wheel => {
                        // A wheel notch is a press and a release of its button.
                        let wheel = if delta > 0 { WHEEL_UP } else { WHEEL_DOWN };
                        self.send(X11Event::PointerEvent((x, y, *buttons | wheel).into())).await?;
                    }
                }
                X11Event::PointerEvent((x, y, *buttons).into())
            }
            Input::Clipboard(text) => {
                let text = latin1(&text);
                if text.is_empty() || clip.as_deref() == Some(text.as_str()) {
                    return Ok(());
                }
                *clip = Some(text.clone());
                X11Event::CopyText(text)
            }
            Input::Resize { width, height } => {
                // Answered by the server as an update the pump absorbs; the
                // request itself waits for that answer, so it runs apart.
                let client = self.client.clone();
                let id = self.id.to_string();
                tokio::spawn(async move {
                    match client.resize_desktop(width, height).await {
                        Ok(layout) => tracing::info!("VNC {id}: desktop resized to {}x{}", layout.width, layout.height),
                        Err(e) => tracing::info!("VNC {id}: resize to {width}x{height} not done: {e}"),
                    }
                });
                return Ok(());
            }
        };
        self.send(event).await
    }

    async fn send(&self, event: X11Event) -> Result<(), String> {
        self.client.input(event).await.map_err(|e| format!("connection ended: {e}"))
    }

    /// Fold one server event into the screen, or act on it.
    fn absorb(&self, event: VncEvent, screen: &mut Screen, clip: &mut Option<String>) -> Result<(), String> {
        match draw(screen, event, self.id) {
            None => {}
            Some(VncEvent::Text(text)) => {
                if clip.as_deref() != Some(text.as_str()) {
                    use tauri_plugin_clipboard_manager::ClipboardExt;
                    if let Err(e) = self.app.clipboard().write_text(text.clone()) {
                        tracing::debug!("VNC {}: could not set the clipboard: {e}", self.id);
                    }
                    *clip = Some(text);
                }
            }
            Some(VncEvent::Error(e)) => return Err(e),
            // The pixel format is ours; a bell has nowhere useful to go.
            Some(_) => {}
        }
        Ok(())
    }
}

/// Apply an event that changes the picture to `screen`. Anything else is
/// handed back.
fn draw(screen: &mut Screen, event: VncEvent, id: &str) -> Option<VncEvent> {
    match event {
        VncEvent::SetResolution(s) => screen.resize(s.width, s.height),
        VncEvent::DesktopUpdate(update) => {
            if let Some(layout) = update.layout {
                screen.resize(layout.width, layout.height);
            }
        }
        VncEvent::RawImage(r, data) => screen.blit_rgba(r.x, r.y, r.width, r.height, &data),
        VncEvent::Copy(dst, src) => screen.copy_rect((dst.x, dst.y), (src.x, src.y), dst.width, dst.height),
        VncEvent::JpegImage(r, data) => match decode_jpeg(&data, r.width, r.height) {
            Ok(pixels) => screen.blit_rgba(r.x, r.y, r.width, r.height, &pixels),
            Err(e) => tracing::debug!("VNC {id}: skipped a JPEG rectangle: {e}"),
        },
        VncEvent::SetCursor(r, data) => {
            if r.width == 0 || r.height == 0 || data.is_empty() {
                screen.set_cursor(Cursor::Hidden);
            } else {
                // The rectangle's corner is the hotspot; the pixels are in
                // the format set at login, with the mask in the alpha.
                screen.set_cursor(Cursor::Shape(Arc::new(DecodedPointer {
                    width: r.width,
                    height: r.height,
                    hotspot_x: r.x,
                    hotspot_y: r.y,
                    bitmap_data: data,
                })));
            }
        }
        other => return Some(other),
    }
    None
}

/// RFB clipboard text is Latin-1, and vnc-rs puts the string's bytes on the
/// wire as they are. ASCII is the part both agree on; anything else would
/// arrive as mojibake, so it is sent as `?`.
fn latin1(text: &str) -> String {
    text.chars()
        .filter(|c| *c != '\r')
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect()
}

/// A Tight JPEG rectangle, as RGBA of exactly `width × height`.
fn decode_jpeg(data: &[u8], width: u16, height: u16) -> Result<Vec<u8>, String> {
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    use zune_jpeg::JpegDecoder;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions().ok_or("no dimensions")?;
    if (w, h) != (usize::from(width), usize::from(height)) {
        return Err(format!("{w}x{h} image for a {width}x{height} rectangle"));
    }
    Ok(pixels)
}

#[cfg(test)]
#[path = "vnc_tests.rs"]
mod tests;
