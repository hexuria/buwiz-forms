//! Background update check for the desktop app.
//!
//! On launch, and then every few hours, the shell reads a release manifest
//! (GitHub's `latest.json`, or `BIR_UPDATE_FEED_URL` for a test server). A
//! newer semver is downloaded for this platform and checked against both the
//! manifest's sha256 and the release's checksum file. The page then offers
//! Install now / Later. Install replaces the running app and relaunches it
//! only after the database owner lock has been dropped: a second process
//! cannot open the encrypted database while this one still holds it.
//!
//! The running bundle is the one that gets replaced. A copy launched from a
//! scratch path updates that copy; an install in `/Applications` updates that
//! install. A binary that is not inside an `.app` is left where it is.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Shown when the bytes, the manifest sha256, and the checksum file disagree.
pub const VERIFY_REJECTED: &str = "The update could not be verified and was not installed.";

/// GitHub Releases feed. `BIR_UPDATE_FEED_URL` replaces it for a local server.
pub const DEFAULT_FEED_URL: &str =
    "https://github.com/hexuria/buwiz-forms/releases/latest/download/latest.json";

/// Database setting. Absent, or any value other than `0` / `false` / `off`,
/// means a newer release is downloaded in the background. The page writes it
/// through `set_auto_update_download`, not through `set_setting`.
pub const AUTO_DOWNLOAD_SETTING: &str = "auto_update_download";

const DEFAULT_INTERVAL: u64 = 4 * 60 * 60;
const DEFAULT_LAUNCH_DELAY: u64 = 2;

static AUTO_DOWNLOAD: AtomicBool = AtomicBool::new(true);
static INSTALLING: AtomicBool = AtomicBool::new(false);
static COMMANDS: OnceLock<mpsc::Sender<Command>> = OnceLock::new();
static STATE: Mutex<State> = Mutex::new(State {
    phase: "idle",
    version: None,
    message: None,
    asset_path: None,
    asset_name: None,
    dismissed: None,
});

#[derive(Clone)]
struct State {
    phase: &'static str,
    version: Option<String>,
    message: Option<String>,
    asset_path: Option<PathBuf>,
    asset_name: Option<String>,
    dismissed: Option<String>,
}

enum Command {
    /// `force_download` fetches even when auto-download is off (the menu item).
    /// `manual` surfaces "already current" and clears a Later dismissal.
    Check { force_download: bool, manual: bool },
}

/// What the page polls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateStatus {
    /// `idle`, `checking`, `downloading`, `ready`, `failed`, `up_to_date`,
    /// `available`, or `installing`.
    pub phase: String,
    /// The release that was found, when there is one.
    pub version: Option<String>,
    /// A sentence the page can show. Install uses its own wording.
    pub message: Option<String>,
    /// Whether a newer release is downloaded without asking first.
    pub auto_download: bool,
    /// The version compiled into this binary.
    pub current_version: &'static str,
}

/// A release manifest (`latest.json`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    /// Semver of the release, with or without a leading `v`.
    pub version: String,
    /// Platform id → asset. Unknown ids are ignored by selection.
    #[serde(default)]
    pub platforms: BTreeMap<String, UpdateAsset>,
}

/// One downloadable build.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UpdateAsset {
    /// Absolute `http` or `https` URL of the installer or archive.
    pub url: String,
    /// Lower or upper case hex sha256 of the asset bytes.
    pub sha256: String,
    /// File name, used to find the asset's line in the checksum file.
    pub name: String,
    /// Checksum sidecar (`*.dmg.sha256` or `SHA256SUMS-*.txt`). Required.
    #[serde(default)]
    pub checksum_url: Option<String>,
}

/// How this Linux process was installed, which picks the asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxInstallKind {
    /// A `.deb` under `/usr` (`dpkg` via pkexec).
    Deb,
    /// An `.rpm` under `/usr` (`rpm` via pkexec).
    Rpm,
    /// A `.pkg.tar.zst` under `/usr` (`pacman` via pkexec). x64 only.
    Arch,
    /// `~/.local/share/ebirforms` or an unpacked directory beside the binary.
    Tarball,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreId {
    Num(u64),
    Text(String),
}

/// A semantic version. Pre-release identifiers sort below the final release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    /// `MAJOR`.
    pub major: u64,
    /// `MINOR`.
    pub minor: u64,
    /// `PATCH`.
    pub patch: u64,
    pre: Vec<PreId>,
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| cmp_pre(&self.pre, &other.pre))
    }
}

fn cmp_pre(left: &[PreId], right: &[PreId]) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Equal, Greater, Less};
    match (left.is_empty(), right.is_empty()) {
        (true, false) => Greater,
        (false, true) => Less,
        (true, true) => Equal,
        (false, false) => {
            for (a, b) in left.iter().zip(right.iter()) {
                let order = cmp_pre_id(a, b);
                if order != Equal {
                    return order;
                }
            }
            left.len().cmp(&right.len())
        }
    }
}

fn cmp_pre_id(left: &PreId, right: &PreId) -> std::cmp::Ordering {
    match (left, right) {
        (PreId::Num(a), PreId::Num(b)) => a.cmp(b),
        // Numeric identifiers have lower precedence than alphanumeric ones.
        (PreId::Num(_), PreId::Text(_)) => std::cmp::Ordering::Less,
        (PreId::Text(_), PreId::Num(_)) => std::cmp::Ordering::Greater,
        (PreId::Text(a), PreId::Text(b)) => a.cmp(b),
    }
}

