//! Developer switches behind the desktop's Dev Mode panel.
//!
//! Every switch exists only in debug builds. A release binary resolves each
//! one to its release default no matter what the database or environment
//! says, so nothing here can be flipped in a shipped app.
//!
//! Precedence for each switch: an environment variable (when set) wins, then
//! the value saved in the settings table under a `dev.` key, then the build's
//! dev default. `bir-core` has no global database handle, so the desktop
//! loads the saved values once at startup ([`load_saved`]) and again whenever
//! the panel changes one ([`save`]); they live in a small process-wide map.
//!
//! "Live BIR" is never a panel option. Live sending still needs
//! `BIR_SFTP_LIVE=1` at launch, exactly as before.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::db::{Database, DbError};

/// Settings-table keys. All start with `dev.`.
pub mod keys {
    pub const SUBMISSION_MODE: &str = "dev.submission_mode";
    pub const SFTP_HOST: &str = "dev.sftp.host";
    pub const SFTP_PORT: &str = "dev.sftp.port";
    pub const SFTP_USERNAME: &str = "dev.sftp.username";
    pub const SFTP_FOLDER: &str = "dev.sftp.folder";
    pub const SFTP_HOST_KEY_SHA256: &str = "dev.sftp.host_key_sha256";
    pub const SFTP_ACCEPT_ANY_HOST_KEY: &str = "dev.sftp.accept_any_host_key";
    pub const RELAXED_TIN: &str = "dev.relaxed_tin";
    pub const AUTO_CONFIRM: &str = "dev.auto_confirm";
    pub const SHOW_UNCERTIFIED_FORMS: &str = "dev.show_uncertified_forms";
    pub const FORM_DIAGNOSTICS: &str = "dev.form_diagnostics";
    pub const EXAMPLE_ALERTS: &str = "dev.example_alerts";
    pub const VERBOSE_LOGS: &str = "dev.verbose_logs";
    pub const SEPARATE_DATABASE: &str = "dev.separate_database";

    pub const ALL: [&str; 14] = [
        SUBMISSION_MODE,
        SFTP_HOST,
        SFTP_PORT,
        SFTP_USERNAME,
        SFTP_FOLDER,
        SFTP_HOST_KEY_SHA256,
        SFTP_ACCEPT_ANY_HOST_KEY,
        RELAXED_TIN,
        AUTO_CONFIRM,
        SHOW_UNCERTIFIED_FORMS,
        FORM_DIAGNOSTICS,
        EXAMPLE_ALERTS,
        VERBOSE_LOGS,
        SEPARATE_DATABASE,
    ];
}

/// Environment variables that override panel values.
pub mod env {
    pub const SFTP_DRY_RUN: &str = "BIR_SFTP_DRY_RUN";
    pub const SFTP_LIVE: &str = "BIR_SFTP_LIVE";
    pub const SFTP_HOST: &str = "BIR_SFTP_HOST";
    pub const SFTP_PORT: &str = "BIR_SFTP_PORT";
    pub const SFTP_USERNAME: &str = "BIR_SFTP_USERNAME";
    pub const SFTP_USER: &str = "BIR_SFTP_USER";
    pub const SFTP_PASSWORD: &str = "BIR_SFTP_PASSWORD";
    pub const SFTP_FOLDER: &str = "BIR_SFTP_FOLDER";
    pub const SFTP_HOST_KEY_SHA256: &str = "BIR_SFTP_HOST_KEY_SHA256";
    pub const SFTP_ACCEPT_ANY_HOST_KEY: &str = "BIR_SFTP_ACCEPT_ANY_HOST_KEY";
    pub const RELAXED_TIN: &str = "EBIR_RELAXED_VALIDATION";
    pub const EXAMPLE_ALERTS: &str = "BIR_SEED_EXAMPLE_ALERTS";
    pub const VERBOSE_LOGS: &str = "DEVELOPER_MODE";
    pub const DATABASE_PATH: &str = "BIR_DATABASE_PATH";
}

/// OS keychain entry for the test SFTP server password.
const KEYCHAIN_SERVICE: &str = "com.ebir.rust";
const KEYCHAIN_ACCOUNT: &str = "dev_sftp_password";

