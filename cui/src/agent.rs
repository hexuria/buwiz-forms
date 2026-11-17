//! Agent remote control (D6) — port in progress.
//!
//! The NSH app's `agent.rs` (the gpui-agent loopback listener and the queued UI commands) is being
//! ported. Without the `agent` cargo feature — and until the port lands — the hooks answer honestly
//! that there is no agent control.

use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use serde_json::Value;

/// The agent's send: validate `draft` and queue it, as the UI's Queue does.
pub(crate) fn send_draft(
    _db: &Database,
    _kind: AgentFormKind,
    _draft: &mut Value,
) -> Result<Value, String> {
    Err("This build has no agent control.".to_string())
}

/// The next UI command the agent host queued for the frame: none.
pub(crate) fn pop_command() -> Option<Value> {
    None
}
