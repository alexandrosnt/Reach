//! The clipboard between this machine and a remote desktop.
//!
//! MS-RDPECLIP is a pull protocol. Each side announces what it has when it
//! changes, and the other side asks for the bytes only when someone pastes.
//! IronRDP speaks the protocol; what it leaves to the embedder is the local
//! end — reading and writing this machine's clipboard, and the files — and
//! that is what lives here.
//!
//! Text goes both ways. Files go both ways too: copy files here and paste
//! them into the remote, or copy them there and paste them here. Android has
//! no file clipboard, so it is text only.
//!
//! Every IronRDP callback arrives on the session thread, which is also the
//! thread that decodes the picture, so nothing may block there. Each callback
//! is one message to a worker thread of the session's own, which owns the OS
//! clipboard and the open files and answers through the session's clipboard
//! channel — unbounded and separate from input, so a large transfer can never
//! backpressure the keyboard.
//!
//! Speed: a file going out is read through a large read-ahead buffer that
//! stays open across the server's requests, so each request is a memcpy, not
//! an open-seek-read. A file coming in is asked for in 1 MiB pieces, one in
//! flight, and written as it lands. The wire is the limit either way.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use ironrdp_client::rdp::RdpInputSender;
use ironrdp_cliprdr::backend::{ClipboardMessage, ClipboardMessageProxy, CliprdrBackend, CliprdrBackendFactory};
use ironrdp_cliprdr::pdu::{
    ClipboardFileAttributes, ClipboardFormat, ClipboardFormatId, ClipboardFormatName, ClipboardGeneralCapabilityFlags,
    FileContentsFlags, FileContentsRequest, FileContentsResponse, FileDescriptor, FormatDataRequest, FormatDataResponse,
    LockDataId, OwnedFormatDataResponse,
};
use ironrdp_pdu::ironrdp_core::{impl_as_any, IntoOwned};
use tauri::AppHandle;

/// How much of an outgoing file is read ahead of the server's requests.
const READ_AHEAD: usize = 2 * 1024 * 1024;

/// How much of an incoming file is asked for at a time.
const FETCH_CHUNK: u32 = 1024 * 1024;

/// The most a remote copy may pull down without being asked. The remote side
/// announces file sizes before a byte moves; a copy larger than this is left
/// on the remote clipboard rather than downloaded on the chance of a paste.
const MAX_FETCH_TOTAL: u64 = 2 * 1024 * 1024 * 1024;

