//! Dragging a remote file out of the window without fetching it first.
//!
//! Each platform does this differently enough that there is no shared code,
//! only a shared idea: offer the file, and produce the bytes only if the drop
//! actually happens.
//!
//!   Windows  the drop target pulls bytes through an `IStream` and writes
//!            them itself; the source is never told where they went.
//!   macOS    the drop target hands the source the destination URL to write
//!            to (`NSFilePromiseProvider`).
//!   Linux    XDS would do it, but it is X11-only, absent on Wayland, and
//!            regressed in Nautilus. Not attempted; the temp-file drag stays.

#[cfg(target_os = "windows")]
pub mod windows;