/// File name of the separate dev database, next to the normal one.
pub const DEV_DATABASE_FILE: &str = "bir_data.dev.db";
/// Startup-only switches mirrored outside the database, because they are
/// needed before the database is open (log level, which database to open).
const STARTUP_FLAGS_FILE: &str = "dev-startup-flags";

static SAVED: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

/// True only in debug builds. Every resolver checks this first.
pub const fn is_debug_build() -> bool {
    cfg!(debug_assertions)
}

/// Where a resolved value came from, for the panel's labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An environment variable set at launch. The panel control is disabled.
    Env(&'static str),
    Saved,
    DevDefault,
    /// Release build: the switch does not exist.
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved<T> {
    pub value: T,
    pub source: Source,
}

impl<T> Resolved<T> {
    pub fn env_name(&self) -> Option<&'static str> {
        match self.source {
            Source::Env(name) => Some(name),
            _ => None,
        }
    }
}

/// Submission switch in the panel. Live is deliberately absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionMode {
    DryRun,
    TestServer,
}

impl SubmissionMode {
    pub fn as_setting(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::TestServer => "test_server",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim() {
            "dry_run" => Some(Self::DryRun),
            "test_server" => Some(Self::TestServer),
            _ => None,
        }
    }
}

/// What actually governs submission right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionControl {
    /// `BIR_SFTP_DRY_RUN` / `BIR_SFTP_LIVE` / `BIR_SFTP_HOST` is set (or this
    /// is a release build, or the panel was never loaded): transport uses the
    /// environment exactly as before.
    Environment,
    /// Debug build, no env override: the panel decides.
    Panel(SubmissionMode),
}

// ---------------------------------------------------------------------------
// Saved values
// ---------------------------------------------------------------------------

/// Read every `dev.` setting into the process-wide map. Call at startup.
pub fn load_saved(db: &Database) {
    let mut map = HashMap::new();
    for key in keys::ALL {
        if let Ok(Some(value)) = db.get_setting(key) {
            map.insert(key.to_string(), value);
        }
    }
    install_saved(map);
}

/// Replace the process-wide map (startup and tests).
pub fn install_saved(map: HashMap<String, String>) {
    if let Ok(mut guard) = SAVED.write() {
        *guard = Some(map);
    }
}

/// Persist one panel value and update the process-wide map. Refused in
/// release builds and for keys outside `dev.`.
pub fn save(db: &Database, key: &str, value: &str) -> Result<(), DbError> {
    if !is_debug_build() {
        return Err(DbError::Other(
            "Dev Mode settings are unavailable in release builds".into(),
        ));
    }
    if !key.starts_with("dev.") {
        return Err(DbError::Other(format!("not a Dev Mode setting: {key}")));
    }
    db.set_setting(key, value)?;
    if let Ok(mut guard) = SAVED.write() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), value.to_string());
    }
    Ok(())
}

/// Whether the desktop has loaded the panel values in this process.
fn panel_loaded() -> bool {
    SAVED.read().map(|guard| guard.is_some()).unwrap_or(false)
}

