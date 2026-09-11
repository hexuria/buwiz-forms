use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use gpui_agent::mailbox::AgentMailbox;
use gpui_agent::security::from_env;
use gpui_agent::server::spawn_mailbox;

fn auth_banner(token_set: bool) -> &'static str {
    if token_set {
        "auth: required (GPUI_AGENT_TOKEN set; CLI/MCP send protocol v2 HMAC, never the raw token)"
    } else {
        "auth: none (GPUI_AGENT_INSECURE_NO_TOKEN=1 demo only; recipe/MCP still need a client token)"
    }
}

/// Mailbox plus the configured host token from `from_env`.
///
/// The token is never logged. Do not add `Debug` that would print it.
#[derive(Clone)]
pub struct StartedAgent {
    pub mailbox: AgentMailbox,
    pub token: Option<String>,
    /// Stops `serve_mailbox` so the TCP bind is released on quit, not only
    /// after process death. Hide-to-dock is not quit.
    pub shutdown: Arc<AtomicBool>,
}

/// Start the localhost control plane when `GPUI_AGENT=1`.
///
/// The TCP thread posts onto `AgentMailbox`; `AppState::render` drains it on
/// the GPUI UI thread so actions mutate the same widgets humans use.
/// The token is never logged.
pub fn maybe_start() -> Option<StartedAgent> {
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
            let token = config.token.clone();
            let auth = auth_banner(token.is_some());
            match spawn_mailbox(
                config.addr,
                config.token,
                mailbox.clone(),
                Duration::from_secs(8),
            ) {
                Ok((addr, shutdown)) => {
                    tracing::info!(
                        %addr,
                        "{auth}; platform=desktop app=bir-desktop; loopback default via from_env; protocol v2 HMAC; GPUI_AGENT_TOKEN required to bind unless GPUI_AGENT_INSECURE_NO_TOKEN=1; delivery=semantic (virtual returns virtual_unavailable; no OS HID); screenshot=macOS mailbox drain screencapture -l (relative .png under screenshot dir), else screenshot_unavailable"
                    );
                    Some(StartedAgent {
                        mailbox,
                        token,
                        shutdown,
                    })
                }
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "gpui-agent failed to bind; quit painted bir or bir-headless, or set GPUI_AGENT_ADDR (two AgentHosts cannot share a bind)"
                    );
                    None
                }
            }
        }
    }
}
