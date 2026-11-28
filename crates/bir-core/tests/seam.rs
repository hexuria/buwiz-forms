//! The BIR_DEVICE_DIR seam's integration tests, in bir-core — a crate the workspace tests run
//! in CI on Linux and Windows too (the contract's point 5 names this crate). They run the
//! probe binary as a SUBPROCESS, one process per scenario: the device dir is a process-wide
//! cached answer, so no in-process test could set the variable without racing the rest of the
//! suite — and a subprocess is what the runner's real app start is. The probe performs the
//! start steps in the door's order (the marker first, then the storage preparation, then a
//! host-level sign-in) and prints one JSON object.
#![cfg(feature = "test-support")]

use bir_core::platform::DEVICE_DIR_ENV;

fn probe_binary() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_BIN_EXE_seam_probe"))
}

fn run_probe(envs: &[(&str, &str)]) -> (std::process::Output, serde_json::Value) {
    run_probe_mode(envs, &[])
}

fn run_probe_mode(
    envs: &[(&str, &str)],
    args: &[&str],
) -> (std::process::Output, serde_json::Value) {
    let output = std::process::Command::new(probe_binary())
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = serde_json::from_str(stdout.trim()).unwrap_or(serde_json::Value::Null);
    (output, json)
}

fn probe_envs<'a>(
    real_home: &'a tempfile::TempDir,
    extra: &'a [(&'a str, &'a str)],
) -> Vec<(&'a str, &'a str)> {
    let home = real_home.path().to_str().unwrap();
    let mut envs: Vec<(&str, &str)> = vec![
        ("EBIR_TEST_ENV", "1"),
        ("BUWIZ_NO_KEYCHAIN", "1"),
        ("HOME", home),
        ("XDG_DATA_HOME", home),
        ("LOCALAPPDATA", home),
    ];
    envs.extend(extra.iter().copied());
    envs
}

// THE KEY TEST (the contract's point 5), named in the PR: HOME (and XDG_DATA_HOME,
// LOCALAPPDATA) at a temp "real" folder, BIR_DEVICE_DIR at another; the real folder
// stays EMPTY through the start steps and a host-level sign-in, the sign-in file lands
// inside the device dir, and single-room is off.
#[test]
fn the_real_home_stays_empty_through_start_and_sign_in() {
    let real_home = tempfile::tempdir().unwrap();
    let device = tempfile::tempdir().unwrap();
    let device_path = device.path().to_str().unwrap().to_owned();
    let extra = [(DEVICE_DIR_ENV, device_path.as_str())];
    let envs = probe_envs(&real_home, &extra);
    let (output, json) = run_probe(&envs);
    assert!(output.status.success(), "probe: {json}");
    assert_eq!(json["device_dir"].as_str().unwrap(), device_path);
    assert_eq!(json["single_room"].as_bool().unwrap(), false);
    assert_eq!(json["sign_in"].as_bool().unwrap(), true);
    assert!(
        std::fs::read_dir(real_home.path())
            .unwrap()
            .next()
            .is_none(),
        "the real folder stays empty after the start steps and a sign-in"
    );
    // the marker exists, atomically written, with exactly the contract's content
    let marker = device.path().join(".ebirforms-test-device.json");
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
    let keys: std::collections::BTreeSet<_> =
        content.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        ["device_dir", "rooms", "single_room", "pid", "seam"]
            .iter()
            .map(|key| key.to_string())
            .collect(),
        "exactly the contract's keys"
    );
    assert_eq!(content["rooms"].as_bool().unwrap(), true);
    assert_eq!(content["single_room"].as_bool().unwrap(), false);
    assert_eq!(content["seam"].as_u64().unwrap(), 1);
    assert_eq!(content["pid"].as_u64().unwrap() > 0, true);
    assert!(
        !content["device_dir"].as_str().unwrap().contains("\\?\\"),
        "no verbatim prefix"
    );
    // the sign-in file lives inside the device dir
    let entries: Vec<_> = std::fs::read_dir(device.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        entries
            .iter()
            .any(|name| name.contains("account") || name.contains("cloud")),
        "the sign-in file is inside the device dir: {entries:?}"
    );
    // no leftover temp files from the atomic write
    assert!(
        !entries.iter().any(|name| name.contains(".write-")),
        "the write temp is renamed away: {entries:?}"
    );
}

// The precedence test (the contract's point 3): both set -> rooms mode in the device dir,
// and NOTHING is created at BIR_DATABASE_PATH's path; only BIR_DATABASE_PATH -> unchanged.
#[test]
fn the_seam_wins_over_the_named_database() {
    let real_home = tempfile::tempdir().unwrap();
    let device = tempfile::tempdir().unwrap();
    let device_path = device.path().to_str().unwrap().to_owned();
    let named = tempfile::tempdir().unwrap();
    let named_path = named
        .path()
        .join("bir_data.db")
        .to_str()
        .unwrap()
        .to_owned();
    let extra = [
        (DEVICE_DIR_ENV, device_path.as_str()),
        ("BIR_DATABASE_PATH", named_path.as_str()),
    ];
    let envs = probe_envs(&real_home, &extra);
    let (output, json) = run_probe(&envs);
    assert!(output.status.success());
    assert_eq!(json["single_room"].as_bool().unwrap(), false);
    assert!(
        !named.path().join("bir_data.db").exists(),
        "nothing is created at BIR_DATABASE_PATH's path"
    );
}

