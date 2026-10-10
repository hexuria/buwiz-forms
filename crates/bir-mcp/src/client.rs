//! The real [`BirHost`]: a gpui-agent client pointed at whichever BIR
//! instance discovery finds, re-resolved on every call so a BIR window
//! started after the adapter is still found.
use crate::discovery::{self, Endpoint};
use crate::server::BirHost;
use crate::tools::HostCall;
use gpui_agent::AgentClient;
use gpui_agent::protocol::Op;
use serde_json::{Value, json};
use std::time::Duration;

pub const TOKEN_ENV: &str = "GPUI_AGENT_TOKEN";
pub const ALLOW_REMOTE_ENV: &str = "GPUI_AGENT_ALLOW_REMOTE";
const TIMEOUT: Duration = Duration::from_secs(10);

pub struct AgentBirHost {
    token: Option<String>,
    allow_remote: bool,
    session: Option<(Endpoint, AgentClient)>,
}

impl AgentBirHost {
    pub fn from_env() -> Self {
        Self {
            token: std::env::var(TOKEN_ENV).ok().filter(|t| !t.is_empty()),
            allow_remote: gpui_agent::security::truthy_env(ALLOW_REMOTE_ENV),
            session: None,
        }
    }

    fn client(&mut self) -> Result<(Endpoint, &mut AgentClient), String> {
        let endpoint = discovery::resolve()?;
        gpui_agent::authorize_client(endpoint.addr, self.token.as_deref(), self.allow_remote)
            .map_err(|error| format!("refusing BIR address {}: {error}", endpoint.addr))?;
        let reuse = matches!(&self.session, Some((current, _)) if current.addr == endpoint.addr);
        if !reuse {
            let mut client = AgentClient::connect(endpoint.addr).with_timeout(TIMEOUT);
            if let Some(token) = &self.token {
                client = client.with_token(token.clone());
            }
            self.session = Some((endpoint.clone(), client));
        }
        let (_, client) = self.session.as_mut().expect("session set");
        Ok((endpoint, client))
    }
}

impl BirHost for AgentBirHost {
    fn call(&mut self, call: HostCall, args: Value) -> Result<Value, String> {
        let (endpoint, client) = self.client()?;
        let op = match call {
            HostCall::Hello => Op::Hello,
            HostCall::Invoke(name) => Op::Invoke {
                name: name.into(),
                args,
            },
        };
        let response = match client.rpc(op) {
            Ok(response) => response,
            Err(error) => {
                self.session = None;
                return Err(format!(
                    "could not reach BIR at {} (via {}): {error}. Open the BIR desktop app with the agent enabled (or run `bir-headless serve`), and make sure {TOKEN_ENV} matches the app's token.",
                    endpoint.addr, endpoint.via
                ));
            }
        };
        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| format!("BIR refused `{}`", call.op_name())));
        }
        Ok(match call {
            HostCall::Hello => json!({
                "connected": true,
                "addr": endpoint.addr.to_string(),
                "via": endpoint.via,
                "hello": response.hello,
            }),
            HostCall::Invoke(_) => response.result.unwrap_or_else(|| json!({ "ok": true })),
        })
    }
}