/// Parse `MAJOR.MINOR.PATCH` with an optional leading `v`, pre-release, and
/// build metadata. Anything else is not a version we can compare.
pub fn parse_version(raw: &str) -> Option<SemVer> {
    let raw = raw.trim();
    let raw = raw.strip_prefix('v').unwrap_or(raw);
    let (core, pre) = match raw.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (raw, None),
    };
    let core = core.split_once('+').map_or(core, |(core, _)| core);
    let pre = pre.map(|pre| pre.split_once('+').map_or(pre, |(pre, _)| pre));
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let pre = match pre {
        None => Vec::new(),
        Some(pre) => pre
            .split('.')
            .map(parse_pre_id)
            .collect::<Option<Vec<_>>>()?,
    };
    Some(SemVer {
        major,
        minor,
        patch,
        pre,
    })
}

fn parse_pre_id(id: &str) -> Option<PreId> {
    if id.is_empty() {
        return None;
    }
    if id.bytes().all(|byte| byte.is_ascii_digit()) {
        if id.len() > 1 && id.starts_with('0') {
            return None;
        }
        return Some(PreId::Num(id.parse().ok()?));
    }
    if id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Some(PreId::Text(id.to_string()));
    }
    None
}

impl SemVer {
    /// `true` when `self` should replace a process running `current`.
    pub fn is_newer_than(&self, current: &SemVer) -> bool {
        self > current
    }
}

/// `0`, `false`, and `off` turn auto-download off. Anything else, including
/// a missing value, leaves it on.
pub fn auto_download_enabled(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "off")
}

/// The manifest key for this operating system, cpu, and Linux install layout.
///
/// `linux` is consulted only when `os` is `linux`. Arch is published for x64
/// only, so an arm64 pacman install has no asset.
pub fn platform_asset_key(
    os: &str,
    arch: &str,
    linux: Option<LinuxInstallKind>,
) -> Option<&'static str> {
    match os {
        "macos" => Some("darwin-universal"),
        "windows" if arch == "x86_64" => Some("windows-x64-setup"),
        "linux" => linux_asset_key(arch, linux.unwrap_or(LinuxInstallKind::Tarball)),
        _ => None,
    }
}

fn linux_asset_key(arch: &str, kind: LinuxInstallKind) -> Option<&'static str> {
    let arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return None,
    };
    match (arch, kind) {
        ("x64", LinuxInstallKind::Deb) => Some("linux-x64-deb"),
        ("arm64", LinuxInstallKind::Deb) => Some("linux-arm64-deb"),
        ("x64", LinuxInstallKind::Rpm) => Some("linux-x64-rpm"),
        ("arm64", LinuxInstallKind::Rpm) => Some("linux-arm64-rpm"),
        ("x64", LinuxInstallKind::Arch) => Some("linux-x64-arch"),
        ("arm64", LinuxInstallKind::Arch) => None,
        ("x64", LinuxInstallKind::Tarball) => Some("linux-x64-tarball"),
        ("arm64", LinuxInstallKind::Tarball) => Some("linux-arm64-tarball"),
        _ => None,
    }
}

/// Deb, rpm, or pacman when the binary is `/usr/bin/ebirforms`; otherwise the
/// unpacked-directory layout (`~/.local/share/ebirforms`, or a directory that
/// is not a system prefix).
pub fn linux_install_kind(executable: &Path, os_release: &str) -> LinuxInstallKind {
    let in_usr_bin = executable.file_name().and_then(|name| name.to_str()) == Some("ebirforms")
        && executable.parent().and_then(|dir| dir.to_str()) == Some("/usr/bin");
    if in_usr_bin {
        linux_family(os_release)
    } else {
        LinuxInstallKind::Tarball
    }
}

fn linux_family(os_release: &str) -> LinuxInstallKind {
    let mut id = String::new();
    let mut like = String::new();
    for line in os_release.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = unquote(value);
        } else if let Some(value) = line.strip_prefix("ID_LIKE=") {
            like = unquote(value);
        }
    }
    let blob = format!("{id} {like}");
    let words: Vec<&str> = blob.split_whitespace().collect();
    if words
        .iter()
        .any(|word| *word == "debian" || *word == "ubuntu")
    {
        LinuxInstallKind::Deb
    } else if words
        .iter()
        .any(|word| matches!(*word, "rhel" | "fedora" | "centos" | "suse" | "sles"))
    {
        LinuxInstallKind::Rpm
    } else if words.contains(&"arch") {
        LinuxInstallKind::Arch
    } else {
        LinuxInstallKind::Tarball
    }
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

/// The asset for `key`. A Windows setup key falls back to the MSI when the
/// release published only that.
pub fn select_asset<'a>(manifest: &'a UpdateManifest, key: &str) -> Option<&'a UpdateAsset> {
    if let Some(asset) = manifest.platforms.get(key) {
        return Some(asset);
    }
    if key == "windows-x64-setup" {
        return manifest.platforms.get("windows-x64-msi");
    }
    None
}

