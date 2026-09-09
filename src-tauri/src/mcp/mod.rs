//! Reach's MCP server: a governed bridge between an AI client and a live shell.
//!
//! Reach already holds authenticated sessions to real machines, which makes it
//! the natural place to expose them to an AI — and the dangerous one. The
//! design assumes the client may be careless, so nothing here relies on the
//! model behaving. The rules are preconditions that fail the call.
//!
//! Layout:
//!
//! * [`agents`] — who the AI is and, crucially, *which tools it gets at all*.
//! * [`guard`] — the compiled-in baseline. Cannot be relaxed by any agent.
//! * [`redact`] — best-effort secret scrubbing on the way out. Not a guarantee.
//!
//! Consent is layered, and every layer defaults to closed: the listener does
//! not exist until the user enables it, enabling it exposes no sessions, each
//! session is shared individually, and every write is confirmed by the user
//! with the model's stated reasoning in front of them.
//!
//! What is deliberately absent: this module never touches the vault. Not
//! filtered, not permission-checked — simply not wired up. Credentials are out
//! of reach structurally rather than by policy.

pub mod agents;
pub mod guard;
pub mod redact;
