//! The NSH host's agent remote control, end to end: an external client (the way `bir-mcp` is one: gpui-agent's
//! `AgentClient`, a token, `hello` and `invoke`) against the listener of `nsh_app_ebirforms::agent`, on a scratch
//! database. The test thread is the engine thread: it boots the door, owns the database slot and drains the mailbox,
//! as the task `Agent.start()` spawns does in the app. Needs `--features agent`.
//!
//! The send safety test is its own file (`agent_send.rs`): it needs a process that starts with a live send asked for.
#![cfg(feature = "agent")]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use gpui_agent::client::AgentClient;
use gpui_agent::discovery::HostRecord;
use gpui_agent::protocol::{Op, PlatformKind};
use nsh_app_ebirforms::agent::{self, Agent, Listener};
use serde_json::{Value, json};

const TOKEN: &str = "nsh-agent-test-token";
const TIN: &str = "00012345300000";
/// "5 Number of Sheet/s Attached" on the 2551Q: a plain text box.
const SHEETS: &str = "frm2551Qv2018:txtSheets";

/// One scenario at a time: they share the process environment and the one door.
static SCENARIO: Mutex<()> = Mutex::new(());

/// The dev environment of the plan plus the agent opt-in, with discovery records in a folder of their own.
fn agent_env<R>(extra: &[(&str, Option<&str>)], runtime: &Path, body: impl FnOnce() -> R) -> R {
    let _one = SCENARIO
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime = runtime.display().to_string();
    let mut vars: Vec<(&str, Option<&str>)> = vec![
        ("GPUI_AGENT", Some("1")),
        ("GPUI_AGENT_TOKEN", Some(TOKEN)),
        ("GPUI_AGENT_ADDR", Some("127.0.0.1:0")),
        ("GPUI_AGENT_RUNTIME_DIR", Some(&runtime)),
        ("GPUI_AGENT_ALLOW_RELEASE", Some("1")),
        ("GPUI_AGENT_INSECURE_NO_TOKEN", None),
        ("GPUI_AGENT_REMOTE", None),
        ("GPUI_AGENT_CONNECT_ID", None),
        ("BUWIZ_NO_KEYCHAIN", Some("1")),
        ("EBIR_TEST_ENV", Some("1")),
        ("BIR_AUTH_BYPASS", Some("1")),
        ("BIR_SFTP_DRY_RUN", Some("1")),
        ("BIR_SFTP_LIVE", None),
        ("BIR_SFTP_HOST", None),
        ("BIR_DATABASE_PATH", None),
    ];
    for (key, value) in extra {
        vars.retain(|(name, _)| name != key);
        vars.push((key, *value));
    }
    temp_env::with_vars(vars, body)
}

/// Run `client` on a thread of its own while this one, the engine thread, drains the mailbox.
fn with_client<R: Send + 'static>(
    listener: &Listener,
    client: impl FnOnce(SocketAddr) -> R + Send + 'static,
) -> R {
    let addr = listener.address();
    let worker = thread::spawn(move || client(addr));
    let started = Instant::now();
    while !worker.is_finished() {
        listener.drain();
        thread::sleep(Duration::from_millis(5));
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the client never finished"
        );
    }
    worker.join().expect("the client thread")
}

fn connect(addr: SocketAddr) -> AgentClient {
    AgentClient::connect(addr)
        .with_timeout(Duration::from_secs(20))
        .with_token(TOKEN)
}

fn invoke(client: &mut AgentClient, name: &str, args: Value) -> Value {
    client
        .invoke(name, args)
        .unwrap_or_else(|error| panic!("invoke {name} failed: {error}"))
        .result
        .unwrap_or(Value::Null)
}

/// The frame is up, nobody stands in front of it and no taxpayer is signed in: what the root tells the host (`Agent.setAccess`) once the
/// app has booted and signed in. The agent is refused until it does. The state belongs to the engine thread, which is the test's own.
fn open_app() {
    Agent::set_access(false, false);
}

/// What the frame would read through `agent_take`, until the queue is empty: the hook the host function calls.
fn queued_commands() -> Vec<Value> {
    let mut commands = Vec::new();
    while let Some(command) = bir_app_api::hooks().agent_pop_command() {
        commands.push(command);
    }
    commands
}

