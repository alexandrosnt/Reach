//! Keys kept encrypted in memory, and never swapped to disk.
//!
//! The vault's long-lived keys (the key that unlocks the vault, each vault's
//! master key, the identity key) are in memory for as long as the vault is
//! unlocked. Held as plain bytes, any read of this process's memory, a dump,
//! a scraper or a side channel such as Spectre or Rowhammer, would hand them
//! over whole.
//!
//! So they are held encrypted, the way OpenSSH shields private keys since
//! 8.1 and Sequoia PGP's `mem::Encrypted` does after it. Once per process a
//! 16 KiB random prekey is made. Each secret gets its own random salt, is
//! sealed with XChaCha20-Poly1305 under SHA-512(salt ‖ prekey), and is opened
//! only for the length of a call ([`Shielded::with`]), into a buffer that is
//! wiped as the call returns. To read a key an attacker needs all 16 KiB of
//! the prekey without a single wrong bit, which is what makes partial and
//! noisy reads useless.
//!
//! The prekey and every opened buffer are locked into RAM (`memsec::mlock`:
//! `VirtualLock` on Windows, `mlock` elsewhere, and on Linux left out of core
//! dumps), so neither reaches the swap file. Locking is best-effort: a system
//! that refuses it (Linux caps how much a process may lock) is logged once,
//! and the keys stay encrypted regardless.

use std::sync::OnceLock;

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use sha2::{Digest, Sha512};
use zeroize::{Zeroize, Zeroizing};

/// OpenSSH and Sequoia both use four 4 KiB pages.
const PREKEY_LEN: usize = 4 * 4096;

struct Prekey {
    bytes: Box<[u8; PREKEY_LEN]>,
    #[cfg(test)]
    locked: bool,
}

/// Made on first use and kept for the life of the process: every shielded
/// secret depends on it.
fn prekey() -> &'static Prekey {
    static PREKEY: OnceLock<Prekey> = OnceLock::new();
    PREKEY.get_or_init(|| {
        let mut bytes = Box::new([0u8; PREKEY_LEN]);
        rand::fill(&mut bytes[..]);
        let locked = lock(bytes.as_mut_ptr(), PREKEY_LEN);
        if !locked {
            tracing::warn!("Could not lock the key-shielding prekey into RAM; keys stay encrypted in memory");
        }
        Prekey {
            bytes,
            #[cfg(test)]
            locked,
        }
    })
}

/// Whether the prekey is locked into RAM. For the tests.
#[cfg(test)]
fn prekey_locked() -> bool {
    prekey().locked
}

fn lock(ptr: *mut u8, len: usize) -> bool {
    // SAFETY: `ptr` points at `len` bytes this module owns and keeps alive
    // until the matching `unlock`.
    unsafe { memsec::mlock(ptr, len) }
}

fn unlock(ptr: *mut u8, len: usize) {
    // SAFETY: as for `lock`; munlock also wipes the bytes first.
    unsafe {
        memsec::munlock(ptr, len);
    }
}

fn sealing_key(salt: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut hash = Sha512::new();
    hash.update(salt);
    hash.update(&prekey().bytes[..]);
    let mut digest = hash.finalize();
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&digest[..32]);
    digest.as_mut_slice().zeroize();
    key
}

/// Every salt is fresh and so is the key it makes, so a nonce of zero is
/// never used twice under one key (the same reasoning as Sequoia's).
const NONCE: [u8; 24] = [0u8; 24];

/// `N` secret bytes, held only encrypted. The salt and the sealed bytes are
/// wiped when it is dropped: without the prekey they open nothing, but there
/// is no reason to leave them in freed memory either.
pub struct Shielded<const N: usize> {
    salt: [u8; 32],
    sealed: Vec<u8>,
}

impl<const N: usize> Drop for Shielded<N> {
    fn drop(&mut self) {
        self.salt.zeroize();
        self.sealed.zeroize();
    }
}

/// A secret opened for one call: locked into RAM, wiped and unlocked on drop.
struct Opened<const N: usize>(Box<[u8; N]>);

impl<const N: usize> Opened<N> {
    fn new() -> Self {
        let mut buf = Box::new([0u8; N]);
        lock(buf.as_mut_ptr(), N);
        Self(buf)
    }
}

impl<const N: usize> Drop for Opened<N> {
    fn drop(&mut self) {
        unlock(self.0.as_mut_ptr(), N);
        self.0.zeroize();
    }
}

impl<const N: usize> Shielded<N> {
    /// Shield a secret written straight into a locked buffer by `fill`, so
    /// no unshielded copy is left behind by the caller.
    pub fn from_fn(fill: impl FnOnce(&mut [u8; N])) -> Self {
        let mut opened = Opened::<N>::new();
        fill(&mut opened.0);
        Self::seal(&opened.0)
    }

    /// As [`Shielded::from_fn`], for a `fill` that can fail (a key
    /// derivation, say). Nothing is kept if it does.
    pub fn try_from_fn<E>(fill: impl FnOnce(&mut [u8; N]) -> Result<(), E>) -> Result<Self, E> {
        let mut opened = Opened::<N>::new();
        fill(&mut opened.0)?;
        Ok(Self::seal(&opened.0))
    }

    /// Shield a secret the caller already holds, and wipe the caller's copy.
    pub fn new(mut secret: [u8; N]) -> Self {
        let shielded = Self::seal(&secret);
        secret.zeroize();
        shielded
    }

    fn seal(secret: &[u8; N]) -> Self {
        let salt: [u8; 32] = rand::random();
        let cipher = XChaCha20Poly1305::new((&*sealing_key(&salt)).into());
        let sealed = cipher
            .encrypt(&XNonce::from(NONCE), secret.as_slice())
            .expect("XChaCha20-Poly1305 encryption of a fixed-size buffer cannot fail");
        Self { salt, sealed }
    }

    /// Open the secret for the length of `use_it`. The opened copy is locked
    /// into RAM and wiped as soon as `use_it` returns.
    pub fn with<T>(&self, use_it: impl FnOnce(&[u8; N]) -> T) -> T {
        let cipher = XChaCha20Poly1305::new((&*sealing_key(&self.salt)).into());
        let mut plain = Zeroizing::new(
            cipher
                .decrypt(&XNonce::from(NONCE), self.sealed.as_slice())
                .expect("a shielded secret always opens under the process prekey"),
        );
        let mut opened = Opened::<N>::new();
        opened.0.copy_from_slice(&plain);
        plain.zeroize();
        use_it(&opened.0)
    }
}

#[cfg(test)]
#[path = "shield_tests.rs"]
mod tests;
