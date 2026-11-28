//! Opt-in Android development configuration; no shell syntax or arbitrary environment keys.

use std::io::Read;
use std::path::Path;

const ALLOWED: &[&str] = &[
    "EBIR_TEST_ENV",
    "BIR_AUTH_BYPASS",
    "BUWIZ_NO_KEYCHAIN",
    "BIR_SFTP_DRY_RUN",
    "BIR_RELAY_URL",
    // The app's agent data plane (src/agent.rs): an Android app process has no launch environment to read them from.
    // Its token is never a setting: a baked value is readable in the built library by anyone given the build, so the
    // token comes from a private file at run time (`agent::take_token`) and `GPUI_AGENT_TOKEN` is refused here.
    "GPUI_AGENT",
    "GPUI_AGENT_ADDR",
    // The Buwiz account server and its site, and the test key a development build's relay client trusts
    // (`bir_core::cloud_account::configured_server` / `configured_account`, `bir_core::relay`): an emulator has no
    // launch environment, so without them an Android development build could only sign in to production. A
    // development build still sends only through a server on this computer, on a test key (`bir_core::relay`), and
    // `BUWIZ_RELAY_SEND` stays out: sending from an Android development build is a later step.
    "BUWIZ_SERVER_URL",
    "BUWIZ_ACCOUNT_URL",
    "BUWIZ_TRUST_KEYS",
    // A test run's database that outlives its process (an emulator check of a sign-in kept across the app being swiped
    // away): without it a test environment opens a fresh scratch database per engine. A relative value is taken under the
    // engine's data folder; the guard accepts only a path inside its `test-data` folder in a test run
    // (`guards::android_test_database_refusal`), never the app's own database.
    "BIR_DATABASE_PATH",
];

/// The dev list's key whose relative value is taken under the engine's data folder.
const DATABASE_PATH: &str = "BIR_DATABASE_PATH";

pub(crate) const ENABLED: bool = enabled(cfg!(debug_assertions), option_env!("BUWIZ_ANDROID_DEV"));

/// Whether the development list is honoured: a debug build, or one built with `BUWIZ_ANDROID_DEV=1` exactly.
const fn enabled(debug_build: bool, android_dev: Option<&str>) -> bool {
    debug_build
        || match android_dev {
            Some(value) => matches!(value.as_bytes(), [b'1']),
            None => false,
        }
}

const BAKED: Option<&str> = honoured(ENABLED, option_env!("BUWIZ_ANDROID_DEV_ENV"));

/// A text compiled into the library exactly when the development list is honoured, and named nowhere else in the app:
/// `scripts/android_bundle.py check` refuses a store bundle whose library carries it. A release built with
/// `BUWIZ_ANDROID_DEV=1` has the production addresses and no development server, so only this tells it apart.
/// (`SENTINEL_TEXT` and `sentinel` are used only in constant evaluation and tests, so a release library holds neither.)
const SENTINEL_TEXT: &str = "nsh-dev-env:development-list-compiled-in";

/// [`SENTINEL_TEXT`] when the development list is honoured; nothing otherwise.
pub(crate) const SENTINEL: Option<&str> = sentinel(ENABLED);

const fn sentinel(enabled: bool) -> Option<&'static str> {
    if enabled { Some(SENTINEL_TEXT) } else { None }
}