/// What the session thread hands the worker. Each is one IronRDP callback,
/// or one request from the app.
enum Cmd {
    Ready,
    Advertise { force: bool },
    FormatData(FormatDataRequest),
    FileContents(FileContentsRequest),
    RemoteCopy(Vec<ClipboardFormat>),
    RemoteData(OwnedFormatDataResponse),
    RemoteFiles(Vec<FileDescriptor>, Option<u32>),
    RemoteFileData(FileContentsResponse<'static>),
    Lock(u32),
    Unlock(u32),
}

/// The app's end: ask the worker to look at the local clipboard again.
/// Dropping it — with the backend and the factory — ends the worker.
pub struct ClipboardHandle {
    tx: Sender<Cmd>,
}

impl ClipboardHandle {
    /// The local clipboard may have changed: offer it to the remote if so.
    /// Called when the desktop gains focus, which is when a paste can follow.
    pub fn sync(&self) {
        let _ = self.tx.send(Cmd::Advertise { force: false });
    }
}

/// Start the worker for one session. Returns the app's handle and the
/// factory IronRDP builds its backend from.
pub fn spawn(id: &str, input: RdpInputSender, app: AppHandle) -> std::io::Result<(ClipboardHandle, Factory)> {
    let temp_dir = std::env::temp_dir().join("reach-rdp-clipboard").join(id);
    if let Err(e) = fs::create_dir_all(&temp_dir) {
        tracing::warn!("RDP {id}: no clipboard temp dir at {}: {e}", temp_dir.display());
    }

    let (tx, rx) = mpsc::channel();
    let worker = Worker {
        id: id.to_string(),
        proxy: Proxy(input),
        local: LocalClipboard::new(app),
        temp_dir: temp_dir.clone(),
        ready: false,
        fingerprint: None,
        offered: Vec::new(),
        locked: HashMap::new(),
        readers: HashMap::new(),
        pending: None,
        fetch: None,
        fetches: 0,
        next_stream: 1,
    };
    std::thread::Builder::new()
        .name(format!("rdp-clip-{id}"))
        .spawn(move || worker.run(rx))?;

    let factory = Factory {
        tx: tx.clone(),
        temp_dir: temp_dir.to_string_lossy().into_owned(),
    };
    Ok((ClipboardHandle { tx }, factory))
}

/// Answers from the worker go to the session through its clipboard channel.
#[derive(Clone, Debug)]
struct Proxy(RdpInputSender);

impl ClipboardMessageProxy for Proxy {
    fn send_clipboard_message(&self, message: ClipboardMessage) {
        if self.0.send_clipboard(message).is_err() {
            tracing::debug!("RDP clipboard: session gone, message dropped");
        }
    }
}

/// IronRDP asks for a backend per channel initialisation; every one it gets
/// talks to the same worker.
pub struct Factory {
    tx: Sender<Cmd>,
    temp_dir: String,
}

impl CliprdrBackendFactory for Factory {
    fn build_cliprdr_backend(&self) -> Box<dyn CliprdrBackend> {
        Box::new(Backend {
            tx: self.tx.clone(),
            temp_dir: self.temp_dir.clone(),
        })
    }
}

/// What IronRDP calls, on the session thread. Every method is a send.
#[derive(Debug)]
struct Backend {
    tx: Sender<Cmd>,
    temp_dir: String,
}

impl_as_any!(Backend);

impl Backend {
    fn send(&self, cmd: Cmd) {
        if self.tx.send(cmd).is_err() {
            tracing::debug!("RDP clipboard: worker gone");
        }
    }
}

impl CliprdrBackend for Backend {
    fn temporary_directory(&self) -> &str {
        &self.temp_dir
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        // Files as streams with no local paths on the wire, locked lists so
        // a copy that follows a copy does not corrupt a paste in flight, and
        // 64-bit offsets so the size of a file is not a limit.
        ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES
            | ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED
            | ClipboardGeneralCapabilityFlags::FILECLIP_NO_FILE_PATHS
            | ClipboardGeneralCapabilityFlags::CAN_LOCK_CLIPDATA
            | ClipboardGeneralCapabilityFlags::HUGE_FILE_SUPPORT_ENABLED
    }

    fn on_ready(&mut self) {
        self.send(Cmd::Ready);
    }

    fn on_request_format_list(&mut self) {
        self.send(Cmd::Advertise { force: true });
    }

    fn on_process_negotiated_capabilities(&mut self, capabilities: ClipboardGeneralCapabilityFlags) {
        tracing::debug!("RDP clipboard: negotiated {capabilities:?}");
    }

    fn on_remote_copy(&mut self, available_formats: &[ClipboardFormat]) {
        self.send(Cmd::RemoteCopy(available_formats.to_vec()));
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        self.send(Cmd::FormatData(request));
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        self.send(Cmd::RemoteData(response.into_owned()));
    }

    fn on_file_contents_request(&mut self, request: FileContentsRequest) {
        self.send(Cmd::FileContents(request));
    }

    fn on_file_contents_response(&mut self, response: FileContentsResponse<'_>) {
        self.send(Cmd::RemoteFileData(response.into_owned()));
    }

    fn on_lock(&mut self, data_id: LockDataId) {
        self.send(Cmd::Lock(data_id.0));
    }

    fn on_unlock(&mut self, data_id: LockDataId) {
        self.send(Cmd::Unlock(data_id.0));
    }