/// Hex sha256 of `bytes`. Tests use it; the download path hashes as it writes.
#[cfg(test)]
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Accept the download only when the computed hash, the manifest's `sha256`,
/// and the checksum file all name the same digest.
///
/// The checksum file is `shasum` / `sha256sum` text (`<hex>  <filename>`,
/// two spaces, or a `*` before the name) or a single bare hex line.
pub fn verify_sha256(
    computed: &str,
    manifest_sha256: &str,
    checksum_text: &str,
    asset_name: &str,
) -> Result<(), &'static str> {
    let computed = normalize_hash(computed).ok_or("computed hash is not sha256")?;
    let expected = normalize_hash(manifest_sha256).ok_or("manifest sha256 is not sha256")?;
    if computed != expected {
        return Err("manifest sha256 does not match the download");
    }
    let from_file = hash_for_asset(checksum_text, asset_name)
        .ok_or("checksum file has no entry for this asset")?;
    if from_file != computed {
        return Err("checksum file does not match the download");
    }
    Ok(())
}

fn normalize_hash(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

/// The hex digest a checksum file records for `asset_name`.
pub fn hash_for_asset(checksum_text: &str, asset_name: &str) -> Option<String> {
    let mut bare = None;
    let mut bare_count = 0;
    let mut named_count = 0;
    for line in checksum_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next().and_then(normalize_hash) else {
            continue;
        };
        match parts.next() {
            None => {
                bare = Some(hash);
                bare_count += 1;
            }
            Some(name) => {
                let name = name.trim_start_matches('*');
                let file = Path::new(name).file_name()?.to_str()?;
                named_count += 1;
                if file == asset_name {
                    return Some(hash);
                }
            }
        }
    }
    if named_count == 0 && bare_count == 1 {
        bare
    } else {
        None
    }
}

/// Start the checker. Call once, on the UI thread, after the database is open
/// so the auto-download setting can be read.
pub fn start() {
    AUTO_DOWNLOAD.store(read_auto_download(), Ordering::SeqCst);
    let (tx, rx) = mpsc::channel();
    if COMMANDS.set(tx).is_err() {
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("bir-updater".into())
        .spawn(move || service(rx))
    {
        tracing::error!("updater not started: {error}");
    }
}

/// Menu item and `bir.check_for_updates`: look now, and download even when
/// auto-download is off.
pub fn check_now() {
    send(Command::Check {
        force_download: true,
        manual: true,
    });
}

/// The page pressed Later.
pub fn dismiss() {
    let mut state = lock_state();
    state.dismissed = state.version.clone();
    if matches!(state.phase, "ready" | "failed" | "up_to_date") {
        state.phase = "idle";
        state.message = None;
        write_status_file(&state);
    }
}

/// Remember the switch. The database write is the host function's.
pub fn set_auto_download(enabled: bool) {
    AUTO_DOWNLOAD.store(enabled, Ordering::SeqCst);
    let state = lock_state();
    write_status_file(&state);
}

/// Snapshot for `bir.update_status`.
pub fn current_status() -> UpdateStatus {
    let state = lock_state();
    UpdateStatus {
        phase: state.phase.to_string(),
        version: state.version.clone(),
        message: state.message.clone(),
        auto_download: AUTO_DOWNLOAD.load(Ordering::SeqCst),
        current_version: env!("CARGO_PKG_VERSION"),
    }
}

/// Install the verified asset, release the database lock, then relaunch or
/// hand off to the system installer. Runs on the UI thread because the
/// database handle is thread-local.
pub fn install_ready_update() {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return;
    }
    let prepared = {
        let state = lock_state();
        let Some(path) = state.asset_path.clone() else {
            drop(state);
            fail_install("There is no verified update to install.");
            return;
        };
        let name = state.asset_name.clone().unwrap_or_default();
        drop(state);
        set_phase("installing", None);
        prepare_install(&path, &name)
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            fail_install(&error);
            return;
        }
    };
    // The new process must be able to take the owner lock. Drop it before
    // the rename-and-exec, while this thread still owns the handle.
    crate::bir_module::shutdown_for_quit();
    if !crate::bir_module::owner_lock_released() {
        tracing::error!("database owner lock is still held; the update was not applied");
        std::process::exit(1);
    }
    if let Err(error) = commit_install(prepared) {
        tracing::error!("install failed after the database was closed: {error}");
        std::process::exit(1);
    }
    std::process::exit(0);
}

fn fail_install(message: &str) {
    INSTALLING.store(false, Ordering::SeqCst);
    let mut state = lock_state();
    state.phase = "failed";
    state.message = Some(message.to_string());
    write_status_file(&state);
}

fn send(command: Command) {
    if let Some(tx) = COMMANDS.get() {
        let _ = tx.send(command);
    }
}

