//! App-instance discovery so several gpui-agent hosts can run side by side.
//! Each host writes `<registry>/<app>-<pid>.json` with its bound endpoint.
//! No token is ever written. Stale records (dead pid) are pruned on read.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const REGISTRY_ENV: &str = "GPUI_AGENT_REGISTRY";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceRecord {
    pub app: String,
    pub pid: u32,
    pub addr: String,
    pub mode: String,
    pub protocol: u32,
}

pub fn registry_dir() -> PathBuf {
    match std::env::var_os(REGISTRY_ENV) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => bir_core::platform::data_dir().join("agent-instances"),
    }
}

fn record_path(app: &str, pid: u32) -> PathBuf {
    registry_dir().join(format!("{app}-{pid}.json"))
}

pub fn write_record(rec: &InstanceRecord) -> std::io::Result<PathBuf> {
    let dir = registry_dir();
    std::fs::create_dir_all(&dir)?;
    let path = record_path(&rec.app, rec.pid);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(rec).expect("record json"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

pub fn remove_record(app: &str, pid: u32) {
    let _ = std::fs::remove_file(record_path(app, pid));
}

pub fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: signal 0 only checks existence/permission.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Live records for `app` (or all apps when `None`); prunes dead ones.
pub fn list(app: Option<&str>) -> Vec<InstanceRecord> {
    let Ok(entries) = std::fs::read_dir(registry_dir()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(rec) = serde_json::from_slice::<InstanceRecord>(&bytes) else {
            continue;
        };
        if !pid_alive(rec.pid) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if app.is_none_or(|a| a == rec.app) {
            out.push(rec);
        }
    }
    out.sort_by_key(|r| r.pid);
    out
}

/// Removes this process's record on drop.
pub struct RecordGuard {
    app: String,
    pid: u32,
}

pub fn register(app: &str, addr: std::net::SocketAddr, mode: &str) -> Option<RecordGuard> {
    let rec = InstanceRecord {
        app: app.into(),
        pid: std::process::id(),
        addr: addr.to_string(),
        mode: mode.into(),
        protocol: 2,
    };
    match write_record(&rec) {
        Ok(_) => Some(RecordGuard {
            app: rec.app,
            pid: rec.pid,
        }),
        Err(e) => {
            eprintln!("agent discovery record: {e}");
            None
        }
    }
}

impl Drop for RecordGuard {
    fn drop(&mut self) {
        remove_record(&self.app, self.pid);
    }
}

pub const ADDR_ENV: &str = "GPUI_AGENT_ADDR";

/// Keep an explicit `GPUI_AGENT_ADDR`; otherwise fall back to an ephemeral
/// loopback port when the default is taken by another gpui-agent app.
pub fn resolve_addr(requested: std::net::SocketAddr) -> std::net::SocketAddr {
    let explicit = std::env::var_os(ADDR_ENV).is_some_and(|v| !v.is_empty());
    if explicit || std::net::TcpListener::bind(requested).is_ok() {
        return requested;
    }
    std::net::SocketAddr::new(requested.ip(), 0)
}

/// Where a client should connect: an explicit address wins; otherwise a live
/// record for `app` (headless first, since `status`/`shutdown` target the
/// daemon); otherwise `default`.
pub fn client_addr(
    explicit: Option<&str>,
    records: &[InstanceRecord],
    default: std::net::SocketAddr,
) -> Result<std::net::SocketAddr, String> {
    if let Some(raw) = explicit.filter(|raw| !raw.is_empty()) {
        return raw
            .parse()
            .map_err(|error| format!("invalid {ADDR_ENV}: {error}"));
    }
    let chosen = records
        .iter()
        .find(|rec| rec.mode == "headless")
        .or_else(|| records.first());
    match chosen {
        Some(rec) => rec
            .addr
            .parse()
            .map_err(|error| format!("bad discovery record for pid {}: {error}", rec.pid)),
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(pid: u32, addr: &str, mode: &str) -> InstanceRecord {
        InstanceRecord {
            app: "bir-desktop".into(),
            pid,
            addr: addr.into(),
            mode: mode.into(),
            protocol: 2,
        }
    }

    #[test]
    fn client_addr_prefers_explicit_then_headless_record_then_default() {
        let default: std::net::SocketAddr = "127.0.0.1:17421".parse().unwrap();
        let records = [
            rec(10, "127.0.0.1:50001", "desktop"),
            rec(11, "127.0.0.1:50002", "headless"),
        ];
        assert_eq!(
            client_addr(Some("127.0.0.1:9"), &records, default)
                .unwrap()
                .port(),
            9
        );
        assert_eq!(client_addr(None, &records, default).unwrap().port(), 50002);
        assert_eq!(
            client_addr(Some(""), &records[..1], default)
                .unwrap()
                .port(),
            50001
        );
        assert_eq!(client_addr(None, &[], default).unwrap(), default);
        assert!(client_addr(Some("nope"), &records, default).is_err());
    }

    #[test]
    fn resolve_addr_falls_back_only_when_implicit() {
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy = holder.local_addr().unwrap();
        temp_env::with_var(ADDR_ENV, None::<&str>, || {
            assert_eq!(resolve_addr(busy).port(), 0)
        });
        temp_env::with_var(ADDR_ENV, Some(busy.to_string()), || {
            assert_eq!(resolve_addr(busy), busy)
        });
    }

    #[test]
    fn write_list_prune_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        temp_env::with_var(REGISTRY_ENV, Some(dir.path()), || {
            let me = std::process::id();
            let live = InstanceRecord {
                app: "bir-desktop".into(),
                pid: me,
                addr: "127.0.0.1:1".into(),
                mode: "headless".into(),
                protocol: 2,
            };
            write_record(&live).unwrap();
            let dead = InstanceRecord {
                pid: 9_999_999,
                app: "bir-desktop".into(),
                ..live.clone()
            };
            write_record(&dead).unwrap();
            let other = InstanceRecord {
                app: "nativechat".into(),
                ..live.clone()
            };
            write_record(&other).unwrap();
            assert_eq!(list(Some("bir-desktop")), vec![live.clone()]);
            assert_eq!(list(None).len(), 2);
            assert!(!record_path("bir-desktop", 9_999_999).exists());
            remove_record("bir-desktop", me);
            assert_eq!(list(Some("bir-desktop")), vec![]);
        });
    }
}
