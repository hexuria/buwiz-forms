//! MCP over stdio: JSON-RPC 2.0, one message per line. Implements
//! `initialize`, `notifications/initialized`, `ping`, `tools/list` and
//! `tools/call`. stdout carries protocol messages only; diagnostics go to
//! stderr.
use crate::tools::{self, HostCall};
use serde_json::{Map, Value, json};
use std::io::{BufRead, Write};

pub const SERVER_NAME: &str = "bir-mcp";
pub const LATEST_PROTOCOL: &str = "2025-06-18";
pub const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

const INSTRUCTIONS: &str = "Draft-only access to the BIR desktop app for forms 2551Q and 1601C. \
You can read taxpayer profiles, dues and form context, fill boxes with a stated source, list the \
boxes that need the user, validate, save a draft and dismiss. You cannot submit, queue, file or \
pay: the user does that in BIR. Only one form is open at a time.";

/// The BIR side, as seen by the protocol layer. `call` only ever receives a
/// [`HostCall`] taken from the closed tool table. `Err` carries BIR's own
/// message (or why BIR could not be reached), shown to the model verbatim.
pub trait BirHost {
    fn call(&mut self, call: HostCall, args: Value) -> Result<Value, String>;
}

pub struct Server<H> {
    host: H,
}

impl<H: BirHost> Server<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }

    pub fn host(&self) -> &H {
        &self.host
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
        Ok(match self.host.call(tool.call, args) {
            Ok(value) => tool_result(
                serde_json::to_string_pretty(&value).expect("json value"),
                false,
            ),
            Err(message) => tool_result(message, true),
        })
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

    /// Stands in for BIR: records every call and answers like the host
    /// contract in `.claude/GOAL.md`.
    #[derive(Default)]
    struct FakeHost {
        calls: Vec<(HostCall, Value)>,
        open_form: Option<String>,
        dirty: bool,
    }

    impl BirHost for FakeHost {
        fn call(&mut self, call: HostCall, args: Value) -> Result<Value, String> {
            self.calls.push((call, args.clone()));
            match call {
                HostCall::Hello => Ok(json!({"connected": true, "hello": {"app": "bir-desktop"}})),
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

    #[test]
    fn protocol_roundtrip_initialize_list_and_call() {
        let mut server = Server::new(FakeHost::default());
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
                call(
                    4,
                    "bir_form_open",
                    json!({"code": "2551Q", "year": 2026, "period": 1, "tin": "111111114000"}),
                ),
                call(
                    5,
                    "bir_form_fill",
                    json!({"fields": {"line_14": "1000.00"}, "source": "document"}),
                ),
            ],
        );
        // The notification produced no line.
        assert_eq!(replies.len(), 5, "{replies:?}");
        assert!(replies.iter().all(|r| r["jsonrpc"] == "2.0"));

        let init = &replies[0];
        assert_eq!(init["id"], 1);
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "bir-mcp");
        assert!(init["result"]["capabilities"]["tools"].is_object());

        let names: Vec<&str> = replies[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "bir_status",
                "bir_profiles_list",
                "bir_dues_list",
                "bir_form_open",
                "bir_form_fields",
                "bir_form_context",
                "bir_form_fill",
                "bir_form_needs_you",
                "bir_form_validate",
                "bir_form_save_draft",
                "bir_form_dismiss",
            ]
        );
        assert!(replies[1]["result"]["tools"][0]["inputSchema"].is_object());

        assert_eq!(replies[2], json!({"jsonrpc": "2.0", "id": 3, "result": {}}));

        assert_eq!(replies[3]["id"], 4);
        assert_eq!(replies[3]["result"]["isError"], false);
        assert_eq!(
            serde_json::from_str::<Value>(text(&replies[3])).unwrap(),
            json!({"form": "2551Q"})
        );
        assert_eq!(replies[4]["result"]["isError"], false);

        assert_eq!(
            server.host().calls,
            vec![
                (
                    HostCall::Invoke("form.open"),
                    json!({"code": "2551Q", "year": 2026, "period": 1, "tin": "111111114000"})
                ),
                (
                    HostCall::Invoke("form.fill"),
                    json!({"fields": {"line_14": "1000.00"}, "source": "document"})
                ),
            ]
        );
    }

    #[test]
    fn host_refusals_become_tool_errors_with_bir_message() {
        let mut server = Server::new(FakeHost::default());
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
        assert_eq!(replies[3]["result"]["isError"], true);
        assert!(text(&replies[3]).contains("unsaved edits"));
        let validated: Value = serde_json::from_str(text(&replies[4])).unwrap();
        assert_eq!(validated["field_errors"][0]["field"], "tin");
        // Default source is `ai`.
        assert_eq!(server.host().calls[1].1["source"], "ai");
    }

    #[test]
    fn bad_arguments_and_unknown_tools_never_reach_the_host() {
        let mut server = Server::new(FakeHost::default());
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
            ],
        );
        assert_eq!(replies.len(), 7);
        for reply in [&replies[0], &replies[1], &replies[4], &replies[6]] {
            assert_eq!(reply["result"]["isError"], true, "{reply}");
        }
        assert!(text(&replies[0]).contains("source"));
        for reply in [&replies[2], &replies[3]] {
            assert_eq!(reply["error"]["code"], INVALID_PARAMS, "{reply}");
        }
        assert_eq!(replies[5]["error"]["code"], METHOD_NOT_FOUND);
        assert!(server.host().calls.is_empty(), "{:?}", server.host().calls);
    }

    #[test]
    fn malformed_lines_get_jsonrpc_errors_and_blank_lines_are_ignored() {
        let mut server = Server::new(FakeHost::default());
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
    fn every_tool_reaches_only_its_mapped_op() {
        let mut server = Server::new(FakeHost::default());
        for (i, tool) in tools::TOOLS.iter().enumerate() {
            let args = match tool.name {
                "bir_form_open" => {
                    json!({"code": "1601C", "year": 2026, "period": 3, "tin": "111111114000"})
                }
                "bir_form_fill" => json!({"fields": {"k": "v"}, "source": "profile"}),
                _ => json!({}),
            };
            let reply = server.handle(call(i as u64, tool.name, args)).unwrap();
            assert!(reply.get("result").is_some(), "{reply}");
            assert_eq!(
                server.host().calls.last().unwrap().0,
                tool.call,
                "{}",
                tool.name
            );
        }
        assert_eq!(server.host().calls.len(), tools::TOOLS.len());
    }
}