fn service(rx: mpsc::Receiver<Command>) {
    let delay = env_secs("BIR_UPDATE_LAUNCH_DELAY_SECS", DEFAULT_LAUNCH_DELAY);
    if delay > 0 {
        std::thread::sleep(Duration::from_secs(delay));
    }
    let interval = env_secs("BIR_UPDATE_INTERVAL_SECS", DEFAULT_INTERVAL).max(1);
    run_check(false, false);
    loop {
        match rx.recv_timeout(Duration::from_secs(interval)) {
            Ok(Command::Check {
                force_download,
                manual,
            }) => run_check(force_download, manual),
            Err(mpsc::RecvTimeoutError::Timeout) => run_check(false, false),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

fn run_check(force_download: bool, manual: bool) {
    if INSTALLING.load(Ordering::SeqCst) {
        return;
    }
    set_phase("checking", None);
    let manifest = match fetch_manifest(&feed_url()) {
        Ok(manifest) => manifest,
        Err(error) => {
            tracing::warn!("update check failed: {error}");
            if manual {
                fail_check(&error);
            } else {
                set_phase("idle", None);
            }
            return;
        }
    };
    let Some(remote) = parse_version(&manifest.version) else {
        fail_check("The update feed has no usable version.");
        return;
    };
    let Some(current) = parse_version(env!("CARGO_PKG_VERSION")) else {
        fail_check("This build's version could not be compared.");
        return;
    };
    if !remote.is_newer_than(&current) {
        if manual {
            let mut state = lock_state();
            state.phase = "up_to_date";
            state.version = Some(manifest.version);
            state.message = Some(format!(
                "Version {} is the latest.",
                env!("CARGO_PKG_VERSION")
            ));
            state.dismissed = None;
            write_status_file(&state);
        } else {
            set_phase("idle", None);
        }
        return;
    }
    let Some(key) = current_platform_key() else {
        fail_check("This computer has no published update package.");
        return;
    };
    let Some(asset) = select_asset(&manifest, key) else {
        fail_check("This release has no update for this computer.");
        return;
    };
    if already_verified(&manifest.version) {
        let mut state = lock_state();
        state.version = Some(manifest.version.clone());
        state.message = Some(format!("Version {} is ready.", manifest.version));
        if manual {
            state.dismissed = None;
            state.phase = "ready";
        } else if state.dismissed.as_deref() == Some(manifest.version.as_str()) {
            state.phase = "idle";
        } else {
            state.phase = "ready";
        }
        write_status_file(&state);
        return;
    }
    if !force_download && !AUTO_DOWNLOAD.load(Ordering::SeqCst) {
        let mut state = lock_state();
        state.phase = "available";
        state.version = Some(manifest.version.clone());
        state.message = Some(format!(
            "Version {} is available. Download updates automatically is off.",
            manifest.version
        ));
        write_status_file(&state);
        return;
    }
    set_phase("downloading", Some(manifest.version.clone()));
    match download_verified(asset) {
        Ok(path) => {
            let mut state = lock_state();
            state.asset_path = Some(path);
            state.asset_name = Some(asset.name.clone());
            state.version = Some(manifest.version.clone());
            state.message = Some(format!("Version {} is ready.", manifest.version));
            if !manual && state.dismissed.as_deref() == Some(manifest.version.as_str()) {
                state.phase = "idle";
            } else {
                state.phase = "ready";
                if manual {
                    state.dismissed = None;
                }
            }
            tracing::info!("update ready: {}", manifest.version);
            write_status_file(&state);
        }
        Err(error) => {
            tracing::warn!("update rejected: {error}");
            let mut state = lock_state();
            state.phase = "failed";
            state.version = Some(manifest.version);
            state.asset_path = None;
            state.message = Some(error);
            write_status_file(&state);
        }
    }
}

fn already_verified(version: &str) -> bool {
    let state = lock_state();
    state.version.as_deref() == Some(version)
        && state.asset_path.as_ref().is_some_and(|path| path.is_file())
        && matches!(state.phase, "ready" | "idle")
}

fn fail_check(message: &str) {
    let mut state = lock_state();
    state.phase = "failed";
    state.message = Some(message.to_string());
    write_status_file(&state);
}

fn set_phase(phase: &'static str, version: Option<String>) {
    let mut state = lock_state();
    state.phase = phase;
    if let Some(version) = version {
        state.version = Some(version);
    }
    write_status_file(&state);
}

fn write_status_file(state: &State) {
    let Some(path) = std::env::var_os("BIR_UPDATE_STATUS_FILE") else {
        return;
    };
    let body = format!(
        "{}\n{}\n{}\n",
        state.phase,
        state.version.as_deref().unwrap_or(""),
        state.message.as_deref().unwrap_or("")
    );
    if let Err(error) = fs::write(path, body) {
        tracing::warn!("could not write the update status file: {error}");
    }
}

fn feed_url() -> String {
    std::env::var("BIR_UPDATE_FEED_URL").unwrap_or_else(|_| DEFAULT_FEED_URL.to_string())
}

fn current_platform_key() -> Option<&'static str> {
    let linux = if cfg!(target_os = "linux") {
        let exe = std::env::current_exe().unwrap_or_default();
        let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();
        Some(linux_install_kind(&exe, &os_release))
    } else {
        None
    };
    platform_asset_key(std::env::consts::OS, std::env::consts::ARCH, linux)
}

fn read_auto_download() -> bool {
    let Ok(handle) = crate::bir_module::database_handle() else {
        return true;
    };
    let Ok(db) = handle.lock() else {
        return true;
    };
    match db.get_setting(AUTO_DOWNLOAD_SETTING) {
        Ok(value) => auto_download_enabled(value.as_deref()),
        Err(_) => true,
    }
}

fn lock_state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(concat!("eBIRForms/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| error.to_string())
}

fn require_http(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        Ok(())
    } else {
        Err("the update URL is not http or https".into())
    }
}

fn fetch_manifest(url: &str) -> Result<UpdateManifest, String> {
    require_http(url)?;
    let client = http_client()?;
    let response = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .map_err(|error| format!("Could not download the update. {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Could not download the update. HTTP {}",
            response.status()
        ));
    }
    let text = response
        .text()
        .map_err(|error| format!("The update feed could not be read. {error}"))?;
    if text.len() > 1024 * 1024 {
        return Err("The update feed could not be read. the file is too large".into());
    }
    serde_json::from_str(&text)
        .map_err(|error| format!("The update feed could not be read. {error}"))
}

fn download_verified(asset: &UpdateAsset) -> Result<PathBuf, String> {
    require_http(&asset.url)?;
    let Some(checksum_url) = asset.checksum_url.as_deref() else {
        return Err(VERIFY_REJECTED.to_string());
    };
    require_http(checksum_url)?;
    let dest = cache_path(&asset.name)?;
    let partial = dest.with_extension("partial");
    let client = http_client()?;
    let hash = match download_file(&client, &asset.url, &partial) {
        Ok(hash) => hash,
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(format!("Could not download the update. {error}"));
        }
    };
    let checksum = match download_text(&client, checksum_url) {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!("checksum file was not fetched: {error}");
            let _ = fs::remove_file(&partial);
            return Err(VERIFY_REJECTED.to_string());
        }
    };
    if let Err(reason) = verify_sha256(&hash, &asset.sha256, &checksum, &asset.name) {
        tracing::warn!("checksum rejected: {reason}");
        let _ = fs::remove_file(&partial);
        return Err(VERIFY_REJECTED.to_string());
    }
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    fs::rename(&partial, &dest).map_err(|error| error.to_string())?;
    Ok(dest)
}

