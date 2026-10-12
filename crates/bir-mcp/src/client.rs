//! The real [`BirHost`]: upstream gpui-agent discovery to find running BIR
//! apps, and a gpui-agent client to the one the server selected. Which app
//! that is never comes from here: [`crate::server::Server`] holds the
//! selection the user made through `bir_connect`.
use crate::apps;
use crate::server::{AppRecord, BirHost};
use crate::tokens::TokenSource;
use crate::tools::HostCall;
use gpui_agent::AgentClient;
use gpui_agent::discovery::HostRecord;
use gpui_agent::protocol::Op;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

pub const ALLOW_REMOTE_ENV: &str = "GPUI_AGENT_ALLOW_REMOTE";
const TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection to one app. Not `Debug`: it holds the token.
struct Session {
    pid: u32,
    addr: SocketAddr,
    token: String,
    client: AgentClient,
}

pub struct AgentBirHost {
    root: PathBuf,
    tokens: TokenSource,
    allow_remote: bool,
    session: Option<Session>,
}

impl AgentBirHost {
    pub fn new(root: PathBuf, tokens: TokenSource, allow_remote: bool) -> Self {
        Self {
            root,
            tokens,
            allow_remote,
            session: None,
        }
    }

    /// Records under upstream `runtime_root()` (`GPUI_AGENT_RUNTIME_DIR`
    /// overrides), tokens from `BIR_MCP_TOKENS` / `GPUI_AGENT_TOKEN`.
    pub fn from_env() -> Self {
        Self::new(
            gpui_agent::discovery::runtime_root(),
            TokenSource::from_env(),
            gpui_agent::security::truthy_env(ALLOW_REMOTE_ENV),
        )
    }

    fn record(&self, record: &HostRecord) -> Result<AppRecord, String> {
        let tokens = self.tokens.load()?;
        Ok(AppRecord {
            id: apps::app_id(record),
            connect_id: record.connect_id.clone(),
            pid: record.pid,
            addr: record.addr.clone(),
            mode: apps::mode(record).into(),
            token_known: tokens
                .for_app(record.connect_id.as_deref(), record.pid)
                .is_some(),
        })
    }

    fn client(&mut self, app: &AppRecord) -> Result<&mut AgentClient, String> {
        let tokens = self.tokens.load()?;
        let token = tokens
            .for_app(app.connect_id.as_deref(), app.pid)
            .ok_or_else(|| format!("no agent token for BIR app `{}`", app.id))?
            .to_string();
        let addr: SocketAddr = app
            .addr
            .parse()
            .map_err(|error| format!("bad address for BIR app `{}`: {error}", app.id))?;
        gpui_agent::authorize_client(addr, Some(&token), self.allow_remote)
            .map_err(|error| format!("refusing BIR address {addr}: {error}"))?;
        let reuse = matches!(
            &self.session,
            Some(session) if session.pid == app.pid && session.addr == addr && session.token == token
        );
        if !reuse {
            let client = AgentClient::connect(addr)
                .with_timeout(TIMEOUT)
                .with_token(token.clone());
            self.session = Some(Session {
                pid: app.pid,
                addr,
                token,
                client,
            });
        }
        Ok(&mut self.session.as_mut().expect("session set").client)
    }
}

impl BirHost for AgentBirHost {
    fn apps(&mut self) -> Result<Vec<AppRecord>, String> {
        apps::live(&self.root)
            .iter()
            .map(|record| self.record(record))
            .collect()
    }

    fn resolve(&mut self, id: &str) -> Result<AppRecord, String> {
        let record = apps::resolve(&self.root, id)?;
        self.record(&record)
    }

    fn alive(&mut self, app: &AppRecord) -> bool {
        apps::still_published(&self.root, app.pid, &app.addr)
    }

    fn call(&mut self, app: &AppRecord, call: HostCall, args: Value) -> Result<Value, String> {
        let op = match call {
            HostCall::Hello => Op::Hello,
            HostCall::Invoke(name) => Op::Invoke {
                name: name.into(),
                args,
            },
        };
        let client = self.client(app)?;
        let response = match client.rpc(op) {
            Ok(response) => response,
            Err(error) => {
                self.session = None;
                return Err(format!(
                    "could not reach BIR app `{}` at {}: {error}. Check that it is still running and that its token in BIR_MCP_TOKENS (or GPUI_AGENT_TOKEN) matches the app's GPUI_AGENT_TOKEN.",
                    app.id, app.addr
                ));
            }
        };
        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| format!("BIR refused `{}`", call.op_name())));
        }
        Ok(match call {
            HostCall::Hello => json!(response.hello),
            HostCall::Invoke(_) => response.result.unwrap_or_else(|| json!({ "ok": true })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_token_means_no_connection_attempt() {
        let root = tempfile::tempdir().unwrap();
        let mut host = AgentBirHost::new(
            root.path().to_path_buf(),
            TokenSource::new(None, None),
            false,
        );
        assert!(host.apps().unwrap().is_empty());
        let app = AppRecord {
            id: "bir-demo".into(),
            connect_id: Some("bir-demo".into()),
            pid: std::process::id(),
            // Nothing listens here; a connection attempt would fail with a
            // different message.
            addr: "127.0.0.1:9".into(),
            mode: "desktop".into(),
            token_known: false,
        };
        let error = host.call(&app, HostCall::Hello, json!({})).unwrap_err();
        assert_eq!(error, "no agent token for BIR app `bir-demo`");
        assert!(host.session.is_none());
        assert!(!host.alive(&app));
        assert!(host.resolve("bir-demo").is_err());
    }
}
