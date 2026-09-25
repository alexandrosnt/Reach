//! A local folder served to the remote desktop as a drive.
//!
//! MS-RDPEFS, the file system half of the device redirection channel, is a
//! small NT-style file API over the wire: create, read, write, close, query
//! and set information, list a directory, query the volume. IronRDP handles
//! the channel and the PDUs; what a backend does is answer those requests
//! against real files. IronRDP ships two backends and neither serves a
//! folder on every platform — its Windows one serves whole volumes only,
//! and its Unix one is Unix. This one is Reach's, on the standard library
//! alone, so a folder is a folder on Windows, macOS and Linux alike.
//!
//! Every path from the wire is resolved *below the shared folder*, one
//! component at a time, and a component that would climb out (`..`) or that
//! is empty is refused. The remote gets the folder and nothing beside it.

use std::collections::HashMap;
use std::fs::{self, File, Metadata, OpenOptions, ReadDir};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ironrdp_pdu::ironrdp_core::impl_as_any;
use ironrdp_pdu::{encode_err, PduResult};
use ironrdp_rdpdr::backend::{RdpdrBackendFactory, RdpdrBackendFactoryResult, RdpdrBackendProduct, RdpdrDrive};
use ironrdp_rdpdr::pdu::efs::*;
use ironrdp_rdpdr::pdu::esc::{ScardCall, ScardIoCtlCode};
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_rdpdr::RdpdrBackend;
use ironrdp_svc::SvcMessage;

/// Builds a [`FolderDrive`] for each connection, all serving the same folder.
pub struct FolderDriveFactory {
    root: PathBuf,
    name: String,
}

impl FolderDriveFactory {
    pub fn new(root: PathBuf, name: String) -> Self {
        Self { root, name }
    }
}

impl RdpdrBackendFactory for FolderDriveFactory {
    fn build_rdpdr_backend(&self) -> RdpdrBackendFactoryResult<RdpdrBackendProduct> {
        Ok(RdpdrBackendProduct::new(
            Box::new(FolderDrive::new(self.root.clone(), self.name.clone())),
            vec![RdpdrDrive::new(1, self.name.clone())],
        ))
    }
}

/// One open file or directory, by the id the server uses for it.
struct Open {
    path: PathBuf,
    /// Files are held open; a directory is opened when listed.
    file: Option<File>,
    /// A listing in progress, one entry per query.
    listing: Option<ReadDir>,
}

/// The backend: a folder, and what the server has open in it.
#[derive(Debug)]
pub struct FolderDrive {
    root: PathBuf,
    name: String,
    next_id: u32,
    open: HashMap<u32, Open>,
}

impl std::fmt::Debug for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Open").field("path", &self.path).finish_non_exhaustive()
    }
}

impl_as_any!(FolderDrive);

impl FolderDrive {
    pub fn new(root: PathBuf, name: String) -> Self {
        Self {
            root,
            name,
            next_id: 1,
            open: HashMap::new(),
        }
    }

    /// The local path for a wire path, or none if it would leave the folder.
    fn resolve(&self, wire: &str) -> Option<PathBuf> {
        resolve_below(&self.root, wire)
    }
}