    fn on_remote_file_list(&mut self, files: &[FileDescriptor], clip_data_id: Option<u32>) {
        self.send(Cmd::RemoteFiles(files.to_vec(), clip_data_id));
    }
}

/// What is on the local clipboard, as far as the remote is concerned.
enum Local {
    Files(Vec<PathBuf>),
    Text(String),
    Empty,
}

/// Enough to know whether the clipboard changed since it was last offered,
/// without keeping the content.
#[derive(PartialEq, Eq)]
enum Fingerprint {
    Files(Vec<PathBuf>),
    Text(u64),
    Empty,
}

impl Fingerprint {
    fn of(local: &Local) -> Self {
        match local {
            Local::Files(paths) => Fingerprint::Files(paths.clone()),
            Local::Text(text) => {
                let mut h = DefaultHasher::new();
                text.hash(&mut h);
                Fingerprint::Text(h.finish())
            }
            Local::Empty => Fingerprint::Empty,
        }
    }
}

/// What was asked of the remote and not yet answered. The protocol allows
/// one outstanding request, and the answer does not say what it answers.
#[derive(Clone, Copy)]
enum Pending {
    Text,
    Files,
}

/// An outgoing file the server is reading, kept open between its requests.
struct Reader {
    file: BufReader<File>,
    pos: u64,
}

/// An incoming copy, written into the temp dir as it lands, one file at a
/// time, one request in flight.
struct Fetch {
    lock: Option<u32>,
    root: PathBuf,
    files: Vec<FileDescriptor>,
    /// Index into `files` of the one being received.
    index: usize,
    out: Option<BufWriter<File>>,
    offset: u64,
    size: u64,
    stream: u32,
}

/// The local clipboard, owned by the worker thread.
#[cfg(not(target_os = "android"))]
struct LocalClipboard {
    inner: Option<arboard::Clipboard>,
}

#[cfg(not(target_os = "android"))]
impl LocalClipboard {
    fn new(_app: AppHandle) -> Self {
        Self { inner: None }
    }

    /// Opened on first use, on this thread, which is what the OS wants.
    fn get(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.inner.is_none() {
            match arboard::Clipboard::new() {
                Ok(cb) => self.inner = Some(cb),
                Err(e) => tracing::warn!("RDP clipboard: no local clipboard: {e}"),
            }
        }
        self.inner.as_mut()
    }

    fn read(&mut self) -> Local {
        let Some(cb) = self.get() else { return Local::Empty };
        if let Ok(paths) = cb.get().file_list() {
            if !paths.is_empty() {
                return Local::Files(paths);
            }
        }
        match cb.get().text() {
            Ok(text) if !text.is_empty() => Local::Text(text),
            _ => Local::Empty,
        }
    }

    fn write_text(&mut self, text: &str) {
        if let Some(cb) = self.get() {
            if let Err(e) = cb.set().text(text) {
                tracing::warn!("RDP clipboard: could not set local text: {e}");
            }
        }
    }

    fn write_files(&mut self, paths: &[PathBuf]) {
        if let Some(cb) = self.get() {
            if let Err(e) = cb.set().file_list(paths) {
                tracing::warn!("RDP clipboard: could not set local files: {e}");
            }
        }
    }
}

/// Android: text through the clipboard plugin, no files.
#[cfg(target_os = "android")]
struct LocalClipboard {
    app: AppHandle,
}

#[cfg(target_os = "android")]
impl LocalClipboard {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn read(&mut self) -> Local {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        match self.app.clipboard().read_text() {
            Ok(text) if !text.is_empty() => Local::Text(text),
            _ => Local::Empty,
        }
    }

    fn write_text(&mut self, text: &str) {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if let Err(e) = self.app.clipboard().write_text(text) {
            tracing::warn!("RDP clipboard: could not set local text: {e}");
        }
    }