fn cache_path(name: &str) -> Result<PathBuf, String> {
    let file = Path::new(name)
        .file_name()
        .ok_or_else(|| "the update has no file name".to_string())?;
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join(file))
}

fn cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BIR_UPDATE_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Caches/com.buwiz.bir/updates");
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("eBIRForms/updates");
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg).join("ebirforms/updates");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache/ebirforms/updates");
    }
    std::env::temp_dir().join("ebirforms-updates")
}

fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
) -> Result<String, String> {
    let mut response = client.get(url).send().map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let mut file = File::create(dest).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let read = response.read(&mut buf).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
        file.write_all(&buf[..read])
            .map_err(|error| error.to_string())?;
    }
    Ok(hex::encode(hasher.finalize()))
}

fn download_text(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let text = response.text().map_err(|error| error.to_string())?;
    if text.len() > 1024 * 1024 {
        return Err("checksum file is too large".into());
    }
    Ok(text)
}

enum Prepared {
    #[cfg(target_os = "macos")]
    Macos { bundle: PathBuf, staged: PathBuf },
    #[cfg(target_os = "windows")]
    Windows { asset: PathBuf },
    #[cfg(target_os = "linux")]
    LinuxPackage {
        asset: PathBuf,
        kind: LinuxInstallKind,
    },
    #[cfg(target_os = "linux")]
    LinuxTree {
        root: PathBuf,
        staged: PathBuf,
        exe_name: std::ffi::OsString,
    },
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Unsupported,
}

fn prepare_install(asset: &Path, name: &str) -> Result<Prepared, String> {
    if !asset.is_file() {
        return Err("the downloaded update is missing".into());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = name;
        prepare_macos(asset)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = name;
        Ok(Prepared::Windows {
            asset: asset.to_path_buf(),
        })
    }
    #[cfg(target_os = "linux")]
    {
        prepare_linux(asset, name)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (asset, name);
        Ok(Prepared::Unsupported)
    }
}

fn commit_install(prepared: Prepared) -> Result<(), String> {
    match prepared {
        #[cfg(target_os = "macos")]
        Prepared::Macos { bundle, staged } => commit_macos(&bundle, &staged),
        #[cfg(target_os = "windows")]
        Prepared::Windows { asset } => {
            spawn_windows_installer(&asset)?;
            Ok(())
        }
        #[cfg(target_os = "linux")]
        Prepared::LinuxPackage { asset, kind } => spawn_pkexec(&asset, kind),
        #[cfg(target_os = "linux")]
        Prepared::LinuxTree {
            root,
            staged,
            exe_name,
        } => commit_linux_tree(&root, &staged, &exe_name),
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        Prepared::Unsupported => Err("updates are not installed on this platform".into()),
    }
}

#[cfg(target_os = "macos")]
fn prepare_macos(dmg: &Path) -> Result<Prepared, String> {
    let bundle = running_app_bundle()?;
    let mount = mount_dmg(dmg)?;
    let outcome = (|| {
        let source = find_app(&mount)
            .ok_or_else(|| "the disk image does not contain an application".to_string())?;
        let staged = sibling(&bundle, ".next");
        if staged.exists() {
            fs::remove_dir_all(&staged).map_err(|error| error.to_string())?;
        }
        let status = std::process::Command::new("ditto")
            .arg(&source)
            .arg(&staged)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("could not copy the updated application".into());
        }
        Ok(Prepared::Macos { bundle, staged })
    })();
    detach_dmg(&mount);
    outcome
}

#[cfg(target_os = "macos")]
fn commit_macos(bundle: &Path, staged: &Path) -> Result<(), String> {
    let previous = sibling(bundle, ".previous");
    if previous.exists() {
        let _ = fs::remove_dir_all(&previous);
    }
    fs::rename(bundle, &previous)
        .map_err(|error| format!("could not move the current application aside: {error}"))?;
    if let Err(error) = fs::rename(staged, bundle) {
        let _ = fs::rename(&previous, bundle);
        return Err(format!("could not put the update in place: {error}"));
    }
    let exe = bundle.join("Contents/MacOS/bir");
    relaunch(&exe)?;
    let _ = fs::remove_dir_all(&previous);
    Ok(())
}

