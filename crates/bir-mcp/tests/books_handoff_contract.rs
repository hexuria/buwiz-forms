//! Books -> Desktop Buwiz Forms (`bir-mcp`) hand-off contract tests.
//!
//! Pins the exact 5 fixture files shared with Buwiz Books (`tests/fixtures/handoff/`)
//! and eBIRForms (`tests/fixtures/books-handoff/`) by `CONTRACT_SHA256`, and runs
//! all three form hand-offs (2551Q, 1601C, 1701MS) end-to-end through `bir-mcp`
//! into a real in-process `bir_app_api::agent::Session` + ephemeral `Database`.

use bir_app_api::agent::{Session, UiQueue};
use bir_core::db::Database;
use bir_mcp::server::{AppRecord, BirHost, Server};
use bir_mcp::tickets::is_ticket;
use bir_mcp::tools::HostCall;
use gpui_agent::AgentHost;
use gpui_agent::protocol::Op;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// SHA-256 over the five shared hand-off fixture files (sorted by name, each
/// `name\0bytes\0`). Must equal `CONTRACT_SHA256` in Books `tests/handoff_contract.rs`
/// and eBIRForms `tests/fill_sources.rs`.
const CONTRACT_SHA256: &str = "f5bdd660adf30fc98d65f7e5b950fca059afef5fbb483d1a0a8e6da003e721bd";

const FIXTURE_FILES: [&str; 5] = [
    "1601c.json",
    "1701ms.json",
    "2551q.json",
    "amounts.json",
    "form-tools.schema.json",
];

const SYNTHETIC_TIN: &str = "12345678800000";

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/books-handoff")
}

struct RealSessionHost {
    app: AppRecord,
    session: Session,
}

impl RealSessionHost {
    fn new() -> Self {
        let db = Database::open_ephemeral().expect("ephemeral db");
        let mut session = Session::new(UiQueue::new());
        session.set_db(Arc::new(Mutex::new(db)));
        session
            .dispatch(&Op::Invoke {
                name: "profile.save".into(),
                args: json!({
                    "tin": SYNTHETIC_TIN,
                    "rdo": "044",
                    "name": "BUWIZ TRADING TEST CORP",
                    "line_of_business": "GENERAL MERCHANDISE",
                    "address": "123 TEST AVE MAKATI",
                    "zip": "1200",
                    "phone": "09171234567",
                    "email": "test@example.invalid"
                }),
            })
            .expect("seed synthetic taxpayer");
        Self {
            app: AppRecord {
                id: "bir-test".into(),
                connect_id: Some("bir-test".into()),
                pid: 4242,
                addr: "127.0.0.1:54242".into(),
                mode: "desktop".into(),
                token_known: true,
            },
            session,
        }
    }
}

impl BirHost for RealSessionHost {
    fn apps(&mut self) -> Result<Vec<AppRecord>, String> {
        Ok(vec![self.app.clone()])
    }

    fn resolve(&mut self, id: &str) -> Result<AppRecord, String> {
        if id == self.app.id {
            Ok(self.app.clone())
        } else {
            Err(format!("unknown app `{id}`"))
        }
    }

    fn alive(&mut self, app: &AppRecord) -> bool {
        app.id == self.app.id
    }

    fn call(&mut self, _app: &AppRecord, call: HostCall, args: Value) -> Result<Value, String> {
        let op = match call {
            HostCall::Hello => Op::Hello,
            HostCall::Invoke(name) => Op::Invoke {
                name: name.to_string(),
                args,
            },
        };
        let mut value = self.session.dispatch(&op)?.value.unwrap_or(Value::Null);
        // Mark test database explicitly so bir_connect does not require confirm_real.
        if call == HostCall::Invoke("app.info")
            && let Some(obj) = value.as_object_mut()
        {
            obj.insert("database".into(), json!("test"));
        }
        Ok(value)
    }
}

fn call_tool(server: &mut Server<RealSessionHost>, id: u64, name: &str, args: Value) -> Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": args }
    });
    server.handle(req).expect("tool response")
}

fn ok_json(reply: &Value, label: &str) -> Value {
    assert_eq!(reply["result"]["isError"], false, "{label}: {reply}");
    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .expect("tool text");
    // Privacy invariant: raw synthetic TIN must never cross MCP.
    assert!(
        !text.contains(SYNTHETIC_TIN) && !text.contains("123456788"),
        "{label} leaked raw TIN on MCP wire: {text}"
    );
    serde_json::from_str(text).expect("tool json")
}

#[test]
fn handoff_fixtures_match_pinned_books_contract_sha256() {
    let dir = fixtures_dir();
    let mut hasher = Sha256::new();
    for name in FIXTURE_FILES {
        let bytes = std::fs::read(dir.join(name)).expect(name);
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(&bytes);
        hasher.update([0]);
    }
    let digest = format!("{:x}", hasher.finalize());
    assert_eq!(
        digest, CONTRACT_SHA256,
        "Books hand-off fixtures drifted from CONTRACT_SHA256"
    );
}

