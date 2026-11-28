//! The BIR_DEVICE_DIR seam's probe, test-support only: a small stand-in for the app's start,
//! run as a subprocess by the seam's tests so each scenario gets its own process — and its own
//! environment — exactly as the runner's real app start does. Prints one JSON object.

fn main() {
    run();
}

fn run() {
    use bir_core::platform::write_device_dir_marker;
    use serde_json::json;
    // the start steps, in the door's order: the marker first, then the storage preparation,
    // then a host-level sign-in
    write_device_dir_marker();
    bir_core::platform::prepare_app_storage();
    let marker = bir_core::platform::device_dir().join(".ebirforms-test-device.json");
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
