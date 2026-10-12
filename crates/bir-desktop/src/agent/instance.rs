//! This process as one of possibly several running gpui-agent hosts.
//!
//! Discovery itself is upstream's (`gpui_agent::discovery`): each host writes
//! `<runtime_root>/hosts/<pid>-<port>.json` (no token, ever) and names itself
//! with `GPUI_AGENT_CONNECT_ID`. BIR adds three things upstream does not do:
//! falling back to an ephemeral port when the default one is taken, finding
//! the daemon for `bir-headless status` / `shutdown`, and labelling the
//! database this process opened as `test` or `real` for `app.info`.
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use gpui_agent::discovery::{self, PublishedHost, ResolveError};

/// `app` in this host's discovery record and `hello`.
pub const APP: &str = "bir-desktop";
pub const ADDR_ENV: &str = "GPUI_AGENT_ADDR";
/// Executable name `bir-headless status` / `shutdown` look for when no
/// `GPUI_AGENT_CONNECT_ID` names the instance.
pub const HEADLESS_EXECUTABLE: &str = "bir-headless";

/// Keep an explicit `GPUI_AGENT_ADDR`; otherwise fall back to an ephemeral
/// loopback port when the default is taken by another gpui-agent app.
pub fn resolve_addr(requested: SocketAddr) -> SocketAddr {
    let explicit = std::env::var_os(ADDR_ENV).is_some_and(|v| !v.is_empty());
    if explicit || std::net::TcpListener::bind(requested).is_ok() {
        return requested;
    }
    SocketAddr::new(requested.ip(), 0)
}

/// Write this process's upstream discovery record for the bound `addr`. The
/// record is removed when the guard drops.
pub fn publish(addr: SocketAddr) -> std::io::Result<PublishedHost> {
    discovery::publish_current(addr, APP)
}

/// Where `bir-headless status` / `shutdown` connect: an explicit address
/// wins; otherwise the instance named by `connect_id` (an unknown name is an
/// error); otherwise the one live `bir-headless` record; otherwise
/// `default`. Two live daemons with no name to tell them apart is an error.
pub fn client_addr(
    root: &Path,
    explicit: Option<&str>,
    connect_id: Option<&str>,
    default: SocketAddr,
) -> Result<SocketAddr, String> {
    if let Some(raw) = explicit.map(str::trim).filter(|raw| !raw.is_empty()) {
        return raw
            .parse()
            .map_err(|error| format!("invalid {ADDR_ENV}: {error}"));
    }
    let named = connect_id.map(str::trim).filter(|id| !id.is_empty());
    let id = named.unwrap_or(HEADLESS_EXECUTABLE);
    match discovery::resolve_id(root, id) {
        Ok(record) => record
            .addr
            .parse()
            .map_err(|error| format!("bad discovery record for pid {}: {error}", record.pid)),
        Err(ResolveError::Missing { .. }) if named.is_none() => Ok(default),
        Err(error) => Err(format!(
            "{error}. Name the instance with {} or set {ADDR_ENV}.",
            discovery::CONNECT_ID_ENV
        )),
    }
}

/// `test` or `real`, from the environment this process started with.
pub fn database_kind() -> &'static str {
    let test_env = std::env::var_os("EBIR_TEST_ENV").is_some();
    let override_path = std::env::var_os("BIR_DATABASE_PATH")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    database_kind_for(
        test_env,
        override_path.as_deref(),
        &bir_core::platform::data_dir(),
    )
}

/// `test` when `EBIR_TEST_ENV` is set or `BIR_DATABASE_PATH` points outside
/// the default data directory; `real` otherwise (including an override that
/// still points inside the data directory).
pub fn database_kind_for(
    test_env: bool,
    override_path: Option<&Path>,
    data_dir: &Path,
) -> &'static str {
    if test_env {
        return "test";
    }
    match override_path {
        Some(path) if !canonical(path).starts_with(canonical(data_dir)) => "test",
        _ => "real",
    }
}