/// The baked list when the development list is honoured; nothing at all otherwise.
const fn honoured(enabled: bool, baked: Option<&'static str>) -> Option<&'static str> {
    if enabled { baked } else { None }
}

pub(crate) fn load(path: &Path) -> Result<(), String> {
    // Keeps the sentinel in a library that honours the development list (the only one that calls this).
    std::hint::black_box(SENTINEL);
    let values = read(path)?;
    let data_dir = path.parent().unwrap_or(Path::new("."));
    let values = under_data_dir(values, data_dir);
    bir_core::config::set_many(values.into_iter().map(|(key, value)| (key, Some(value))));
    Ok(())
}

/// `BIR_DATABASE_PATH` given relative (`test-data/bir_data.db`, the same in every install) is taken under `data_dir`.
fn under_data_dir(values: Vec<(String, String)>, data_dir: &Path) -> Vec<(String, String)> {
    values
        .into_iter()
        .map(|(key, value)| {
            if key == DATABASE_PATH && !value.is_empty() && Path::new(&value).is_relative() {
                let joined = data_dir.join(&value).to_string_lossy().into_owned();
                (key, joined)
            } else {
                (key, value)
            }
        })
        .collect()
}

pub(crate) fn read(path: &Path) -> Result<Vec<(String, String)>, String> {
    merge(BAKED, read_file(path)?)
}

fn merge(
    baked: Option<&str>,
    overrides: Vec<(String, String)>,
) -> Result<Vec<(String, String)>, String> {
    let mut values = parse_baked(baked.unwrap_or_default())?;
    for (key, value) in overrides {
        if let Some((_, current)) = values.iter_mut().find(|(name, _)| name == &key) {
            *current = value;
        } else {
            values.push((key, value));
        }
    }
    Ok(values)
}

fn parse_baked(text: &str) -> Result<Vec<(String, String)>, String> {
    if text.len() > 16_384 {
        return Err("BUWIZ_ANDROID_DEV_ENV exceeds 16384 bytes".into());
    }
    parse(&text.replace(';', "\n"))
        .map_err(|error| format!("Invalid BUWIZ_ANDROID_DEV_ENV: {error}"))
}

fn read_file(path: &Path) -> Result<Vec<(String, String)>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Cannot read dev.env: {error}")),
    };
    let mut text = String::new();
    file.take(16_385)
        .read_to_string(&mut text)
        .map_err(|error| format!("Cannot read dev.env: {error}"))?;
    if text.len() > 16_384 {
        return Err("dev.env exceeds 16384 bytes".into());
    }
    parse(&text)
}

