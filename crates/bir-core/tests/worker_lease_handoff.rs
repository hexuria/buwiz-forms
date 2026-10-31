//! Two BIR apps on one database file: the background worker runs only in
//! the process holding the file's worker lease, and a follower takes over
//! when the holder lets go. Its own process: `start_cron_jobs` installs a
//! process-global wake channel.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bir_core::background_cron;
use bir_core::db::{Database, WorkerLease};

/// The key comes from the environment: never the OS keychain.
#[test]
fn a_follower_becomes_the_holder_when_the_holder_quits() {
    let key = "ab".repeat(32);
    temp_env::with_vars(
        [
            ("BIR_DB_KEY", Some(key.as_str())),
            ("BIR_NO_KEYCHAIN", Some("1")),
        ],
        || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            runtime.block_on(handoff());
        },
    );
}

async fn handoff() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("bir_data.db");
    let db = Arc::new(Mutex::new(Database::open(&path).expect("file database")));

    // Another app started first and holds the lease.
    let other = WorkerLease::try_acquire(&path)
        .expect("lease file")
        .expect("the other app is first");

    let worker = tokio::spawn(background_cron::start_cron_jobs(db));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !background_cron::holds_worker_lease(),
        "a follower must not run the jobs while another app holds the lease"
    );

    // The other app quits; a wake makes the follower try again now rather
    // than after its 30 s retry.
    drop(other);
    background_cron::wake();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !background_cron::holds_worker_lease() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        background_cron::holds_worker_lease(),
        "the follower took over"
    );
    assert!(
        WorkerLease::try_acquire(&path)
            .expect("lease file")
            .is_none(),
        "and now holds it against everyone else"
    );
    background_cron::request_shutdown();
    tokio::time::timeout(Duration::from_secs(15), worker)
        .await
        .expect("the worker stopped")
        .expect("no panic");
    assert!(!background_cron::holds_worker_lease());
    assert!(
        WorkerLease::try_acquire(&path)
            .expect("lease file")
            .is_some(),
        "a stopped worker leaves the lease free"
    );
}
