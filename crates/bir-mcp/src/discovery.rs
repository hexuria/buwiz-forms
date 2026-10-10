//! Finds a running BIR host through the gpui-agent discovery records that
//! `bir-desktop` writes (`<registry>/<app>-<pid>.json`, see
//! `crates/bir-desktop/src/agent/discovery.rs`). This side only reads them.
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

pub const REGISTRY_ENV: &str = "GPUI_AGENT_REGISTRY";
pub const ADDR_ENV: &str = "GPUI_AGENT_ADDR";
pub const BIR_APP: &str = "bir-desktop";
pub const DEFAULT_ADDR: &str = "127.0.0.1:17421";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct InstanceRecord {
    pub app: String,
    pub pid: u32,
    pub addr: String,
    pub mode: String,
    pub protocol: u32,
}

/// Where the adapter will connect, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub addr: SocketAddr,
    pub via: String,
}

pub fn registry_dir() -> PathBuf {
    match std::env::var_os(REGISTRY_ENV) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => bir_core::platform::data_dir().join("agent-instances"),
    }
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: signal 0 only checks existence/permission.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }
    #[cfg(windows)]
    {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
            fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
        }
        // SAFETY: Win32 process-query handle, closed before returning.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            ok && code == STILL_ACTIVE
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        true
    }
}

/// Live records for `app` in `dir`, sorted by pid. Dead or unreadable
/// records are skipped (not deleted: the host that wrote them prunes).
pub fn list_in(dir: &Path, app: &str) -> Vec<InstanceRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<InstanceRecord> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter_map(|path| std::fs::read(path).ok())
        .filter_map(|bytes| serde_json::from_slice::<InstanceRecord>(&bytes).ok())
        .filter(|rec| rec.app == app && pid_alive(rec.pid))
        .collect();
    out.sort_by_key(|r| r.pid);
    out
}

/// An explicit address wins; then a live BIR `desktop` record (the user is
/// watching that window); then a `headless` record; then any BIR record;
/// then `default`.
pub fn choose(
    explicit: Option<&str>,
    records: &[InstanceRecord],
    default: SocketAddr,
) -> Result<Endpoint, String> {
    if let Some(raw) = explicit.map(str::trim).filter(|raw| !raw.is_empty()) {
        let addr = raw
            .parse()
            .map_err(|error| format!("invalid {ADDR_ENV} `{raw}`: {error}"))?;
        return Ok(Endpoint {
            addr,
            via: ADDR_ENV.into(),
        });
    }
    let bir = || records.iter().filter(|rec| rec.app == BIR_APP);
    let chosen = bir()
        .find(|rec| rec.mode == "desktop")
        .or_else(|| bir().find(|rec| rec.mode == "headless"))
        .or_else(|| bir().next());
    match chosen {
        Some(rec) => {
            let addr = rec
                .addr
                .parse()
                .map_err(|error| format!("bad discovery record for pid {}: {error}", rec.pid))?;
            Ok(Endpoint {
                addr,
                via: format!("discovery record {}-{} ({})", rec.app, rec.pid, rec.mode),
            })
        }
        None => Ok(Endpoint {
            addr: default,
            via: "default address".into(),
        }),
    }
}

/// Resolve from the environment and the registry on disk.
pub fn resolve() -> Result<Endpoint, String> {
    let explicit = std::env::var(ADDR_ENV).ok();
    let records = list_in(&registry_dir(), BIR_APP);
    choose(
        explicit.as_deref(),
        &records,
        DEFAULT_ADDR.parse().expect("default addr"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(app: &str, pid: u32, addr: &str, mode: &str) -> InstanceRecord {
        InstanceRecord {
            app: app.into(),
            pid,
            addr: addr.into(),
            mode: mode.into(),
            protocol: 2,
        }
    }

    fn default() -> SocketAddr {
        DEFAULT_ADDR.parse().unwrap()
    }

    #[test]
    fn discovery_prefers_explicit_then_desktop_then_headless_then_default() {
        let records = [
            rec(BIR_APP, 11, "127.0.0.1:50002", "headless"),
            rec("nativechat", 9, "127.0.0.1:50009", "desktop"),
            rec(BIR_APP, 12, "127.0.0.1:50001", "desktop"),
        ];
        // Explicit address wins over every record.
        let explicit = choose(Some("127.0.0.1:9"), &records, default()).unwrap();
        assert_eq!(explicit.addr.port(), 9);
        assert_eq!(explicit.via, ADDR_ENV);
        // Desktop BIR record next, never another app's desktop record.
        let desktop = choose(None, &records, default()).unwrap();
        assert_eq!(desktop.addr.port(), 50001);
        assert!(desktop.via.contains("desktop"), "{}", desktop.via);
        // Blank explicit counts as unset; headless when no desktop.
        let headless = choose(Some("  "), &records[..2], default()).unwrap();
        assert_eq!(headless.addr.port(), 50002);
        // Only foreign apps, or nothing at all: default.
        assert_eq!(
            choose(None, &records[1..2], default()).unwrap().addr,
            default()
        );
        assert_eq!(choose(None, &[], default()).unwrap().addr, default());
        // Bad input is an error, not a silent fallback.
        assert!(choose(Some("nope"), &records, default()).is_err());
        assert!(choose(None, &[rec(BIR_APP, 1, "x", "desktop")], default()).is_err());
    }

    #[test]
    fn discovery_lists_live_bir_records_only() {
        let dir = tempfile::tempdir().unwrap();
        let me = std::process::id();
        let write = |name: &str, body: String| std::fs::write(dir.path().join(name), body).unwrap();
        let json = |app: &str, pid: u32, mode: &str| {
            format!(
                r#"{{"app":"{app}","pid":{pid},"addr":"127.0.0.1:5{mode_len}","mode":"{mode}","protocol":2}}"#,
                mode_len = mode.len()
            )
        };
        write(
            &format!("bir-desktop-{me}.json"),
            json(BIR_APP, me, "headless"),
        );
        write(
            "bir-desktop-9999999.json",
            json(BIR_APP, 9_999_999, "desktop"),
        );
        write(
            &format!("nativechat-{me}.json"),
            json("nativechat", me, "desktop"),
        );
        write("garbage.json", "{not json".into());
        write("bir-desktop-1.json.tmp", json(BIR_APP, me, "desktop"));
        let live = list_in(dir.path(), BIR_APP);
        assert_eq!(live.len(), 1, "{live:?}");
        assert_eq!(live[0].pid, me);
        assert_eq!(live[0].mode, "headless");
        // Reading never deletes the dead record (its host prunes it).
        assert!(dir.path().join("bir-desktop-9999999.json").exists());
        assert!(list_in(&dir.path().join("missing"), BIR_APP).is_empty());
    }
}
