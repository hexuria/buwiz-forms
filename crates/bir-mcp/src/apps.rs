//! Running BIR apps, as published by upstream gpui-agent discovery
//! (`gpui_agent::discovery`: `<runtime_root>/hosts/<pid>-<port>.json`, no
//! token). This side only reads records; it never writes one.
use gpui_agent::discovery::{self, HostRecord};
use std::path::Path;

/// `app` in a painted BIR host's record. `bir-shell` publishes its own name.
pub const BIR_APP: &str = "bir-desktop";
/// `app` values that are BIR. Headless still publishes `bir-desktop`.
pub const BIR_APPS: [&str; 2] = [BIR_APP, "bir-shell"];
/// Executable basenames of the BIR hosts. A packaged shell is renamed to `bir`.
pub const BIR_EXECUTABLES: [&str; 3] = ["bir", "bir-headless", "bir-shell"];
const HEADLESS_EXECUTABLE: &str = "bir-headless";

fn executable_name(record: &HostRecord) -> &str {
    Path::new(&record.executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&record.executable)
}

pub fn is_bir(record: &HostRecord) -> bool {
    BIR_APPS.contains(&record.app.as_str()) || BIR_EXECUTABLES.contains(&executable_name(record))
}

/// `desktop` or `headless`, from the record alone (`app.info` is the
/// authoritative answer once a token is known).
pub fn mode(record: &HostRecord) -> &'static str {
    if executable_name(record) == HEADLESS_EXECUTABLE {
        "headless"
    } else {
        "desktop"
    }
}

/// The id a user picks: the host's `GPUI_AGENT_CONNECT_ID`, else `pid:<pid>`.
pub fn app_id(record: &HostRecord) -> String {
    record
        .connect_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map_or_else(|| format!("pid:{}", record.pid), str::to_string)
}

/// Live BIR records under `root` (pid alive and port accepting), sorted by
/// pid then address. Records of other apps are ignored; nothing is deleted
/// here (upstream `resolve_id` prunes stale records).
pub fn live(root: &Path) -> Vec<HostRecord> {
    let Ok(entries) = std::fs::read_dir(root.join("hosts")) else {
        return Vec::new();
    };
    let mut out: Vec<HostRecord> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter_map(|path| std::fs::read(path).ok())
        .filter_map(|bytes| serde_json::from_slice::<HostRecord>(&bytes).ok())
        .filter(|record| {
            is_bir(record) && discovery::pid_alive(record.pid) && discovery::tcp_open(&record.addr)
        })
        .collect();
    out.sort_by(|a, b| (a.pid, &a.addr).cmp(&(b.pid, &b.addr)));
    out
}

