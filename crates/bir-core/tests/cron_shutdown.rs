//! The background cron must return when quit asks it to, so the submission
//! worker can drop the database and the process can release the owner lock.
//! This is its own process: `start_cron_jobs` installs a process-global wake
//! channel that other tests in the library still expect to be unset.

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test]
async fn request_shutdown_stops_the_background_cron() {
    let db = Arc::new(Mutex::new(
        bir_core::db::Database::open_ephemeral().expect("ephemeral database"),
    ));
    let job = tokio::spawn(bir_core::background_cron::start_cron_jobs(db));
    tokio::time::sleep(Duration::from_millis(50)).await;
    bir_core::background_cron::request_shutdown();
    tokio::time::timeout(Duration::from_secs(15), job)
        .await
        .expect("the cron loop stopped after request_shutdown")
        .expect("the cron task did not panic");
    assert!(bir_core::background_cron::shutdown_requested());
}
