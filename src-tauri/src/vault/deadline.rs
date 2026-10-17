//! A blocking call that is given up on after a while.
//!
//! The Secret Service can leave a call unanswered forever: reading from a
//! locked collection asks the desktop for an unlock prompt and waits for its
//! answer, with no limit, and some sessions never show the prompt (issue #87:
//! Reach hung on Arch-based systems, with nothing in the log). A caller that
//! waits without a limit hangs with it, along with everything queued behind
//! it, so the call runs on a thread of its own and is left behind when it
//! takes too long. The thread stays blocked until the call returns, if ever;
//! the caller moves on.

use std::sync::mpsc;
use std::time::Duration;

/// Runs `f` on a thread of its own and waits up to `limit` for it. `None`
/// when it did not finish in time; a panic in `f` is passed on.
pub fn call_with_deadline<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limit) {
        Ok(v) => Some(v),
        Err(mpsc::RecvTimeoutError::Timeout) => None,
        // The sender went away without sending: `f` panicked.
        Err(mpsc::RecvTimeoutError::Disconnected) => match thread.join() {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(()) => unreachable!("the thread sends before it ends"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn returns_what_the_call_returns() {
        assert_eq!(call_with_deadline(Duration::from_secs(5), || 42), Some(42));
    }

    #[test]
    fn gives_up_on_a_call_that_never_returns() {
        let started = Instant::now();
        let r = call_with_deadline(Duration::from_millis(200), || -> u8 {
            loop {
                std::thread::park();
            }
        });
        assert!(r.is_none());
        assert!(started.elapsed() < Duration::from_secs(2), "waited {:?}", started.elapsed());
    }

    #[test]
    fn passes_a_panic_on() {
        let r = std::panic::catch_unwind(|| call_with_deadline(Duration::from_secs(5), || -> u8 { panic!("boom") }));
        assert!(r.is_err());
    }
}
