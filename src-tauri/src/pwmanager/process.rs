//! Running a password manager's command-line tool without a window, without
//! a terminal to prompt on, and without letting it run or talk for ever.
//!
//! The child gets no stdin (any prompt it tries reads end-of-file and fails
//! at once), is killed when the handle is dropped, and is cut off at a
//! deadline. Its output is read into buffers that are wiped when dropped,
//! up to a cap: past it, the rest is read and thrown away and the call fails.
//!
//! Two more limits. At most [`MAX_CONCURRENT_RUNS`] logins run at once
//! (see [`cli_slot`]): a jump chain, a reconnect storm or a picker typed into
//! during its first load must not start a dozen full logins (and reuse one
//! TOTP code until the server rate-limits the account). And on Windows the
//! child is put in a job object that kills it when the job's last handle
//! closes, so quitting Reach mid-call does not leave the CLI running, with
//! the secrets in its environment, until its own timeout.

use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{Semaphore, SemaphorePermit};
use zeroize::{Zeroize, Zeroizing};

/// How many password-manager CLIs may run at the same time, for every
/// provider together.
pub(crate) const MAX_CONCURRENT_RUNS: usize = 2;

static CLI_SLOTS: Semaphore = Semaphore::const_new(MAX_CONCURRENT_RUNS);

/// Wait for a free slot to run a CLI that logs in. Hold the permit until the
/// child is gone. Asking a tool its version needs none.
pub(crate) async fn cli_slot() -> Result<SemaphorePermit<'static>, String> {
    // The semaphore is a static that is never closed, so this cannot fail;
    // the error is only there to avoid a panic path.
    CLI_SLOTS.acquire().await.map_err(|_| "the password manager runner is shutting down".to_string())
}

/// A command set up to run silently: no console window on Windows, stdin
/// closed, stdout and stderr captured, killed with its handle.
pub(crate) fn silent_command(program: &std::path::Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd
}

/// What a finished child left.
pub(crate) struct Output {
    pub status: ExitStatus,
    pub stdout: Zeroizing<Vec<u8>>,
    pub stderr: Zeroizing<Vec<u8>>,
}

#[derive(Debug)]
pub(crate) enum RunError {
    /// The program could not be started.
    Spawn(std::io::Error),
    /// It did not finish in time and was killed.
    Timeout,
    /// It printed more than the cap allows.
    TooLarge,
    /// Reading its output or waiting for it failed.
    Io(std::io::Error),
}

/// Read everything from `reader`, keeping at most `cap` bytes. Growing the
/// buffer copies into a fresh one and wipes the old, so no stray copy of the
/// output is left in freed memory. `Ok(None)` when the cap was exceeded (the
/// rest was drained, so the child is never blocked on a full pipe).
async fn read_capped<R: AsyncRead + Unpin>(
    mut reader: R,
    cap: usize,
) -> std::io::Result<Option<Zeroizing<Vec<u8>>>> {
    let mut out = Zeroizing::new(Vec::with_capacity(cap.min(16 * 1024)));
    let mut chunk = Zeroizing::new([0u8; 8192]);
    let mut overflow = false;
    loop {
        let n = reader.read(&mut chunk[..]).await?;
        if n == 0 {
            break;
        }
        if overflow {
            continue;
        }
        if out.len() + n > cap {
            overflow = true;
            out.zeroize();
            continue;
        }
        if out.len() + n > out.capacity() {
            let wanted = (out.capacity() * 2).max(out.len() + n).min(cap);
            let mut bigger = Zeroizing::new(Vec::with_capacity(wanted));
            bigger.extend_from_slice(&out);
            out = bigger;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(if overflow { None } else { Some(out) })
}

/// Start `cmd` (from [`silent_command`]) and collect its output, giving up
/// after `timeout`.
pub(crate) async fn run(
    mut cmd: tokio::process::Command,
    timeout: Duration,
    stdout_cap: usize,
    stderr_cap: usize,
) -> Result<Output, RunError> {
    let mut child = cmd.spawn().map_err(RunError::Spawn)?;
    // Held until this function returns, by which time the child has exited
    // or been killed. If Reach itself goes away first, the last handle to the
    // job closes and Windows kills the child with it. (Between spawn and
    // assignment there is a moment in which it is not yet in the job; the
    // CLI does nothing in that time that outlives it.)
    #[cfg(windows)]
    let _job = job::KillOnClose::for_child(&child);
    // The command holds the secrets it was given in its environment; it is
    // not needed once the child is running.
    drop(cmd);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let collect = async {
        let read_out = async {
            match stdout {
                Some(s) => read_capped(s, stdout_cap).await,
                None => Ok(Some(Zeroizing::new(Vec::new()))),
            }
        };
        let read_err = async {
            match stderr {
                Some(s) => read_capped(s, stderr_cap).await,
                None => Ok(Some(Zeroizing::new(Vec::new()))),
            }
        };
        let (out, err) = tokio::join!(read_out, read_err);
        let status = child.wait().await.map_err(RunError::Io)?;
        let out = out.map_err(RunError::Io)?.ok_or(RunError::TooLarge)?;
        // Too much on stderr is not fatal: the tail of an error is enough.
        let err = err.map_err(RunError::Io)?.unwrap_or_default();
        Ok(Output { status, stdout: out, stderr: err })
    };

    match tokio::time::timeout(timeout, collect).await {
        Ok(result) => result,
        Err(_) => {
            // kill_on_drop would do it too; say so explicitly.
            let _ = child.start_kill();
            Err(RunError::Timeout)
        }
    }
}

/// A Windows job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, one per
/// child. The same pattern as the X server's job (ssh/xserver.rs).
#[cfg(windows)]
mod job {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub(super) struct KillOnClose(HANDLE);

    // SAFETY: a job handle may be used and closed from any thread.
    unsafe impl Send for KillOnClose {}
    // SAFETY: nothing mutates through a shared reference.
    unsafe impl Sync for KillOnClose {}

    impl KillOnClose {
        /// A new job holding `child`. `None` (and a warning) if Windows
        /// refuses: the call still runs, just without this safety net.
        pub(super) fn for_child(child: &tokio::process::Child) -> Option<Self> {
            let process = child.raw_handle()?;
            // SAFETY: plain Win32 calls with valid arguments. The job handle
            // is owned by the returned value and closed once, in Drop; the
            // process handle is borrowed from `child`, alive for the call.
            unsafe {
                let job = match CreateJobObjectW(None, None) {
                    Ok(h) => KillOnClose(h),
                    Err(e) => {
                        tracing::warn!("pwmanager: no job object for the CLI: {e}");
                        return None;
                    }
                };
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set = SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                let assigned = set.and_then(|()| AssignProcessToJobObject(job.0, HANDLE(process)));
                match assigned {
                    Ok(()) => Some(job),
                    Err(e) => {
                        tracing::warn!("pwmanager: the CLI is not tied to Reach's lifetime: {e}");
                        None
                    }
                }
            }
        }
    }

    impl Drop for KillOnClose {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateJobObjectW and is closed once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