fn parse_pid(id: &str) -> Option<u32> {
    let digits = id.strip_prefix("pid:").unwrap_or(id).trim();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The one live BIR app `id` names: `pid:<n>` (or a bare pid), else
/// whatever upstream `resolve_id` matches (connect id, bundle id,
/// executable, app). Missing, ambiguous and non-BIR matches are errors.
pub fn resolve(root: &Path, id: &str) -> Result<HostRecord, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("`id` is empty: pass an id from bir_list_apps".into());
    }
    if let Some(pid) = parse_pid(id) {
        let mut matches: Vec<HostRecord> = live(root)
            .into_iter()
            .filter(|record| record.pid == pid)
            .collect();
        return match matches.len() {
            0 => Err(format!(
                "no running BIR app has pid {pid}; call bir_list_apps for the current list"
            )),
            1 => Ok(matches.remove(0)),
            _ => Err(format!(
                "pid {pid} has {} BIR agent listeners ({}); ask the user to restart that BIR app",
                matches.len(),
                matches
                    .iter()
                    .map(|record| record.addr.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        };
    }
    match discovery::resolve_id(root, id) {
        Ok(record) if is_bir(&record) => Ok(record),
        Ok(record) => Err(format!(
            "`{id}` is {} ({}), not a BIR app",
            record.app, record.executable
        )),
        Err(error) => Err(format!(
            "{error}. Use an id exactly as bir_list_apps returned it (e.g. `pid:12345` when several apps share a name)."
        )),
    }
}

/// Whether the app at `pid` still publishes `addr` (no network probe).
pub fn still_published(root: &Path, pid: u32, addr: &str) -> bool {
    if !discovery::pid_alive(pid) {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(root.join("hosts")) else {
        return false;
    };
    entries
        .flatten()
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<HostRecord>(&bytes).ok())
        .any(|record| record.pid == pid && record.addr == addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_agent::discovery::publish_record;
    use std::net::TcpListener;

    fn record(addr: &str, app: &str, executable: &str, connect_id: Option<&str>) -> HostRecord {
        HostRecord {
            addr: addr.into(),
            pid: std::process::id(),
            app: app.into(),
            executable: executable.into(),
            bundle_id: None,
            connect_id: connect_id.map(str::to_string),
        }
    }

    fn listener() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        (listener, addr)
    }

    #[test]
    fn lists_only_live_bir_records_with_ids_and_modes() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let (_gui, gui) = listener();
        let (_daemon, daemon) = listener();
        let (_chat, chat) = listener();
        let _a = publish_record(
            root,
            &record(&gui, BIR_APP, "/x/target/debug/bir", Some("bir-demo")),
        )
        .unwrap();
        let _b = publish_record(root, &record(&daemon, BIR_APP, "bir-headless", None)).unwrap();
        let _c = publish_record(root, &record(&chat, "nativechat", "nativechat", None)).unwrap();
        // Dead pid and a closed port are not live.
        let dead = HostRecord {
            pid: i32::MAX as u32,
            ..record(&gui, BIR_APP, "bir", None)
        };
        std::mem::forget(publish_record(root, &dead).unwrap());
        std::mem::forget(
            publish_record(root, &record("127.0.0.1:1", BIR_APP, "bir", None)).unwrap(),
        );
        std::fs::write(root.join("hosts").join("garbage.json"), "{nope").unwrap();

        let found = live(root);
        assert_eq!(found.len(), 2, "{found:?}");
        let gui_rec = found.iter().find(|r| r.addr == gui).unwrap();
        let daemon_rec = found.iter().find(|r| r.addr == daemon).unwrap();
        assert_eq!(app_id(gui_rec), "bir-demo");
        assert_eq!(mode(gui_rec), "desktop");
        assert_eq!(app_id(daemon_rec), format!("pid:{}", std::process::id()));
        assert_eq!(mode(daemon_rec), "headless");
        assert!(live(&root.join("missing")).is_empty());
    }

    #[test]
    fn resolve_by_connect_id_pid_and_refuses_foreign_or_unknown() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let (_gui, gui) = listener();
        let (_chat, chat) = listener();
        let _a = publish_record(root, &record(&gui, BIR_APP, "bir", Some("bir-demo"))).unwrap();
        let _c = publish_record(root, &record(&chat, "nativechat", "nativechat", None)).unwrap();

        assert_eq!(resolve(root, " bir-demo ").unwrap().addr, gui);
        let me = std::process::id();
        assert_eq!(resolve(root, &format!("pid:{me}")).unwrap().addr, gui);
        assert_eq!(resolve(root, &me.to_string()).unwrap().addr, gui);
        let foreign = resolve(root, "nativechat").unwrap_err();
        assert!(foreign.contains("not a BIR app"), "{foreign}");
        let unknown = resolve(root, "bir-nope").unwrap_err();
        assert!(unknown.contains("bir-nope"), "{unknown}");
        assert!(resolve(root, "pid:1").unwrap_err().contains("pid 1"));
        assert!(resolve(root, "  ").is_err());

        // Two BIR listeners in one process: a pid no longer names one app.
        let (_second, second) = listener();
        let _b = publish_record(root, &record(&second, BIR_APP, "bir", None)).unwrap();
        let both = resolve(root, &format!("pid:{me}")).unwrap_err();
        assert!(both.contains("2 BIR agent listeners"), "{both}");
        assert!(still_published(root, me, &gui));
        assert!(!still_published(root, me, "127.0.0.1:9"));
        assert!(!still_published(root, i32::MAX as u32, &gui));
    }

    #[test]
    fn bir_shell_is_a_desktop_bir_app_and_a_foreign_app_is_not() {
        assert!(is_bir(&record(
            "127.0.0.1:9",
            "bir-shell",
            "ebirforms",
            None
        )));
        assert!(is_bir(&record(
            "127.0.0.1:9",
            "custom",
            "/usr/bin/bir-shell",
            None
        )));
        assert!(!is_bir(&record(
            "127.0.0.1:9",
            "nativechat",
            "nativechat",
            None
        )));

        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let (_shell, shell) = listener();
        let (_chat, chat) = listener();
        let _a = publish_record(
            root,
            &record(
                &shell,
                "bir-shell",
                "/x/target/debug/bir-shell",
                Some("bir-shell"),
            ),
        )
        .unwrap();
        let _c = publish_record(root, &record(&chat, "nativechat", "nativechat", None)).unwrap();

        let found = live(root);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(app_id(&found[0]), "bir-shell");
        assert_eq!(mode(&found[0]), "desktop");
        assert_eq!(resolve(root, "bir-shell").unwrap().addr, shell);
        let foreign = resolve(root, "nativechat").unwrap_err();
        assert!(foreign.contains("not a BIR app"), "{foreign}");

        // A packaged install renames the binary to `bir` and still publishes
        // app `bir-shell`. Two of those share a name, so connect needs an id.
        let (_renamed, renamed) = listener();
        let _b =
            publish_record(root, &record(&renamed, "bir-shell", "/usr/bin/bir", None)).unwrap();
        let found = live(root);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(
            mode(found.iter().find(|r| r.addr == renamed).unwrap()),
            "desktop"
        );
        let both = resolve(root, "bir-shell").unwrap_err();
        assert!(both.contains("more than one"), "{both}");
    }
}