fn saved(key: &str) -> Option<String> {
    SAVED
        .read()
        .ok()?
        .as_ref()?
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn saved_value(key: &str) -> Option<String> {
    if is_debug_build() { saved(key) } else { None }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn parse_truthy(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Pure resolvers (unit-tested). The public wrappers feed them the live env.
// ---------------------------------------------------------------------------

/// A set environment variable: its name, value, and what the value means.
pub type EnvFlag<'a> = (&'static str, &'a str, fn(&str) -> bool);

/// Resolve one boolean switch. `env` is `(name, value, parse)`; `parse`
/// turns a set variable into its meaning (env vars keep the exact semantics
/// they had before the panel existed).
pub fn resolve_flag(
    debug: bool,
    env: Option<EnvFlag<'_>>,
    saved: Option<&str>,
    dev_default: bool,
    release_default: bool,
) -> Resolved<bool> {
    if !debug {
        return Resolved {
            value: release_default,
            source: Source::Release,
        };
    }
    if let Some((name, value, parse)) = env {
        return Resolved {
            value: parse(value),
            source: Source::Env(name),
        };
    }
    if let Some(value) = saved.and_then(parse_truthy) {
        return Resolved {
            value,
            source: Source::Saved,
        };
    }
    Resolved {
        value: dev_default,
        source: Source::DevDefault,
    }
}

fn env_truthy(value: &str) -> bool {
    parse_truthy(value) == Some(true)
}

fn env_present(_value: &str) -> bool {
    true
}

fn env_is_true_literal(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("true")
}

/// Which env var (if any) takes submission away from the panel.
pub fn submission_env_override_from(get: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    [env::SFTP_LIVE, env::SFTP_DRY_RUN, env::SFTP_HOST]
        .into_iter()
        .find(|name| get(name).is_some())
}

pub fn resolve_submission(
    debug: bool,
    loaded: bool,
    env_override: Option<&'static str>,
    saved: Option<&str>,
) -> Resolved<SubmissionControl> {
    if !debug {
        return Resolved {
            value: SubmissionControl::Environment,
            source: Source::Release,
        };
    }
    if let Some(name) = env_override {
        return Resolved {
            value: SubmissionControl::Environment,
            source: Source::Env(name),
        };
    }
    if !loaded {
        // The panel never ran in this process (CLI, tests, headless):
        // behave exactly as before the panel existed.
        return Resolved {
            value: SubmissionControl::Environment,
            source: Source::DevDefault,
        };
    }
    match saved.and_then(SubmissionMode::from_setting) {
        Some(mode) => Resolved {
            value: SubmissionControl::Panel(mode),
            source: Source::Saved,
        },
        None => Resolved {
            value: SubmissionControl::Panel(SubmissionMode::DryRun),
            source: Source::DevDefault,
        },
    }
}

// ---------------------------------------------------------------------------
// Public resolvers
// ---------------------------------------------------------------------------

pub fn submission() -> Resolved<SubmissionControl> {
    resolve_submission(
        is_debug_build(),
        panel_loaded(),
        submission_env_override_from(env_value),
        saved(keys::SUBMISSION_MODE).as_deref(),
    )
}

/// The panel's submission mode, or `None` when the environment (or a
/// release build) governs submission.
pub fn panel_submission_mode() -> Option<SubmissionMode> {
    match submission().value {
        SubmissionControl::Panel(mode) => Some(mode),
        SubmissionControl::Environment => None,
    }
}

/// `BIR_SFTP_LIVE=1` was set at launch. Shown as a red pill.
pub fn live_bir_env() -> bool {
    env_value(env::SFTP_LIVE).is_some_and(|value| env_truthy(&value))
}

pub fn relaxed_tin() -> Resolved<bool> {
    let raw = env_value(env::RELAXED_TIN);
    resolve_flag(
        is_debug_build(),
        raw.as_deref()
            .map(|v| (env::RELAXED_TIN, v, env_truthy as fn(&str) -> bool)),
        saved(keys::RELAXED_TIN).as_deref(),
        false,
        false,
    )
}

pub fn auto_confirm() -> Resolved<bool> {
    resolve_flag(
        is_debug_build(),
        None,
        saved(keys::AUTO_CONFIRM).as_deref(),
        false,
        false,
    )
}

/// `dev_tools` is the desktop's `dev-tools` cargo feature: it keeps its old
/// meaning (certification drafts visible) in any build.
pub fn show_uncertified_forms(dev_tools: bool) -> Resolved<bool> {
    let resolved = resolve_flag(
        is_debug_build(),
        None,
        saved(keys::SHOW_UNCERTIFIED_FORMS).as_deref(),
        true,
        false,
    );
    if !is_debug_build() && dev_tools {
        return Resolved {
            value: true,
            source: Source::Release,
        };
    }
    resolved
}

/// Developer diagnostics on forms. The `dev-tools` feature forces it on.
pub fn form_diagnostics(dev_tools: bool) -> Resolved<bool> {
    if dev_tools {
        return Resolved {
            value: true,
            source: Source::Env("--features dev-tools"),
        };
    }
    resolve_flag(
        is_debug_build(),
        None,
        saved(keys::FORM_DIAGNOSTICS).as_deref(),
        false,
        false,
    )
}

pub fn example_alerts() -> Resolved<bool> {
    let raw = std::env::var_os(env::EXAMPLE_ALERTS).map(|v| v.to_string_lossy().into_owned());
    resolve_flag(
        is_debug_build(),
        raw.as_deref()
            .map(|v| (env::EXAMPLE_ALERTS, v, env_present as fn(&str) -> bool)),
        saved(keys::EXAMPLE_ALERTS).as_deref(),
        false,
        false,
    )
}

/// Verbose logs. Read at tracing init, before the database is open, so the
/// saved value comes from the startup-flags file the panel mirrors.
pub fn verbose_logs() -> Resolved<bool> {
    let raw = std::env::var(env::VERBOSE_LOGS).ok();
    resolve_flag(
        is_debug_build(),
        raw.as_deref().map(|v| {
            (
                env::VERBOSE_LOGS,
                v,
                env_is_true_literal as fn(&str) -> bool,
            )
        }),
        startup_flag(keys::VERBOSE_LOGS).as_deref(),
        false,
        false,
    )
}

/// "Use a separate dev database". Same mechanism as `BIR_DATABASE_PATH`,
/// which wins when set.
pub fn separate_database() -> Resolved<bool> {
    let env_set = std::env::var_os(env::DATABASE_PATH).is_some_and(|v| !v.is_empty());
    resolve_flag(
        is_debug_build(),
        env_set.then_some((env::DATABASE_PATH, "", env_present as fn(&str) -> bool)),
        startup_flag(keys::SEPARATE_DATABASE).as_deref(),
        false,
        false,
    )
}

/// Path of the separate dev database, when that switch is on and the
/// environment does not already pick a database.
pub fn dev_database_path() -> Option<std::path::PathBuf> {
    let resolved = separate_database();
    (resolved.value && resolved.source != Source::Env(env::DATABASE_PATH))
        .then(|| crate::platform::data_dir().join(DEV_DATABASE_FILE))
}

// ---------------------------------------------------------------------------
// Startup flags (verbose logs, separate database)
// ---------------------------------------------------------------------------

fn startup_flags_path() -> std::path::PathBuf {
    crate::platform::data_dir().join(STARTUP_FLAGS_FILE)
}

pub fn parse_startup_flags(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .filter(|(key, _)| key.starts_with("dev."))
        .collect()
}

fn startup_flag(key: &str) -> Option<String> {
    if !is_debug_build() {
        return None;
    }
    let text = std::fs::read_to_string(startup_flags_path()).ok()?;
    parse_startup_flags(&text).remove(key)
}

/// Save a startup-only switch: the settings table (like every other dev
/// switch) plus the startup-flags file read before the database opens.
pub fn save_startup_flag(db: &Database, key: &str, value: bool) -> Result<(), DbError> {
    let text = if value { "true" } else { "false" };
    save(db, key, text)?;
    let path = startup_flags_path();
    let mut flags = std::fs::read_to_string(&path)
        .map(|text| parse_startup_flags(&text))
        .unwrap_or_default();
    flags.insert(key.to_string(), text.to_string());
    let mut lines: Vec<String> = flags.iter().map(|(k, v)| format!("{k}={v}")).collect();
    lines.sort();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, lines.join("\n") + "\n")
        .map_err(|error| DbError::Other(format!("could not write {}: {error}", path.display())))
}

// ---------------------------------------------------------------------------
// Test SFTP server
// ---------------------------------------------------------------------------

/// One test-server field with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub value: String,
    pub env: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestServerFields {
    pub host: Field,
    pub port: Field,
    pub username: Field,
    pub folder: Field,
    pub host_key_sha256: Field,
    pub accept_any_host_key: Resolved<bool>,
    /// `BIR_SFTP_PASSWORD` is set (the keychain value is then ignored).
    pub password_env: bool,
}

fn field(env_names: &[&'static str], key: &str) -> Field {
    for name in env_names {
        if let Some(value) = env_value(name) {
            return Field {
                value,
                env: Some(name),
            };
        }
    }
    Field {
        value: saved(key).unwrap_or_default(),
        env: None,
    }
}

/// Test-server fields, env first. Empty in release builds.
pub fn test_server_fields() -> TestServerFields {
    if !is_debug_build() {
        let empty = Field {
            value: String::new(),
            env: None,
        };
        return TestServerFields {
            host: empty.clone(),
            port: empty.clone(),
            username: empty.clone(),
            folder: empty.clone(),
            host_key_sha256: empty,
            accept_any_host_key: Resolved {
                value: false,
                source: Source::Release,
            },
            password_env: false,
        };
    }
    let accept_raw = env_value(env::SFTP_ACCEPT_ANY_HOST_KEY);
    TestServerFields {
        host: field(&[env::SFTP_HOST], keys::SFTP_HOST),
        port: field(&[env::SFTP_PORT], keys::SFTP_PORT),
        username: field(&[env::SFTP_USERNAME, env::SFTP_USER], keys::SFTP_USERNAME),
        folder: field(&[env::SFTP_FOLDER], keys::SFTP_FOLDER),
        host_key_sha256: field(&[env::SFTP_HOST_KEY_SHA256], keys::SFTP_HOST_KEY_SHA256),
        accept_any_host_key: resolve_flag(
            true,
            accept_raw.as_deref().map(|v| {
                (
                    env::SFTP_ACCEPT_ANY_HOST_KEY,
                    v,
                    env_truthy as fn(&str) -> bool,
                )
            }),
            saved(keys::SFTP_ACCEPT_ANY_HOST_KEY).as_deref(),
            false,
            false,
        ),
        password_env: env_value(env::SFTP_PASSWORD).is_some(),
    }
}

/// Store the test-server password in the OS keychain. Never in settings.
pub fn set_test_server_password(password: &str) -> Result<(), String> {
    if !is_debug_build() {
        return Err("Dev Mode is unavailable in release builds".into());
    }
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|error| format!("Could not open OS keychain: {error}"))?;
    if password.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!(
                "Could not remove password from OS keychain: {error}"
            )),
        };
    }
    entry
        .set_password(password)
        .map_err(|error| format!("Could not store password in OS keychain: {error}"))
}