#[cfg(target_os = "macos")]
fn running_app_bundle() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut cursor = exe.as_path();
    loop {
        if cursor.extension().and_then(|ext| ext.to_str()) == Some("app") {
            return Ok(cursor.to_path_buf());
        }
        cursor = cursor.parent().ok_or_else(|| {
            "this copy is not inside an application bundle, so it was not replaced".to_string()
        })?;
    }
}

#[cfg(target_os = "macos")]
fn mount_dmg(dmg: &Path) -> Result<PathBuf, String> {
    let mount = std::env::temp_dir().join(format!(
        "ebirforms-update-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&mount).map_err(|error| error.to_string())?;
    let status = std::process::Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(dmg)
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        let _ = fs::remove_dir_all(&mount);
        return Err("could not mount the update disk image".into());
    }
    Ok(mount)
}

#[cfg(target_os = "macos")]
fn detach_dmg(mount: &Path) {
    let _ = std::process::Command::new("hdiutil")
        .args(["detach", "-force"])
        .arg(mount)
        .status();
    let _ = fs::remove_dir_all(mount);
}

#[cfg(target_os = "macos")]
fn find_app(mount: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(mount).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("app") && path.is_dir() {
            return Some(path);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn spawn_windows_installer(asset: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let path = asset.display();
    let msi = asset
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"));
    // The installer is delayed so this process can exit and unlock the exe.
    // Inno's launch step is skipifsilent, so /VERYSILENT does not relaunch;
    // the installer itself is the hand-off. MSI is quiet and does not reboot.
    let script = if msi {
        format!("timeout /t 2 /nobreak >nul & msiexec /i \"{path}\" /qn /norestart")
    } else {
        format!("timeout /t 2 /nobreak >nul & \"{path}\" /VERYSILENT /NORESTART /SP-")
    };
    std::process::Command::new("cmd")
        .args(["/C", &script])
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|error| format!("could not start the installer: {error}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn prepare_linux(asset: &Path, name: &str) -> Result<Prepared, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let detected = linux_install_kind(&exe, &os_release);
    let kind = kind_for_asset(name, detected);
    if kind == LinuxInstallKind::Tarball {
        let root = tarball_install_root(&exe)?;
        let staged = sibling(&root, ".next");
        if staged.exists() {
            fs::remove_dir_all(&staged).map_err(|error| error.to_string())?;
        }
        fs::create_dir_all(&staged).map_err(|error| error.to_string())?;
        let status = std::process::Command::new("tar")
            .args(["-xzf"])
            .arg(asset)
            .arg("-C")
            .arg(&staged)
            .arg("--strip-components=1")
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            let _ = fs::remove_dir_all(&staged);
            return Err("could not unpack the update".into());
        }
        let exe_name = exe
            .file_name()
            .ok_or_else(|| "the running binary has no name".to_string())?
            .to_os_string();
        return Ok(Prepared::LinuxTree {
            root,
            staged,
            exe_name,
        });
    }
    world_readable(asset)?;
    Ok(Prepared::LinuxPackage {
        asset: asset.to_path_buf(),
        kind,
    })
}

#[cfg(target_os = "linux")]
fn kind_for_asset(name: &str, detected: LinuxInstallKind) -> LinuxInstallKind {
    let name = name.to_ascii_lowercase();
    if name.ends_with(".deb") {
        LinuxInstallKind::Deb
    } else if name.ends_with(".rpm") {
        LinuxInstallKind::Rpm
    } else if name.ends_with(".pkg.tar.zst") {
        LinuxInstallKind::Arch
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        LinuxInstallKind::Tarball
    } else {
        detected
    }
}

#[cfg(target_os = "linux")]
fn tarball_install_root(exe: &Path) -> Result<PathBuf, String> {
    let text = exe.to_string_lossy();
    const MARKER: &str = ".local/share/ebirforms";
    if let Some(index) = text.find(MARKER) {
        return Ok(PathBuf::from(&text[..index + MARKER.len()]));
    }
    let parent = exe
        .parent()
        .ok_or_else(|| "the running binary has no directory".to_string())?;
    if is_system_dir(parent) {
        return Err(
            "this install is a system package, and the release has no matching package".into(),
        );
    }
    Ok(parent.to_path_buf())
}

#[cfg(target_os = "linux")]
fn is_system_dir(path: &Path) -> bool {
    matches!(
        path.to_str(),
        Some(
            "/" | "/usr"
                | "/usr/bin"
                | "/usr/local"
                | "/usr/local/bin"
                | "/bin"
                | "/opt"
                | "/Applications"
        )
    )
}

#[cfg(target_os = "linux")]
fn world_readable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    // pkexec runs as root and must be able to read a file in the user's cache.
    let mut permissions = fs::metadata(path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(path, permissions).map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn spawn_pkexec(asset: &Path, kind: LinuxInstallKind) -> Result<(), String> {
    let mut command = std::process::Command::new("pkexec");
    match kind {
        LinuxInstallKind::Deb => {
            command.args(["dpkg", "-i"]).arg(asset);
        }
        LinuxInstallKind::Rpm => {
            command.args(["rpm", "-U"]).arg(asset);
        }
        LinuxInstallKind::Arch => {
            command.args(["pacman", "-U", "--noconfirm"]).arg(asset);
        }
        LinuxInstallKind::Tarball => {
            return Err("a tarball is not installed with pkexec".into());
        }
    }
    command
        .spawn()
        .map_err(|error| format!("could not start pkexec: {error}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn commit_linux_tree(root: &Path, staged: &Path, exe_name: &std::ffi::OsStr) -> Result<(), String> {
    let previous = sibling(root, ".previous");
    if previous.exists() {
        let _ = fs::remove_dir_all(&previous);
    }
    if root.exists() {
        fs::rename(root, &previous).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(staged, root) {
        let _ = fs::rename(&previous, root);
        return Err(format!("could not put the update in place: {error}"));
    }
    relaunch(&root.join(exe_name))?;
    let _ = fs::remove_dir_all(&previous);
    Ok(())
}

// macOS renames the .app aside and Linux renames a tarball tree. Windows
// starts the verified installer and does not call either helper.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{name}{suffix}"))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn relaunch(exe: &Path) -> Result<(), String> {
    if !exe.is_file() {
        return Err(format!(
            "the updated application has no executable at {}",
            exe.display()
        ));
    }
    let mut command = std::process::Command::new(exe);
    // `open` would send the new app through Launch Services, which raises
    // Gatekeeper dialogs for a just-downloaded bundle. Exec the binary.
    if std::env::var_os("BIR_UPDATE_SHOW_ABOUT_AFTER_INSTALL").is_some() {
        command.env("BIR_SHOW_ABOUT", "1");
    }
    command
        .spawn()
        .map_err(|error| format!("could not relaunch: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(raw: &str) -> SemVer {
        parse_version(raw).unwrap_or_else(|| panic!("{raw} should parse"))
    }

    #[test]
    fn newer_release_compares_above_the_running_version() {
        assert!(version("0.1.1").is_newer_than(&version("0.1.0")));
        assert!(version("0.2.0").is_newer_than(&version("0.1.9")));
        assert!(version("1.0.0").is_newer_than(&version("0.9.9")));
        assert!(!version("0.1.0").is_newer_than(&version("0.1.0")));
        assert!(!version("0.1.0").is_newer_than(&version("0.1.1")));
    }

    #[test]
    fn a_leading_v_parses_and_build_metadata_is_ignored() {
        assert_eq!(parse_version("v0.1.1"), parse_version("0.1.1"));
        assert_eq!(parse_version("0.1.1+build.4"), parse_version("0.1.1"));
        assert!(parse_version("0.1").is_none());
        assert!(parse_version("not-a-version").is_none());
    }

    #[test]
    fn prerelease_is_older_than_the_release() {
        assert!(version("1.0.0").is_newer_than(&version("1.0.0-rc.1")));
        assert!(version("1.0.0-alpha.1").is_newer_than(&version("1.0.0-alpha")));
        assert!(version("1.0.0-alpha.beta").is_newer_than(&version("1.0.0-alpha.1")));
        assert!(version("1.0.0-beta").is_newer_than(&version("1.0.0-alpha.beta")));
        assert!(version("1.0.0-beta.11").is_newer_than(&version("1.0.0-beta.2")));
        assert!(version("1.0.0-rc.1").is_newer_than(&version("1.0.0-beta.11")));
    }

    #[test]
    fn auto_download_turns_off_only_for_explicit_false() {
        assert!(auto_download_enabled(None));
        assert!(auto_download_enabled(Some("")));
        assert!(auto_download_enabled(Some("1")));
        assert!(auto_download_enabled(Some("true")));
        assert!(!auto_download_enabled(Some("0")));
        assert!(!auto_download_enabled(Some("false")));
        assert!(!auto_download_enabled(Some("OFF")));
    }

    fn manifest() -> UpdateManifest {
        let platforms = [
            (
                "darwin-universal",
                "eBIRForms-macOS-universal-0.1.1.dmg",
                "dmg",
            ),
            (
                "windows-x64-setup",
                "eBIRForms-Windows-x64-0.1.1-Setup.exe",
                "exe",
            ),
            ("windows-x64-msi", "eBIRForms-Windows-x64-0.1.1.msi", "msi"),
            ("linux-x64-deb", "eBIRForms-Linux-x64-0.1.1.deb", "deb"),
            ("linux-x64-rpm", "eBIRForms-Linux-x64-0.1.1.rpm", "rpm"),
            (
                "linux-x64-tarball",
                "eBIRForms-Linux-x64-0.1.1.tar.gz",
                "tar",
            ),
            (
                "linux-x64-arch",
                "eBIRForms-Linux-x64-0.1.1.pkg.tar.zst",
                "zst",
            ),
            ("linux-arm64-deb", "eBIRForms-Linux-arm64-0.1.1.deb", "deb"),
            (
                "linux-arm64-tarball",
                "eBIRForms-Linux-arm64-0.1.1.tar.gz",
                "tar",
            ),
        ]
        .into_iter()
        .map(|(key, name, _kind)| {
            (
                key.to_string(),
                UpdateAsset {
                    url: format!("https://example.test/{name}"),
                    sha256: "ab".repeat(32),
                    name: name.to_string(),
                    checksum_url: Some(format!("https://example.test/{name}.sha256")),
                },
            )
        })
        .collect();
        UpdateManifest {
            version: "0.1.1".into(),
            platforms,
        }
    }

    #[test]
    fn each_platform_selects_its_own_asset() {
        let manifest = manifest();
        let name = |key| select_asset(&manifest, key).map(|asset| asset.name.as_str());
        assert_eq!(
            name("darwin-universal"),
            Some("eBIRForms-macOS-universal-0.1.1.dmg")
        );
        assert_eq!(
            name("windows-x64-setup"),
            Some("eBIRForms-Windows-x64-0.1.1-Setup.exe")
        );
        assert_eq!(
            name("windows-x64-msi"),
            Some("eBIRForms-Windows-x64-0.1.1.msi")
        );
        assert_eq!(name("linux-x64-deb"), Some("eBIRForms-Linux-x64-0.1.1.deb"));
        assert_eq!(name("linux-x64-rpm"), Some("eBIRForms-Linux-x64-0.1.1.rpm"));
        assert_eq!(
            name("linux-x64-tarball"),
            Some("eBIRForms-Linux-x64-0.1.1.tar.gz")
        );
        assert_eq!(
            name("linux-x64-arch"),
            Some("eBIRForms-Linux-x64-0.1.1.pkg.tar.zst")
        );
        assert_eq!(
            name("linux-arm64-deb"),
            Some("eBIRForms-Linux-arm64-0.1.1.deb")
        );
        assert_eq!(
            name("linux-arm64-tarball"),
            Some("eBIRForms-Linux-arm64-0.1.1.tar.gz")
        );
        assert_eq!(name("linux-arm64-arch"), None);
    }

    #[test]
    fn windows_setup_falls_back_to_the_msi() {
        let mut manifest = manifest();
        manifest.platforms.remove("windows-x64-setup");
        let asset = select_asset(&manifest, "windows-x64-setup").expect("msi fallback");
        assert_eq!(asset.name, "eBIRForms-Windows-x64-0.1.1.msi");
    }

    #[test]
    fn asset_keys_follow_the_install_layout() {
        assert_eq!(
            platform_asset_key("macos", "aarch64", None),
            Some("darwin-universal")
        );
        assert_eq!(
            platform_asset_key("macos", "x86_64", None),
            Some("darwin-universal")
        );
        assert_eq!(
            platform_asset_key("windows", "x86_64", None),
            Some("windows-x64-setup")
        );
        assert_eq!(
            platform_asset_key("linux", "x86_64", Some(LinuxInstallKind::Deb)),
            Some("linux-x64-deb")
        );
        assert_eq!(
            platform_asset_key("linux", "aarch64", Some(LinuxInstallKind::Rpm)),
            Some("linux-arm64-rpm")
        );
        assert_eq!(
            platform_asset_key("linux", "x86_64", Some(LinuxInstallKind::Arch)),
            Some("linux-x64-arch")
        );
        assert_eq!(
            platform_asset_key("linux", "aarch64", Some(LinuxInstallKind::Arch)),
            None
        );
        assert_eq!(
            platform_asset_key("linux", "x86_64", Some(LinuxInstallKind::Tarball)),
            Some("linux-x64-tarball")
        );
        assert_eq!(
            linux_install_kind(
                Path::new("/usr/bin/ebirforms"),
                "ID=ubuntu\nID_LIKE=debian\n"
            ),
            LinuxInstallKind::Deb
        );
        assert_eq!(
            linux_install_kind(Path::new("/usr/bin/ebirforms"), "ID=fedora\n"),
            LinuxInstallKind::Rpm
        );
        assert_eq!(
            linux_install_kind(Path::new("/usr/bin/ebirforms"), "ID=arch\n"),
            LinuxInstallKind::Arch
        );
        assert_eq!(
            linux_install_kind(
                Path::new("/home/me/.local/share/ebirforms/bir"),
                "ID=ubuntu\n"
            ),
            LinuxInstallKind::Tarball
        );
    }

    fn sample_bytes() -> &'static [u8] {
        b"ebirforms-update"
    }

    #[test]
    fn a_matching_checksum_is_accepted() {
        let hash = sha256_hex(sample_bytes());
        let sidecar = format!("{hash}  eBIRForms-macOS-universal-0.1.1.dmg\n");
        assert!(
            verify_sha256(
                &hash,
                &hash,
                &sidecar,
                "eBIRForms-macOS-universal-0.1.1.dmg"
            )
            .is_ok()
        );
        let starred = format!("{hash} *eBIRForms-Linux-x64-0.1.1.deb\n");
        assert!(verify_sha256(&hash, &hash, &starred, "eBIRForms-Linux-x64-0.1.1.deb").is_ok());
        assert!(verify_sha256(&hash, &hash.to_uppercase(), &hash, "anything.dmg").is_ok());
    }

    #[test]
    fn a_bad_manifest_checksum_is_rejected() {
        let hash = sha256_hex(sample_bytes());
        let other = "ab".repeat(32);
        let sidecar = format!("{hash}  app.dmg\n");
        let error = verify_sha256(&hash, &other, &sidecar, "app.dmg").unwrap_err();
        assert_eq!(error, "manifest sha256 does not match the download");
    }

    #[test]
    fn a_bad_checksum_file_is_rejected() {
        let hash = sha256_hex(sample_bytes());
        let other = "cd".repeat(32);
        let sidecar = format!("{other}  app.dmg\n");
        let error = verify_sha256(&hash, &hash, &sidecar, "app.dmg").unwrap_err();
        assert_eq!(error, "checksum file does not match the download");
    }

    #[test]
    fn a_checksum_file_for_a_different_asset_is_rejected() {
        let hash = sha256_hex(sample_bytes());
        let sidecar = format!("{hash}  other.dmg\n{hash}  third.dmg\n");
        let error = verify_sha256(&hash, &hash, &sidecar, "app.dmg").unwrap_err();
        assert_eq!(error, "checksum file has no entry for this asset");
    }
}