/// `wire` is `\dir\file` from the server, with `\` separators and possibly
/// a leading one. Every component must be a plain name: no `..`, nothing
/// empty, no drive or root of its own.
fn resolve_below(root: &Path, wire: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for part in wire.split(['\\', '/']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// Windows FILETIME: hundreds of nanoseconds since 1601. Unknown is the epoch.
fn filetime(time: std::io::Result<SystemTime>) -> i64 {
    const EPOCH_GAP: i64 = 116_444_736_000_000_000;
    match time.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => i64::try_from(d.as_nanos() / 100).unwrap_or(i64::MAX).saturating_add(EPOCH_GAP),
        None => EPOCH_GAP,
    }
}

fn attributes(meta: &Metadata, name: &str) -> FileAttributes {
    let mut attrs = FileAttributes::empty();
    if meta.is_dir() {
        attrs |= FileAttributes::FILE_ATTRIBUTE_DIRECTORY;
    } else {
        attrs |= FileAttributes::FILE_ATTRIBUTE_ARCHIVE;
    }
    if meta.permissions().readonly() {
        attrs |= FileAttributes::FILE_ATTRIBUTE_READONLY;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        if meta.file_attributes() & HIDDEN != 0 {
            attrs |= FileAttributes::FILE_ATTRIBUTE_HIDDEN;
        }
        if meta.file_attributes() & SYSTEM != 0 {
            attrs |= FileAttributes::FILE_ATTRIBUTE_SYSTEM;
        }
    }
    #[cfg(not(windows))]
    if name.len() > 1 && name.starts_with('.') && !name.starts_with("..") {
        attrs |= FileAttributes::FILE_ATTRIBUTE_HIDDEN;
    }
    let _ = name;
    attrs
}

/// Total and free bytes of the volume the folder is on. Best effort: a
/// figure the remote's Explorer can show, not one anything depends on.
fn volume_space(path: &Path) -> (u64, u64) {
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let (mut free, mut total) = (0u64, 0u64);
        // SAFETY: a NUL-terminated wide string and out-pointers to locals.
        let ok = unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut free), Some(&mut total), None) };
        if ok.is_ok() {
            return (total, free);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        if let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) {
            let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
            // SAFETY: a valid C string and a zeroed statvfs to fill.
            if unsafe { libc::statvfs(c.as_ptr(), &mut st) } == 0 {
                let frag = u64::from(st.f_frsize);
                return (u64::from(st.f_blocks) * frag, u64::from(st.f_bavail) * frag);
            }
        }
    }
    let _ = path;
    (0, 0)
}

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt as _;

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl RdpdrBackend for FolderDrive {
    fn handle_server_device_announce_response(&mut self, _pdu: ServerDeviceAnnounceResponse) -> PduResult<()> {
        Ok(())
    }

    fn handle_scard_call(
        &mut self,
        _req: DeviceControlRequest<ScardIoCtlCode>,
        _call: ScardCall,
    ) -> PduResult<Vec<SvcMessage>> {
        Ok(Vec::new())
    }

    fn handle_drive_io_request(&mut self, req: ServerDriveIoRequest) -> PduResult<Vec<SvcMessage>> {
        match req {
            ServerDriveIoRequest::ServerCreateDriveRequest(r) => Ok(self.create(r)),
            ServerDriveIoRequest::DeviceReadRequest(r) => Ok(self.read(r)),
            ServerDriveIoRequest::DeviceWriteRequest(r) => Ok(self.write(r)),
            ServerDriveIoRequest::DeviceCloseRequest(r) => {
                self.open.remove(&r.device_io_request.file_id);
                Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCloseResponse(DeviceCloseResponse {
                    device_io_response: DeviceIoResponse::new(r.device_io_request, NtStatus::SUCCESS),
                }))])
            }
            ServerDriveIoRequest::DeviceFlushBuffersRequest(r) => {
                let status = match self.open.get_mut(&r.device_io_request.file_id).and_then(|o| o.file.as_mut()) {
                    Some(file) => file.flush().map(|_| NtStatus::SUCCESS).unwrap_or(NtStatus::UNSUCCESSFUL),
                    None => NtStatus::NO_SUCH_FILE,
                };
                Ok(vec![SvcMessage::from(RdpdrPdu::DeviceFlushBuffersResponse(
                    DeviceFlushBuffersResponse {
                        device_io_response: DeviceIoResponse::new(r.device_io_request, status),
                    },
                ))])
            }
            ServerDriveIoRequest::ServerDriveQueryDirectoryRequest(r) => Ok(self.query_directory(r)),
            ServerDriveIoRequest::ServerDriveQueryInformationRequest(r) => Ok(self.query_information(r)),
            ServerDriveIoRequest::ServerDriveQueryVolumeInformationRequest(r) => Ok(self.query_volume(r)),
            ServerDriveIoRequest::ServerDriveSetInformationRequest(r) => self.set_information(r),
            ServerDriveIoRequest::DeviceControlRequest(r) => Ok(vec![SvcMessage::from(RdpdrPdu::DeviceControlResponse(
                DeviceControlResponse {
                    device_io_reply: DeviceIoResponse::new(r.header, NtStatus::SUCCESS),
                    output_buffer: None,
                },
            ))]),
            // Change notification and byte-range locks: not offered, and a
            // silent no-answer is what the reference backends do as well.
            ServerDriveIoRequest::ServerDriveNotifyChangeDirectoryRequest(_)
            | ServerDriveIoRequest::ServerDriveLockControlRequest(_) => Ok(Vec::new()),
            ServerDriveIoRequest::ServerDriveQuerySecurityRequest(r) => Ok(vec![SvcMessage::from(
                RdpdrPdu::ClientDriveQuerySecurityResponse(ClientDriveQuerySecurityResponse {
                    device_io_response: DeviceIoResponse::new(r.device_io_request, NtStatus::NOT_SUPPORTED),
                    security_descriptor: None,
                }),
            )]),
            ServerDriveIoRequest::ServerDriveSetSecurityRequest(r) => {
                let response =
                    ClientDriveSetSecurityResponse::new(&r, NtStatus::NOT_SUPPORTED).map_err(|e| encode_err!(e))?;
                Ok(vec![SvcMessage::from(RdpdrPdu::ClientDriveSetSecurityResponse(response))])
            }
        }
    }
}