/// Whether a test-server password is stored in the keychain.
pub fn has_test_server_password() -> bool {
    is_debug_build() && keychain_password().is_some()
}

fn keychain_password() -> Option<String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()?
        .get_password()
        .ok()
        .filter(|value| !value.is_empty())
}

/// Endpoint for the panel's "Test server" mode. Env fields win per field;
/// the password comes from `BIR_SFTP_PASSWORD` or the keychain.
pub fn test_server_endpoint(
    form_type: &str,
) -> Result<crate::transport::SftpEndpoint, crate::transport::TransportError> {
    use crate::transport::{HostKeyPolicy, TransportError};
    if !is_debug_build() {
        return Err(TransportError::Config(
            "Dev Mode test server is unavailable in release builds".into(),
        ));
    }
    let fields = test_server_fields();
    let missing = |what: &str| {
        TransportError::Config(format!(
            "Dev Mode test server: {what} is missing (set it in the Dev Mode panel)"
        ))
    };
    if fields.host.value.is_empty() {
        return Err(missing("host"));
    }
    if fields.username.value.is_empty() {
        return Err(missing("username"));
    }
    let password = env_value(env::SFTP_PASSWORD)
        .or_else(keychain_password)
        .ok_or_else(|| missing("password"))?;
    let port = if fields.port.value.is_empty() {
        22
    } else {
        fields.port.value.parse::<u16>().map_err(|error| {
            TransportError::Config(format!("Dev Mode test server: invalid port: {error}"))
        })?
    };
    let host_key_policy = if !fields.host_key_sha256.value.is_empty() {
        HostKeyPolicy::PinnedSha256(fields.host_key_sha256.value.to_ascii_lowercase())
    } else if fields.accept_any_host_key.value {
        HostKeyPolicy::AcceptAnyLab
    } else {
        return Err(missing(
            "host-key SHA-256 (or turn on \"accept any host key\")",
        ));
    };
    let remote_folder = if fields.folder.value.is_empty() {
        form_type.to_string()
    } else {
        fields.folder.value
    };
    Ok(crate::transport::SftpEndpoint {
        host: fields.host.value,
        port,
        username: fields.username.value,
        password: zeroize::Zeroizing::new(password),
        remote_folder,
        host_key_policy,
    })
}

