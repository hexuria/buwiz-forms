//! MCP over stdio: JSON-RPC 2.0, one message per line. Implements
//! `initialize`, `notifications/initialized`, `ping`, `tools/list` and
//! `tools/call`. stdout carries protocol messages only; diagnostics go to
//! stderr.
use crate::license::License;
use crate::tools::{self, APP_INFO, Action, HostCall};
use serde_json::{Map, Value, json};
use std::io::{BufRead, Write};

pub const SERVER_NAME: &str = "bir-mcp";
pub const LATEST_PROTOCOL: &str = "2025-06-18";
pub const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

const INSTRUCTIONS: &str = "Draft-only access to the BIR desktop app for all 43 BIR forms. \
Several BIR apps may be running. First call bir_list_apps, show the list to the user, ask which \
app to use, then bir_connect with that id; never pick one for the user. An app on the REAL \
taxpayer database needs the user's explicit confirmation (bir_connect confirm_real). Then you can \
read taxpayer profiles, dues and form context, fill any box by its official key with a stated \
source, list the boxes that need the user (or get them as a chat form card), validate with the \
page's own rules, preview the filled sheet, save a draft and dismiss. You cannot submit, queue, \
file or pay: when a draft validates, bir_form_review brings it up in BIR with Submit highlighted \
and the user clicks Submit there. Only one form is open at a time.";

/// Refusal for every form/profile/dues tool while no app is selected.
pub const NOT_SELECTED: &str =
    "no BIR app selected — call bir_list_apps, ask the user which one, then bir_connect";

/// One running BIR app, as discovery reports it. Never holds a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRecord {
    /// What the user picks: the app's connect id, else `pid:<pid>`.
    pub id: String,
    pub connect_id: Option<String>,
    pub pid: u32,
    pub addr: String,
    /// `desktop` or `headless`, from the record (`app.info` refines it).
    pub mode: String,
    /// Whether `bir-mcp` holds a token for this app.
    pub token_known: bool,
}

/// The BIR side, as seen by the protocol layer: discovery plus calls to one
/// app. `call` only ever receives a [`HostCall`] taken from the closed tool
/// table (or [`APP_INFO`]). `Err` carries BIR's own message (or why BIR
/// could not be reached), shown to the model verbatim.
pub trait BirHost {
    /// Every live BIR app.
    fn apps(&mut self) -> Result<Vec<AppRecord>, String>;
    /// The one live BIR app `id` names.
    fn resolve(&mut self, id: &str) -> Result<AppRecord, String>;
    /// Whether `app` is still the same running app.
    fn alive(&mut self, app: &AppRecord) -> bool;
    fn call(&mut self, app: &AppRecord, call: HostCall, args: Value) -> Result<Value, String>;
}

/// The app this `bir-mcp` process works on, chosen through `bir_connect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub app: AppRecord,
    pub mode: String,
    /// `test` or `real` (anything `app.info` does not call `test` is real).
    pub database: String,
}

impl Selection {
    fn label(&self) -> Value {
        json!({ "id": self.app.id, "mode": self.mode, "database": self.database })
    }

    fn suffix(&self) -> String {
        format!(
            "[BIR app: {} ({}, {} database)]",
            self.app.id, self.mode, self.database
        )
    }
}

pub struct Server<H> {
    host: H,
    selected: Option<Selection>,
    license: License,
}

impl<H: BirHost> Server<H> {
    pub fn new(host: H) -> Self {
        Self::with_license(host, License::open())
    }

    /// `license` is checked before every tool. [`License::open`] is what
    /// [`Server::new`] uses so tool tests do not call buwiz-server. The
    /// `bir-mcp` binary passes a desktop session or an agent token instead.
    pub fn with_license(host: H, license: License) -> Self {
        Self {
            host,
            selected: None,
            license,
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn selected(&self) -> Option<&Selection> {
        self.selected.as_ref()
    }

    /// Handle one raw line. `None` means nothing is written back
    /// (notifications, responses, blank lines).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        if line.trim().is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle(message),
            Err(error) => Some(error_response(
                Value::Null,
                PARSE_ERROR,
                &format!("parse error: {error}"),
            )),
        };
        reply.map(|value| value.to_string())
    }