#[test]
fn books_handoff_fixtures_open_fill_and_preserve_source_ref_without_raw_tin() {
    let mut server = Server::new(RealSessionHost::new());

    // 1. List apps and connect -> receive opaque taxpayer ticket, never raw TIN.
    let listed = ok_json(
        &call_tool(&mut server, 1, "bir_list_apps", json!({})),
        "list_apps",
    );
    let ticket = listed["apps"][0]["taxpayers"][0]["tin"]
        .as_str()
        .expect("ticket")
        .to_string();
    assert!(is_ticket(&ticket), "expected tkt_... ticket, got {ticket}");

    let connected = ok_json(
        &call_tool(&mut server, 2, "bir_connect", json!({"id": "bir-test"})),
        "connect",
    );
    assert_eq!(connected["taxpayers"][0]["tin"], ticket);

    let profiles = ok_json(
        &call_tool(&mut server, 3, "bir_profiles_list", json!({})),
        "profiles_list",
    );
    assert_eq!(profiles["result"][0]["tin"], ticket);

    // 2. Raw TIN is refused at bir_form_open.
    let raw_tin_reply = call_tool(
        &mut server,
        4,
        "bir_form_open",
        json!({"code": "2551Q", "year": 2026, "period": 3, "tin": SYNTHETIC_TIN}),
    );
    assert_eq!(raw_tin_reply["result"]["isError"], true);

    // 3. Run all three Books hand-off fixtures end-to-end.
    let mut req_id = 10u64;
    for fixture_name in ["2551q.json", "1601c.json", "1701ms.json"] {
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string(fixtures_dir().join(fixture_name)).expect(fixture_name),
        )
        .expect("valid json");

        let mut open_args = fixture["form_open"].clone();
        open_args
            .as_object_mut()
            .unwrap()
            .insert("tin".into(), Value::String(ticket.clone()));
        let opened = ok_json(
            &call_tool(&mut server, req_id, "bir_form_open", open_args),
            &format!("{fixture_name} open"),
        );
        req_id += 1;
        assert_eq!(opened["result"]["tin"], ticket);

        let fill_args = fixture["form_fill"].clone();
        let expected_fields = fill_args["fields"].as_object().unwrap().clone();
        let expected_source_ref = fill_args["source_ref"].clone();

        let filled = ok_json(
            &call_tool(&mut server, req_id, "bir_form_fill", fill_args),
            &format!("{fixture_name} fill"),
        );
        req_id += 1;
        assert!(
            filled["result"].get("refused").is_none(),
            "{fixture_name} refused fields: {filled}"
        );
        assert_eq!(filled["result"]["source"], "document");
        assert_eq!(filled["result"]["source_ref"], expected_source_ref);

        let fields_out = ok_json(
            &call_tool(&mut server, req_id, "bir_form_fields", json!({})),
            &format!("{fixture_name} fields"),
        );
        req_id += 1;
        let boxes = fields_out["result"]["boxes"]
            .as_array()
            .expect("boxes array");
        for key in expected_fields.keys() {
            let entry = boxes
                .iter()
                .find(|b| b["field"].as_str() == Some(key.as_str()))
                .unwrap_or_else(|| {
                    panic!("{fixture_name}: missing box `{key}` in bir_form_fields")
                });
            assert_eq!(
                entry["source"], "document",
                "{fixture_name} box `{key}` source: {entry}"
            );
            assert_eq!(
                entry["source_ref"], expected_source_ref,
                "{fixture_name} box `{key}` source_ref: {entry}"
            );
        }

        // Verify overwriting one box without source_ref drops source_ref for that box.
        let first_key = expected_fields.keys().next().unwrap().clone();
        let first_val = expected_fields.get(&first_key).unwrap().clone();
        let _ = ok_json(
            &call_tool(
                &mut server,
                req_id,
                "bir_form_fill",
                json!({"fields": { first_key.clone(): first_val }, "source": "ai"}),
            ),
            &format!("{fixture_name} overwrite"),
        );
        req_id += 1;
        let after_overwrite = ok_json(
            &call_tool(&mut server, req_id, "bir_form_fields", json!({})),
            &format!("{fixture_name} fields after overwrite"),
        );
        req_id += 1;
        let overwritten_box = after_overwrite["result"]["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["field"].as_str() == Some(first_key.as_str()))
            .unwrap();
        assert_eq!(overwritten_box["source"], "ai");
        assert!(
            overwritten_box.get("source_ref").is_none(),
            "{fixture_name} box `{first_key}` kept stale source_ref: {overwritten_box}"
        );

        // Save draft & dismiss so the next form can open cleanly.
        let _ = ok_json(
            &call_tool(&mut server, req_id, "bir_form_save_draft", json!({})),
            &format!("{fixture_name} save_draft"),
        );
        req_id += 1;
        let _ = ok_json(
            &call_tool(&mut server, req_id, "bir_form_dismiss", json!({})),
            &format!("{fixture_name} dismiss"),
        );
        req_id += 1;
    }
}