    fn write_files(&mut self, _paths: &[PathBuf]) {}
}

/// The session's clipboard thread. See the module docs for why it exists.
struct Worker {
    id: String,
    proxy: Proxy,
    local: LocalClipboard,
    temp_dir: PathBuf,
    /// The channel is up and capabilities are negotiated; files may be offered.
    ready: bool,
    /// What was last offered to the remote, so the same content is not
    /// offered twice and what came *from* the remote is not offered back.
    fingerprint: Option<Fingerprint>,
    /// The files behind the last offer, by the index the server uses.
    offered: Vec<PathBuf>,
    /// Offers the server locked, by its lock id: a request that names one
    /// must read from that list even after a newer offer replaced it.
    locked: HashMap<u32, Vec<PathBuf>>,
    /// Open outgoing files, by (lock, index).
    readers: HashMap<(Option<u32>, i32), Reader>,
    pending: Option<Pending>,
    fetch: Option<Fetch>,
    /// Incoming copies so far, to give each its own directory.
    fetches: u32,
    next_stream: u32,
}

impl Worker {
    fn run(mut self, rx: Receiver<Cmd>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Cmd::Ready => {
                    self.ready = true;
                    self.advertise(true);
                }
                Cmd::Advertise { force } => self.advertise(force),
                Cmd::FormatData(request) => self.format_data(request),
                Cmd::FileContents(request) => self.file_contents(request),
                Cmd::RemoteCopy(formats) => self.remote_copy(&formats),
                Cmd::RemoteData(response) => self.remote_data(response),
                Cmd::RemoteFiles(files, lock) => self.remote_files(files, lock),
                Cmd::RemoteFileData(response) => self.remote_file_data(response),
                Cmd::Lock(id) => {
                    self.locked.insert(id, self.offered.clone());
                }
                Cmd::Unlock(id) => {
                    self.locked.remove(&id);
                    self.readers.retain(|(lock, _), _| *lock != Some(id));
                }
            }
        }
        // Every sender is gone: the session ended. What the remote copied
        // here lived in the temp dir, and the session it belonged to is over.
        if let Err(e) = fs::remove_dir_all(&self.temp_dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!("RDP {}: clipboard temp dir left behind: {e}", self.id);
            }
        }
    }

    fn send(&self, message: ClipboardMessage) {
        self.proxy.send_clipboard_message(message);
    }

    fn stream_id(&mut self) -> u32 {
        let id = self.next_stream;
        self.next_stream = self.next_stream.wrapping_add(1).max(1);
        id
    }

    /// Tell the remote what this machine's clipboard holds, if that changed.
    fn advertise(&mut self, force: bool) {
        let local = self.local.read();
        let fingerprint = Fingerprint::of(&local);
        if !force && self.fingerprint.as_ref() == Some(&fingerprint) {
            return;
        }

        match local {
            // Files need the channel ready; before that, offer nothing and
            // the `Ready` that follows offers them properly.
            Local::Files(paths) if self.ready => {
                let (descriptors, files) = describe(&paths);
                // A new offer replaces the unlocked readers of the old one.
                self.readers.retain(|(lock, _), _| lock.is_some());
                self.offered = files;
                if descriptors.is_empty() {
                    self.send(ClipboardMessage::SendInitiateCopy(Vec::new()));
                } else {
                    tracing::info!("RDP {}: offering {} file(s) to the remote", self.id, descriptors.len());
                    self.send(ClipboardMessage::SendInitiateFileCopy(descriptors));
                }
            }
            Local::Files(_) => self.send(ClipboardMessage::SendInitiateCopy(Vec::new())),
            Local::Text(_) => self.send(ClipboardMessage::SendInitiateCopy(vec![ClipboardFormat::new(
                ClipboardFormatId::CF_UNICODETEXT,
            )])),
            Local::Empty => self.send(ClipboardMessage::SendInitiateCopy(Vec::new())),
        }
        self.fingerprint = Some(fingerprint);
    }

    /// The remote is pasting our text.
    fn format_data(&mut self, request: FormatDataRequest) {
        let response = match (request.format, self.local.read()) {
            (ClipboardFormatId::CF_UNICODETEXT, Local::Text(text)) => {
                FormatDataResponse::new_unicode_string(&text).into_owned()
            }
            _ => FormatDataResponse::new_error(),
        };
        self.send(ClipboardMessage::SendFormatData(response));
    }

    /// The remote is pasting our files: a size, or a range of one of them.
    fn file_contents(&mut self, request: FileContentsRequest) {
        let stream = request.stream_id;
        let response = match self.read_file_contents(&request) {
            Ok(response) => response,
            Err(e) => {
                tracing::warn!("RDP {}: file contents request {request:?} failed: {e}", self.id);
                FileContentsResponse::new_error(stream)
            }
        };
        self.send(ClipboardMessage::SendFileContentsResponse(response));
    }

    fn read_file_contents(&mut self, request: &FileContentsRequest) -> std::io::Result<FileContentsResponse<'static>> {
        let list = match request.data_id {
            Some(lock) => self.locked.get(&lock),
            None => Some(&self.offered),
        };
        let path = usize::try_from(request.index)
            .ok()
            .and_then(|i| list.and_then(|l| l.get(i)))
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such offered file"))?;

        if request.flags.contains(FileContentsFlags::SIZE) {
            let size = fs::metadata(&path)?.len();
            return Ok(FileContentsResponse::new_size_response(request.stream_id, size));
        }

        let reader = match self.readers.entry((request.data_id, request.index)) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(Reader {
                file: BufReader::with_capacity(READ_AHEAD, File::open(&path)?),
                pos: 0,
            }),
        };
        if reader.pos != request.position {
            reader.file.seek(SeekFrom::Start(request.position))?;
            reader.pos = request.position;
        }

        let mut data = vec![0u8; request.requested_size as usize];
        let mut filled = 0;
        while filled < data.len() {
            let n = reader.file.read(&mut data[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        data.truncate(filled);
        reader.pos += filled as u64;
        Ok(FileContentsResponse::new_data_response(request.stream_id, data))
    }

    /// The remote copied something. Ask for it now: files by their list
    /// (IronRDP parses it and calls back with the descriptors), else text.
    fn remote_copy(&mut self, formats: &[ClipboardFormat]) {
        let files = formats
            .iter()
            .find(|f| f.name.as_ref().is_some_and(|n| n.value() == ClipboardFormatName::FILE_LIST.value()))
            .map(|f| f.id());
        let text = formats.iter().any(|f| f.id() == ClipboardFormatId::CF_UNICODETEXT);

        // A copy on the remote supersedes a transfer from the previous one.
        self.abandon_fetch();

        if let Some(id) = files {
            self.pending = Some(Pending::Files);
            self.send(ClipboardMessage::SendInitiatePaste(id));
        } else if text {
            self.pending = Some(Pending::Text);
            self.send(ClipboardMessage::SendInitiatePaste(ClipboardFormatId::CF_UNICODETEXT));
        } else {
            self.pending = None;
        }
    }

    /// The remote's text arrived: it becomes the local clipboard.
    fn remote_data(&mut self, response: OwnedFormatDataResponse) {
        let pending = self.pending.take();
        if response.is_error() {
            return;
        }
        if let Some(Pending::Text) = pending {
            match response.to_unicode_string() {
                Ok(text) => {
                    self.local.write_text(&text);
                    // Ours now, but not to be offered back where it came from.
                    self.fingerprint = Some(Fingerprint::of(&Local::Text(text)));
                }
                Err(e) => tracing::debug!("RDP {}: remote text unreadable: {e}", self.id),
            }
        }
    }

    /// The remote's file list arrived: fetch the lot into the temp dir, then
    /// put the paths on the local clipboard.
    fn remote_files(&mut self, files: Vec<FileDescriptor>, lock: Option<u32>) {
        self.pending = None;
        let total: u64 = files.iter().filter_map(|f| f.file_size).sum();
        if total > MAX_FETCH_TOTAL {
            tracing::info!("RDP {}: remote copied {total} bytes of files; too large to fetch unasked", self.id);
            return;
        }
        self.fetches += 1;
        let root = self.temp_dir.join(self.fetches.to_string());
        if let Err(e) = fs::create_dir_all(&root) {
            tracing::warn!("RDP {}: cannot receive files: {e}", self.id);
            return;
        }
        tracing::info!("RDP {}: receiving {} file(s), {total} bytes", self.id, files.len());
        self.fetch = Some(Fetch {
            lock,
            root,
            files,
            index: 0,
            out: None,
            offset: 0,
            size: 0,
            stream: 0,
        });
        self.fetch_step();
    }

    /// Open the next file to receive and ask for its first piece, or finish.
    fn fetch_step(&mut self) {
        loop {
            let Some(fetch) = self.fetch.as_mut() else { return };
            if fetch.out.is_some() {
                break;
            }
            let Some(desc) = fetch.files.get(fetch.index) else {
                self.finish_fetch();
                return;
            };
            let target = local_path(&fetch.root, desc);
            let is_dir = desc
                .attributes
                .is_some_and(|a| a.contains(ClipboardFileAttributes::DIRECTORY));
            let opened = if is_dir {
                fs::create_dir_all(&target).map(|_| None)
            } else {
                target
                    .parent()
                    .map(fs::create_dir_all)
                    .unwrap_or(Ok(()))
                    .and_then(|_| File::create(&target))
                    .map(|f| Some(BufWriter::with_capacity(READ_AHEAD, f)))
            };
            match opened {
                Ok(None) => fetch.index += 1,
                Ok(Some(out)) => {
                    fetch.out = Some(out);
                    fetch.offset = 0;
                    fetch.size = desc.file_size.unwrap_or(0);
                    if fetch.size == 0 {
                        // Empty, or of unknown size: ask and see.
                        fetch.out = None;
                        fetch.index += 1;
                        continue;
                    }
                    break;
                }
                Err(e) => {
                    tracing::warn!("RDP {}: cannot write {}: {e}", self.id, target.display());
                    self.abandon_fetch();
                    return;
                }
            }
        }
        self.fetch_request();
    }

    fn fetch_request(&mut self) {
        let stream = self.stream_id();
        let Some(fetch) = self.fetch.as_mut() else { return };
        fetch.stream = stream;
        let remaining = fetch.size - fetch.offset;
        let request = FileContentsRequest {
            stream_id: stream,
            index: fetch.index as i32,
            flags: FileContentsFlags::RANGE,
            position: fetch.offset,
            requested_size: u32::try_from(remaining).unwrap_or(FETCH_CHUNK).min(FETCH_CHUNK),
            data_id: fetch.lock,
        };
        self.send(ClipboardMessage::SendFileContentsRequest(request));
    }

    /// A piece of an incoming file.
    fn remote_file_data(&mut self, response: FileContentsResponse<'static>) {
        let Some(fetch) = self.fetch.as_mut() else { return };
        if response.stream_id() != fetch.stream {
            return;
        }
        let data = response.data();
        if response.is_error() || data.is_empty() {
            tracing::warn!("RDP {}: remote stopped sending file {}", self.id, fetch.index);
            self.abandon_fetch();
            return;
        }
        let Some(out) = fetch.out.as_mut() else { return };
        if let Err(e) = out.write_all(data) {
            tracing::warn!("RDP {}: cannot write received file: {e}", self.id);
            self.abandon_fetch();
            return;
        }
        fetch.offset += data.len() as u64;
        if fetch.offset >= fetch.size {
            let done = fetch.out.take();
            if let Some(mut out) = done {
                let _ = out.flush();
            }
            fetch.index += 1;
            self.fetch_step();
        } else {
            self.fetch_request();
        }
    }

    fn finish_fetch(&mut self) {
        let Some(fetch) = self.fetch.take() else { return };
        let paths: Vec<PathBuf> = fetch
            .files
            .iter()
            .filter(|f| f.relative_path.as_deref().is_none_or(str::is_empty))
            .map(|f| fetch.root.join(&f.name))
            .collect();
        tracing::info!("RDP {}: received {} item(s) into {}", self.id, paths.len(), fetch.root.display());
        self.local.write_files(&paths);
        self.fingerprint = Some(Fingerprint::Files(paths));
    }

    fn abandon_fetch(&mut self) {
        if let Some(fetch) = self.fetch.take() {
            drop(fetch.out);
            let _ = fs::remove_dir_all(&fetch.root);
        }
    }
}

/// Where a received descriptor lands under `root`. IronRDP has already
/// rejected names that escape; this only turns the wire's `\` into a path.
fn local_path(root: &Path, desc: &FileDescriptor) -> PathBuf {
    let mut path = root.to_path_buf();
    if let Some(rel) = desc.relative_path.as_deref().filter(|r| !r.is_empty()) {
        for part in rel.split('\\').filter(|p| !p.is_empty()) {
            path.push(part);
        }
    }
    path.push(&desc.name);
    path
}

/// Describe local paths for the remote, directories walked, and the local
/// path behind each descriptor in the same order — that order is the index
/// the server uses when it asks for contents.
fn describe(paths: &[PathBuf]) -> (Vec<FileDescriptor>, Vec<PathBuf>) {
    let mut descriptors = Vec::new();
    let mut files = Vec::new();
    for path in paths {
        walk(path, None, &mut descriptors, &mut files);
    }
    (descriptors, files)
}

fn walk(path: &Path, relative: Option<&str>, descriptors: &mut Vec<FileDescriptor>, files: &mut Vec<PathBuf>) {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return };
    let Ok(meta) = fs::metadata(path) else { return };

    let mut desc = FileDescriptor::new(name).with_last_write_time(filetime(meta.modified().ok()));
    if let Some(rel) = relative {
        desc = desc.with_relative_path(rel);
    }

    if meta.is_dir() {
        descriptors.push(desc.with_attributes(ClipboardFileAttributes::DIRECTORY).with_file_size(0));
        files.push(path.to_path_buf());
        let below = match relative {
            Some(rel) => format!("{rel}\\{name}"),
            None => name.to_string(),
        };
        if let Ok(entries) = fs::read_dir(path) {
            let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            entries.sort();
            for entry in entries {
                walk(&entry, Some(&below), descriptors, files);
            }
        }
    } else {
        descriptors.push(
            desc.with_attributes(ClipboardFileAttributes::NORMAL)
                .with_file_size(meta.len()),
        );
        files.push(path.to_path_buf());
    }
}

