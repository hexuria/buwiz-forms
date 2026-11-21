//! Opt-in Android development configuration; no shell syntax or arbitrary environment keys.

use std::io::Read;
use std::path::Path;

const ALLOWED: &[&str] = &[
    "EBIR_TEST_ENV",
    "BIR_AUTH_BYPASS",
    "BUWIZ_NO_KEYCHAIN",
    "BIR_SFTP_DRY_RUN",
    "BIR_RELAY_URL",
];

pub(crate) const ENABLED: bool = cfg!(debug_assertions)
    || match option_env!("BUWIZ_ANDROID_DEV") {
        Some(value) => matches!(value.as_bytes(), [b'1']),
        None => false,
    };

const BAKED: Option<&str> = {
    if ENABLED {
        option_env!("BUWIZ_ANDROID_DEV_ENV")
    } else {
        None
    }
};

pub(crate) fn load(path: &Path) -> Result<(), String> {
    let values = read(path)?;
    bir_core::config::set_many(values.into_iter().map(|(key, value)| (key, Some(value))));
    Ok(())
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
        std::fs::write(&path, "EBIR_TEST_ENV=1\nBIR_AUTH_BYPASS=1\nBUWIZ_NO_KEYCHAIN=1\nBIR_SFTP_DRY_RUN=1\nBIR_RELAY_URL=http://127.0.0.1:8787").unwrap();
        load(&path).unwrap();
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
