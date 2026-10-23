use super::*;

#[test]
fn a_secret_opens_to_what_it_was() {
    let shielded = Shielded::new([7u8; 32]);
    shielded.with(|k| assert_eq!(k, &[7u8; 32]));
    shielded.with(|k| assert_eq!(k, &[7u8; 32]));
}

#[test]
fn the_same_secret_is_sealed_differently_every_time() {
    let a = Shielded::new([7u8; 32]);
    let b = Shielded::new([7u8; 32]);
    assert_ne!(a.salt, b.salt);
    assert_ne!(a.sealed, b.sealed);
}

#[test]
fn the_prekey_is_locked_into_ram() {
    assert!(prekey_locked(), "the prekey could be swapped to disk");
}

/// The attack itself: a key is made, and the whole of this process's
/// readable memory is searched for its bytes. They must not be there.
/// The key is never held in the clear by the test either: it is
/// generated inside the shield and compared through a transform.
#[cfg(any(windows, target_os = "linux"))]
#[test]
fn the_key_cannot_be_found_anywhere_in_memory() {
    // Recognise the key by its bytes XORed with 0xA5, so the search
    // pattern in memory is not itself the key.
    let mut masked = [0u8; 32];
    let shielded = Shielded::<32>::from_fn(|k| {
        rand::fill(&mut k[..]);
        for (m, b) in masked.iter_mut().zip(k.iter()) {
            *m = b ^ 0xA5;
        }
    });
    // The key is usable...
    shielded.with(|k| assert!(k.iter().zip(masked.iter()).all(|(b, m)| b ^ 0xA5 == *m)));
    // ...and, outside that call, nowhere in memory.
    let hits = memory_scan::count(&masked, 0xA5);
    assert_eq!(hits, 0, "the key's bytes were found in process memory {hits} time(s)");
}

/// The same attack on a real vault key: derived from a password with
/// Argon2id, as unlocking does, and then looked for everywhere. Argon2's
/// working memory, the derivation's output and any copy on the way would
/// all show up here.
#[cfg(any(windows, target_os = "linux"))]
#[test]
fn a_password_derived_vault_key_is_nowhere_in_memory() {
    let salt = [3u8; 32];
    let kek = crate::vault::kdf::derive_kek(b"a vault password", &salt).unwrap();
    let mut masked = [0u8; 32];
    kek.with_key(|k| {
        for (m, b) in masked.iter_mut().zip(k.iter()) {
            *m = b ^ 0x3C;
        }
    });
    let hits = memory_scan::count(&masked, 0x3C);
    assert_eq!(hits, 0, "the derived key was found in process memory {hits} time(s)");
}

/// The control for the test above: a key held the ordinary way is found.
/// Without it a scan that finds nothing would prove nothing.
#[cfg(any(windows, target_os = "linux"))]
#[test]
fn a_key_held_in_the_clear_is_found() {
    let mut masked = [0u8; 32];
    rand::fill(&mut masked[..]);
    let plain: Box<[u8; 32]> = Box::new(std::array::from_fn(|i| masked[i] ^ 0x5A));
    assert!(memory_scan::count(&masked, 0x5A) >= 1, "the scan cannot see a key in plain memory");
    drop(plain);
}

#[cfg(any(windows, target_os = "linux"))]
mod memory_scan {
    /// How many times `masked ^ mask` appears in this process's readable
    /// memory.
    pub fn count(masked: &[u8; 32], mask: u8) -> usize {
        let mut hits = 0;
        for (start, len) in regions() {
            let mut buf = vec![0u8; len];
            if !read(start, &mut buf) {
                continue;
            }
            let first = masked[0] ^ mask;
            let mut i = 0;
            while i + 32 <= buf.len() {
                if buf[i] == first && buf[i..i + 32].iter().zip(masked).all(|(b, m)| b ^ mask == *m) {
                    hits += 1;
                }
                i += 1;
            }
        }
        hits
    }

    #[cfg(windows)]
    fn regions() -> Vec<(usize, usize)> {
        use windows::Win32::System::Memory::{
            VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
        };
        let mut out = Vec::new();
        let mut addr = 0usize;
        loop {
            let mut info = MEMORY_BASIC_INFORMATION::default();
            // SAFETY: VirtualQuery only reads the address space map.
            let n = unsafe {
                VirtualQuery(Some(addr as *const _), &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>())
            };
            if n == 0 {
                break;
            }
            let readable = info.State == MEM_COMMIT
                && (info.Protect.0 & PAGE_NOACCESS.0) == 0
                && (info.Protect.0 & PAGE_GUARD.0) == 0;
            if readable {
                out.push((info.BaseAddress as usize, info.RegionSize));
            }
            addr = info.BaseAddress as usize + info.RegionSize;
        }
        out
    }

    #[cfg(windows)]
    fn read(start: usize, buf: &mut [u8]) -> bool {
        use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
        use windows::Win32::System::Threading::GetCurrentProcess;
        // SAFETY: ReadProcessMemory copies into `buf` and fails rather
        // than faulting on memory that changed under it.
        unsafe {
            ReadProcessMemory(GetCurrentProcess(), start as *const _, buf.as_mut_ptr().cast(), buf.len(), None)
                .is_ok()
        }
    }

    #[cfg(target_os = "linux")]
    fn regions() -> Vec<(usize, usize)> {
        std::fs::read_to_string("/proc/self/maps")
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let range = parts.next()?;
                let perms = parts.next()?;
                if !perms.starts_with('r') {
                    return None;
                }
                let (a, b) = range.split_once('-')?;
                let (a, b) = (usize::from_str_radix(a, 16).ok()?, usize::from_str_radix(b, 16).ok()?);
                // Skip the vsyscall page and huge reservations.
                (b > a && b - a < (1 << 30)).then_some((a, b - a))
            })
            .collect()
    }

    /// Through process_vm_readv on this process: /proc/self/mem is closed
    /// even to the process itself once it is not dumpable (see
    /// crate::hardening), and a scan through it would quietly see
    /// nothing and prove nothing.
    #[cfg(target_os = "linux")]
    fn read(start: usize, buf: &mut [u8]) -> bool {
        let local = libc::iovec { iov_base: buf.as_mut_ptr().cast(), iov_len: buf.len() };
        let remote = libc::iovec { iov_base: start as *mut libc::c_void, iov_len: buf.len() };
        // SAFETY: copies from this process's own mapping into `buf`; the
        // kernel returns an error rather than faulting on a bad range.
        let n = unsafe { libc::process_vm_readv(libc::getpid(), &local, 1, &remote, 1, 0) };
        n == buf.len() as isize
    }
}

#[test]
fn dropping_a_shielded_secret_wipes_its_salt_and_sealed_bytes() {
    let mut slot = std::mem::ManuallyDrop::new(Shielded::new([7u8; 32]));
    assert_ne!(slot.salt, [0u8; 32]);
    assert!(!slot.sealed.is_empty());
    // SAFETY: dropped exactly once; ManuallyDrop keeps the struct's own
    // storage, and afterwards only the inline salt and the vector's length
    // are read, never its (freed) buffer.
    unsafe { std::ptr::drop_in_place(&mut *slot) };
    assert_eq!(slot.salt, [0u8; 32], "the salt is wiped");
    assert!(slot.sealed.is_empty(), "the sealed bytes are wiped (zeroize clears the vector)");
}
