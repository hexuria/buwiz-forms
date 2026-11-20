//! Opt-in Android test commands; no UI, Activity or process-environment access.

use std::io::Read;
use std::path::Path;

pub(crate) fn enabled(android: bool, dev: bool, test_env: Option<&str>) -> bool {
    android && dev && test_env == Some("1")
}

fn parse(command: &str) -> Option<&'static str> {
    match command.trim() {
        "lock" => Some("lock"),
        "logout" => Some("logout"),
        _ => None,
    }
}

pub(crate) fn consume(data_dir: &Path, enabled: bool) -> Result<Option<&'static str>, String> {
    if !enabled {
        return Ok(None);
    }
    let path = data_dir.join("dev-command");
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot open dev-command: {error}")),
    };
    let mut bytes = Vec::new();
    let read = file.take(65).read_to_end(&mut bytes);
    std::fs::remove_file(&path).map_err(|error| format!("Cannot consume dev-command: {error}"))?;
    read.map_err(|error| format!("Cannot read dev-command: {error}"))?;
    let received_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let command = std::str::from_utf8(&bytes).ok();
    tracing::info!(received_at_ms, command = ?command, "Received Android dev-command");
    let parsed = command.filter(|_| bytes.len() <= 64).and_then(parse);
    if parsed.is_none() {
        tracing::warn!(received_at_ms, "Rejected unknown Android dev-command");
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_requires_android_dev_opt_in_and_exact_test_environment() {
        for android in [false, true] {
            for dev in [false, true] {
                for test_env in [
                    None,
                    Some("0"),
                    Some("true"),
                    Some("yes"),
                    Some(" 1 "),
                    Some("1"),
                ] {
                    assert_eq!(
                        enabled(android, dev, test_env),
                        android && dev && test_env == Some("1")
                    );
                }
            }
        }
    }

    #[test]
    fn parse_accepts_only_lock_and_logout() {
        assert_eq!(parse("lock\n"), Some("lock"));
        assert_eq!(parse(" logout\r\n"), Some("logout"));
        for unknown in [
            "",
            "LOCK",
            "signout",
            "lock logout",
            "lock\nlogout",
            "lock;logout",
        ] {
            assert_eq!(parse(unknown), None);
        }
    }

    #[test]
    fn consume_removes_commands_before_returning_and_never_replays_them() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("app-private-data");
        std::fs::create_dir(&data).unwrap();
        let mailbox = data.join("dev-command");
        for command in ["lock", "logout"] {
            std::fs::write(&mailbox, format!("{command}\n")).unwrap();
            assert_eq!(consume(&data, true).unwrap(), Some(command));
            assert!(!mailbox.exists());
            assert_eq!(consume(&data, true).unwrap(), None);
        }
    }

    #[test]
    fn disabled_mailbox_is_untouched_and_unknown_or_invalid_commands_are_consumed() {
        let root = tempfile::tempdir().unwrap();
        let mailbox = root.path().join("dev-command");
        std::fs::write(&mailbox, b"lock").unwrap();
        assert_eq!(consume(root.path(), false).unwrap(), None);
        assert_eq!(std::fs::read(&mailbox).unwrap(), b"lock");
        for unknown in [
            b"".to_vec(),
            b"unknown".to_vec(),
            vec![0xff],
            vec![b'x'; 65],
        ] {
            std::fs::write(&mailbox, unknown).unwrap();
            assert_eq!(consume(root.path(), true).unwrap(), None);
            assert!(!mailbox.exists());
        }
        std::fs::create_dir(&mailbox).unwrap();
        assert!(consume(root.path(), true).is_err());
    }
}
