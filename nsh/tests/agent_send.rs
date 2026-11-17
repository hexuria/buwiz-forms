//! Send safety of the agent's queue request in the NSH host. Its own file, so its own process: the door's send guard
//! decides once per process, and this one starts the way a careless launch would, with a live send asked for.
#![cfg(feature = "agent")]

use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

use gpui_agent::client::AgentClient;
use nsh_app_ebirforms::agent::{self, Listener};
use nsh_app_ebirforms::door;
use nsh_app_ebirforms::guards::SendMode;
use serde_json::{Value, json};

const TOKEN: &str = "nsh-agent-send-token";
const TIN: &str = "00012345300000";

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

fn invoke(client: &mut AgentClient, name: &str, args: Value) -> Value {
    client
        .invoke(name, args)
        .unwrap_or_else(|error| panic!("invoke {name} failed: {error}"))
        .result
        .unwrap_or(Value::Null)
}

#[test]
fn an_agent_queued_return_goes_out_as_a_dry_run_the_door_forces_never_as_a_live_send() {
    let folder = tempfile::tempdir().expect("temp folder");
    let runtime = folder.path().join("runtime");
    let database = folder.path().join("data").join("bir_data.db");
    std::fs::create_dir_all(database.parent().expect("a folder")).expect("data folder");
    let (runtime, database) = (
        runtime.display().to_string(),
        database.display().to_string(),
    );
    temp_env::with_vars(
        [
            ("GPUI_AGENT", Some("1")),
            ("GPUI_AGENT_TOKEN", Some(TOKEN)),
            ("GPUI_AGENT_ADDR", Some("127.0.0.1:0")),
            ("GPUI_AGENT_RUNTIME_DIR", Some(runtime.as_str())),
            ("GPUI_AGENT_ALLOW_RELEASE", Some("1")),
            ("BUWIZ_NO_KEYCHAIN", Some("1")),
            // Not a test environment: nothing forces a dry run but the door's own guard.
            ("EBIR_TEST_ENV", None),
            ("BIR_AUTH_BYPASS", None),
            // A careless launch: a live acknowledgement, a host that is not this computer, no dry-run flag.
            ("BIR_SFTP_LIVE", Some("1")),
            ("BIR_SFTP_DRY_RUN", None),
            ("BIR_SFTP_HOST", Some("192.0.2.1")),
            ("BIR_SFTP_PORT", None),
            ("BIR_DATABASE_PATH", Some(database.as_str())),
        ],
        || {
            assert!(
                !bir_core::transport::is_sftp_dry_run(),
                "the setup must ask for a live send, or this test proves nothing"
            );

            // Starting the listener boots the door first, and the door's send guard runs there.
            let listener = agent::listen().expect("not refused").expect("started");
            assert_eq!(door::send_mode(), &SendMode::DryRun);
            assert!(
                bir_core::transport::is_sftp_dry_run(),
                "the transport is a dry run now"
            );
            assert!(
                std::env::var_os("BIR_SFTP_LIVE").is_none(),
                "the live acknowledgement is gone"
            );
            assert_eq!(std::env::var("BIR_SFTP_DRY_RUN").as_deref(), Ok("1"));

            // The frame is up (the agent is refused while the app is locked), and the sign-in opens the database and
            // starts the submission worker, which the guard allowed.
            agent::Agent::set_access(false, false);
            bir_app_api::account_gate::set_unlocked(true);
            bir_app_api::bir_module::open_database_at_launch();

            with_client(&listener, |addr| {
                let mut client = AgentClient::connect(addr)
                    .with_timeout(Duration::from_secs(20))
                    .with_token(TOKEN);
                invoke(
                    &mut client,
                    "profile.save",
                    json!({"tin": TIN, "name": "Juana Santos", "rdo_code": "039", "address": "Quezon City", "zip_code": "1100", "phone": "09170000000", "email": "juana@example.com"}),
                );
            });
            // What the person does on the taxpayer page: the year's facts and the standing OK for this form.
            door::call(
                "set_year_facts",
                vec![json!(TIN), json!(2025), json!({"tax_classification": "SelfEmployed", "income_tax_election": "GraduatedUnspecified"})],
            )
            .expect("year facts");
            door::call("grant_send", vec![json!(TIN), json!("2551Q")]).expect("standing OK");

            let sent = with_client(&listener, |addr| {
                let mut client = AgentClient::connect(addr)
                    .with_timeout(Duration::from_secs(20))
                    .with_token(TOKEN);
                invoke(
                    &mut client,
                    "form.open",
                    json!({"code": "2551Q", "year": 2025, "period": 1, "tin": TIN}),
                );
                invoke(&mut client, "form.send", json!({}))
            });
            assert_eq!(sent["queued"], true, "{sent}");
            assert_eq!(sent["status"], "queued", "{sent}");
            assert_eq!(
                sent["dry_run"], true,
                "the host function reports the transport it will use: {sent}"
            );
            assert_eq!(sent["filed"], false);

            // The worker handles it as a dry run (a live attempt at the host above would leave it Queued or Failed).
            let started = Instant::now();
            let status = loop {
                let rows = door::call("list_returns", vec![json!("all")]).expect("returns");
                let status = rows[0]["status"].as_str().unwrap_or("").to_owned();
                if status != "Queued" || started.elapsed() > Duration::from_secs(30) {
                    break status;
                }
                thread::sleep(Duration::from_millis(250));
            };
            assert_eq!(
                status, "Submitted",
                "the dry-run worker took it, nothing else did"
            );
            assert_eq!(door::send_mode(), &SendMode::DryRun);
            assert!(std::env::var_os("BIR_SFTP_LIVE").is_none());
        },
    );
}
