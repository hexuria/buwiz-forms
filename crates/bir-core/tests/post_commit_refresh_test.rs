use bir_core::db::{Database, PostCommitRefreshStatus};
use bir_core::profile::TaxpayerProfile;
use rusqlite::Connection;
use tempfile::NamedTempFile;

fn test_profile() -> TaxpayerProfile {
    serde_json::from_value(serde_json::json!({
        "id": null,
        "full_name": "Post Commit Refresh Test",
        "tin": {
            "segment1": "741",
            "segment2": "852",
            "segment3": "963",
            "branch": "000"
        },
        "rdo_code": "018",
        "line_of_business": "Testing Services",
        "registered_address": "Olongapo City",
        "zip_code": "2200",
        "phone": "09123456789",
        "email": "refresh@example.com",
        "default_form_type": "2551Qv2018",
        "taxpayer_type": "Individual",
        "business_start_date": "2020-01-01"
    }))
    .expect("minimal taxpayer profile fixture should deserialize")
}

fn install_refresh_request_failure(path: &std::path::Path) {
    let conn = Connection::open(path).expect("test database should reopen");
    conn.execute_batch(
        "PRAGMA key = \"x'0000000000000000000000000000000000000000000000000000000000000000'\";
         DELETE FROM settings WHERE key = 'google_calendar_sync_requested';
         CREATE TRIGGER reject_calendar_refresh_insert
         BEFORE INSERT ON settings
         WHEN NEW.key = 'google_calendar_sync_requested'
         BEGIN
           SELECT RAISE(ABORT, 'forced refresh request failure');
         END;
         CREATE TRIGGER reject_calendar_refresh_update
         BEFORE UPDATE ON settings
         WHEN NEW.key = 'google_calendar_sync_requested'
         BEGIN
           SELECT RAISE(ABORT, 'forced refresh request failure');
         END;",
    )
    .expect("refresh failure trigger should install");
}

#[test]
fn profile_save_reports_recorded_post_commit_refresh_request() {
    temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
        let file = NamedTempFile::new().expect("temporary database should open");
        let db = Database::open(file.path()).expect("database should initialize");

        let outcome = db
            .save_profile_with_post_commit_status(test_profile())
            .expect("profile transaction should commit");

        assert!(matches!(
            outcome.refresh_status(),
            PostCommitRefreshStatus::Deferred { warning }
                if warning.contains("recorded for retry")
                    && warning.contains("not running yet")
        ));
        assert!(outcome.committed().id.is_some());
        assert_eq!(
            db.get_setting("google_calendar_sync_requested").unwrap(),
            Some("true".to_string())
        );
    });
}

#[test]
fn profile_refresh_failure_warns_without_rolling_back_committed_profile() {
    temp_env::with_var("EBIR_TEST_ENV", Some("1"), || {
        let file = NamedTempFile::new().expect("temporary database should open");
        let db = Database::open(file.path()).expect("database should initialize");
        install_refresh_request_failure(file.path());

        let outcome = db
            .save_profile_with_post_commit_status(test_profile())
            .expect("refresh failure must not misreport the committed profile as rolled back");

        assert!(matches!(
            outcome.refresh_status(),
            PostCommitRefreshStatus::Failed { warning }
                if warning.contains("committed")
                    && warning.contains("forced refresh request failure")
        ));
        assert!(
            db.get_profile(&outcome.committed().tin.full())
                .expect("profile lookup should succeed")
                .is_some()
        );
    });
}