    pub fn handle(&mut self, message: Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(error_response(
                Value::Null,
                INVALID_REQUEST,
                "expected a single JSON-RPC object",
            ));
        };
        let id = object.get("id").cloned();
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            // A response from the client (we never send requests) or junk.
            return id.map(|id| error_response(id, INVALID_REQUEST, "missing method"));
        };
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        // Notifications get no reply, whatever the method.
        let id = id?;
        let outcome = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(tools::list()),
            "tools/call" => self.tools_call(&params),
            other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
        };
        Some(match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error_response(id, code, &message),
        })
    }

    fn tools_call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).ok_or((
            INVALID_PARAMS,
            "tools/call requires params.name".to_string(),
        ))?;
        // A missing or unlicensed Buwiz token stops every tool, including
        // list and connect, before this process talks to a BIR app.
        if let Err(message) = self.license.allow() {
            return Ok(tool_result(message, true));
        }
        let tool = tools::find(name).ok_or((INVALID_PARAMS, format!("unknown tool: {name}")))?;
        let empty = Map::new();
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => &empty,
            Some(Value::Object(arguments)) => arguments,
            Some(_) => {
                return Ok(tool_result("arguments must be a JSON object".into(), true));
            }
        };
        let args = match tool.host_args(arguments) {
            Ok(args) => args,
            Err(message) => return Ok(tool_result(format!("{name}: {message}"), true)),
        };
        let outcome = match tool.action {
            Action::ListApps => self.list_apps(),
            Action::Connect => self.connect(&args),
            Action::Status => self.status(),
            Action::Host(call) => self.host_call(call, args),
        };
        Ok(match outcome {
            Ok(value) => tool_result(
                serde_json::to_string_pretty(&value).expect("json value"),
                false,
            ),
            Err(message) => tool_result(message, true),
        })
    }

    fn list_apps(&mut self) -> Result<Value, String> {
        let apps = self.host.apps()?;
        let selected_id = self.selected.as_ref().map(|s| s.app.id.clone());
        let mut rows = Vec::with_capacity(apps.len());
        for app in &apps {
            let mut row = json!({
                "id": app.id,
                "pid": app.pid,
                "addr": app.addr,
                "mode": app.mode,
                "token": if app.token_known { "known" } else { "missing" },
            });
            if app.token_known {
                match self.host.call(app, APP_INFO, json!({})) {
                    Ok(info) => {
                        if let Some(mode) = info.get("mode").and_then(Value::as_str) {
                            row["mode"] = mode.into();
                        }
                        row["database"] = database_of(&info).into();
                        row["taxpayers"] = info.get("profiles").cloned().unwrap_or(json!([]));
                    }
                    Err(error) => row["error"] = error.into(),
                }
            }
            row["selected"] = (selected_id.as_deref() == Some(app.id.as_str())).into();
            rows.push(row);
        }
        let next = if rows.is_empty() {
            "No BIR app is running. Ask the user to open BIR with the agent enabled (GPUI_AGENT=1), then call bir_list_apps again."
        } else {
            "Show this list to the user and ask which app to use, then call bir_connect with that id. Never pick one yourself. An app with token \"missing\" cannot be used until the user adds its token to the BIR_MCP_TOKENS file."
        };
        Ok(json!({
            "apps": rows,
            "selected": selected_id,
            "next": next,
        }))
    }

    fn connect(&mut self, args: &Value) -> Result<Value, String> {
        // Whatever happens next, the previous selection is gone: a failed
        // switch must not leave the model working on the old app.
        self.selected = None;
        let id = args["id"].as_str().unwrap_or_default();
        let confirm_real = args["confirm_real"].as_bool().unwrap_or(false);
        let app = self.host.resolve(id)?;
        if !app.token_known {
            return Err(format!(
                "no agent token for BIR app `{}`: the user must add `\"{}\": \"<token>\"` to the file named by BIR_MCP_TOKENS (mode 0600), or set GPUI_AGENT_TOKEN, then call bir_connect again. Never ask the user to paste a token into this chat.",
                app.id, app.id
            ));
        }
        let info = self.host.call(&app, APP_INFO, json!({}))?;
        let database = database_of(&info);
        if database != "test" && !confirm_real {
            return Err(format!(
                "BIR app `{}` uses the REAL taxpayer database. Ask the user whether they really want to prepare drafts on real taxpayer data; only if they say yes, call bir_connect {{\"id\": \"{}\", \"confirm_real\": true}}.",
                app.id, app.id
            ));
        }
        let mode = info
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or(&app.mode)
            .to_string();
        let selection = Selection {
            app,
            mode,
            database: database.to_string(),
        };
        let reply = json!({
            "connected": true,
            "app": selection.label(),
            "pid": selection.app.pid,
            "addr": selection.app.addr,
            "taxpayers": info.get("profiles").cloned().unwrap_or(json!([])),
        });
        self.selected = Some(selection);
        Ok(reply)
    }

    /// The selection, if its app is still running; a vanished app clears it.
    fn live_selection(&mut self) -> Result<Selection, String> {
        let selection = self.selected.clone().ok_or(NOT_SELECTED)?;
        if !self.host.alive(&selection.app) {
            self.selected = None;
            return Err(format!(
                "the selected BIR app `{}` is no longer running, so no app is selected now — call bir_list_apps, ask the user which one, then bir_connect",
                selection.app.id
            ));
        }
        Ok(selection)
    }

    fn status(&mut self) -> Result<Value, String> {
        if self.selected.is_none() {
            return Ok(json!({ "connected": false, "message": NOT_SELECTED }));
        }
        let selection = self.live_selection()?;
        let hello = self
            .host
            .call(&selection.app, HostCall::Hello, json!({}))
            .map_err(|error| format!("{error}\n{}", selection.suffix()))?;
        Ok(json!({
            "connected": true,
            "app": selection.label(),
            "pid": selection.app.pid,
            "addr": selection.app.addr,
            "hello": hello,
        }))
    }

    fn host_call(&mut self, call: HostCall, args: Value) -> Result<Value, String> {
        let selection = self.live_selection()?;
        match self.host.call(&selection.app, call, args) {
            Ok(result) => Ok(json!({ "app": selection.label(), "result": result })),
            Err(error) => Err(format!("{error}\n{}", selection.suffix())),
        }
    }
}

