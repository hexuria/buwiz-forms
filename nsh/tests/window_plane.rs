//! An external agent against the REAL NSH window: NSH's own control plane (the semantic tree, a press, typed text, a screenshot)
//! and this app's data plane (`app.info`, `profile.*`, `form.*`) whose UI commands the window applies. Needs a RUNNING app, so it
//! is ignored by default: `just nsh-agent-check` starts one (macOS, a real window, a scratch database, the host built with the
//! `agent` feature) and runs this against it.
//!
//! The window's plane is nsh-platform-macos' `gpui-agent-server` at `GPUI_AGENT_ADDR`; the data plane is a second listener of this
//! app, found through its discovery record (app `bir-shell`) in `GPUI_AGENT_RUNTIME_DIR`, as `bir-mcp` finds it.
#![cfg(all(feature = "agent", target_os = "macos"))]

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use gpui_agent::client::AgentClient;
use gpui_agent::discovery::{self, HostRecord};
use gpui_agent::protocol::Op;
use gpui_agent::tree::{UiNode, UiTree};
use serde_json::{Value, json};

const TIN: &str = "00012345300000";
const SHEETS: &str = "fp-frm2551Qv2018:txtSheets";

fn env(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} is not set: run this through `just nsh-agent-check`"))
}

fn flat<'a>(nodes: &'a [UiNode], out: &mut Vec<&'a UiNode>) {
    for node in nodes {
        out.push(node);
        flat(&node.children, out);
    }
}

fn node<'a>(tree: &'a UiTree, id: &str) -> Option<&'a UiNode> {
    let mut all = Vec::new();
    flat(&tree.nodes, &mut all);
    all.into_iter().find(|node| node.id == id)
}

/// Every node as `id|role|name|value`, for a failure message.
fn describe(tree: &UiTree) -> String {
    let mut all = Vec::new();
    flat(&tree.nodes, &mut all);
    all.iter()
        .map(|n| {
            format!(
                "{}|{}|{}|{}",
                n.id,
                n.role,
                n.name,
                n.value.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The window's tree now.
fn tree(ui: &mut AgentClient) -> UiTree {
    ui.snapshot().expect("snapshot").tree.expect("a tree")
}

/// Looks at the window until `ready` holds (a command takes a moment to be applied), and answers the tree it held on.
fn until(ui: &mut AgentClient, what: &str, ready: impl Fn(&UiTree) -> bool) -> UiTree {
    let started = Instant::now();
    loop {
        let now = tree(ui);
        if ready(&now) {
            return now;
        }
        if started.elapsed() >= Duration::from_secs(30) {
            let shot = std::env::temp_dir().join("nsh-agent-failure.png");
            let _ = ui.screenshot(shot.display().to_string());
            panic!(
                "the window never showed: {what} (what it showed: {})\n{}",
                shot.display(),
                describe(&now)
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn invoke(client: &mut AgentClient, name: &str, args: Value) -> Value {
    client
        .invoke(name, args)
        .unwrap_or_else(|error| panic!("invoke {name} failed: {error}"))
        .result
        .unwrap_or(Value::Null)
}

#[test]
#[ignore = "needs a running app: just nsh-agent-check"]
fn an_external_agent_reads_presses_types_and_shoots_the_window_and_its_commands_reach_it() {
    let token = env("GPUI_AGENT_TOKEN");
    let ui_addr: SocketAddr = env("GPUI_AGENT_ADDR").parse().expect("GPUI_AGENT_ADDR");
    let mut ui = AgentClient::connect(ui_addr)
        .with_timeout(Duration::from_secs(30))
        .with_token(token.clone());

    // The window's plane: who answers, and what is on screen.
    let hello = ui
        .expect_ok(Op::Hello)
        .expect("hello")
        .hello
        .expect("hello info");
    assert!(hello.ready);
    let first = tree(&mut ui);
    assert!(first.ready);

    // Press: a fresh scratch database starts on the first-run question; "Just me" leads to the New taxpayer page.
    assert!(
        node(&first, "setup-solo").is_some(),
        "a scratch database starts on the first-run question"
    );
    ui.click("setup-solo").expect("press Just me");
    until(&mut ui, "the New taxpayer page", |t| {
        node(t, "new-tp-name").is_some()
    });

    // Type: text lands in the box a person would type in, and the tree reads it back.
    ui.type_text("new-tp-name", "Juana Santos")
        .expect("type the name");
    let page = until(&mut ui, "the typed name", |t| {
        node(t, "new-tp-name").is_some_and(|n| n.value.as_deref() == Some("Juana Santos"))
    });
    assert_eq!(
        node(&page, "new-tp-name").map(|n| n.role.as_str()),
        Some("textfield")
    );

    // Screenshot: the real window, as a PNG.
    let png = std::env::temp_dir().join(format!("nsh-agent-window-{}.png", std::process::id()));
    ui.screenshot(png.display().to_string())
        .expect("screenshot");
    let bytes = std::fs::read(&png).expect("the screenshot file");
    let _ = std::fs::remove_file(&png);
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
    assert!(
        bytes.len() > 10_000,
        "an empty picture: {} bytes",
        bytes.len()
    );

    // The data plane: found as bir-mcp finds it (a discovery record of app bir-shell), the same token.
    let root = discovery::runtime_root();
    let record: HostRecord = std::fs::read_dir(root.join("hosts"))
        .expect("hosts folder")
        .flatten()
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<HostRecord>(&bytes).ok())
        .find(|record| record.app == "bir-shell")
        .expect("the app publishes a bir-shell record");
    assert_ne!(
        record.addr,
        ui_addr.to_string(),
        "two planes, two listeners"
    );
    let mut data = AgentClient::connect(record.addr.parse().expect("record address"))
        .with_timeout(Duration::from_secs(30))
        .with_token(token);

    let info = invoke(&mut data, "app.info", json!({}));
    assert_eq!(info["app"], "bir-shell");
    assert_eq!(info["database"], "test");
    invoke(
        &mut data,
        "profile.save",
        json!({"tin": TIN, "name": "Juana Santos", "rdo_code": "039", "address": "Quezon City", "zip_code": "1100", "phone": "09170000000", "email": "juana@example.com"}),
    );
    assert_eq!(
        invoke(&mut data, "profile.list", json!({}))[0]["name"],
        "Juana Santos"
    );

    // Its UI commands reach the window: a view change, then a form opened and a box filled.
    invoke(&mut data, "nav.go", json!({"page": "filings"}));
    until(&mut ui, "the Filings page", |t| {
        node(t, "nav-filings").is_some() && node(t, "new-tp-name").is_none()
    });
    let year = 2025;
    invoke(
        &mut data,
        "form.open",
        json!({"code": "2551Q", "year": year, "period": 1, "tin": TIN}),
    );
    until(&mut ui, "the 2551Q on the form page", |t| {
        node(t, "fp-queue").is_some()
    });
    ui.click("fp-parts-toggle").expect("press Expand all parts");
    invoke(
        &mut data,
        "form.fill",
        json!({"fields": {"frm2551Qv2018:txtSheets": "3"}, "source": "ai"}),
    );
    let form = until(&mut ui, "the filled box", |t| {
        node(t, SHEETS).is_some_and(|n| n.value.as_deref() == Some("3"))
    });
    assert_eq!(
        node(&form, SHEETS).map(|n| n.value.clone()),
        Some(Some("3".to_owned()))
    );
}