#[test]
fn an_external_agent_creates_the_demo_taxpayer_opens_a_2551q_fills_a_box_and_reads_it_back() {
    let runtime = tempfile::tempdir().expect("runtime folder");
    agent_env(&[], runtime.path(), || {
        assert!(
            !Agent::status().active,
            "nothing listens before the app starts it"
        );
        let listener = agent::listen()
            .expect("not refused")
            .expect("GPUI_AGENT=1 starts it");
        open_app();
        assert!(listener.address().ip().is_loopback());
        assert_ne!(
            listener.address().port(),
            0,
            "the real port, not the one asked for"
        );

        let (hello, info, saved, listed, opened, filled, fields, nav) = with_client(
            &listener,
            |addr| {
                let mut client = connect(addr);
                let hello = client
                    .expect_ok(Op::Hello)
                    .expect("hello")
                    .hello
                    .expect("hello info");
                let info = invoke(&mut client, "app.info", json!({}));
                let saved = invoke(
                    &mut client,
                    "profile.save",
                    json!({"tin": TIN, "name": "Juana Santos", "rdo_code": "039", "address": "Quezon City", "zip_code": "1100"}),
                );
                let listed = invoke(&mut client, "profile.list", json!({}));
                let opened = invoke(
                    &mut client,
                    "form.open",
                    json!({"code": "2551Q", "year": 2025, "period": 1, "tin": TIN}),
                );
                let filled = invoke(
                    &mut client,
                    "form.fill",
                    json!({"fields": {SHEETS: "3"}, "source": "ai"}),
                );
                let fields = invoke(&mut client, "form.fields", json!({}));
                let nav = invoke(&mut client, "nav.go", json!({"page": "filings"}));
                (hello, info, saved, listed, opened, filled, fields, nav)
            },
        );

        // The listener speaks the old app's protocol, as the app and, for bir-mcp, as a desktop BIR host.
        assert_eq!(hello.app, "bir-shell");
        assert_eq!(hello.platform, PlatformKind::Desktop);
        assert!(hello.ready);
        assert_eq!(info["app"], "bir-shell");
        assert_eq!(info["mode"], "desktop");
        assert_eq!(info["database"], "test");

        // The demo taxpayer is created through the agent...
        assert_eq!(saved["created"], true, "{saved}");
        assert_eq!(listed[0]["tin"], TIN);
        assert_eq!(listed[0]["name"], "Juana Santos");
        // ...a 2551Q opens, a box is filled, and the same box reads back.
        assert_eq!(opened["form"], "2551Q");
        assert_eq!(filled["filled"][0], SHEETS, "{filled}");
        let sheets = fields["boxes"]
            .as_array()
            .and_then(|boxes| boxes.iter().find(|card| card["key"] == SHEETS))
            .expect("the box is on the card");
        assert_eq!(sheets["value"], "3");
        assert_eq!(sheets["source"], "ai");
        assert_eq!(nav["view"], "filings");

        // It is the door's database: the app reads the same taxpayer the agent created.
        let people =
            nsh_app_ebirforms::door::call("list_taxpayers", Vec::new()).expect("the door answers");
        assert_eq!(people[0]["name"], "Juana Santos", "{people}");

        // What the frame is asked to show, in order, through the host function's hook.
        let commands = queued_commands();
        let ops: Vec<&str> = commands
            .iter()
            .map(|command| command["op"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(
            ops,
            ["refresh", "refresh", "open", "choose", "go"],
            "{commands:?}"
        );
        assert_eq!(commands[2]["code"], "2551Q");
        assert_eq!(commands[2]["tin"], TIN);
        assert_eq!(commands[3]["edits"][SHEETS], "3");
        assert_eq!(commands[4]["view"], "filings");
        assert_eq!(
            bir_app_api::hooks().agent_pop_command(),
            None,
            "the queue is empty"
        );

        // The discovery record is the one bir-mcp lists: app `bir-shell`, executable `bir`, the bound address.
        let records: Vec<HostRecord> = std::fs::read_dir(runtime.path().join("hosts"))
            .expect("hosts folder")
            .flatten()
            .filter_map(|entry| std::fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect();
        assert_eq!(records.len(), 1, "{records:?}");
        assert_eq!(records[0].app, "bir-shell");
        assert_eq!(records[0].executable, "bir");
        assert_eq!(records[0].addr, listener.address().to_string());
        assert_eq!(records[0].pid, std::process::id());

        // The session's shutdown becomes the frame's quit.
        let quit = with_client(&listener, |addr| {
            connect(addr).expect_ok(Op::Shutdown).is_ok()
        });
        assert!(quit);
        assert_eq!(
            queued_commands()
                .last()
                .map(|command| command["op"].clone()),
            Some(json!("quit"))
        );
    });
}

#[test]
fn the_listener_refuses_a_client_without_the_token_and_a_wrong_one() {
    let runtime = tempfile::tempdir().expect("runtime folder");
    agent_env(&[], runtime.path(), || {
        let listener = agent::listen().expect("not refused").expect("started");
        open_app();
        let (none, wrong, right) = with_client(&listener, |addr| {
            let short = Duration::from_secs(5);
            let none = AgentClient::connect(addr)
                .with_timeout(short)
                .rpc(Op::Hello);
            let wrong = AgentClient::connect(addr)
                .with_timeout(short)
                .with_token("not-the-token")
                .rpc(Op::Hello);
            let right = connect(addr).rpc(Op::Hello);
            (none, wrong, right)
        });
        for (who, answer) in [("no token", none), ("a wrong token", wrong)] {
            assert!(
                answer.as_ref().map_or(true, |response| !response.ok),
                "{who} was let in: {answer:?}"
            );
        }
        assert!(right.expect("the right token connects").ok);
    });
}

#[test]
fn the_listener_answers_nothing_while_the_app_is_locked_or_a_taxpayer_is_signed_in() {
    let runtime = tempfile::tempdir().expect("runtime folder");
    agent_env(&[], runtime.path(), || {
        let listener = agent::listen().expect("not refused").expect("started");
        let hello = |listener: &Listener| {
            with_client(listener, |addr| {
                let response = connect(addr).rpc(Op::Hello).expect("the listener answers");
                (response.ok, response.error)
            })
        };
        let refused = |answer: (bool, Option<String>), sentence: &str| {
            assert!(!answer.0, "the request was let in: {answer:?}");
            assert_eq!(answer.1.as_deref(), Some(sentence));
        };
        let data = |listener: &Listener| {
            with_client(listener, |addr| {
                let mut client = connect(addr);
                client.invoke("profile.list", json!({})).map(|response| response.result)
            })
        };

        // Before the frame has said anything the app counts as locked: nothing is answered.
        refused(hello(&listener), agent::LOCKED);
        assert!(data(&listener).is_err(), "no data while locked");
        // The sign-in screen (or the Buwiz gate) is in front: still locked.
        Agent::set_access(true, false);
        refused(hello(&listener), agent::LOCKED);
        // The person signs in: the frame is open.
        open_app();
        assert!(hello(&listener).0, "an open frame answers");
        assert!(data(&listener).is_ok());
        // A taxpayer signs in: the tools are not limited to one taxpayer's data, so none is answered.
        Agent::set_access(false, true);
        refused(hello(&listener), agent::TAXPAYER_SESSION_REFUSED);
        assert!(data(&listener).is_err(), "no data in a taxpayer's session");
        // They sign out and the owner signs in.
        Agent::set_access(false, false);
        assert!(hello(&listener).0);
        // Locked again (Lock app, Log out).
        Agent::set_access(true, false);
        refused(hello(&listener), agent::LOCKED);
        // Nothing the refused requests asked for was queued for the frame.
        assert!(queued_commands().is_empty());
    });
}

#[test]
fn it_starts_only_when_asked_for_with_a_token_and_on_loopback() {
    let runtime = tempfile::tempdir().expect("runtime folder");
    // Not opted in: nothing listens, nothing is written.
    agent_env(&[("GPUI_AGENT", None)], runtime.path(), || {
        assert!(agent::listen().expect("off is not an error").is_none());
        assert!(!runtime.path().join("hosts").exists());
    });
    // Opted in with no token: refused, as the old app does.
    agent_env(&[("GPUI_AGENT_TOKEN", None)], runtime.path(), || {
        let error = agent::listen()
            .err()
            .expect("a loopback listener needs a token");
        assert!(error.contains("GPUI_AGENT_TOKEN"), "{error}");
    });
    // The demo opt-out of the token is the protocol's own.
    agent_env(
        &[
            ("GPUI_AGENT_TOKEN", None),
            ("GPUI_AGENT_INSECURE_NO_TOKEN", Some("1")),
        ],
        runtime.path(),
        || {
            assert!(agent::listen().expect("insecure demo").is_some());
        },
    );
    // Not loopback: refused even with a token and the protocol's remote opt-in. The NSH host never listens off this computer.
    agent_env(
        &[
            ("GPUI_AGENT_ADDR", Some("0.0.0.0:0")),
            ("GPUI_AGENT_REMOTE", Some("1")),
        ],
        runtime.path(),
        || {
            let error = agent::listen().err().expect("loopback only");
            assert!(error.contains("loopback"), "{error}");
        },
    );
}
