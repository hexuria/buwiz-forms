//! Local / live SFTP submission harness.
//!
//! Default `--dry-run` never talks to BIR.
//! `--live-connect` fetches dummy dispatcher fields, logs in, and opens SFTP.
//! `--live-put` also uploads the dummy 00000000000000 IAF ciphertext.
//! Secrets are never printed.

use bir_core::naming::{Tin, iaf_filename};
use bir_core::transport::{
    fetch_sftp_endpoint, probe_iaf_endpoint, submit_iaf_with_endpoint,
    unwrap_dispatcher_field, wrap_dispatcher_field, SftpEndpoint,
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

fn print_endpoint_meta(endpoint: &SftpEndpoint) {
    println!("host={}", endpoint.host);
    println!("port={}", endpoint.port);
    println!("username_len={}", endpoint.username.len());
    println!("password_len={}", endpoint.password.len());
    println!("folder={}", endpoint.remote_folder);
}

async fn live_connect() -> Result<String, String> {
    let endpoint = fetch_sftp_endpoint(DUMMY_TIN, DUMMY_FORM)
        .await
        .map_err(|error| format!("dispatcher: {error}"))?;
    print_endpoint_meta(&endpoint);
    probe_iaf_endpoint(DUMMY_FORM, endpoint)
        .await
        .map_err(|error| format!("sftp: {error}"))
}

async fn live_put() -> Result<String, String> {
    let endpoint = fetch_sftp_endpoint(DUMMY_TIN, DUMMY_FORM)
        .await
        .map_err(|error| format!("dispatcher: {error}"))?;
    print_endpoint_meta(&endpoint);
    let payload = fs::read(DUMMY_IAF).map_err(|error| format!("read dummy iaf: {error}"))?;
    println!("payload_len={}", payload.len());
    let filename = Path::new(DUMMY_IAF)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dummy.xml")
        .to_string();
    submit_iaf_with_endpoint(DUMMY_FORM, &filename, &payload, endpoint)
        .await
        .map_err(|error| format!("put: {error}"))?;
    Ok(filename)
}


#[tokio::main]
async fn main() -> ExitCode {
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

    let endpoint = match SftpEndpoint::from_env() {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("Set TEST_SFTP_HOST / TEST_SFTP_PORT / TEST_SFTP_USER / TEST_SFTP_PASSWORD");
            return ExitCode::from(2);
        }
    };
    let payload = b"dummy-encrypted-iaf";
    match submit_iaf_with_endpoint(DUMMY_FORM, &filename, payload, endpoint).await {
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
