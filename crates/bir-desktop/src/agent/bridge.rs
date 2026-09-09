use std::time::Duration;

use gpui_agent::mailbox::AgentMailbox;
use gpui_agent::security::from_env;
use gpui_agent::server::spawn_mailbox;

fn auth_banner(token_set: bool) -> &'static str {
    if token_set {
        "auth: required (GPUI_AGENT_TOKEN set; recipe/MCP clients must send the same token)"
    } else {
        "auth: none (one-off click/snapshot ok; recipe run and mcp need the same token on host and client)"
    }
}

/// Start the localhost control plane when `GPUI_AGENT=1`.
///
/// The TCP thread posts onto `AgentMailbox`; `AppState::render` drains it on
/// the GPUI UI thread so actions mutate the same widgets humans use.
/// The token is never logged.
pub fn maybe_start() -> Option<AgentMailbox> {
    match from_env() {
        Ok(None) => {
            tracing::info!("gpui-agent control plane off (set GPUI_AGENT=1 to opt in)");
            None
        }
        Err(err) => {
            tracing::warn!(error = %err, "gpui-agent control plane refused");
            None
        }
        Ok(Some(config)) => {
            let mailbox = AgentMailbox::new();
            let auth = auth_banner(config.token.is_some());
            match spawn_mailbox(
                config.addr,
                config.token,
                mailbox.clone(),
                Duration::from_secs(8),
            ) {
                Ok((addr, _)) => {
                    tracing::info!(
                        %addr,
                        "{auth}; platform=desktop app=bir-desktop; loopback only; protocol v1; delivery=semantic (virtual returns virtual_unavailable; no OS HID)"
                    );
                    Some(mailbox)
                }
                Err(err) => {
                    tracing::error!(error = %err, "gpui-agent failed to bind");
                    None
                }
            }
        }
    }
}