#[test]
fn without_the_seam_the_named_database_stays_single_room() {
    let real_home = tempfile::tempdir().unwrap();
    let named = tempfile::tempdir().unwrap();
    let named_path = named
        .path()
        .join("bir_data.db")
        .to_str()
        .unwrap()
        .to_owned();
    let extra = [("BIR_DATABASE_PATH", named_path.as_str())];
    let envs = probe_envs(&real_home, &extra);
    let (output, json) = run_probe_mode(&envs, &["eval"]);
    assert!(output.status.success());
    assert_eq!(json["single_room"].as_bool().unwrap(), true);
    assert_eq!(json["marker_exists"].as_bool().unwrap(), false);
}

// Outside the test environment the seam is ignored: no override, no marker. (The release
// half of the decision is the same condition — cfg!(debug_assertions) && test_environment
// — with its one warning line.)
#[test]
fn the_seam_is_nothing_outside_the_test_environment() {
    let real_home = tempfile::tempdir().unwrap();
    let device = tempfile::tempdir().unwrap();
    let device_path = device.path().to_str().unwrap().to_owned();
    let extra = [(DEVICE_DIR_ENV, device_path.as_str())];
    let envs: Vec<(&str, &str)> = probe_envs(&real_home, &extra)
        .into_iter()
        .filter(|(key, _)| *key != "EBIR_TEST_ENV")
        .chain([("EBIR_TEST_ENV", "0")])
        .collect();
    let (output, json) = run_probe_mode(&envs, &["eval"]);
    assert!(output.status.success());
    assert_ne!(json["device_dir"].as_str().unwrap(), device_path);
    assert!(
        !device.path().join(".ebirforms-test-device.json").exists(),
        "no marker without the test environment"
    );
}

// An invalid value exits the process EARLY with the contract's one stderr line: a relative
// path and a missing folder, each refused — never a fall-back to the OS default.
#[test]
fn an_invalid_seam_exits_early_with_one_stderr_line() {
    let real_home = tempfile::tempdir().unwrap();
    for (value, wrong) in [
        ("relative/folder", "not an absolute path"),
        ("/definitely/not/there-ebir-seam", "does not exist"),
    ] {
        let extra = [(DEVICE_DIR_ENV, value)];
        let envs = probe_envs(&real_home, &extra);
        let output = std::process::Command::new(probe_binary())
            .envs(envs)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{value} exits early");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("BIR_DEVICE_DIR must be an existing absolute folder: ")
                && stderr.contains(wrong),
            "the one stderr line: {stderr}"
        );
    }
}

// The ignore warning really appears, on stderr — never stdout (the registration rule): the
// variable set outside the test environment warns once, even though the probe then refuses.
#[test]
fn the_ignore_warning_appears_on_stderr() {
    let real_home = tempfile::tempdir().unwrap();
    let device = tempfile::tempdir().unwrap();
    let device_path = device.path().to_str().unwrap().to_owned();
    let extra = [(DEVICE_DIR_ENV, device_path.as_str())];
    let envs: Vec<(&str, &str)> = probe_envs(&real_home, &extra)
        .into_iter()
        .filter(|(key, _)| *key != "EBIR_TEST_ENV")
        .chain([("EBIR_TEST_ENV", "0")])
        .collect();
    let output = std::process::Command::new(probe_binary())
        .envs(envs)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("BIR_DEVICE_DIR is ignored outside a debug build in the test environment"),
        "the warning really appears on stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("BIR_DEVICE_DIR is ignored"),
        "never stdout: {stdout}"
    );
}

// The probe itself refuses outside the seam, so a hand run can never stage a sign-in in a
// real folder: one stderr line, nonzero exit, nothing written.
#[test]
fn the_probe_refuses_without_the_seam() {
    let real_home = tempfile::tempdir().unwrap();
    let home = real_home.path().to_str().unwrap();
    let output = std::process::Command::new(probe_binary())
        .env("HOME", home)
        .env("XDG_DATA_HOME", home)
        .env("LOCALAPPDATA", home)
        .env_remove("EBIR_TEST_ENV")
        .env_remove("BIR_DEVICE_DIR")
        .output()
        .unwrap();
    assert!(!output.status.success(), "refused: nonzero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("seam_probe runs only with EBIR_TEST_ENV=1 and BIR_DEVICE_DIR set"),
        "one stderr line: {stderr}"
    );
    assert!(
        !real_home.path().join("buwiz-cloud-account.json").exists(),
        "no sign-in is ever staged"
    );
}
