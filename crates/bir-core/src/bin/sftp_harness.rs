//! Local SFTP submission harness.
//!
//! Reproduces filename generation, dispatcher-field wrap/unwrap, and SFTP PUT
//! mechanics using placeholder credentials. It never embeds live BIR secrets.
//!
//! Example:
//!   TEST_SFTP_HOST=127.0.0.1 TEST_SFTP_PORT=2222 TEST_SFTP_USER=test \\
//!   TEST_SFTP_PASSWORD=test TEST_SFTP_FOLDER=1601Cv2018 \\
//!   cargo run -p bir-core --bin sftp_harness -- --dry-run

use bir_core::naming::{Tin, iaf_filename};
use bir_core::transport::{SftpEndpoint, submit_iaf_with_endpoint, unwrap_dispatcher_field, wrap_dispatcher_field};
use std::env;
use std::process::ExitCode;

fn placeholder_filename() -> String {
    let tin = Tin {
        segment1: "000".into(),
        segment2: "000".into(),
        segment3: "000".into(),
        branch: "00000".into(),
    };
    iaf_filename(&tin, "1601Cv2018", "092026", "test@example.com")
}

#[tokio::main]
async fn main() -> ExitCode {
    let dry_run = env::args().any(|arg| arg == "--dry-run");
    let filename = placeholder_filename();
    println!("filename={filename}");
    println!("filename_prefix_len={}", filename.split('-').next().unwrap_or("").len());

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

    if dry_run {
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
    match submit_iaf_with_endpoint("1601Cv2018", &filename, payload, endpoint).await {
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