fn parse(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut values = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let invalid = || {
            format!(
                "Invalid dev.env line {}: expected an allowlisted KEY=VALUE",
                index + 1
            )
        };
        let (key, value) = line.split_once('=').ok_or_else(invalid)?;
        let (key, value) = (key.trim(), value.trim());
        if !ALLOWED.contains(&key)
            || value.contains('\0')
            || values.iter().any(|(name, _)| name == key)
        {
            return Err(invalid());
        }
        values.push((key.to_owned(), value.to_owned()));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_database_path_is_taken_under_the_data_folder() {
        let dir = Path::new("/data/user/0/dev.goldcoders.bir/files/nsh/dev.goldcoders.bir");
        let values = under_data_dir(
            vec![
                ("BIR_DATABASE_PATH".into(), "test-data/bir_data.db".into()),
                ("EBIR_TEST_ENV".into(), "1".into()),
            ],
            dir,
        );
        // Built with `join` so the separator is the host's: the code runs on Android, the test on every desktop.
        assert_eq!(values[0].1, dir.join("test-data/bir_data.db").to_string_lossy());
        assert_eq!(values[1].1, "1");
        let absolute_path = std::env::temp_dir().join("bir_data.db").to_string_lossy().into_owned();
        let absolute = under_data_dir(vec![("BIR_DATABASE_PATH".into(), absolute_path.clone())], dir);
        assert_eq!(absolute[0].1, absolute_path);
        assert!(parse("BIR_DATABASE_PATH=test-data/bir_data.db").is_ok());
    }

    #[test]
    fn android_load_reaches_core_and_account_gate_without_changing_the_environment() {
        const CHILD: &str = "BIR_CONFIG_LOAD_CHILD";
        const TEST: &str = "dev_env::tests::android_load_reaches_core_and_account_gate_without_changing_the_environment";
        if std::env::var_os(CHILD).is_none() {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .env("BIR_SFTP_LIVE", "1");
            for key in ALLOWED {
                command.env_remove(key);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let before: Vec<_> = ALLOWED.iter().map(std::env::var_os).collect();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("dev.env");
        std::fs::write(&path, "EBIR_TEST_ENV=1\nBIR_AUTH_BYPASS=1\nBUWIZ_NO_KEYCHAIN=1\nBIR_SFTP_DRY_RUN=1\nBIR_RELAY_URL=http://127.0.0.1:8787\nBUWIZ_SERVER_URL=http://localhost:3008\nBUWIZ_ACCOUNT_URL=http://localhost:3008\nBUWIZ_TRUST_KEYS=dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk").unwrap();
        load(&path).unwrap();
        let server = bir_core::cloud_account::configured_server().unwrap();
        assert_eq!(server.url, "http://localhost:3008");
        assert_eq!(server.source, bir_core::cloud_account::UrlSource::Environment);
        assert_eq!(bir_core::cloud_account::configured_account().unwrap().url, "http://localhost:3008");
        // A development build makes its relay client on the loaded server and test key; production's key or a
        // server off this computer loaded the same way is still refused.
        #[cfg(debug_assertions)]
        {
            assert_eq!(bir_core::relay::server().unwrap(), "http://localhost:3008");
            // Production's trust key (`bir_submit::trust::TRUST_KEYS`, public); nsh does not depend on bir-submit.
            bir_core::config::force("BUWIZ_TRUST_KEYS", Some("qyjpSeVbiWvpVOX9mIzkPsuXARRi0de8SQTetHj5o4M"));
            assert!(bir_core::relay::server().unwrap_err().to_string().contains("test key"));
            bir_core::config::force("BUWIZ_TRUST_KEYS", Some("dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk"));
            bir_core::config::force("BUWIZ_SERVER_URL", Some("https://api.buwiz.com"));
            assert!(bir_core::relay::server().unwrap_err().to_string().contains("on this computer"));
            bir_core::config::force("BUWIZ_SERVER_URL", Some("http://localhost:3008"));
        }
        assert!(bir_core::db::test_environment());
        #[cfg(debug_assertions)]
        assert!(bir_app_api::account_gate::auth_bypass_enabled());
        assert!(bir_core::platform::keychain_disabled());
        assert!(bir_core::transport::is_sftp_dry_run());
        assert_eq!(
            bir_core::transport::relay_url().unwrap().unwrap(),
            "http://127.0.0.1:8787"
        );
        #[cfg(debug_assertions)]
        assert!(matches!(
            bir_core::dev_mode::submission().source,
            bir_core::dev_mode::Source::Env(_)
        ));
        assert_eq!(
            ALLOWED
                .iter()
                .map(std::env::var_os)
                .collect::<Vec<_>>(),
            before
        );
        std::fs::write(&path, "EBIR_TEST_ENV=0\nPATH=/bad").unwrap();
        assert!(load(&path).is_err());
        assert!(bir_core::db::test_environment());
        assert_eq!(
            ALLOWED
                .iter()
                .map(std::env::var_os)
                .collect::<Vec<_>>(),
            before
        );
        bir_core::config::force("BIR_SFTP_LIVE", None);
        bir_core::config::force("BIR_SFTP_DRY_RUN", Some("1"));
        assert!(bir_core::config::var("BIR_SFTP_LIVE").is_err());
        assert_eq!(std::env::var("BIR_SFTP_LIVE").as_deref(), Ok("1"));
        assert!(bir_core::transport::is_sftp_dry_run());
        assert_eq!(
            ALLOWED
                .iter()
                .map(std::env::var_os)
                .collect::<Vec<_>>(),
            before
        );
    }

    #[test]
    fn only_a_debug_or_buwiz_android_dev_build_honours_the_baked_list() {
        assert!(enabled(true, None));
        assert!(enabled(false, Some("1")));
        for value in [None, Some(""), Some("0"), Some("true"), Some("1 "), Some("11")] {
            assert!(!enabled(false, value), "{value:?}");
        }
        // A release build ignores whatever was baked, the account server and trust key included: nothing is loaded,
        // so it keeps the compiled server and the built-in production trust key.
        let baked = "BUWIZ_SERVER_URL=http://localhost:3008;BUWIZ_TRUST_KEYS=dev-test-key-not-production";
        assert_eq!(honoured(false, Some(baked)), None);
        assert!(merge(honoured(false, Some(baked)), Vec::new()).unwrap().is_empty());
        assert_eq!(honoured(true, Some(baked)), Some(baked));
        if !ENABLED {
            assert_eq!(BAKED, None);
        }
    }

    #[test]
    fn the_sentinel_exists_only_where_the_development_list_is_honoured() {
        assert_eq!(sentinel(false), None);
        assert_eq!(sentinel(true), Some(SENTINEL_TEXT));
        assert_eq!(SENTINEL.is_some(), ENABLED);
    }

    #[test]
    fn the_account_server_and_trust_key_are_allowed_but_sending_is_not() {
        assert_eq!(
            parse_baked(
                "BUWIZ_SERVER_URL=http://localhost:3008;BUWIZ_ACCOUNT_URL=http://localhost:3008;BUWIZ_TRUST_KEYS=dev-test-key-not-production"
            )
            .unwrap(),
            vec![
                ("BUWIZ_SERVER_URL".into(), "http://localhost:3008".into()),
                ("BUWIZ_ACCOUNT_URL".into(), "http://localhost:3008".into()),
                ("BUWIZ_TRUST_KEYS".into(), "dev-test-key-not-production".into()),
            ]
        );
        assert!(parse_baked("BUWIZ_RELAY_SEND=1").is_err());
        assert!(!ALLOWED.contains(&"BUWIZ_RELAY_SEND"));
    }

    #[test]
    fn parses_only_allowlisted_values_without_shell_expansion() {
        assert_eq!(
            parse("# phone\n\n EBIR_TEST_ENV = 1\nBIR_RELAY_URL=http://127.0.0.1:8787\n").unwrap(),
            vec![
                ("EBIR_TEST_ENV".into(), "1".into()),
                ("BIR_RELAY_URL".into(), "http://127.0.0.1:8787".into()),
            ]
        );
        assert!(parse("").unwrap().is_empty());
        for text in [
            "PATH=/tmp",
            "export EBIR_TEST_ENV=1",
            "EBIR_TEST_ENV",
            "EBIR_TEST_ENV=1\nEBIR_TEST_ENV=0",
            "BIR_RELAY_URL=a\0b",
        ] {
            assert!(parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn missing_file_is_empty_and_oversized_file_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("dev.env");
        assert!(read_file(&path).unwrap().is_empty());
        assert_eq!(read(&path), merge(BAKED, Vec::new()));
        std::fs::write(&path, "#".repeat(16_385)).unwrap();
        assert!(read_file(&path).is_err());
    }

    #[test]
    fn baked_entries_accept_semicolons_and_newlines_with_the_same_validation() {
        assert_eq!(
            parse_baked("EBIR_TEST_ENV=1; BIR_AUTH_BYPASS=1\nBIR_RELAY_URL=http://127.0.0.1:8787")
                .unwrap(),
            vec![
                ("EBIR_TEST_ENV".into(), "1".into()),
                ("BIR_AUTH_BYPASS".into(), "1".into()),
                ("BIR_RELAY_URL".into(), "http://127.0.0.1:8787".into()),
            ]
        );
        assert!(parse_baked("").unwrap().is_empty());
        for text in [
            "PATH=/tmp",
            "EBIR_TEST_ENV=1;EBIR_TEST_ENV=0",
            "EBIR_TEST_ENV=1;broken",
            "BIR_RELAY_URL=a\0b",
        ] {
            assert!(parse_baked(text).is_err(), "{text:?}");
        }
        assert!(parse_baked(&"#".repeat(16_385)).is_err());
    }

    #[test]
    fn the_agent_token_is_refused_baked_and_in_the_file() {
        assert_eq!(
            parse_baked("GPUI_AGENT=1;GPUI_AGENT_ADDR=127.0.0.1:17422").unwrap(),
            vec![
                ("GPUI_AGENT".into(), "1".into()),
                ("GPUI_AGENT_ADDR".into(), "127.0.0.1:17422".into()),
            ]
        );
        assert!(!ALLOWED.iter().any(|key| key.contains("TOKEN")));
        for text in ["GPUI_AGENT_TOKEN=secret", "GPUI_AGENT=1;GPUI_AGENT_TOKEN=secret", "BIR_AGENT_TOKEN=secret"] {
            assert!(parse_baked(text).is_err(), "{text:?}");
            assert!(merge(Some(text), Vec::new()).is_err(), "{text:?}");
            assert!(parse(&text.replace(';', "\n")).is_err(), "{text:?}");
        }
    }

    #[test]
    fn private_file_overrides_baked_values_per_key() {
        assert_eq!(
            merge(
                Some("EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1"),
                parse("EBIR_TEST_ENV=0\nBIR_RELAY_URL=http://127.0.0.1:8787").unwrap()
            )
            .unwrap(),
            vec![
                ("EBIR_TEST_ENV".into(), "0".into()),
                ("BIR_AUTH_BYPASS".into(), "1".into()),
                ("BIR_RELAY_URL".into(), "http://127.0.0.1:8787".into()),
            ]
        );
        assert!(merge(Some("PATH=/tmp"), Vec::new()).is_err());
    }
}