/// Windows FILETIME: hundreds of nanoseconds since 1601.
fn filetime(time: Option<SystemTime>) -> u64 {
    const EPOCH_GAP_SECS: u64 = 11_644_473_600;
    let since_unix = time
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    (since_unix.as_secs() + EPOCH_GAP_SECS) * 10_000_000 + u64::from(since_unix.subsec_nanos() / 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_described_with_its_contents_in_index_order() {
        let dir = std::env::temp_dir().join(format!("reach-clip-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), b"hello").unwrap();
        fs::write(dir.join("sub").join("b.bin"), [0u8; 3]).unwrap();

        let (descriptors, files) = describe(&[dir.clone()]);
        assert_eq!(descriptors.len(), files.len());
        let names: Vec<(String, Option<String>, Option<u64>)> = descriptors
            .iter()
            .map(|d| (d.name.clone(), d.relative_path.clone(), d.file_size))
            .collect();
        let top = dir.file_name().unwrap().to_str().unwrap().to_string();
        assert_eq!(names[0], (top.clone(), None, Some(0)));
        assert_eq!(names[1], ("a.txt".into(), Some(top.clone()), Some(5)));
        assert_eq!(names[2], ("sub".into(), Some(top.clone()), Some(0)));
        assert_eq!(names[3], ("b.bin".into(), Some(format!("{top}\\sub")), Some(3)));
        assert!(descriptors[0].attributes.unwrap().contains(ClipboardFileAttributes::DIRECTORY));
        assert_eq!(files[3], dir.join("sub").join("b.bin"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn received_paths_follow_the_wire_separator() {
        let d = FileDescriptor::new("b.bin").with_relative_path("top\\sub");
        assert_eq!(local_path(Path::new("/r"), &d), Path::new("/r").join("top").join("sub").join("b.bin"));
    }

    #[test]
    fn filetime_of_the_unix_epoch_is_the_documented_constant() {
        assert_eq!(filetime(Some(UNIX_EPOCH)), 116_444_736_000_000_000);
        assert_eq!(filetime(None), 116_444_736_000_000_000);
    }
}