/// Absolute path with symlinks resolved as far as the path exists, so a
/// not-yet-created file and `..` segments still compare correctly.
fn canonical(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if let Ok(resolved) = absolute.canonicalize() {
        return resolved;
    }
    match (absolute.parent(), absolute.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => absolute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_agent::discovery::{HostRecord, publish_record};
    use std::net::TcpListener;

    #[test]
    fn resolve_addr_falls_back_only_when_implicit() {
        let holder = TcpListener::bind("127.0.0.1:0").unwrap();
        let busy = holder.local_addr().unwrap();
        temp_env::with_var(ADDR_ENV, None::<&str>, || {
            assert_eq!(resolve_addr(busy).port(), 0)
        });
        temp_env::with_var(ADDR_ENV, Some(busy.to_string()), || {
            assert_eq!(resolve_addr(busy), busy)
        });
    }

    fn record(addr: SocketAddr, executable: &str, connect_id: Option<&str>) -> HostRecord {
        HostRecord {
            addr: addr.to_string(),
            pid: std::process::id(),
            app: APP.into(),
            executable: executable.into(),
            bundle_id: None,
            connect_id: connect_id.map(str::to_string),
        }
    }

    #[test]
    fn client_addr_prefers_explicit_then_named_then_headless_then_default() {
        let root = tempfile::tempdir().unwrap();
        let default: SocketAddr = "127.0.0.1:17421".parse().unwrap();
        let gui = TcpListener::bind("127.0.0.1:0").unwrap();
        let daemon = TcpListener::bind("127.0.0.1:0").unwrap();
        let gui_addr = gui.local_addr().unwrap();
        let daemon_addr = daemon.local_addr().unwrap();
        let root = root.path();

        // Nothing published: the default address, unless a name was given.
        assert_eq!(client_addr(root, None, None, default).unwrap(), default);
        let missing = client_addr(root, None, Some("bir-demo"), default).unwrap_err();
        assert!(missing.contains("bir-demo"), "{missing}");

        let _gui = publish_record(root, &record(gui_addr, "bir", Some("bir-demo"))).unwrap();
        let _daemon =
            publish_record(root, &record(daemon_addr, HEADLESS_EXECUTABLE, None)).unwrap();
        assert_eq!(
            client_addr(root, Some("127.0.0.1:9"), Some("bir-demo"), default)
                .unwrap()
                .port(),
            9
        );
        assert!(client_addr(root, Some("nope"), None, default).is_err());
        assert_eq!(
            client_addr(root, Some("  "), None, default).unwrap(),
            daemon_addr
        );
        assert_eq!(
            client_addr(root, None, Some("bir-demo"), default).unwrap(),
            gui_addr
        );

        // A second daemon with no name to tell them apart is refused.
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        let _other = publish_record(
            root,
            &record(other.local_addr().unwrap(), HEADLESS_EXECUTABLE, None),
        )
        .unwrap();
        let ambiguous = client_addr(root, None, None, default).unwrap_err();
        assert!(ambiguous.contains("more than one"), "{ambiguous}");
        assert!(ambiguous.contains("GPUI_AGENT_CONNECT_ID"), "{ambiguous}");
    }

    #[test]
    fn publish_writes_an_upstream_record_under_the_runtime_dir() {
        let root = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        temp_env::with_vars(
            [
                (discovery::RUNTIME_DIR_ENV, Some(root.path().as_os_str())),
                (
                    discovery::CONNECT_ID_ENV,
                    Some(std::ffi::OsStr::new("bir-unit")),
                ),
            ],
            || {
                let guard = publish(addr).unwrap();
                assert!(guard.path().starts_with(root.path()), "{:?}", guard.path());
                let found = discovery::resolve_id(root.path(), "bir-unit").unwrap();
                assert_eq!(found.addr, addr.to_string());
                assert_eq!(found.app, APP);
                assert_eq!(found.pid, std::process::id());
                let path = guard.path().to_path_buf();
                drop(guard);
                assert!(!path.exists(), "record removed on drop");
            },
        );
    }

    #[test]
    fn database_kind_is_test_only_for_test_env_or_a_path_outside_the_data_dir() {
        let data = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let data_dir = data.path();
        let inside = data_dir.join("bir_data.db");
        let outside = elsewhere.path().join("bir_data.db");
        let sneaky = data_dir.join("..").join("x").join("bir_data.db");
        assert_eq!(database_kind_for(false, None, data_dir), "real");
        assert_eq!(database_kind_for(false, Some(&inside), data_dir), "real");
        assert_eq!(database_kind_for(false, Some(&outside), data_dir), "test");
        assert_eq!(database_kind_for(false, Some(&sneaky), data_dir), "test");
        assert_eq!(database_kind_for(true, None, data_dir), "test");
        assert_eq!(database_kind_for(true, Some(&inside), data_dir), "test");
    }
}