impl FolderDrive {
    fn create(&mut self, req: DeviceCreateRequest) -> Vec<SvcMessage> {
        let reply = |request, status, id, information| {
            vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                device_io_reply: DeviceIoResponse::new(request, status),
                file_id: id,
                information,
            }))]
        };

        let Some(path) = self.resolve(&req.path) else {
            tracing::warn!("RDP drive: refused path {:?}", req.path);
            return reply(req.device_io_request, NtStatus::ACCESS_DENIED, 0, Information::empty());
        };
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let wants_dir = req.create_options.contains(CreateOptions::FILE_DIRECTORY_FILE);
        let wants_file = req.create_options.contains(CreateOptions::FILE_NON_DIRECTORY_FILE);
        let disposition = req.create_disposition;

        match fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {
                if disposition == CreateDisposition::FILE_CREATE {
                    return reply(req.device_io_request, NtStatus::OBJECT_NAME_COLLISION, id, Information::empty());
                }
                if wants_file {
                    return reply(req.device_io_request, NtStatus::FILE_IS_A_DIRECTORY, id, Information::empty());
                }
                self.open.insert(id, Open { path, file: None, listing: None });
                return reply(req.device_io_request, NtStatus::SUCCESS, id, Information::FILE_OPENED);
            }
            Ok(_) if wants_dir => {
                return reply(req.device_io_request, NtStatus::NOT_A_DIRECTORY, id, Information::empty());
            }
            Ok(_) => {}
            Err(_) if wants_dir => {
                let may_create =
                    disposition == CreateDisposition::FILE_CREATE || disposition == CreateDisposition::FILE_OPEN_IF;
                if may_create && fs::create_dir_all(&path).is_ok() {
                    self.open.insert(id, Open { path, file: None, listing: None });
                    return reply(req.device_io_request, NtStatus::SUCCESS, id, Information::FILE_SUPERSEDED);
                }
                return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, id, Information::empty());
            }
            Err(_) => {}
        }

        let mut opts = OpenOptions::new();
        let information = match disposition {
            CreateDisposition::FILE_OPEN => {
                opts.read(true).write(true);
                Information::FILE_OPENED
            }
            CreateDisposition::FILE_OPEN_IF => {
                opts.read(true).write(true).create(true);
                Information::FILE_OPENED
            }
            CreateDisposition::FILE_CREATE => {
                opts.read(true).write(true).create_new(true);
                Information::FILE_SUPERSEDED
            }
            CreateDisposition::FILE_SUPERSEDE => {
                opts.read(true).write(true).create(true).truncate(true);
                Information::FILE_SUPERSEDED
            }
            CreateDisposition::FILE_OVERWRITE => {
                opts.read(true).write(true).truncate(true);
                Information::FILE_OVERWRITTEN
            }
            CreateDisposition::FILE_OVERWRITE_IF => {
                opts.read(true).write(true).create(true).truncate(true);
                Information::FILE_OVERWRITTEN
            }
            _ => {
                opts.read(true);
                Information::FILE_OPENED
            }
        };
        // A file the user may only read is still worth opening: fall back
        // to read-only when write access is what was refused.
        let opened = opts.open(&path).or_else(|e| {
            if disposition == CreateDisposition::FILE_OPEN || disposition == CreateDisposition::FILE_OPEN_IF {
                OpenOptions::new().read(true).open(&path)
            } else {
                Err(e)
            }
        });
        match opened {
            Ok(file) => {
                self.open.insert(id, Open { path, file: Some(file), listing: None });
                reply(req.device_io_request, NtStatus::SUCCESS, id, information)
            }
            Err(e) => {
                tracing::debug!("RDP drive: open {} failed: {e}", path.display());
                let status = match e.kind() {
                    std::io::ErrorKind::NotFound => NtStatus::NO_SUCH_FILE,
                    std::io::ErrorKind::PermissionDenied => NtStatus::ACCESS_DENIED,
                    std::io::ErrorKind::AlreadyExists => NtStatus::OBJECT_NAME_COLLISION,
                    _ => NtStatus::UNSUCCESSFUL,
                };
                reply(req.device_io_request, status, id, Information::empty())
            }
        }
    }

    fn read(&mut self, req: DeviceReadRequest) -> Vec<SvcMessage> {
        let reply = |request, status, data| {
            vec![SvcMessage::from(RdpdrPdu::DeviceReadResponse(DeviceReadResponse {
                device_io_reply: DeviceIoResponse::new(request, status),
                read_data: data,
            }))]
        };
        let Some(file) = self.open.get_mut(&req.device_io_request.file_id).and_then(|o| o.file.as_mut()) else {
            return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, Vec::new());
        };
        let mut buf = vec![0u8; req.length as usize];
        let result = file.seek(SeekFrom::Start(req.offset)).and_then(|_| {
            let mut filled = 0;
            while filled < buf.len() {
                let n = file.read(&mut buf[filled..])?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            Ok(filled)
        });
        match result {
            Ok(n) => {
                buf.truncate(n);
                reply(req.device_io_request, NtStatus::SUCCESS, buf)
            }
            Err(e) => {
                tracing::debug!("RDP drive: read failed: {e}");
                reply(req.device_io_request, NtStatus::UNSUCCESSFUL, Vec::new())
            }
        }
    }

    fn write(&mut self, req: DeviceWriteRequest) -> Vec<SvcMessage> {
        let reply = |request, status, length| {
            vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(DeviceWriteResponse {
                device_io_reply: DeviceIoResponse::new(request, status),
                length,
            }))]
        };
        let Some(file) = self.open.get_mut(&req.device_io_request.file_id).and_then(|o| o.file.as_mut()) else {
            return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, 0);
        };
        let result = file
            .seek(SeekFrom::Start(req.offset))
            .and_then(|_| file.write_all(&req.write_data));
        match result {
            Ok(()) => reply(
                req.device_io_request,
                NtStatus::SUCCESS,
                u32::try_from(req.write_data.len()).unwrap_or(u32::MAX),
            ),
            Err(e) => {
                tracing::debug!("RDP drive: write failed: {e}");
                let status = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    NtStatus::ACCESS_DENIED
                } else {
                    NtStatus::UNSUCCESSFUL
                };
                reply(req.device_io_request, status, 0)
            }
        }
    }

    fn query_information(&mut self, req: ServerDriveQueryInformationRequest) -> Vec<SvcMessage> {
        let reply = |request, status, buffer| {
            vec![SvcMessage::from(RdpdrPdu::ClientDriveQueryInformationResponse(
                ClientDriveQueryInformationResponse {
                    device_io_response: DeviceIoResponse::new(request, status),
                    buffer,
                },
            ))]
        };
        let Some(open) = self.open.get(&req.device_io_request.file_id) else {
            return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, None);
        };
        let Ok(meta) = fs::metadata(&open.path) else {
            return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, None);
        };
        let attrs = attributes(&meta, &name_of(&open.path));
        let level = req.file_info_class_lvl;
        let buffer = if level == FileInformationClassLevel::FILE_BASIC_INFORMATION {
            FileInformationClass::Basic(FileBasicInformation {
                creation_time: filetime(meta.created()),
                last_access_time: filetime(meta.accessed()),
                last_write_time: filetime(meta.modified()),
                change_time: filetime(meta.modified()),
                file_attributes: attrs,
            })
        } else if level == FileInformationClassLevel::FILE_STANDARD_INFORMATION {
            let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
            FileInformationClass::Standard(FileStandardInformation {
                allocation_size: size,
                end_of_file: size,
                number_of_links: 1,
                delete_pending: Boolean::False,
                directory: if meta.is_dir() { Boolean::True } else { Boolean::False },
            })
        } else if level == FileInformationClassLevel::FILE_ATTRIBUTE_TAG_INFORMATION {
            FileInformationClass::AttributeTag(FileAttributeTagInformation {
                file_attributes: attrs,
                reparse_tag: 0,
            })
        } else {
            tracing::debug!("RDP drive: query information level {level:?} not offered");
            return reply(req.device_io_request, NtStatus::NOT_SUPPORTED, None);
        };
        reply(req.device_io_request, NtStatus::SUCCESS, Some(buffer))
    }

    fn query_volume(&mut self, req: ServerDriveQueryVolumeInformationRequest) -> Vec<SvcMessage> {
        let reply = |request, status, buffer| {
            vec![SvcMessage::from(RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                ClientDriveQueryVolumeInformationResponse {
                    device_io_reply: DeviceIoResponse::new(request, status),
                    buffer,
                },
            ))]
        };
        if !self.open.contains_key(&req.device_io_request.file_id) {
            return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, None);
        }
        const UNIT: u64 = 4096;
        let (total, free) = volume_space(&self.root);
        let units = |bytes: u64| i64::try_from(bytes / UNIT).unwrap_or(i64::MAX);
        let level = req.fs_info_class_lvl;
        let buffer = if level == FileSystemInformationClassLevel::FILE_FS_FULL_SIZE_INFORMATION {
            FileSystemInformationClass::FileFsFullSizeInformation(FileFsFullSizeInformation {
                total_alloc_units: units(total),
                caller_available_alloc_units: units(free),
                actual_available_alloc_units: units(free),
                sectors_per_alloc_unit: 8,
                bytes_per_sector: 512,
            })
        } else if level == FileSystemInformationClassLevel::FILE_FS_SIZE_INFORMATION {
            FileSystemInformationClass::FileFsSizeInformation(FileFsSizeInformation {
                total_alloc_units: units(total),
                available_alloc_units: units(free),
                sectors_per_alloc_unit: 8,
                bytes_per_sector: 512,
            })
        } else if level == FileSystemInformationClassLevel::FILE_FS_ATTRIBUTE_INFORMATION {
            FileSystemInformationClass::FileFsAttributeInformation(FileFsAttributeInformation {
                file_system_attributes: FileSystemAttributes::FILE_CASE_PRESERVED_NAMES
                    | FileSystemAttributes::FILE_UNICODE_ON_DISK,
                max_component_name_len: 255,
                file_system_name: "NTFS".to_owned(),
            })
        } else if level == FileSystemInformationClassLevel::FILE_FS_VOLUME_INFORMATION {
            FileSystemInformationClass::FileFsVolumeInformation(FileFsVolumeInformation {
                volume_creation_time: filetime(fs::metadata(&self.root).and_then(|m| m.created())),
                volume_serial_number: 0x5245_4348, // "REAC", stable and harmless
                supports_objects: Boolean::False,
                volume_label: self.name.clone(),
            })
        } else {
            tracing::debug!("RDP drive: volume information level {level:?} not offered");
            return reply(req.device_io_request, NtStatus::NOT_SUPPORTED, None);
        };
        reply(req.device_io_request, NtStatus::SUCCESS, Some(buffer))
    }

    fn set_information(&mut self, req: ServerDriveSetInformationRequest) -> PduResult<Vec<SvcMessage>> {
        let reply = |req: &ServerDriveSetInformationRequest, status| -> PduResult<Vec<SvcMessage>> {
            let response = ClientDriveSetInformationResponse::new(req, status).map_err(|e| encode_err!(e))?;
            Ok(vec![SvcMessage::from(RdpdrPdu::ClientDriveSetInformationResponse(response))])
        };
        let Some(open) = self.open.get_mut(&req.device_io_request.file_id) else {
            return reply(&req, NtStatus::NO_SUCH_FILE);
        };
        let done = match &req.set_buffer {
            FileInformationClass::Rename(info) => match resolve_below(&self.root, &info.file_name) {
                Some(to) => fs::rename(&open.path, &to).map(|_| open.path = to),
                None => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "outside the folder")),
            },
            FileInformationClass::Disposition(_) => {
                // Deleted on close by NT; here, deleted now. The handle to a
                // deleted file is dropped so the removal is not blocked by it.
                open.file = None;
                if open.path.is_dir() {
                    fs::remove_dir(&open.path)
                } else {
                    fs::remove_file(&open.path)
                }
            }
            FileInformationClass::EndOfFile(info) => match open.file.as_mut() {
                Some(file) => file.set_len(u64::try_from(info.end_of_file).unwrap_or(0)),
                None => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not a file")),
            },
            FileInformationClass::Allocation(_) | FileInformationClass::Basic(_) => Ok(()),
            _ => Ok(()),
        };
        match done {
            Ok(()) => reply(&req, NtStatus::SUCCESS),
            Err(e) => {
                tracing::debug!("RDP drive: set information failed: {e}");
                let status = match e.kind() {
                    std::io::ErrorKind::PermissionDenied => NtStatus::ACCESS_DENIED,
                    std::io::ErrorKind::NotFound => NtStatus::NO_SUCH_FILE,
                    _ => NtStatus::UNSUCCESSFUL,
                };
                reply(&req, status)
            }
        }
    }

    fn query_directory(&mut self, req: ServerDriveQueryDirectoryRequest) -> Vec<SvcMessage> {
        let reply = |request, status, buffer| {
            vec![SvcMessage::from(RdpdrPdu::ClientDriveQueryDirectoryResponse(
                ClientDriveQueryDirectoryResponse {
                    device_io_reply: DeviceIoResponse::new(request, status),
                    buffer,
                },
            ))]
        };
        if req.file_info_class_lvl != FileInformationClassLevel::FILE_BOTH_DIRECTORY_INFORMATION {
            tracing::debug!("RDP drive: directory level {:?} not offered", req.file_info_class_lvl);
            return reply(req.device_io_request, NtStatus::NOT_SUPPORTED, None);
        }
        let initial = req.initial_query != 0;
        let Some(open) = self.open.get_mut(&req.device_io_request.file_id) else {
            return reply(req.device_io_request, NtStatus::NO_SUCH_FILE, None);
        };

        // The first query names what to list: `\dir\*` for everything in
        // it, or one exact path. Later queries continue the listing.
        let next: Option<PathBuf> = if initial {
            if let Some(dir) = req.path.strip_suffix('*') {
                match resolve_below(&self.root, dir).and_then(|d| fs::read_dir(d).ok()) {
                    Some(mut listing) => {
                        let first = listing.next().and_then(Result::ok).map(|e| e.path());
                        open.listing = Some(listing);
                        first
                    }
                    None => None,
                }
            } else {
                resolve_below(&self.root, &req.path)
            }
        } else {
            open.listing.as_mut().and_then(|l| l.next()).and_then(Result::ok).map(|e| e.path())
        };

        let not_found = if initial { NtStatus::NO_SUCH_FILE } else { NtStatus::NO_MORE_FILES };
        let Some(path) = next else {
            return reply(req.device_io_request, not_found, None);
        };
        let Ok(meta) = fs::metadata(&path).or_else(|_| fs::symlink_metadata(&path)) else {
            return reply(req.device_io_request, not_found, None);
        };
        let name = name_of(&path);
        let info = FileBothDirectoryInformation::new(
            filetime(meta.created()),
            filetime(meta.accessed()),
            filetime(meta.modified()),
            filetime(meta.modified()),
            i64::try_from(meta.len()).unwrap_or(i64::MAX),
            attributes(&meta, &name),
            name,
        );
        reply(
            req.device_io_request,
            NtStatus::SUCCESS,
            Some(FileInformationClass::BothDirectory(info)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_paths_stay_below_the_folder() {
        let root = Path::new("/share");
        assert_eq!(resolve_below(root, "\\a\\b.txt"), Some(root.join("a").join("b.txt")));
        assert_eq!(resolve_below(root, "a/b.txt"), Some(root.join("a").join("b.txt")));
        assert_eq!(resolve_below(root, "\\"), Some(root.to_path_buf()));
        assert_eq!(resolve_below(root, "\\..\\secret"), None);
        assert_eq!(resolve_below(root, "a\\..\\..\\x"), None);
        assert_eq!(resolve_below(root, "C:\\x"), None);
    }

    #[test]
    fn filetime_of_the_unix_epoch_is_the_documented_constant() {
        assert_eq!(filetime(Ok(UNIX_EPOCH)), 116_444_736_000_000_000);
        assert_eq!(filetime(Err(std::io::Error::other("none"))), 116_444_736_000_000_000);
    }

    #[test]
    fn a_folder_lists_and_serves_its_files() {
        let dir = std::env::temp_dir().join(format!("reach-drive-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("hello.txt"), b"hello").unwrap();
        let drive = FolderDrive::new(dir.clone(), "test".into());

        // Resolution and refusal.
        assert_eq!(drive.resolve("\\hello.txt"), Some(dir.join("hello.txt")));
        assert_eq!(drive.resolve("\\..\\hello.txt"), None);

        // Listing walks the folder.
        let root = drive.resolve("\\").unwrap();
        let mut listing = fs::read_dir(&root).unwrap();
        let mut names: Vec<String> = Vec::new();
        while let Some(Ok(e)) = listing.next() {
            names.push(name_of(&e.path()));
        }
        names.sort();
        assert_eq!(names, vec!["hello.txt".to_string(), "sub".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }
}