// ---------------------------------------------------------------------------
// Confirmation: simulate and auto-confirm
// ---------------------------------------------------------------------------

/// Build the receipt BIR's email would carry for `submission_filename`.
fn simulated_receipt(submission_filename: &str) -> crate::receipt::BirReceiptConfirmation {
    let stem = submission_filename
        .strip_suffix(".xml")
        .unwrap_or(submission_filename);
    let base = stem.split('#').next().unwrap_or(stem);
    // BIR stamps receipts in Philippine time; a minute ahead keeps the
    // receipt after the submission timestamp it must follow.
    let local_now = chrono::Utc::now() + chrono::Duration::hours(8) + chrono::Duration::minutes(1);
    crate::receipt::BirReceiptConfirmation {
        filename: format!("{base}.xml"),
        date_received: local_now.date_naive(),
        time_received: local_now.time(),
        source_from: Some("dev-mode-simulated@localhost".to_string()),
        raw_text: "Dev Mode: simulated BIR confirmation (nothing was sent to BIR)".to_string(),
        raw_html: None,
    }
}

/// Move a Submitted return to Confirmed through the same path the email
/// confirmation uses (`save_submission_receipt` +
/// `confirm_submission_from_receipt`). Debug builds only.
pub fn simulate_bir_confirmation(
    db: &Database,
    submission_filename: &str,
) -> Result<crate::db::ReceiptConfirmationOutcome, DbError> {
    if !is_debug_build() {
        return Err(DbError::Other(
            "Simulated BIR confirmation is unavailable in release builds".into(),
        ));
    }
    let receipt = simulated_receipt(submission_filename);
    let (saved, _) = db.save_submission_receipt(&receipt)?;
    db.confirm_submission_from_receipt(&saved)
}

