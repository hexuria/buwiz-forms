#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use bir_core::db::Database;
use std::sync::{Arc, Mutex};
use tracing::{error, info};
use tracing_subscriber;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    info!("Starting BIR Vault background daemon...");

    // Find database path
    let db_path = bir_core::db::default_database_path();

    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let (db, _recovered) = match Database::open_or_recreate(&db_path) {
        Ok(db) => db,
        Err(e) => {
            error!(
                "Daemon failed to open database at {}: {}",
                db_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    let db = Arc::new(Mutex::new(db));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(bir_core::background_cron::start_cron_jobs(db));
}