/// `test` only when `app.info` says so; anything else is treated as real.
fn database_of(info: &Value) -> &'static str {
    if info.get("database").and_then(Value::as_str) == Some("test") {
        "test"
    } else {
        "real"
    }
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|v| SUPPORTED_PROTOCOLS.contains(v))
        .unwrap_or(LATEST_PROTOCOL);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn tool_result(text: String, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// Serve until `input` reaches EOF.
pub fn serve<H: BirHost>(
    server: &mut Server<H>,
    input: impl BufRead,
    mut output: impl Write,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if let Some(reply) = server.handle_line(&line) {
            output.write_all(reply.as_bytes())?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for discovery plus BIR: a fixed set of apps, each with a
    /// database label, answering like the host contract in `.claude/GOAL.md`.
    /// Records every call with the app it went to.
    struct FakeHost {
        apps: Vec<(AppRecord, &'static str)>,
        dead: Vec<String>,
        calls: Vec<(String, HostCall, Value)>,
        open_form: Option<String>,
        dirty: bool,
    }

    fn app(id: &str, pid: u32, mode: &str, token_known: bool) -> AppRecord {
        AppRecord {
            id: id.into(),
            connect_id: (!id.starts_with("pid:")).then(|| id.to_string()),
            pid,
            addr: format!("127.0.0.1:{}", 50000 + pid),
            mode: mode.into(),
            token_known,
        }
    }

    impl FakeHost {
        fn with(apps: Vec<(AppRecord, &'static str)>) -> Self {
            Self {
                apps,
                dead: Vec::new(),
                calls: Vec::new(),
                open_form: None,
                dirty: false,
            }
        }

        /// One test app (token known) only: the case where auto-picking it
        /// would be most tempting.
        fn single() -> Self {
            Self::with(vec![(app("bir-demo", 11, "desktop", true), "test")])
        }

        fn host_calls(&self) -> Vec<(HostCall, Value)> {
            self.calls
                .iter()
                .filter(|(_, call, _)| *call != APP_INFO)
                .map(|(_, call, args)| (*call, args.clone()))
                .collect()
        }
    }

    impl BirHost for FakeHost {
        fn apps(&mut self) -> Result<Vec<AppRecord>, String> {
            Ok(self
                .apps
                .iter()
                .filter(|(app, _)| !self.dead.contains(&app.id))
                .map(|(app, _)| app.clone())
                .collect())
        }

        fn resolve(&mut self, id: &str) -> Result<AppRecord, String> {
            self.apps()?
                .into_iter()
                .find(|app| app.id == id)
                .ok_or_else(|| format!("no AgentHost is running for '{id}'"))
        }

        fn alive(&mut self, app: &AppRecord) -> bool {
            !self.dead.contains(&app.id)
        }

        fn call(&mut self, app: &AppRecord, call: HostCall, args: Value) -> Result<Value, String> {
            self.calls.push((app.id.clone(), call, args.clone()));
            assert!(app.token_known, "called {} without a token", app.id);
            match call {
                HostCall::Hello => Ok(json!({"app": "bir-desktop"})),
                HostCall::Invoke("app.info") => {
                    let database = self
                        .apps
                        .iter()
                        .find(|(known, _)| known.id == app.id)
                        .map(|(_, database)| *database)
                        .unwrap();
                    Ok(json!({
                        "app": "bir-desktop",
                        "mode": app.mode,
                        "database": database,
                        "profiles": [{"tin": "11111111400000", "name": "Test Taxpayer"}]
                    }))
                }
                HostCall::Invoke("form.open") => {
                    if let Some(open) = &self.open_form {
                        return Err(format!(
                            "form already open: {open}; save it or dismiss it first"
                        ));
                    }
                    let code = args["code"].as_str().unwrap().to_string();
                    self.open_form = Some(code.clone());
                    Ok(json!({"form": code}))
                }
                HostCall::Invoke("form.fill") => {
                    self.dirty = true;
                    let keys: Vec<&String> = args["fields"].as_object().unwrap().keys().collect();
                    Ok(json!({"filled": keys, "kept_user_boxes": []}))
                }
                HostCall::Invoke("form.dismiss") => {
                    if self.dirty {
                        return Err("unsaved edits: ask the user to press Dismiss in BIR".into());
                    }
                    Ok(json!({"dismissed": true, "form": self.open_form.take()}))
                }
                HostCall::Invoke("form.validate") => Ok(json!({
                    "ok": false,
                    "errors": 1,
                    "field_errors": [{"field": "tin", "message": "TIN is required"}]
                })),
                HostCall::Invoke(_) => Ok(json!({"ok": true})),
            }
        }
    }

    fn roundtrip(server: &mut Server<FakeHost>, lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        serve(server, input.as_bytes(), &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn call(id: u64, name: &str, arguments: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
               "params": {"name": name, "arguments": arguments}})
    }

    fn text(reply: &Value) -> &str {
        reply["result"]["content"][0]["text"].as_str().unwrap()
    }

    fn payload(reply: &Value) -> Value {
        assert_eq!(reply["result"]["isError"], false, "{reply}");
        serde_json::from_str(text(reply)).unwrap()
    }

    fn connected(host: FakeHost, id: &str) -> Server<FakeHost> {
        let mut server = Server::new(host);
        let reply = server
            .handle(call(0, "bir_connect", json!({"id": id})))
            .unwrap();
        assert_eq!(payload(&reply)["connected"], true);
        server
    }

    #[test]
    fn protocol_roundtrip_initialize_list_connect_and_call() {
        let mut server = Server::new(FakeHost::single());
        let replies = roundtrip(
            &mut server,
            &[
                json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {"name": "test", "version": "0"}
                }}),
                json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
                json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
                call(4, "bir_list_apps", json!({})),
                call(5, "bir_connect", json!({"id": "bir-demo"})),
                call(
                    6,
                    "bir_form_open",
                    json!({"code": "2551Q", "year": 2026, "period": 1, "tin": "111111114000"}),
                ),
                call(
                    7,
                    "bir_form_fill",
                    json!({"fields": {"line_14": "1000.00"}, "source": "document"}),
                ),
                call(8, "bir_status", json!({})),
            ],
        );
        // The notification produced no line.
        assert_eq!(replies.len(), 8, "{replies:?}");
        assert!(replies.iter().all(|r| r["jsonrpc"] == "2.0"));

        let init = &replies[0];
        assert_eq!(init["id"], 1);
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "bir-mcp");
        assert!(init["result"]["capabilities"]["tools"].is_object());
        assert!(
            init["result"]["instructions"]
                .as_str()
                .unwrap()
                .contains("bir_list_apps")
        );

        let names: Vec<&str> = replies[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "bir_list_apps",
                "bir_connect",
                "bir_status",
                "bir_profiles_list",
                "bir_dues_list",
                "bir_form_open",
                "bir_form_fields",
                "bir_form_context",
                "bir_form_fill",
                "bir_form_needs_you",
                "bir_form_validate",
                "bir_form_preview",
                "bir_form_card",
                "bir_form_review",
                "bir_form_save_draft",
                "bir_form_dismiss",
            ]
        );
        assert!(replies[1]["result"]["tools"][0]["inputSchema"].is_object());

        assert_eq!(replies[2], json!({"jsonrpc": "2.0", "id": 3, "result": {}}));

        let listed = payload(&replies[3]);
        assert_eq!(listed["selected"], Value::Null);
        assert_eq!(
            listed["apps"],
            json!([{
                "id": "bir-demo", "pid": 11, "addr": "127.0.0.1:50011",
                "mode": "desktop", "token": "known", "database": "test",
                "taxpayers": [{"tin": "11111111400000", "name": "Test Taxpayer"}],
                "selected": false
            }])
        );
        assert!(listed["next"].as_str().unwrap().contains("ask"));

        let connect = payload(&replies[4]);
        assert_eq!(
            connect["app"],
            json!({"id": "bir-demo", "mode": "desktop", "database": "test"})
        );

        let opened = payload(&replies[5]);
        assert_eq!(
            opened,
            json!({"app": {"id": "bir-demo", "mode": "desktop", "database": "test"},
                   "result": {"form": "2551Q"}})
        );
        assert_eq!(payload(&replies[6])["app"]["id"], "bir-demo");
        let status = payload(&replies[7]);
        assert_eq!(status["connected"], true);
        assert_eq!(status["app"]["database"], "test");

        assert_eq!(
            server.host().host_calls(),
            vec![
                (
                    HostCall::Invoke("form.open"),
                    json!({"code": "2551Q", "year": 2026, "period": 1, "tin": "111111114000"})
                ),
                (
                    HostCall::Invoke("form.fill"),
                    json!({"fields": {"line_14": "1000.00"}, "source": "document"})
                ),
                (HostCall::Hello, json!({})),
            ]
        );
        assert!(
            server
                .host()
                .calls
                .iter()
                .all(|(id, _, _)| id == "bir-demo")
        );
    }

    #[test]
    fn every_bir_tool_refuses_until_an_app_is_selected_even_with_one_live() {
        let mut server = Server::new(FakeHost::single());
        for (i, tool) in tools::TOOLS.iter().enumerate() {
            if !matches!(tool.action, Action::Host(_)) {
                continue;
            }
            let args = match tool.name {
                "bir_form_open" => {
                    json!({"code": "1601C", "year": 2026, "period": 3, "tin": "111111114000"})
                }
                "bir_form_fill" => json!({"fields": {"k": "v"}, "source": "profile"}),
                _ => json!({}),
            };
            let reply = server.handle(call(i as u64, tool.name, args)).unwrap();
            assert_eq!(reply["result"]["isError"], true, "{}: {reply}", tool.name);
            assert_eq!(text(&reply), NOT_SELECTED, "{}", tool.name);
        }
        // Status says so without an error, and nothing reached BIR.
        let status = payload(&server.handle(call(99, "bir_status", json!({}))).unwrap());
        assert_eq!(status["connected"], false);
        assert_eq!(status["message"], NOT_SELECTED);
        assert!(server.host().calls.is_empty(), "{:?}", server.host().calls);
        assert!(server.selected().is_none());
    }

    #[test]
    fn list_apps_reads_app_info_only_where_a_token_is_known() {
        let host = FakeHost::with(vec![
            (app("bir-demo", 11, "desktop", true), "test"),
            (app("pid:12", 12, "headless", false), "real"),
            (app("bir-live", 13, "desktop", true), "real"),
        ]);
        let mut server = Server::new(host);
        let listed = payload(&server.handle(call(1, "bir_list_apps", json!({}))).unwrap());
        let apps = listed["apps"].as_array().unwrap();
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0]["database"], "test");
        assert_eq!(apps[1]["token"], "missing");
        assert_eq!(apps[1]["mode"], "headless");
        assert!(apps[1].get("database").is_none(), "{}", apps[1]);
        assert!(apps[1].get("taxpayers").is_none(), "{}", apps[1]);
        assert_eq!(apps[2]["database"], "real");
        assert_eq!(apps[2]["token"], "known");
        let reached: Vec<&str> = server
            .host()
            .calls
            .iter()
            .map(|(id, _, _)| id.as_str())
            .collect();
        assert_eq!(reached, ["bir-demo", "bir-live"]);
        // Listing never selects anything.
        assert!(server.selected().is_none());

        let empty = payload(
            &Server::new(FakeHost::with(vec![]))
                .handle(call(2, "bir_list_apps", json!({})))
                .unwrap(),
        );
        assert_eq!(empty["apps"], json!([]));
        assert!(empty["next"].as_str().unwrap().contains("No BIR app"));
    }

    #[test]
    fn connect_needs_a_token_and_confirm_real_for_a_real_database() {
        let host = FakeHost::with(vec![
            (app("bir-demo", 11, "desktop", true), "test"),
            (app("pid:12", 12, "headless", false), "test"),
            (app("bir-live", 13, "desktop", true), "real"),
        ]);
        let mut server = Server::new(host);

        let no_token = server
            .handle(call(1, "bir_connect", json!({"id": "pid:12"})))
            .unwrap();
        assert_eq!(no_token["result"]["isError"], true);
        assert!(text(&no_token).contains("BIR_MCP_TOKENS"), "{no_token}");
        assert!(server.selected().is_none());

        let real = server
            .handle(call(2, "bir_connect", json!({"id": "bir-live"})))
            .unwrap();
        assert_eq!(real["result"]["isError"], true);
        assert!(text(&real).contains("REAL"), "{real}");
        assert!(text(&real).contains("confirm_real"), "{real}");
        assert!(server.selected().is_none());

        let unknown = server
            .handle(call(3, "bir_connect", json!({"id": "bir-nope"})))
            .unwrap();
        assert_eq!(unknown["result"]["isError"], true);

        let confirmed = payload(
            &server
                .handle(call(
                    4,
                    "bir_connect",
                    json!({"id": "bir-live", "confirm_real": true}),
                ))
                .unwrap(),
        );
        assert_eq!(confirmed["app"]["database"], "real");
        let profiles = payload(
            &server
                .handle(call(5, "bir_profiles_list", json!({})))
                .unwrap(),
        );
        assert_eq!(profiles["app"]["id"], "bir-live");
        assert_eq!(profiles["app"]["database"], "real");

        // A failed switch leaves nothing selected, not the old app.
        let failed = server
            .handle(call(6, "bir_connect", json!({"id": "pid:12"})))
            .unwrap();
        assert_eq!(failed["result"]["isError"], true);
        assert!(server.selected().is_none());
        let refused = server
            .handle(call(7, "bir_profiles_list", json!({})))
            .unwrap();
        assert_eq!(text(&refused), NOT_SELECTED);

        // Switching to the test app works and is reported.
        let switched = payload(
            &server
                .handle(call(8, "bir_connect", json!({"id": "bir-demo"})))
                .unwrap(),
        );
        assert_eq!(switched["app"]["database"], "test");
        let listed = payload(&server.handle(call(9, "bir_list_apps", json!({}))).unwrap());
        assert_eq!(listed["selected"], "bir-demo");
        assert_eq!(listed["apps"][0]["selected"], true);
        assert_eq!(listed["apps"][2]["selected"], false);
    }

    #[test]
    fn a_vanished_app_clears_the_selection() {
        let mut server = connected(FakeHost::single(), "bir-demo");
        server.host.dead.push("bir-demo".into());
        let reply = server
            .handle(call(1, "bir_form_fields", json!({})))
            .unwrap();
        assert_eq!(reply["result"]["isError"], true);
        assert!(text(&reply).contains("no longer running"), "{reply}");
        assert!(server.selected().is_none());
        assert!(server.host().host_calls().is_empty());
    }

    #[test]
    fn host_refusals_become_tool_errors_with_bir_message_and_app_label() {
        let mut server = connected(FakeHost::single(), "bir-demo");
        let open = |id, code| {
            call(
                id,
                "bir_form_open",
                json!({"code": code, "year": 2026, "period": 1, "tin": "111111114000"}),
            )
        };
        let replies = roundtrip(
            &mut server,
            &[
                open(1, "2551Q"),
                call(2, "bir_form_fill", json!({"fields": {"line_14": 5}})),
                open(3, "1601C"),
                call(4, "bir_form_dismiss", json!({})),
                call(5, "bir_form_validate", json!({})),
            ],
        );
        assert_eq!(replies[2]["result"]["isError"], true);
        assert!(text(&replies[2]).contains("form already open"));
        assert!(
            text(&replies[2]).contains("[BIR app: bir-demo (desktop, test database)]"),
            "{}",
            text(&replies[2])
        );
        assert_eq!(replies[3]["result"]["isError"], true);
        assert!(text(&replies[3]).contains("unsaved edits"));
        let validated = payload(&replies[4]);
        assert_eq!(validated["result"]["field_errors"][0]["field"], "tin");
        assert_eq!(validated["app"]["database"], "test");
        // Default source is `ai`.
        assert_eq!(server.host().host_calls()[1].1["source"], "ai");
    }

    #[test]
    fn bad_arguments_and_unknown_tools_never_reach_the_host() {
        let mut server = connected(FakeHost::single(), "bir-demo");
        let before = server.host().calls.len();
        let replies = roundtrip(
            &mut server,
            &[
                call(
                    1,
                    "bir_form_fill",
                    json!({"fields": {"a": "1"}, "source": "user"}),
                ),
                call(
                    2,
                    "bir_form_open",
                    json!({"code": "2551Q", "year": 2026, "period": 1, "tin": "1", "confirm": true}),
                ),
                call(3, "bir_form_submit", json!({})),
                call(4, "form.queue", json!({})),
                call(5, "bir_form_validate", json!({"anything": 1})),
                json!({"jsonrpc": "2.0", "id": 6, "method": "resources/list"}),
                json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "bir_status", "arguments": [1]}}),
                call(8, "bir_connect", json!({"id": "bir-demo", "token": "x"})),
            ],
        );
        assert_eq!(replies.len(), 8);
        for reply in [
            &replies[0],
            &replies[1],
            &replies[4],
            &replies[6],
            &replies[7],
        ] {
            assert_eq!(reply["result"]["isError"], true, "{reply}");
        }
        assert!(text(&replies[0]).contains("source"));
        for reply in [&replies[2], &replies[3]] {
            assert_eq!(reply["error"]["code"], INVALID_PARAMS, "{reply}");
        }
        assert_eq!(replies[5]["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(
            server.host().calls.len(),
            before,
            "{:?}",
            server.host().calls
        );
        // A refused bir_connect argument check does not drop the selection.
        assert!(server.selected().is_some());
    }

    #[test]
    fn an_unlicensed_token_refuses_every_tool_before_the_host_is_touched() {
        use crate::license::{License, UNLICENSED};
        let server_stub = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr = server_stub.server_addr().to_ip().unwrap();
        let served = std::thread::spawn(move || {
            let request = server_stub.recv().unwrap();
            let body = r#"{"jsonrpc":"2.0","id":1,"result":{"isError":false,"structuredContent":{"profiles":[]}}}"#;
            request
                .respond(tiny_http::Response::from_string(body).with_status_code(200))
                .unwrap();
        });
        let mut server = Server::with_license(
            FakeHost::single(),
            License::required(format!("http://{addr}"), "agent-token"),
        );
        let reply = server.handle(call(1, "bir_list_apps", json!({}))).unwrap();
        assert_eq!(reply["result"]["isError"], true);
        assert_eq!(text(&reply), UNLICENSED);
        assert!(server.host().calls.is_empty());
        served.join().unwrap();
    }

    #[test]
    fn malformed_lines_get_jsonrpc_errors_and_blank_lines_are_ignored() {
        let mut server = Server::new(FakeHost::single());
        assert_eq!(server.handle_line("   "), None);
        let parse: Value = serde_json::from_str(&server.handle_line("{nope").unwrap()).unwrap();
        assert_eq!(parse["error"]["code"], PARSE_ERROR);
        assert_eq!(parse["id"], Value::Null);
        let batch: Value = serde_json::from_str(&server.handle_line("[]").unwrap()).unwrap();
        assert_eq!(batch["error"]["code"], INVALID_REQUEST);
        // Unknown protocol versions fall back to ours.
        let init = server
            .handle(json!({"jsonrpc": "2.0", "id": "a", "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}))
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], LATEST_PROTOCOL);
        assert_eq!(init["id"], "a");
    }

    #[test]
    fn every_tool_reaches_only_its_mapped_ops() {
        let mut server = connected(FakeHost::single(), "bir-demo");
        for (i, tool) in tools::TOOLS.iter().enumerate() {
            let args = match tool.name {
                "bir_form_open" => {
                    json!({"code": "1601C", "year": 2026, "period": 3, "tin": "111111114000"})
                }
                "bir_form_fill" => json!({"fields": {"k": "v"}, "source": "profile"}),
                "bir_connect" => json!({"id": "bir-demo"}),
                _ => json!({}),
            };
            let before = server.host().calls.len();
            // `bir_form_dismiss` is refused (unsaved fill); it still reached
            // only its own op.
            let reply = server.handle(call(i as u64, tool.name, args)).unwrap();
            assert!(reply.get("result").is_some(), "{reply}");
            let reached: Vec<&str> = server.host().calls[before..]
                .iter()
                .map(|(_, call, _)| call.op_name())
                .collect();
            assert!(!reached.is_empty(), "{} reached nothing", tool.name);
            let allowed = tool.action.ops();
            assert!(
                reached.iter().all(|op| allowed.contains(op)),
                "{} reached {reached:?}, allowed {allowed:?}",
                tool.name
            );
        }
    }
}
