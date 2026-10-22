//! The Windows side of [`super::passbolt::check_cli_file`]: what Unix checks
//! with owner and mode bits, read from the file's owner and access list.
//!
//! The CLI is handed the private key and its passphrase, so it must be a
//! program only the user or the system can change. Owned by the user,
//! LocalSystem, Administrators or TrustedInstaller, and no access entry
//! lets anyone else write to it, delete it or change its permissions. That
//! leaves Program Files and the user's own profile (winget, Scoop, a copy
//! in Downloads) and refuses a folder like `C:\tools`, where Windows lets
//! every signed-in user change what is created under `C:\`.

use std::path::Path;

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows::Win32::Security::{
    GetAce, GetTokenInformation, TokenUser, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
    OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::passbolt::NAME;

const LOCAL_SYSTEM: &str = "S-1-5-18";
const ADMINISTRATORS: &str = "S-1-5-32-544";
const TRUSTED_INSTALLER: &str = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
/// OWNER RIGHTS: stands for the owner, which is checked on its own.
const OWNER_RIGHTS: &str = "S-1-3-4";

const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const INHERIT_ONLY_ACE: u8 = 0x08;

/// Rights that let someone replace what runs: write or append (add a file
/// or folder, for a folder), delete it or a child, change its permissions
/// or owner, and the generic rights that include those.
const REPLACING_RIGHTS: u32 = 0x0002 // FILE_WRITE_DATA / FILE_ADD_FILE
    | 0x0004 // FILE_APPEND_DATA / FILE_ADD_SUBDIRECTORY
    | 0x0040 // FILE_DELETE_CHILD
    | 0x0001_0000 // DELETE
    | 0x0004_0000 // WRITE_DAC
    | 0x0008_0000 // WRITE_OWNER
    | 0x1000_0000 // GENERIC_ALL
    | 0x4000_0000; // GENERIC_WRITE

/// Owned by the user or the system, and nobody else can replace it.
pub fn check_not_replaceable(path: &Path, what: &str) -> Result<(), String> {
    let me = current_user_sid().map_err(|e| format!("{NAME}: cannot read who runs Reach: {e}"))?;
    let (owner, writers) = read_access(path)
        .map_err(|e| format!("{NAME}: the permissions of {what} at {} cannot be read: {e}", path.display()))?;
    let trusted = |sid: &str| sid == me || [LOCAL_SYSTEM, ADMINISTRATORS, TRUSTED_INSTALLER].contains(&sid);
    if !trusted(&owner) {
        return Err(format!("{NAME}: {what} at {} belongs to another user", path.display()));
    }
    if writers.iter().any(|sid| !trusted(sid) && sid != OWNER_RIGHTS) {
        return Err(format!("{NAME}: {what} at {} can be changed by other users", path.display()));
    }
    Ok(())
}

/// The owner, and every account an entry that applies to this object lets
/// replace it. `None` access list (no DACL) means everyone may.
fn read_access(path: &Path) -> Result<(String, Vec<String>), String> {
    let mut owner = PSID::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the out pointers point into the security descriptor the call
    // allocates, which is freed below after the last read of them.
    let status = unsafe {
        GetNamedSecurityInfoW(
            &HSTRING::from(path.as_os_str()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            Some(&mut dacl),
            None,
            &mut sd,
        )
    };
    if status.is_err() {
        return Err(format!("{status:?}"));
    }
    let result = (|| {
        let owner = sid_string(owner)?;
        if dacl.is_null() {
            return Ok((owner, vec!["S-1-1-0".to_string()]));
        }
        let mut writers = Vec::new();
        // SAFETY: `dacl` is a valid ACL inside `sd`; GetAce returns pointers
        // to entries within it, each starting with its ACE_HEADER.
        let count = unsafe { (*dacl).AceCount };
        for i in 0..u32::from(count) {
            let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
            unsafe { GetAce(dacl, i, &mut ace) }.map_err(|e| e.to_string())?;
            let header = unsafe { &*(ace as *const ACE_HEADER) };
            if header.AceType != ACCESS_ALLOWED_ACE_TYPE || header.AceFlags & INHERIT_ONLY_ACE != 0 {
                continue;
            }
            let allowed = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
            if allowed.Mask & REPLACING_RIGHTS != 0 {
                writers.push(sid_string(PSID(&allowed.SidStart as *const u32 as *mut _))?);
            }
        }
        Ok((owner, writers))
    })();
    // SAFETY: `sd` was allocated by GetNamedSecurityInfoW for the caller to free.
    unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    result
}

fn sid_string(sid: PSID) -> Result<String, String> {
    let mut text = PWSTR::null();
    // SAFETY: `sid` is a valid SID; the string is allocated for us to free.
    unsafe { ConvertSidToStringSidW(sid, &mut text) }.map_err(|e| e.to_string())?;
    let s = unsafe { text.to_string() }.map_err(|e| e.to_string());
    unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    s
}

fn current_user_sid() -> Result<String, String> {
    // SAFETY: this process's own token, closed on every path; the buffer is
    // sized from the API's own answer and outlives the read of the SID in it.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(|e| e.to_string())?;
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        let read = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), needed, &mut needed);
        let _ = CloseHandle(token);
        read.map_err(|e| e.to_string())?;
        sid_string((*(buf.as_ptr() as *const TOKEN_USER)).User.Sid)
    }
}
