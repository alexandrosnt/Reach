//! Dragging a remote file out of the window without downloading it first.
//!
//! The shape of this on Windows is not "tell me where it was dropped" — the
//! source is never told. Instead the app offers a *promise*: a data object
//! advertising `CFSTR_FILEDESCRIPTORW` (the name and size) alongside
//! `CFSTR_FILECONTENTS` (an `IStream`). Explorer takes the descriptor during
//! the drag, and only when the user lets go does it ask for the stream and
//! read the bytes, writing them itself wherever the drop landed. Nothing is
//! transferred until then, which is the whole point.
//!
//! This file is a spike. It serves fixed bytes rather than an SFTP download,
//! because the question it exists to answer is the one nobody has published an
//! answer to: whether a drag source like this works at all from a window whose
//! mouse capture belongs to WebView2. If it does, the stream gets replaced by
//! one that pulls from a real transfer. If it does not, no amount of plumbing
//! behind it would have helped.
//!
//! Three COM objects are needed and all three are required — Explorer calls
//! `EnumFormatEtc` during a drag, so returning `E_NOTIMPL` there (as the
//! existing drag crate does) means the promise is never even noticed.

#![cfg(target_os = "windows")]

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::mpsc::Receiver;

use windows::core::{implement, Result as WinResult, BOOL, HRESULT, PWSTR};
use windows::Win32::Foundation::{
    DATA_S_SAMEFORMATETC, DV_E_FORMATETC, E_NOTIMPL, HGLOBAL, OLE_E_ADVISENOTSUPPORTED,
    POINT, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    CoTaskMemAlloc, IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC,
    IEnumFORMATETC_Impl, ISequentialStream_Impl, IStream, IStream_Impl, DATADIR, DATADIR_GET,
    FORMATETC, LOCKTYPE, STATFLAG, STATSTG, STGC, STGMEDIUM, STREAM_SEEK, TYMED_HGLOBAL,
    TYMED_ISTREAM,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GLOBAL_ALLOC_FLAGS};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, OleInitialize, DROPEFFECT, DROPEFFECT_COPY,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{
    IDataObjectAsyncCapability, IDataObjectAsyncCapability_Impl, FD_ATTRIBUTES, FD_FILESIZE,
    FD_PROGRESSUI,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;

const GMEM_MOVEABLE: GLOBAL_ALLOC_FLAGS = GLOBAL_ALLOC_FLAGS(0x0002);
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

/// One file being offered.
///
/// `open` is not called while dragging. It runs when the drop target asks for
/// the contents, which is after the drop — so nothing crosses the network for
/// a drag that is cancelled, or one that never leaves the window.
pub struct PromisedFile {
    pub name: String,
    pub size: u64,
    pub open: Box<dyn Fn() -> Receiver<Vec<u8>> + Send + Sync>,
}

/// An `IStream` fed by a channel, so the drop target's reads pull bytes off
/// the network as it goes rather than after a whole file is buffered.
///
/// Explorer reads in its own rhythm and the channel is bounded, so a slow
/// write applies back-pressure all the way to the SSH channel instead of
/// piling up in memory.
#[implement(IStream)]
struct ChannelStream {
    rx: RefCell<Option<Receiver<Vec<u8>>>>,
    /// Bytes received but not yet handed over, when a read asked for less
    /// than one chunk.
    pending: RefCell<Vec<u8>>,
    size: u64,
    read: RefCell<u64>,
    name: String,
}

impl ISequentialStream_Impl for ChannelStream_Impl {
    fn Read(&self, pv: *mut c_void, cb: u32, pcbread: *mut u32) -> HRESULT {
        let mut written = 0usize;
        let want = cb as usize;
        let out = pv as *mut u8;

        while written < want {
            let mut pending = self.pending.borrow_mut();
            if pending.is_empty() {
                // Only block for more when nothing is left to hand over.
                let next = {
                    let rx = self.rx.borrow();
                    match rx.as_ref() {
                        Some(rx) => rx.recv().ok(),
                        None => None,
                    }
                };
                match next {
                    Some(chunk) => *pending = chunk,
                    // Sender gone: the file ended, or the transfer failed. A
                    // short read is how end-of-stream is reported.
                    None => break,
                }
            }
            let take = (want - written).min(pending.len());
            unsafe { std::ptr::copy_nonoverlapping(pending.as_ptr(), out.add(written), take) };
            pending.drain(..take);
            written += take;
        }

        *self.read.borrow_mut() += written as u64;
        if !pcbread.is_null() {
            unsafe { *pcbread = written as u32 };
        }
        S_OK
    }

    fn Write(&self, _pv: *const c_void, _cb: u32, _written: *mut u32) -> HRESULT {
        E_NOTIMPL
    }
}

impl IStream_Impl for ChannelStream_Impl {
    fn Seek(&self, _move: i64, _origin: STREAM_SEEK, newpos: *mut u64) -> WinResult<()> {
        // A network stream cannot rewind. Explorer tolerates this as long as
        // it is told the current position when it asks for it.
        if !newpos.is_null() {
            unsafe { *newpos = *self.read.borrow() };
            return Ok(());
        }
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn SetSize(&self, _size: u64) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn CopyTo(
        &self,
        _stm: windows::core::Ref<IStream>,
        _cb: u64,
        _read: *mut u64,
        _written: *mut u64,
    ) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn Commit(&self, _flags: &STGC) -> WinResult<()> {
        Ok(())
    }

    fn Revert(&self) -> WinResult<()> {
        Ok(())
    }

    fn LockRegion(&self, _off: u64, _cb: u64, _type: &LOCKTYPE) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn UnlockRegion(&self, _off: u64, _cb: u64, _type: u32) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn Stat(&self, pstatstg: *mut STATSTG, _flag: &STATFLAG) -> WinResult<()> {
        // The size here is what drives Explorer's progress dialog.
        unsafe {
            let mut stat = STATSTG::default();
            stat.cbSize = self.size;
            stat.r#type = 2; // STGTY_STREAM
            let wide: Vec<u16> = self
                .name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let bytes = wide.len() * 2;
            let mem = CoTaskMemAlloc(bytes) as *mut u16;
            std::ptr::copy_nonoverlapping(wide.as_ptr(), mem, wide.len());
            stat.pwcsName = PWSTR(mem);
            *pstatstg = stat;
        }
        Ok(())
    }

    fn Clone(&self) -> WinResult<IStream> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }
}

fn clipboard_format(name: &str) -> u16 {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { RegisterClipboardFormatW(windows::core::PCWSTR(wide.as_ptr())) as u16 }
}

fn descriptor_format() -> u16 {
    clipboard_format("FileGroupDescriptorW")
}

fn contents_format() -> u16 {
    clipboard_format("FileContents")
}

/// Explorer asks what is on offer before it will look for a promise, so this
/// has to be a real enumerator rather than `E_NOTIMPL`.
#[implement(IEnumFORMATETC)]
struct FormatEnumerator {
    formats: Vec<FORMATETC>,
    index: RefCell<usize>,
}

impl FormatEnumerator {
    fn new(formats: Vec<FORMATETC>) -> Self {
        Self {
            formats,
            index: RefCell::new(0),
        }
    }
}

impl IEnumFORMATETC_Impl for FormatEnumerator_Impl {
    fn Next(&self, celt: u32, rgelt: *mut FORMATETC, pceltfetched: *mut u32) -> HRESULT {
        let mut index = self.index.borrow_mut();
        let mut written = 0u32;
        while written < celt && *index < self.formats.len() {
            unsafe { *rgelt.add(written as usize) = self.formats[*index] };
            *index += 1;
            written += 1;
        }
        if !pceltfetched.is_null() {
            unsafe { *pceltfetched = written };
        }
        if written == celt {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn Skip(&self, celt: u32) -> WinResult<()> {
        let mut index = self.index.borrow_mut();
        *index = (*index + celt as usize).min(self.formats.len());
        Ok(())
    }

    fn Reset(&self) -> WinResult<()> {
        *self.index.borrow_mut() = 0;
        Ok(())
    }

    fn Clone(&self) -> WinResult<IEnumFORMATETC> {
        let clone = FormatEnumerator {
            formats: self.formats.clone(),
            index: RefCell::new(*self.index.borrow()),
        };
        Ok(clone.into())
    }
}

/// The promise itself.
///
/// Also declares `IDataObjectAsyncCapability`. Without it the drop target
/// extracts on the thread that called `DoDragDrop` — ours, the one running
/// the window — and a large transfer would freeze the interface until it
/// finished. With it, the target does the work on a thread of its own.
#[implement(IDataObject, IDataObjectAsyncCapability)]
struct PromiseDataObject {
    files: Vec<PromisedFile>,
    async_mode: RefCell<bool>,
    in_operation: RefCell<bool>,
}

impl PromiseDataObject {
    /// The `FILEGROUPDESCRIPTORW` Explorer reads during the drag: names and
    /// sizes only, so it can show what is coming without fetching anything.
    fn descriptor_block(&self) -> WinResult<HGLOBAL> {
        // cItems, then one FILEDESCRIPTORW per file. Laid out by hand because
        // the generated struct carries a single trailing element.
        const DESCRIPTOR_SIZE: usize = 592;
        let total = 4 + DESCRIPTOR_SIZE * self.files.len();
        let hglobal = unsafe { GlobalAlloc(GMEM_MOVEABLE, total)? };
        unsafe {
            let base = GlobalLock(hglobal) as *mut u8;
            std::ptr::write_bytes(base, 0, total);
            *(base as *mut u32) = self.files.len() as u32;

            for (i, file) in self.files.iter().enumerate() {
                let fd = base.add(4 + i * DESCRIPTOR_SIZE);
                // dwFlags: we know the size and the attributes, and want the
                // copy dialog, so Explorer shows progress for a slow stream.
                *(fd as *mut u32) = FD_ATTRIBUTES.0 as u32
                    | FD_FILESIZE.0 as u32
                    | FD_PROGRESSUI.0 as u32;
                // dwFileAttributes sits after dwFlags, CLSID, SIZEL, POINTL.
                *(fd.add(4 + 16 + 8 + 8) as *mut u32) = FILE_ATTRIBUTE_NORMAL;
                // Three FILETIMEs follow, then the size as two u32s.
                let size_off = 4 + 16 + 8 + 8 + 4 + 8 * 3;
                *(fd.add(size_off) as *mut u32) = (file.size >> 32) as u32;
                *(fd.add(size_off + 4) as *mut u32) = (file.size & 0xFFFF_FFFF) as u32;
                // cFileName[MAX_PATH], UTF-16, NUL terminated.
                let name_off = size_off + 8;
                let wide: Vec<u16> = file.name.encode_utf16().take(259).collect();
                std::ptr::copy_nonoverlapping(
                    wide.as_ptr(),
                    fd.add(name_off) as *mut u16,
                    wide.len(),
                );
            }
            let _ = GlobalUnlock(hglobal);
        }
        Ok(hglobal)
    }
}

impl IDataObject_Impl for PromiseDataObject_Impl {
    fn GetData(&self, pformatetc: *const FORMATETC) -> WinResult<STGMEDIUM> {
        let format = unsafe { *pformatetc };
        if format.cfFormat == descriptor_format() {
            return Ok(STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: windows::Win32::System::Com::STGMEDIUM_0 {
                    hGlobal: self.descriptor_block()?,
                },
                pUnkForRelease: std::mem::ManuallyDrop::new(None),
            });
        }
        if format.cfFormat == contents_format() {
            // lindex says *which* file. This is the moment the drop happened;
            // everything before it was just the descriptor.
            let index = if format.lindex < 0 {
                0
            } else {
                format.lindex as usize
            };
            let file = self
                .files
                .get(index)
                .ok_or_else(|| windows::core::Error::from(DV_E_FORMATETC))?;
            // This is the moment the transfer starts. Everything before it was
            // the descriptor, which costs nothing.
            let stream: IStream = ChannelStream {
                rx: RefCell::new(Some((file.open)())),
                pending: RefCell::new(Vec::new()),
                size: file.size,
                read: RefCell::new(0),
                name: file.name.clone(),
            }
            .into();
            return Ok(STGMEDIUM {
                tymed: TYMED_ISTREAM.0 as u32,
                u: windows::Win32::System::Com::STGMEDIUM_0 {
                    pstm: std::mem::ManuallyDrop::new(Some(stream)),
                },
                pUnkForRelease: std::mem::ManuallyDrop::new(None),
            });
        }
        Err(windows::core::Error::from(DV_E_FORMATETC))
    }

    fn GetDataHere(&self, _fmt: *const FORMATETC, _med: *mut STGMEDIUM) -> WinResult<()> {
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        let format = unsafe { *pformatetc };
        if format.cfFormat == descriptor_format() || format.cfFormat == contents_format() {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        _in_fmt: *const FORMATETC,
        out_fmt: *mut FORMATETC,
    ) -> HRESULT {
        unsafe { (*out_fmt).ptd = std::ptr::null_mut() };
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        _fmt: *const FORMATETC,
        _med: *const STGMEDIUM,
        _release: BOOL,
    ) -> WinResult<()> {
        // Explorer reports the performed effect back through here. Nothing is
        // done with it for a copy, but it must not fail.
        Ok(())
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> WinResult<IEnumFORMATETC> {
        if DATADIR(dwdirection as i32) != DATADIR_GET {
            return Err(windows::core::Error::from(E_NOTIMPL));
        }
        let formats = vec![
            FORMATETC {
                cfFormat: descriptor_format(),
                ptd: std::ptr::null_mut(),
                dwAspect: 1,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as u32,
            },
            FORMATETC {
                cfFormat: contents_format(),
                ptd: std::ptr::null_mut(),
                dwAspect: 1,
                lindex: 0,
                tymed: TYMED_ISTREAM.0 as u32,
            },
        ];
        Ok(FormatEnumerator::new(formats).into())
    }

    fn DAdvise(&self, _f: *const FORMATETC, _a: u32, _s: windows::core::Ref<IAdviseSink>) -> WinResult<u32> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }

    fn DUnadvise(&self, _c: u32) -> WinResult<()> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }

    fn EnumDAdvise(&self) -> WinResult<windows::Win32::System::Com::IEnumSTATDATA> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }
}

impl IDataObjectAsyncCapability_Impl for PromiseDataObject_Impl {
    fn SetAsyncMode(&self, fdoopasync: BOOL) -> WinResult<()> {
        *self.async_mode.borrow_mut() = fdoopasync.as_bool();
        Ok(())
    }

    fn GetAsyncMode(&self) -> WinResult<BOOL> {
        Ok(BOOL::from(*self.async_mode.borrow()))
    }

    fn StartOperation(&self, _reserved: windows::core::Ref<windows::Win32::System::Com::IBindCtx>) -> WinResult<()> {
        *self.in_operation.borrow_mut() = true;
        Ok(())
    }

    fn InOperation(&self) -> WinResult<BOOL> {
        Ok(BOOL::from(*self.in_operation.borrow()))
    }

    fn EndOperation(
        &self,
        _hresult: HRESULT,
        _reserved: windows::core::Ref<windows::Win32::System::Com::IBindCtx>,
        _effects: u32,
    ) -> WinResult<()> {
        *self.in_operation.borrow_mut() = false;
        Ok(())
    }
}

/// Ends the drag when the button comes up, and cancels on Escape.
#[implement(IDropSource)]
struct DropSource;

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        const MK_LBUTTON: u32 = 0x0001;
        if fescapepressed.as_bool() {
            windows::Win32::Foundation::DRAGDROP_S_CANCEL
        } else if grfkeystate.0 & MK_LBUTTON == 0 {
            windows::Win32::Foundation::DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        windows::Win32::Foundation::DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// Start a promise drag. Blocks until the drop happens or is cancelled.
///
/// `DoDragDrop` runs its own message loop on this thread, so it must be the
/// thread that owns the window. The data object declares itself async, which
/// asks the drop target to extract on a thread of its own — without that, the
/// stream reads below would run inside this call and freeze the window for
/// the length of the transfer.
pub fn drag_promised_files(files: Vec<PromisedFile>) -> Result<bool, String> {
    unsafe {
        let _ = OleInitialize(None);
        let data: IDataObject = PromiseDataObject {
            files,
            // Asked for up front; the target may still decline it.
            async_mode: RefCell::new(true),
            in_operation: RefCell::new(false),
        }
        .into();
        let source: IDropSource = DropSource.into();
        let mut effect = DROPEFFECT::default();
        let hr = DoDragDrop(&data, &source, DROPEFFECT_COPY, &mut effect);
        Ok(hr == windows::Win32::Foundation::DRAGDROP_S_DROP)
    }
}

/// Cursor position, used only to log where a drag ended for diagnosis.
pub fn cursor_position() -> (i32, i32) {
    let mut point = POINT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
    }
    (point.x, point.y)
}
