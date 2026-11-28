//! The BIR_DEVICE_DIR seam's probe, test-support only (required-features): a small stand-in
//! for the app's start, run as a subprocess by the seam's tests so each scenario gets its own
//! process — and its own environment — exactly as the runner's real app start does.
//!
//! `seam_probe start` performs the start steps in the door's order (the marker first, then
//! the storage preparation, then a host-level sign-in) and prints one JSON object. It REFUSES
//! to run outside the seam (no EBIR_TEST_ENV=1 or no BIR_DEVICE_DIR), so a hand run can never
//! stage a sign-in in a real folder. `seam_probe eval` only evaluates — the device dir, the
//! marker write (itself a no-op outside the seam), the single-room answer — and never saves
//! a sign-in, so it needs no guard.

fn main() {
    // Evaluate the seam FIRST: an invalid value exits here with the contract's one stderr
    // line, and the ignore warning fires, before any refusal of the probe's own.
    let _ = bir_core::platform::device_dir();
    let eval_only = std::env::args().nth(1).as_deref() == Some("eval");
    if !eval_only {
        let test_env = bir_core::db::test_environment();
        let seam = bir_core::config::var(bir_core::platform::DEVICE_DIR_ENV).is_ok();
        if !(test_env && seam) {
            eprintln!("seam_probe runs only with EBIR_TEST_ENV=1 and BIR_DEVICE_DIR set");
            std::process::exit(2);
        }
    }
    use bir_core::platform::write_device_dir_marker;
    use serde_json::json;
    write_device_dir_marker();
    let marker = bir_core::platform::device_dir().join(".ebirforms-test-device.json");
    if eval_only {
        println!(
            "{}",
            json!({
                "device_dir": bir_core::platform::device_dir().to_string_lossy(),
                "data_dir": bir_core::platform::data_dir().to_string_lossy(),
                "marker_exists": marker.exists(),
                "single_room": bir_core::db::single_room_database().is_some(),
                "sign_in": false,
            })
        );
        return;
    }
    bir_core::platform::prepare_app_storage();
    let signed_in =
        bir_core::cloud_account::save_stored_account(&bir_core::cloud_account::StoredAccount {
            server_url: "http://localhost".to_owned(),
            access_token: "probe-access".to_owned(),
            refresh_token: "probe-refresh".to_owned(),
            access_expires_at_unix: 0,
            user_id: "probe-user".to_owned(),
            email: Some("probe@example.test".to_owned()),
            offline: false,
            practice: false,
            last_refresh_unix: 0,
        });
    println!(
        "{}",
        json!({
            "device_dir": bir_core::platform::device_dir().to_string_lossy(),
            "data_dir": bir_core::platform::data_dir().to_string_lossy(),
            "marker_exists": marker.exists(),
            "marker_path": marker.to_string_lossy(),
            "single_room": bir_core::db::single_room_database().is_some(),
            "sign_in": signed_in.is_ok(),
        })
    );
}