/// Pure gate for auto-confirm: on, debug, and never a live BIR send.
pub fn auto_confirm_allowed(
    debug: bool,
    enabled: bool,
    source: crate::transport::SftpEndpointSource,
    live_env: bool,
) -> bool {
    use crate::transport::SftpEndpointSource;
    debug
        && enabled
        && !live_env
        && matches!(source, SftpEndpointSource::DryRun | SftpEndpointSource::Env)
}

/// Called by the queue worker right after a return reaches Submitted.
pub fn maybe_auto_confirm(
    db: &Database,
    source: crate::transport::SftpEndpointSource,
    submission_filename: &str,
) {
    if !auto_confirm_allowed(
        is_debug_build(),
        auto_confirm().value,
        source,
        live_bir_env(),
    ) {
        return;
    }
    match simulate_bir_confirmation(db, submission_filename) {
        Ok(outcome) => tracing::info!(
            ?outcome,
            file = submission_filename,
            "Dev Mode auto-confirm"
        ),
        Err(error) => tracing::warn!(%error, "Dev Mode auto-confirm failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::SftpEndpointSource;

    fn truthy(v: &str) -> bool {
        env_truthy(v)
    }

    #[test]
    fn flag_default_saved_env_and_release() {
        let f = truthy as fn(&str) -> bool;
        // default
        let r = resolve_flag(true, None, None, true, false);
        assert_eq!((r.value, r.source), (true, Source::DevDefault));
        // saved
        let r = resolve_flag(true, None, Some("false"), true, false);
        assert_eq!((r.value, r.source), (false, Source::Saved));
        // unparseable saved falls back to default
        let r = resolve_flag(true, None, Some("maybe"), true, false);
        assert_eq!(r.source, Source::DevDefault);
        // env override beats saved
        let r = resolve_flag(true, Some(("X", "1", f)), Some("false"), false, false);
        assert_eq!((r.value, r.source), (true, Source::Env("X")));
        let r = resolve_flag(true, Some(("X", "0", f)), Some("true"), false, false);
        assert_eq!((r.value, r.source), (false, Source::Env("X")));
        // release forced to release default
        let r = resolve_flag(false, Some(("X", "1", f)), Some("true"), true, false);
        assert_eq!((r.value, r.source), (false, Source::Release));
    }

    #[test]
    fn relaxed_tin_env_semantics_are_unchanged() {
        for v in ["1", "true", "yes", "on", "TRUE"] {
            assert!(env_truthy(v), "{v}");
        }
        for v in ["0", "false", "enabled", ""] {
            assert!(!env_truthy(v), "{v}");
        }
    }

    #[test]
    fn example_alerts_env_is_presence_and_verbose_is_true_literal() {
        assert!(env_present(""));
        assert!(env_is_true_literal("TRUE"));
        assert!(!env_is_true_literal("1"));
    }

    #[test]
    fn submission_default_saved_env_and_release() {
        // default (panel loaded, nothing saved) is dry run
        let r = resolve_submission(true, true, None, None);
        assert_eq!(r.value, SubmissionControl::Panel(SubmissionMode::DryRun));
        assert_eq!(r.source, Source::DevDefault);
        // saved test server
        let r = resolve_submission(true, true, None, Some("test_server"));
        assert_eq!(
            r.value,
            SubmissionControl::Panel(SubmissionMode::TestServer)
        );
        assert_eq!(r.source, Source::Saved);
        // "live" can never be saved into the panel
        let r = resolve_submission(true, true, None, Some("live"));
        assert_eq!(r.value, SubmissionControl::Panel(SubmissionMode::DryRun));
        // env override hands control back to the environment
        let r = resolve_submission(true, true, Some("BIR_SFTP_LIVE"), Some("test_server"));
        assert_eq!(r.value, SubmissionControl::Environment);
        assert_eq!(r.source, Source::Env("BIR_SFTP_LIVE"));
        // panel never loaded: old behaviour
        let r = resolve_submission(true, false, None, Some("test_server"));
        assert_eq!(r.value, SubmissionControl::Environment);
        // release
        let r = resolve_submission(false, true, None, Some("test_server"));
        assert_eq!(r.value, SubmissionControl::Environment);
        assert_eq!(r.source, Source::Release);
    }

    #[test]
    fn submission_env_override_detects_each_variable() {
        assert_eq!(submission_env_override_from(|_| None), None);
        for name in [env::SFTP_LIVE, env::SFTP_DRY_RUN, env::SFTP_HOST] {
            let got = submission_env_override_from(|n| (n == name).then(|| "1".to_string()));
            assert_eq!(got, Some(name));
        }
    }

    #[test]
    fn auto_confirm_never_for_live_or_release() {
        assert!(auto_confirm_allowed(
            true,
            true,
            SftpEndpointSource::DryRun,
            false
        ));
        assert!(auto_confirm_allowed(
            true,
            true,
            SftpEndpointSource::Env,
            false
        ));
        assert!(!auto_confirm_allowed(
            true,
            true,
            SftpEndpointSource::Dispatcher,
            false
        ));
        assert!(!auto_confirm_allowed(
            true,
            true,
            SftpEndpointSource::Env,
            true
        ));
        assert!(!auto_confirm_allowed(
            true,
            false,
            SftpEndpointSource::DryRun,
            false
        ));
        assert!(!auto_confirm_allowed(
            false,
            true,
            SftpEndpointSource::DryRun,
            false
        ));
    }

    #[test]
    fn startup_flags_parse_only_dev_keys() {
        let flags = parse_startup_flags("dev.verbose_logs=true\nother=1\n junk\n");
        assert_eq!(
            flags.get(keys::VERBOSE_LOGS).map(String::as_str),
            Some("true")
        );
        assert_eq!(flags.len(), 1);
    }

    #[test]
    fn simulated_receipt_uses_the_base_filename() {
        let receipt = simulated_receipt("00000000000000-1601Cv2018-092026#a@b.com#.xml");
        assert_eq!(receipt.filename, "00000000000000-1601Cv2018-092026.xml");
    }

    #[test]
    fn simulate_confirmation_ignores_a_filename_nothing_was_submitted_under() {
        let db = Database::open_in_memory_for_tests().unwrap();
        // Nothing submitted under that name: the shared path ignores it.
        let outcome =
            simulate_bir_confirmation(&db, "123456789000-2551Qv2018-122026Q1.xml").unwrap();
        assert_eq!(outcome, crate::db::ReceiptConfirmationOutcome::Ignored);
    }

    #[test]
    fn submission_mode_round_trips() {
        for mode in [SubmissionMode::DryRun, SubmissionMode::TestServer] {
            assert_eq!(SubmissionMode::from_setting(mode.as_setting()), Some(mode));
        }
    }
}
