//! Local / live SFTP submission harness.
//!
//! Default `--dry-run` never talks to BIR (filename + wrap roundtrip only).
//! `--live-connect` / `--live-put` use `resolve_sftp_endpoint`:
//! dry-run env, then BIR_SFTP_*, then TEST_SFTP_*, else dispatcher.
//! Secrets are never printed.

use bir_core::naming::{Tin, iaf_filename};
use bir_core::transport::{
    ResolvedSftpTarget, probe_sftp_target, resolve_sftp_endpoint, submit_sftp_target,
    unwrap_dispatcher_field, wrap_dispatcher_field,
};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const DUMMY_TIN: &str = "00000000000000";
const DUMMY_FORM: &str = "1601Cv2018";
const DUMMY_IAF: &str =
    r"C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml";

fn placeholder_filename() -> String {
    let tin = Tin {
        segment1: "000".into(),
        segment2: "000".into(),
        segment3: "000".into(),
        branch: "00000".into(),
    };
    iaf_filename(&tin, DUMMY_FORM, "092026", "test@example.com")
}

fn print_target(target: &ResolvedSftpTarget) {
    println!("source={}", target.source().as_str());
    match target {
        ResolvedSftpTarget::DryRun { folder } => {
            println!("folder={folder}");
        }
        ResolvedSftpTarget::Live { endpoint, .. } => {
            println!("host={}", endpoint.host);
            println!("port={}", endpoint.port);
            println!("username_len={}", endpoint.username.len());
            println!("password_len={}", endpoint.password.len());
            println!("folder={}", endpoint.remote_folder);
        }
    }
}

async fn live_connect() -> Result<String, String> {
    let target = resolve_sftp_endpoint(DUMMY_FORM, DUMMY_TIN)
        .await
        .map_err(|error| format!("resolve: {error}"))?;
    print_target(&target);
    probe_sftp_target(DUMMY_FORM, target)
        .await
        .map_err(|error| format!("sftp: {error}"))
}

async fn live_put() -> Result<String, String> {
    let filename = Path::new(DUMMY_IAF)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dummy.xml")
        .to_string();
    let payload = fs::read(DUMMY_IAF).map_err(|error| format!("read dummy iaf: {error}"))?;
    println!("payload_len={}", payload.len());
    let target = resolve_sftp_endpoint(DUMMY_FORM, DUMMY_TIN)
        .await
        .map_err(|error| format!("resolve: {error}"))?;
    print_target(&target);
    submit_sftp_target(DUMMY_FORM, &filename, &payload, target)
        .await
        .map_err(|error| format!("put: {error}"))?;
    Ok(filename)
}

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();
    let args: Vec<String> = env::args().collect();
    let dry_run = args.iter().any(|arg| arg == "--dry-run") || args.len() == 1;
    let live_connect_flag = args.iter().any(|arg| arg == "--live-connect");
    let live_put_flag = args.iter().any(|arg| arg == "--live-put");

    if live_connect_flag {
        match live_connect().await {
            Ok(cwd) => {
                println!("auth=ok");
                println!("sftp=ok");
                println!("cwd={cwd}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("live-connect failed: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if live_put_flag {
        match live_put().await {
            Ok(filename) => {
                println!("upload=ok");
                println!("filename={filename}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("live-put failed: {error}");
                return ExitCode::from(1);
            }
        }
    }

    let filename = placeholder_filename();
    println!("filename={filename}");
    println!(
        "filename_prefix_len={}",
        filename.split('-').next().unwrap_or("").len()
    );

    match wrap_dispatcher_field("TEST_SFTP_HOST") {
        Ok(wrapped) => match unwrap_dispatcher_field(&wrapped) {
            Ok(plain) => println!("dispatcher_wrap_roundtrip={plain}"),
            Err(error) => {
                eprintln!("dispatcher unwrap failed: {error}");
                return ExitCode::from(2);
            }
        },
        Err(error) => {
            eprintln!("dispatcher wrap failed: {error}");
            return ExitCode::from(2);
        }
    }

    if dry_run && !args.iter().any(|arg| arg == "--upload") {
        println!("dry_run=1 skipped_sftp_connect");
        return ExitCode::SUCCESS;
    }

    match resolve_sftp_endpoint(DUMMY_FORM, DUMMY_TIN).await {
        Ok(target) => {
            print_target(&target);
            match submit_sftp_target(DUMMY_FORM, &filename, b"dummy-encrypted-iaf", target).await {
                Ok(()) => {
                    println!("upload=ok");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("upload failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        Err(error) => {
            eprintln!("resolve failed: {error}");
            ExitCode::from(1)
        }
    }
}
