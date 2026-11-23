//! `bir`: the gpui-agent session with no window (`serve`, `status`, `logs`, `shutdown`).
//!
//! The session, the daemon and the command line live in `bir_app_api::agent`; this crate only installs the
//! hooks the headless session calls. See `docs/agent.md`.

use bir_app_api::hooks::Hooks;
use bir_core::db::Database;
use bir_core::forms::agent::AgentFormKind;
use serde_json::Value;

struct HeadlessHooks;

impl Hooks for HeadlessHooks {
    fn agent_send_draft(
        &self,
        db: &Database,
        kind: AgentFormKind,
        draft: &mut Value,
    ) -> Result<Value, String> {
        bir_app_api::agent::send_draft(db, kind, draft)
    }

    fn agent_pop_command(&self) -> Option<Value> {
        bir_app_api::agent::pop_command()
    }
}

fn main() -> std::process::ExitCode {
    let _ = bir_app_api::install_hooks(HeadlessHooks);
    bir_app_api::agent::headless::run()
}
