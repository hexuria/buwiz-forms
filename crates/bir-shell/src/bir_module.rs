//! The `bir` host module: the JavaScript-facing bridge into bir-core.
//!
//! Everything here crosses the boundary as `HostValue` — JSON-shaped records,
//! never Rust types. Functions are synchronous (main thread, no engine
//! re-entry); slow work (SFTP, exports) gets an `async_function` in a later
//! slice. The database opens lazily through `open_live_database` so
//! `BIR_DATABASE_PATH` keeps working for tests exactly like painted `bir`.

use std::cell::RefCell;

use bir_core::db::{self, Database};
use bir_core::forms::FORM_REGISTRY;
use bir_core::forms::queueable::QueueableForm as _;
use gpui_shell::{HostError, HostModule, HostObject, HostResult, HostValue};

use crate::auth_limiter;

thread_local! {
    static DATABASE: RefCell<Option<Database>> = const { RefCell::new(None) };
}

/// One connection for the process — the same owner-lock discipline as painted
/// `bir` (a second app instance cannot take it while we hold it).
pub(crate) fn with_database<R>(
    f: impl FnOnce(&Database) -> Result<R, HostError>,
) -> Result<R, HostError> {
    DATABASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let (database, _path, _recovered) =
                db::open_live_database().map_err(|error| HostError::new(error.to_string()))?;
            *slot = Some(database);
        }
        f(slot.as_ref().expect("database opened"))
    })
}

/// Tests install a fresh ephemeral database in this thread's slot, so each
/// test (one thread each) runs against its own scratch data and never opens
/// the live database through `BIR_DATABASE_PATH`.
#[cfg(test)]
pub(crate) fn use_ephemeral_database() {
    DATABASE.with(|slot| {
        *slot.borrow_mut() = Some(Database::open_ephemeral().expect("ephemeral database opens"));
    });
}

/// `serde_json::Value` → `HostValue`. bir-core serializes cleanly; the
/// boundary keeps JSON's six shapes exactly.
pub(crate) fn json(value: serde_json::Value) -> HostValue {
    match value {
        serde_json::Value::Null => HostValue::Null,
        serde_json::Value::Bool(v) => HostValue::Bool(v),
        serde_json::Value::Number(v) => HostValue::Number(v.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(v) => HostValue::Str(v),
        serde_json::Value::Array(v) => HostValue::Array(v.into_iter().map(json).collect()),
        serde_json::Value::Object(v) => HostValue::Object(
            v.into_iter()
                .map(|(key, value)| (key, json(value)))
                .collect(),
        ),
    }
}

pub(crate) fn serialize<T: serde::Serialize>(value: &T) -> Result<HostValue, HostError> {
    serde_json::to_value(value)
        .map(json)
        .map_err(|error| HostError::new(error.to_string()))
}

/// `HostValue` → `serde_json::Value`: the inverse of `json`, for functions
/// that take structured arguments (a whole profile record, a draft).
pub(crate) fn to_json(value: &HostValue) -> serde_json::Value {
    match value {
        HostValue::Null => serde_json::Value::Null,
        HostValue::Bool(v) => serde_json::Value::Bool(*v),
        // Script numbers arrive as f64, but typed drafts declare ints — an
        // integral double must round-trip as a JSON integer or serde rejects
        // `0.0` for `u16` fields.
        HostValue::Number(v) => {
            if v.fract() == 0.0 && *v >= i64::MIN as f64 && *v <= i64::MAX as f64 {
                serde_json::json!(*v as i64)
            } else {
                serde_json::json!(v)
            }
        }
        HostValue::Str(v) => serde_json::Value::String(v.clone()),
        HostValue::Array(v) => serde_json::Value::Array(v.iter().map(to_json).collect()),
        HostValue::Object(v) => serde_json::Value::Object(
            v.iter()
                .map(|(key, value)| (key.clone(), to_json(value)))
                .collect(),
        ),
    }
}

fn system_info() -> HostResult {
    Ok(HostObject::new()
        .field("os", std::env::consts::OS)
        .field("arch", std::env::consts::ARCH)
        .field("app_version", env!("CARGO_PKG_VERSION"))
        .field(
            "database_path",
            db::app_database_path().display().to_string(),
        )
        .field("runtime", "gpui-shell")
        .into())
}

fn list_profiles() -> HostResult {
    with_database(|db| {
        let profiles = db
            .list_profiles()
            .map_err(|error| HostError::new(error.to_string()))?;
        let mut rows = Vec::with_capacity(profiles.len());
        for profile in &profiles {
            let tin = profile.tin.full();
            rows.push(
                HostObject::new()
                    .field("id", profile.id.unwrap_or_default())
                    .field("tin", tin)
                    .field("full_name", profile.full_name.as_str())
                    .field("rdo_code", profile.rdo_code.as_str())
                    .field("email", profile.email.as_str())
                    .field("taxpayer_type", format!("{:?}", profile.taxpayer_type))
                    .field("is_vat_registered", profile.is_vat_registered)
                    .field("is_archived", profile.is_archived)
                    .field("default_form_type", profile.default_form_type.as_str())
                    .field(
                        "applicable_forms",
                        profile
                            .applicable_forms()
                            .into_iter()
                            .map(HostValue::Str)
                            .collect::<Vec<_>>(),
                    ),
            );
        }
        Ok(HostValue::Array(
            rows.into_iter().map(HostValue::from).collect(),
        ))
    })
}

fn forms_catalog() -> HostResult {
    let mut rows = Vec::with_capacity(FORM_REGISTRY.len());
    for form in FORM_REGISTRY {
        rows.push(HostValue::from(
            HostObject::new()
                .field("code", form.code)
                .field("title", form.title)
                .field("category", form.category)
                .field("frequency", format!("{:?}", form.frequency))
                .field("requires_employees", form.requires_employees)
                .field("is_deprecated", form.is_deprecated),
        ));
    }
    Ok(HostValue::Array(rows))
}

fn get_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = arguments.string(0)?.to_owned();
    with_database(|db| {
        db.get_setting(&key)
            .map(HostValue::from)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

fn set_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = arguments.string(0)?.to_owned();
    let value = arguments.string(1)?.to_owned();
    with_database(|db| {
        db.set_setting(&key, &value)
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

// ---------- S2: auth + profile management ----------

fn get_profile_by_tin(
    db: &Database,
    tin: &str,
) -> Result<bir_core::profile::TaxpayerProfile, HostError> {
    db.get_profile_by_tin(tin)
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new(format!("no taxpayer profile with TIN {tin}")))
}

fn profile_detail(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| serialize(&get_profile_by_tin(db, tin)?))
}

fn save_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    let profile: bir_core::profile::TaxpayerProfile =
        serde_json::from_value(to_json(arguments.value(0)?))
            .map_err(|error| HostError::new(format!("profile payload: {error}")))?;
    with_database(|db| {
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(|saved| serialize(&saved))
    })
}

fn set_profile_archived(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let archived = arguments.boolean(1)?;
    with_database(|db| {
        if archived {
            guard_last_admin_branch(db, &tin, "archived", |profile| profile.is_archived)?;
        }
        let mut profile = get_profile_by_tin(db, &tin)?;
        profile.is_archived = archived;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(|saved| serialize(&saved))
    })
}

fn delete_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        guard_last_admin_branch(db, tin, "deleted", |_| false)?;
        db.delete_profile(tin)
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

// ---------- sign-in: one lock per taxpayer, admin by access mode ----------
//
// A PIN and an authenticator belong to the taxpayer (9-digit TIN root) and
// are stored once, on the `taxpayers` row; every branch shares them. The
// admin account depends on the access mode chosen at first launch:
// - `solo`: the admin IS the owner's taxpayer (`admin_tin_root`), so the
//   admin PIN/authenticator are that taxpayer's.
// - `helpdesk`: the admin is the reserved root `000000000`, not a taxpayer;
//   its PIN/authenticator are the `app_lock_pin_hash` / `app_totp_secret`
//   settings, and it must always keep at least one of them.
// Recovery ladder: authenticator -> recovery key -> Mac login.

const ACCESS_MODE: &str = "access_mode";
const ADMIN_TIN_ROOT: &str = "admin_tin_root";
const ADMIN_PIN_HASH: &str = "app_lock_pin_hash";
const ADMIN_TOTP_SECRET: &str = "app_totp_secret";
const RECOVERY_KEY_HASH: &str = "admin_recovery_key_hash";

const WRONG_CODE: &str =
    "That code does not match. Type the 6-digit code your authenticator app shows now.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccessMode {
    /// The owner's own taxpayer is the admin.
    Solo(String),
    Helpdesk,
}

fn db_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

fn setting(db: &Database, key: &str) -> Result<Option<String>, HostError> {
    db.get_setting(key).map_err(db_error)
}

/// The chosen access mode; `None` on a fresh install not yet set up.
fn access_mode(db: &Database) -> Result<Option<AccessMode>, HostError> {
    Ok(match setting(db, ACCESS_MODE)?.as_deref() {
        Some("solo") => setting(db, ADMIN_TIN_ROOT)?.map(AccessMode::Solo),
        Some("helpdesk") => Some(AccessMode::Helpdesk),
        _ => None,
    })
}

fn require_access_mode(db: &Database) -> Result<AccessMode, HostError> {
    access_mode(db)?.ok_or_else(|| HostError::new("This app is not set up yet."))
}

/// The TIN root of the solo owner, whose taxpayer must never disappear.
fn solo_admin_root(db: &Database) -> Result<Option<String>, HostError> {
    Ok(match access_mode(db)? {
        Some(AccessMode::Solo(root)) => Some(root),
        _ => None,
    })
}

fn tin_root_argument(raw: &str) -> Result<String, HostError> {
    bir_core::tax_profile::parse_tin_root(raw).map_err(db_error)
}

/// A PIN argument: `null` clears, otherwise exactly 4 digits.
fn pin_argument(value: &HostValue) -> Result<Option<String>, HostError> {
    match value {
        HostValue::Null => Ok(None),
        HostValue::Str(pin) if pin.len() == 4 && pin.bytes().all(|b| b.is_ascii_digit()) => {
            Ok(Some(pin.clone()))
        }
        _ => Err(HostError::new("A PIN is 4 digits.")),
    }
}

/// An authenticator argument pair: `null` clears; a secret needs a current
/// code from the app, checked against the new secret, so a mistyped
/// enrolment cannot lock anyone out.
fn totp_argument(
    arguments: &gpui_shell::HostArguments,
    secret_index: usize,
) -> Result<Option<String>, HostError> {
    let secret = arguments.value(secret_index)?.as_str().map(str::to_owned);
    let token = arguments
        .get(secret_index + 1)
        .and_then(HostValue::as_str)
        .unwrap_or_default();
    if let Some(secret) = &secret
        && !bir_core::crypto::validate_totp(secret, token)
    {
        return Err(HostError::new(WRONG_CODE));
    }
    Ok(secret)
}

fn taxpayer_credentials(
    db: &Database,
    tin_root: &str,
) -> Result<bir_core::tax_profile::TaxpayerCredentials, HostError> {
    db.taxpayer_credentials(tin_root).map_err(db_error)
}

/// The admin's PIN hash and authenticator secret under `mode`.
fn admin_credentials(
    db: &Database,
    mode: &AccessMode,
) -> Result<bir_core::tax_profile::TaxpayerCredentials, HostError> {
    match mode {
        AccessMode::Solo(root) => taxpayer_credentials(db, root),
        AccessMode::Helpdesk => Ok(bir_core::tax_profile::TaxpayerCredentials {
            pin_hash: setting(db, ADMIN_PIN_HASH)?,
            totp_secret: setting(db, ADMIN_TOTP_SECRET)?,
        }),
    }
}

fn pin_matches(stored: Option<&str>, pin: &str) -> bool {
    stored.is_some_and(|stored| stored == bir_core::crypto::hash_pin(pin))
}

fn totp_matches(secret: Option<&str>, token: &str) -> bool {
    secret.is_some_and(|secret| bir_core::crypto::validate_totp(secret, token))
}

fn generate_totp_secret(arguments: &gpui_shell::HostArguments) -> HostResult {
    let account = arguments.string(0)?;
    let (secret, qr_path) = bir_core::crypto::generate_totp_secret(account);
    Ok(HostObject::new()
        .field("secret", secret)
        .field("qr_path", qr_path.display().to_string())
        .into())
}

fn delete_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = arguments.string(0)?.to_owned();
    with_database(|db| {
        db.delete_setting(&key)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

/// What the lock screen and first-launch setup read once per render.
fn auth_state() -> HostResult {
    with_database(|db| {
        let mode = access_mode(db)?;
        let credentials = match &mode {
            Some(mode) => admin_credentials(db, mode)?,
            None => Default::default(),
        };
        let (mode_name, admin_root) = match &mode {
            Some(AccessMode::Solo(root)) => {
                (HostValue::from("solo"), HostValue::from(root.as_str()))
            }
            Some(AccessMode::Helpdesk) => (
                HostValue::from("helpdesk"),
                HostValue::from(bir_core::tax_profile::RESERVED_ADMIN_TIN_ROOT),
            ),
            None => (HostValue::Null, HostValue::Null),
        };
        Ok(HostObject::new()
            .field("mode", mode_name)
            .field("admin_root", admin_root)
            .field("has_admin_pin", credentials.pin_hash.is_some())
            .field("has_admin_totp", credentials.totp_secret.is_some())
            .field(
                "has_recovery_key",
                setting(db, RECOVERY_KEY_HASH)?.is_some(),
            )
            .field("mac_login_available", mac_login_available())
            .into())
    })
}

fn taxpayer_auth(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        let credentials = taxpayer_credentials(db, &tin_root)?;
        Ok(HostObject::new()
            .field("has_pin", credentials.pin_hash.is_some())
            .field("has_totp", credentials.totp_secret.is_some())
            .into())
    })
}

/// Set (`pin`) or clear (`null`) a taxpayer's PIN.
fn set_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let pin = pin_argument(arguments.value(1)?)?;
    with_database(|db| write_taxpayer_pin(db, &tin_root, pin.as_deref()))
}

fn write_taxpayer_pin(db: &Database, tin_root: &str, pin: Option<&str>) -> HostResult {
    let hash = pin.map(bir_core::crypto::hash_pin);
    db.set_taxpayer_pin_hash(tin_root, hash.as_deref())
        .map(|_| HostValue::Null)
        .map_err(db_error)
}

/// Set (with a current code) or clear (`null`) a taxpayer's authenticator.
fn set_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let secret = totp_argument(arguments, 1)?;
    with_database(|db| {
        db.set_taxpayer_totp_secret(&tin_root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn verify_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let pin = arguments.string(1)?;
    let ok = auth_limiter::guarded(&auth_limiter::taxpayer(&tin_root), || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            Ok(pin_matches(credentials.pin_hash.as_deref(), pin))
        })
    })?;
    Ok(HostValue::Bool(ok))
}

fn verify_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let token = arguments.string(1)?;
    let ok = auth_limiter::guarded(&auth_limiter::taxpayer(&tin_root), || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            Ok(totp_matches(credentials.totp_secret.as_deref(), token))
        })
    })?;
    Ok(HostValue::Bool(ok))
}

/// Legacy per-branch entry point: a branch TIN's PIN is its taxpayer's.
fn set_profile_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let pin = pin_argument(arguments.value(1)?)?;
    with_database(|db| {
        let root = get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string();
        write_taxpayer_pin(db, &root, pin.as_deref())
    })
}

/// Legacy per-branch entry point: a branch TIN's authenticator is its
/// taxpayer's. Setting needs a current code; `null` clears.
fn set_profile_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let secret = totp_argument(arguments, 1)?;
    with_database(|db| {
        let root = get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string();
        db.set_taxpayer_totp_secret(&root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn verify_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = arguments.string(0)?;
    let ok = auth_limiter::guarded(auth_limiter::ADMIN, || {
        with_database(|db| {
            Ok(match access_mode(db)? {
                Some(mode) => pin_matches(admin_credentials(db, &mode)?.pin_hash.as_deref(), pin),
                None => false,
            })
        })
    })?;
    Ok(HostValue::Bool(ok))
}

fn verify_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let token = arguments.string(0)?;
    let ok = auth_limiter::guarded(auth_limiter::ADMIN, || {
        with_database(|db| {
            Ok(match access_mode(db)? {
                Some(mode) => {
                    totp_matches(admin_credentials(db, &mode)?.totp_secret.as_deref(), token)
                }
                None => false,
            })
        })
    })?;
    Ok(HostValue::Bool(ok))
}

const KEEP_ONE_WAY_IN: &str = "The admin must keep at least one way to sign in. Set up the other one (PIN or authenticator) before removing this one.";

/// Set or clear the admin PIN. Solo: the owner taxpayer's PIN (clearing is
/// allowed). Help desk: the settings PIN, refused to clear without an
/// authenticator. Only the hash is stored; the script never sees it.
fn set_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = pin_argument(arguments.value(0)?)?;
    with_database(|db| match require_access_mode(db)? {
        AccessMode::Solo(root) => write_taxpayer_pin(db, &root, pin.as_deref()),
        AccessMode::Helpdesk => match pin {
            Some(pin) => db.set_setting(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin(&pin)),
            None => {
                if setting(db, ADMIN_TOTP_SECRET)?.is_none() {
                    return Err(HostError::new(KEEP_ONE_WAY_IN));
                }
                db.delete_setting(ADMIN_PIN_HASH)
            }
        }
        .map(|_| HostValue::Null)
        .map_err(db_error),
    })
}

/// Set (with a current code) or clear the admin authenticator, mode-aware
/// like [`set_admin_pin`].
fn set_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let secret = totp_argument(arguments, 0)?;
    with_database(|db| match require_access_mode(db)? {
        AccessMode::Solo(root) => db
            .set_taxpayer_totp_secret(&root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error),
        AccessMode::Helpdesk => match secret {
            Some(secret) => db.set_setting(ADMIN_TOTP_SECRET, &secret),
            None => {
                if setting(db, ADMIN_PIN_HASH)?.is_none() {
                    return Err(HostError::new(KEEP_ONE_WAY_IN));
                }
                db.delete_setting(ADMIN_TOTP_SECRET)
            }
        }
        .map(|_| HostValue::Null)
        .map_err(db_error),
    })
}

const ALREADY_SET_UP: &str = "This app is already set up.";

/// First launch, "Just me or my business": the taxpayer the script just
/// created is the owner and the admin.
fn setup_solo(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        if setting(db, ACCESS_MODE)?.is_some() {
            return Err(HostError::new(ALREADY_SET_UP));
        }
        bir_core::tax_profile::reject_reserved_tin_root(&root).map_err(db_error)?;
        if db.get_taxpayer(&root).map_err(db_error)?.is_none() {
            return Err(HostError::new(format!("No taxpayer with TIN {root}")));
        }
        db.set_setting(ADMIN_TIN_ROOT, &root).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "solo").map_err(db_error)?;
        Ok(HostValue::Null)
    })
}

/// First launch, "I help other taxpayers": admin PIN plus a recovery key.
fn setup_helpdesk(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin =
        pin_argument(arguments.value(0)?)?.ok_or_else(|| HostError::new("A PIN is 4 digits."))?;
    with_database(|db| {
        if setting(db, ACCESS_MODE)?.is_some() {
            return Err(HostError::new(ALREADY_SET_UP));
        }
        db.set_setting(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin(&pin))
            .map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "helpdesk").map_err(db_error)?;
        db.delete_setting(ADMIN_TIN_ROOT).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

// ---------- change mode / owner (admin only, after first launch) ----------
//
// Settings › Computer setup. Each change re-checks the current admin's PIN
// or authenticator code here, never trusting the script, refuses a change
// that would leave nobody able to sign in as admin, and replaces the
// recovery key so the previous admin's key stops working.

const NOT_THE_ADMIN: &str =
    "That PIN or code is not the admin's. Type the current admin PIN or authenticator code.";

/// The current admin's PIN (`pin`) or authenticator code (`code`), checked
/// against the stored credentials of `mode`.
fn confirm_admin(
    db: &Database,
    mode: &AccessMode,
    pin: Option<&str>,
    code: Option<&str>,
) -> Result<(), HostError> {
    let credentials = admin_credentials(db, mode)?;
    let by_pin = pin.is_some_and(|pin| pin_matches(credentials.pin_hash.as_deref(), pin));
    let by_code = code.is_some_and(|code| totp_matches(credentials.totp_secret.as_deref(), code));
    if by_pin || by_code {
        Ok(())
    } else {
        Err(HostError::new(NOT_THE_ADMIN))
    }
}

/// `null` or a non-empty string.
fn optional_text(arguments: &gpui_shell::HostArguments, index: usize) -> Option<String> {
    arguments
        .get(index)
        .and_then(HostValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// A taxpayer that can become the owner: exists, not the reserved root,
/// has a branch that is not archived, and has a PIN or authenticator so
/// someone can sign in as admin afterwards.
fn check_new_owner(db: &Database, root: &str) -> Result<(), HostError> {
    bir_core::tax_profile::reject_reserved_tin_root(root).map_err(db_error)?;
    let Some(taxpayer) = db.get_taxpayer(root).map_err(db_error)? else {
        return Err(HostError::new(format!("No taxpayer with TIN {root}")));
    };
    let mut active = false;
    for branch in &taxpayer.branches {
        if let Some(profile) = db.get_profile_by_tin(&branch.tin).map_err(db_error)?
            && !profile.is_archived
        {
            active = true;
            break;
        }
    }
    if !active {
        return Err(HostError::new(
            "This taxpayer is archived. Unarchive it before making it the owner.",
        ));
    }
    let credentials = taxpayer_credentials(db, root)?;
    if credentials.pin_hash.is_none() && credentials.totp_secret.is_none() {
        return Err(HostError::new(
            "This taxpayer has no PIN or authenticator yet. Set one for them first, so the new owner can sign in as admin.",
        ));
    }
    Ok(())
}

/// Make `tin_root` the owner (solo mode), from solo or from help desk.
/// Arguments: `tin_root`, then the current admin's `pin` and `code` (either
/// may be `null`). Help desk's admin PIN and authenticator are removed.
/// Answers a new recovery key.
fn change_owner(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    let pin = optional_text(arguments, 1);
    let code = optional_text(arguments, 2);
    with_database(|db| {
        let mode = require_access_mode(db)?;
        confirm_admin(db, &mode, pin.as_deref(), code.as_deref())?;
        if mode == AccessMode::Solo(root.clone()) {
            return Err(HostError::new("This taxpayer is already the owner."));
        }
        check_new_owner(db, &root)?;
        // Owner first, then the mode: a failure in between leaves a working
        // setup either way.
        db.set_setting(ADMIN_TIN_ROOT, &root).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "solo").map_err(db_error)?;
        db.delete_setting(ADMIN_PIN_HASH).map_err(db_error)?;
        db.delete_setting(ADMIN_TOTP_SECRET).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

/// Switch from solo to help desk. Arguments: the current admin's `pin` and
/// `code` (either may be `null`), then the new admin PIN. The former owner
/// stays an ordinary taxpayer with their own PIN. Answers a new recovery
/// key.
fn change_to_helpdesk(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = optional_text(arguments, 0);
    let code = optional_text(arguments, 1);
    let new_pin =
        pin_argument(arguments.value(2)?)?.ok_or_else(|| HostError::new("A PIN is 4 digits."))?;
    with_database(|db| {
        let mode = require_access_mode(db)?;
        if mode == AccessMode::Helpdesk {
            return Err(HostError::new(
                "This computer is already set up for a help desk.",
            ));
        }
        confirm_admin(db, &mode, pin.as_deref(), code.as_deref())?;
        // The admin PIN first, then the mode: a failure in between leaves a
        // working solo setup.
        db.set_setting(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin(&new_pin))
            .map_err(db_error)?;
        db.delete_setting(ADMIN_TOTP_SECRET).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "helpdesk").map_err(db_error)?;
        db.delete_setting(ADMIN_TIN_ROOT).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

/// A fresh recovery key replaces any earlier one; only its hash is kept.
fn store_new_recovery_key(db: &Database) -> HostResult {
    let key = bir_core::crypto::generate_recovery_key();
    db.set_setting(
        RECOVERY_KEY_HASH,
        &bir_core::crypto::hash_recovery_key(&key),
    )
    .map_err(db_error)?;
    Ok(HostObject::new().field("recovery_key", key).into())
}

fn create_recovery_key() -> HostResult {
    with_database(|db| {
        require_access_mode(db)?;
        store_new_recovery_key(db)
    })
}

/// Single use: a match answers true and forgets the key. Case, dashes and
/// spaces do not matter.
fn use_recovery_key(arguments: &gpui_shell::HostArguments) -> HostResult {
    let typed = bir_core::crypto::hash_recovery_key(arguments.string(0)?);
    let matches = auth_limiter::guarded(auth_limiter::RECOVERY_KEY, || {
        with_database(|db| {
            let matches = setting(db, RECOVERY_KEY_HASH)?.is_some_and(|stored| stored == typed);
            if matches {
                db.delete_setting(RECOVERY_KEY_HASH).map_err(db_error)?;
            }
            Ok(matches)
        })
    })?;
    Ok(HostValue::Bool(matches))
}

fn mac_login_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Ask the operating system to confirm the signed-in user (Touch ID or the
/// account password on macOS, Windows Hello elsewhere). The prompt runs on
/// its own thread, so the UI thread never waits; other platforms answer
/// false.
fn mac_login(
    arguments: &gpui_shell::HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let reason = arguments
        .get(0)
        .and_then(HostValue::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("sign in as the admin")
        .to_owned();
    let (sender, receiver) = futures::channel::oneshot::channel::<bool>();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    std::thread::Builder::new()
        .name("mac-login".into())
        .spawn(move || {
            let _ = sender.send(os_authenticate(&reason));
        })
        .map_err(db_error)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = reason;
        let _ = sender.send(false);
    }
    Ok(async move { Ok(HostValue::Bool(receiver.await.unwrap_or(false))) })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn os_authenticate(reason: &str) -> bool {
    use robius_authentication::{
        AndroidText, BiometricStrength, Context, PolicyBuilder, Text, WindowsText,
    };
    let Some(policy) = PolicyBuilder::new()
        .biometrics(Some(BiometricStrength::Strong))
        .password(true)
        .watch(true)
        .build()
    else {
        tracing::warn!("OS sign-in policy unavailable");
        return false;
    };
    let text = Text {
        android: AndroidText {
            title: "Admin sign-in",
            subtitle: None,
            description: None,
        },
        apple: reason,
        windows: WindowsText::new_truncated("Admin sign-in", reason),
    };
    futures::executor::block_on(Context::new(()).authenticate(text, &policy)).is_ok()
}

/// Refuse to archive or delete the solo owner (the admin) or the reserved
/// help desk root.
fn guard_admin_taxpayer(db: &Database, root: &str, action: &str) -> Result<(), HostError> {
    if root == bir_core::tax_profile::RESERVED_ADMIN_TIN_ROOT
        || solo_admin_root(db)?.as_deref() == Some(root)
    {
        return Err(HostError::new(format!(
            "This taxpayer is the admin account and cannot be {action}."
        )));
    }
    Ok(())
}

/// Archiving or deleting one branch must not take the solo owner's last
/// usable branch with it: refuse when every other branch of the admin
/// taxpayer is already `gone`.
fn guard_last_admin_branch(
    db: &Database,
    tin: &str,
    action: &str,
    gone: impl Fn(&bir_core::profile::TaxpayerProfile) -> bool,
) -> Result<(), HostError> {
    let Some(root) = solo_admin_root(db)? else {
        return Ok(());
    };
    if tin.len() < 9 || tin[..9] != root {
        return Ok(());
    }
    let Some(taxpayer) = db.get_taxpayer(&root).map_err(db_error)? else {
        return Ok(());
    };
    for branch in taxpayer.branches.iter().filter(|branch| branch.tin != tin) {
        if let Some(profile) = db.get_profile_by_tin(&branch.tin).map_err(db_error)?
            && !gone(&profile)
        {
            return Ok(());
        }
    }
    Err(HostError::new(format!(
        "This is the admin taxpayer's last branch and cannot be {action}."
    )))
}

fn set_taxpayer_archived(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    let archived = arguments.boolean(1)?;
    with_database(|db| {
        if archived {
            guard_admin_taxpayer(db, &root, "archived")?;
        }
        db.set_taxpayer_archived(&root, archived)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn delete_taxpayer(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        guard_admin_taxpayer(db, &root, "deleted")?;
        db.delete_taxpayer(&root)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

// ---------- S3: form drafts ----------

fn list_drafts(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let year = arguments.integer(1)? as u16;
    with_database(|db| {
        let rows = db
            .list_draft_summaries(&tin, year)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Array(
            rows.iter()
                .map(|row| {
                    HostValue::from(
                        HostObject::new()
                            .field("id", row.id)
                            .field("tin", row.tin.as_str())
                            .field("form_code", row.form_code.as_str())
                            .field("taxable_year", row.taxable_year as i64)
                            .field(
                                "quarter",
                                row.quarter.map_or(HostValue::Null, HostValue::from),
                            )
                            .field("month", row.month.map_or(HostValue::Null, HostValue::from))
                            .field("status", format!("{:?}", row.status))
                            .field("updated_at", row.updated_at.as_str()),
                    )
                })
                .collect(),
        ))
    })
}

fn filing_status(name: &str) -> Result<bir_core::forms::FilingStatus, HostError> {
    use bir_core::forms::FilingStatus::*;
    match name {
        "Draft" => Ok(Draft),
        "Queued" => Ok(Queued),
        "Submitted" | "Filed" => Ok(Submitted),
        "Confirmed" => Ok(Confirmed),
        "Paid" => Ok(Paid),
        other => Err(HostError::new(format!("unknown filing status {other:?}"))),
    }
}

/// `quarter` argument is `Option<u8>`: pass null for annual/open-ended slots
/// and 0 for the first open-ended period (the DB auto-numbers those).
fn optional_quarter(
    arguments: &gpui_shell::HostArguments,
    index: usize,
) -> Result<Option<u8>, HostError> {
    match arguments.get(index) {
        Some(HostValue::Null) | None => Ok(None),
        Some(HostValue::Number(q)) => Ok(Some(*q as u8)),
        Some(other) => Err(HostError::new(format!(
            "quarter must be a number or null, got {}",
            other.describe()
        ))),
    }
}

fn get_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    with_database(|db| {
        let draft: Option<serde_json::Value> = match form_code.as_str() {
            "2551Q" => {
                let q = quarter.unwrap_or(1);
                db.get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d).map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            "1601C" => {
                let month = quarter.unwrap_or(1);
                db.get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d).map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            _ => db
                .get_form_draft(&tin, &form_code, year, quarter)
                .map_err(|error| HostError::new(error.to_string()))?,
        };
        match draft {
            Some(value) => serialize(&value),
            None => Ok(HostValue::Null),
        }
    })
}

/// Fields the script reports merge over a profile-derived draft before the
/// typed save — the same overlay shape the painted app uses, so the draft
/// keeps every engine-required field even when the UI edits a subset.
fn overlay_fields<T: serde::Serialize + serde::de::DeserializeOwned>(
    base: T,
    data: &serde_json::Value,
) -> Result<T, HostError> {
    let mut merged =
        serde_json::to_value(&base).map_err(|error| HostError::new(error.to_string()))?;
    if let (Some(target), Some(source)) = (merged.as_object_mut(), data.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(merged).map_err(|error| HostError::new(error.to_string()))
}

fn save_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let status = filing_status(arguments.string(4)?)?;
    let data = to_json(arguments.value(5)?);
    with_database(|db| {
        // The engine requires dedicated typed persistence for these two forms;
        // route them inside the host so the script keeps one uniform call.
        match form_code.as_str() {
            "2551Q" => {
                let profile = db
                    .get_profile(&tin)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new(format!("no profile for TIN {tin}")))?;
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let base = bir_core::forms::Form2551QDraft::new_from_profile(&profile, year, q);
                let draft = overlay_fields(base, &data)?;
                db.save_2551q_draft(&draft)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
            "1601C" => {
                let profile = db
                    .get_profile(&tin)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new(format!("no profile for TIN {tin}")))?;
                let month = quarter.ok_or_else(|| {
                    HostError::new("1601C drafts are monthly: pass the month in the quarter slot")
                })?;
                let base = bir_core::forms::Form1601CDraft::new_from_profile(&profile, year, month);
                let draft = overlay_fields(base, &data)?;
                db.save_1601c_draft(&draft)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
            _ => db
                .save_form_draft(&tin, &form_code, year, quarter, &status, &data)
                .map(|id| HostValue::Number(id as f64))
                .map_err(|error| HostError::new(error.to_string())),
        }
    })
}

/// Every queued submission across profiles — the queue page's feed.
fn list_queued() -> HostResult {
    with_database(|db| {
        let summaries = db
            .list_all_queued_submissions()
            .map_err(|error| HostError::new(error.to_string()))?;
        let items: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "tin": s.tin,
                    "form_code": s.form_code,
                    "taxable_year": s.taxable_year,
                    "quarter": s.quarter,
                    "month": s.month,
                    "status": format!("{:?}", s.status),
                    "updated_at": s.updated_at,
                })
            })
            .collect();
        serialize(&items)
    })
}

/// Move a saved draft into the queue. Generic forms go through
/// `QueueableKind` + `save_queued_queueable`; the two engine-guarded forms
/// use their dedicated transition + persistence, mirroring the painted views.
fn queue_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    with_database(|db| {
        let queued: Result<i64, HostError> = (|| match form_code.as_str() {
            "2551Q" => {
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let mut draft = db
                    .get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 2551Q draft for that period"))?;
                draft.transition_to_queued().map_err(|errors| {
                    HostError::new(
                        errors
                            .iter()
                            .map(|(field, msg)| format!("{field}: {msg}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
                db.save_queued_2551q_draft_and_election(&draft)
                    .map_err(|error| HostError::new(error.to_string()))
            }
            "1601C" => {
                let month = quarter
                    .ok_or_else(|| HostError::new("1601C drafts are monthly: pass the month"))?;
                let mut draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 1601C draft for that period"))?;
                draft.transition_to_queued().map_err(|errors| {
                    HostError::new(
                        errors
                            .iter()
                            .map(|(field, msg)| format!("{field}: {msg}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
                db.save_queued_1601c_draft(&draft)
                    .map_err(|error| HostError::new(error.to_string()))
            }
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                bir_core::with_queueable_kind!(kind, F => {
                    let mut draft: F = db
                        .get_form_draft(&tin, &form_code, year, quarter)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No saved {form_code} draft for that period"))
                        })?;
                    draft
                        .queue(bir_core::filing_queue::QueueAuthSource::Gui)
                        .map_err(|errors| {
                            HostError::new(
                                errors
                                    .iter()
                                    .map(|(field, msg)| format!("{field}: {msg}"))
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            )
                        })?;
                    db.save_queued_queueable(&draft)
                        .map_err(|error| HostError::new(error.to_string()))
                })
            }
        })();
        queued.map(|id| HostValue::Number(id as f64))
    })
}

/// Cancel an unclaimed queued submission back to Draft (same guards as the
/// painted views: claimed or submitted work cannot be canceled this way).
fn cancel_queued(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    with_database(|db| {
        let result: Result<(), HostError> = (|| match form_code.as_str() {
            "2551Q" => {
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let draft = db
                    .get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No queued 2551Q draft for that period"))?;
                db.cancel_queued_2551q_submission(&draft)
                    .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            }
            "1601C" => {
                let month = quarter
                    .ok_or_else(|| HostError::new("1601C drafts are monthly: pass the month"))?;
                let draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No queued 1601C draft for that period"))?;
                db.cancel_queued_1601c_submission(&draft)
                    .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            }
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                bir_core::with_queueable_kind!(kind, F => {
                    let draft: F = db
                        .get_form_draft(&tin, &form_code, year, quarter)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No queued {form_code} draft for that period"))
                        })?;
                    db.cancel_queued_queueable(&draft)
                        .map_err(|error| HostError::new(error.to_string()))?;
                    Ok(())
                })
            }
        })();
        result.map(|_| HostValue::Null)
    })
}

/// The painted Background Tasks page merges `job_queue` rows with every
/// queued filing submission — same two sources, same label via
/// `FilingJobLabel`.
fn list_background_jobs() -> HostResult {
    with_database(|db| {
        let mut items: Vec<serde_json::Value> = db
            .list_jobs()
            .map_err(|error| HostError::new(error.to_string()))?
            .iter()
            .map(|job| {
                serde_json::json!({
                    "kind": "job",
                    "id": job.id,
                    "name": job.name,
                    "job_type": job.job_type,
                    "cron_expr": job.cron_expr,
                    "status": job.status,
                    "retries": job.retries,
                    "last_run_at": job.last_run_at,
                    "next_run_at": job.next_run_at,
                    "created_at": job.created_at,
                    "output_log": job.output_log,
                })
            })
            .collect();
        let emails_by_tin: std::collections::HashMap<String, String> = db
            .list_profiles()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|profile| {
                profile
                    .tracking_mailbox()
                    .map(|email| (profile.tin.full(), email.to_string()))
            })
            .collect();
        for sum in db
            .list_all_queued_submissions()
            .map_err(|error| HostError::new(error.to_string()))?
        {
            let email = emails_by_tin.get(&sum.tin).map(String::as_str);
            items.push(serde_json::json!({
                "kind": "submission",
                "id": sum.id,
                "tin": sum.tin,
                "form_code": sum.form_code,
                "taxable_year": sum.taxable_year,
                "quarter": sum.quarter,
                "month": sum.month,
                "name": bir_core::job_display::FilingJobLabel::from_summary(&sum, email)
                    .submit_name(),
                "job_type": "System",
                "cron_expr": null,
                "status": format!("{:?}", sum.status),
                "retries": 0,
                "last_run_at": null,
                "next_run_at": null,
                "created_at": sum.updated_at,
                "output_log": null,
            }));
        }
        serialize(&items)
    })
}

fn delete_background_job(arguments: &gpui_shell::HostArguments) -> HostResult {
    let id = arguments.integer(0)?;
    with_database(|db| {
        db.delete_job(id)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// Archive a job the painted way — status `Archived` via `save_job`.
fn archive_background_job(arguments: &gpui_shell::HostArguments) -> HostResult {
    let id = arguments.integer(0)?;
    with_database(|db| {
        let mut job = db
            .list_jobs()
            .map_err(|error| HostError::new(error.to_string()))?
            .into_iter()
            .find(|job| job.id == Some(id))
            .ok_or_else(|| HostError::new("No job with that id"))?;
        job.status = "Archived".to_string();
        db.save_job(job)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

fn purge_archived_jobs() -> HostResult {
    with_database(|db| {
        db.delete_archived_jobs()
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// Today's rotated log (`ebirforms.YYYY-MM-DD.log`), tailed like the painted
/// Logs tab — it keeps at most the newest 5,000 lines.
fn job_log_tail() -> HostResult {
    let logs_path = bir_core::log_files::today_log_path(&bir_core::log_files::log_dir());
    let file_name = logs_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (content, lines) = match std::fs::read_to_string(&logs_path) {
        Ok(text) => {
            let total = text.lines().count();
            let tail = if total <= 5_000 {
                text
            } else {
                text.lines()
                    .skip(total - 5_000)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            (tail, total)
        }
        Err(_) => ("No log entries yet today.".to_string(), 0),
    };
    serialize(&serde_json::json!({
        "file_name": file_name,
        "lines": lines,
        "content": content,
    }))
}

/// The frozen print-sheet slug each form's painted `PRINT_SLUG` names.
pub(crate) fn print_slug_for(form_code: &str) -> Option<&'static str> {
    Some(match form_code {
        "0605" => "0605-1999",
        "0619E" => "0619e-2018",
        "0619F" => "0619f-2018",
        "1600PT" => "1600-pt-2018",
        "1600VT" => "1600-vt-2018",
        "1600WP" => "1600wp-2010",
        "1601C" => "1601c-2018",
        "1601EQ" => "1601eq-2019",
        "1601FQ" => "1601-fq-2020",
        "1602Q" => "1602q-2019",
        "1603Q" => "1603q-2018",
        "1604C" => "1604c-2018",
        "1604E" => "1604e-2018",
        "1604F" => "1604f-2018",
        "1606" => "1606-2018",
        "1700" => "1700-2018",
        "1701" => "1701-2018",
        "1701A" => "1701a-2018",
        "1701MS" => "1701ms-2024",
        "1701Q" => "1701q-2018",
        "1702EX" => "1702ex-2018",
        "1702MX" => "1702mx-2018c",
        "1702Q" => "1702q-2018",
        "1702RT" => "1702rt-2018c",
        "1706" => "1706-2018",
        "1707" => "1707-2021",
        "1707A" => "1707a-2021",
        "1800" => "1800-2018",
        "1801" => "1801-2018",
        "2000" => "2000-dst-2018",
        "2000OT" => "2000-ot-2018",
        "2200A" => "2200a-2020",
        "2200AN" => "2200an-2018",
        "2200C" => "2200c-2018",
        "2200M" => "2200m-2018",
        "2200P" => "2200p-2020",
        "2200S" => "2200s-2018",
        "2200T" => "2200t-2022",
        "2550M" => "2550m-2007",
        "2550Q" => "2550q-2024",
        "2551Q" => "2551q-2018",
        "2552" => "2552-2018",
        "2553" => "2553-1999",
        _ => return None,
    })
}

/// The receipt page a confirmed draft appends — the shell port of the painted
/// `form_html_preview_launcher::receipt_page_for`.
pub(crate) fn receipt_page(
    db: &Database,
    receipt_id: Option<i64>,
    mailbox: &str,
) -> Option<bir_print::frozen_html::ReceiptPage> {
    let receipt_id = receipt_id?;
    let receipt = db.get_submission_receipt_by_id(receipt_id).ok().flatten()?;
    Some(bir_print::frozen_html::ReceiptPage {
        filename: receipt.filename.clone(),
        subject: "Tax Return Receipt Confirmation".to_string(),
        from: receipt
            .source_from
            .clone()
            .unwrap_or_else(|| "ebirforms-noreply@bir.gov.ph".to_string()),
        to: mailbox.to_string(),
        received_at: bir_core::background_cron::display_received_at(
            &receipt.received_date,
            &receipt.received_time,
        ),
        email_received_at: receipt
            .email_received_at
            .as_deref()
            .and_then(bir_core::receipt::display_email_received_at),
        body_text: receipt.raw_text.clone(),
        body_html: receipt
            .raw_html
            .as_deref()
            .map(bir_core::receipt::sanitized_receipt_html),
    })
}

/// The stored draft's print field map (writer key -> value) and, for a
/// confirmed return, its receipt page: what `form_print_html` fills the
/// frozen sheet with.
pub(crate) fn print_fields(
    db: &Database,
    tin: &str,
    form_code: &str,
    year: u16,
    quarter: Option<u8>,
) -> Result<
    (
        std::collections::BTreeMap<String, String>,
        Option<bir_print::frozen_html::ReceiptPage>,
    ),
    HostError,
> {
    Ok(match form_code {
        "2551Q" => {
            let q = quarter.ok_or_else(|| HostError::new("2551Q drafts are quarterly"))?;
            let draft = db
                .get_2551q_draft(tin, year, q)
                .map_err(|error| HostError::new(error.to_string()))?
                .ok_or_else(|| HostError::new("No saved 2551Q draft for that period"))?;
            let page = receipt_page(db, draft.receipt_id, &draft.email);
            (draft.to_print_field_map(), page)
        }
        "1601C" => {
            let month = quarter.ok_or_else(|| HostError::new("1601C drafts are monthly"))?;
            let draft = db
                .get_1601c_draft(tin, year, month)
                .map_err(|error| HostError::new(error.to_string()))?
                .ok_or_else(|| HostError::new("No saved 1601C draft for that period"))?;
            let page = receipt_page(db, draft.receipt_id, &draft.email_address);
            (draft.to_print_field_map(), page)
        }
        _ => {
            use bir_core::forms::queueable::QueueableKind;
            let kind = QueueableKind::from_form_code(form_code)
                .ok_or_else(|| HostError::new(format!("Form {form_code} is not printable")))?;
            bir_core::with_queueable_kind!(kind, F => {
                let draft: F = db
                    .get_form_draft(tin, form_code, year, quarter)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| {
                        HostError::new(format!("No saved {form_code} draft for that period"))
                    })?;
                let page = receipt_page(
                    db,
                    draft.lifecycle().receipt_id,
                    draft.submission_email(),
                );
                (draft.print_field_map(), page)
            })
        }
    })
}

/// Fill the form's frozen print sheet, write it to a temp HTML file, and open
/// it with the platform's default handler (the painted app shows it in a wry
/// preview window — the shell has no wry, and `cx.open_url` is HTTP-only, so
/// the printable equivalent is the system browser via xdg-open/open/start).
fn form_print_html(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let slug = print_slug_for(&form_code)
        .ok_or_else(|| HostError::new(format!("Form {form_code} has no print sheet")))?;
    with_database(|db| {
        let (fields, receipt) = print_fields(db, &tin, &form_code, year, quarter)?;
        let profile = db
            .get_profile_by_tin(&tin)
            .ok()
            .flatten()
            .map(|p| p.full_name.clone());
        open_print_preview(slug, &form_code, year, &fields, receipt.as_ref(), profile)
    })
}

/// Fill `slug`'s frozen sheet with `fields`, write it to a temp HTML file and
/// open it in the system browser. Answers `{ path }`.
pub(crate) fn open_print_preview(
    slug: &str,
    form_code: &str,
    year: u16,
    fields: &std::collections::BTreeMap<String, String>,
    receipt: Option<&bir_print::frozen_html::ReceiptPage>,
    profile_name: Option<String>,
) -> HostResult {
    let html = bir_print::frozen_html::filled_document_with_receipt(slug, fields, receipt)
        .map_err(HostError::new)?;
    // A4 layout + static markup (no scripts, values as text): prints one A4
    // page per form page from any engine; `print-overrides/<slug>.css` on top.
    let html =
        crate::static_html::staticize(&crate::print_sheet::prepare_html_for(slug, &html, true));
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let path =
        std::env::temp_dir().join(format!("bir-{form_code}-{year}-{timestamp}-preview.html"));
    std::fs::write(&path, html).map_err(|error| HostError::new(error.to_string()))?;
    // In-app preview window (Blitz view + hidden WebKit PDF for Print/Share);
    // one window per form, re-opening refreshes it in place.
    let title = match profile_name.filter(|n| !n.trim().is_empty()) {
        Some(name) => format!("BIR Form {form_code} \u{2014} {name}"),
        None => format!("BIR Form {form_code}"),
    };
    crate::print_preview_window::request_keyed(path.clone(), title, form_code.to_string());
    Ok(HostObject::new()
        .field("path", path.display().to_string())
        .into())
}

/// Launch the platform's default HTML viewer on `path` — detached, so the
/// host call returns immediately like the painted preview window does.
#[allow(dead_code)]
fn open_in_system_browser(path: &std::path::Path) -> Result<(), HostError> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args([
            "/c",
            "start",
            "",
            path.as_os_str().to_string_lossy().as_ref(),
        ]);
        cmd
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(path);
        cmd
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
}

/// Drop the cached connection — used after a database-replace import so the
/// next `with_database` reopens the freshly extracted file (and re-acquires
/// the owner lock the dropped handle was holding).
fn reset_database() {
    DATABASE.with(|slot| {
        slot.borrow_mut().take();
    });
}

/// Native file picker + archive handling stay host-side: the script has no
/// file dialog. Mirrors `ImportExportView`: a zip with `bir_data.db` is a
/// full database backup, one with a top-level `profile.json` is a profile
/// export, anything else is invalid. Optional `path` argument skips the
/// dialog (used by tests and scripted imports).
fn import_archive(arguments: &gpui_shell::HostArguments) -> HostResult {
    let path = arguments
        .value(0)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            rfd::FileDialog::new()
                .set_title("Select an exported .zip archive")
                .add_filter("Export Archive", &["zip"])
                .pick_file()
        });
    let Some(path) = path else {
        return Ok(HostObject::new().field("kind", "cancelled").into());
    };

    let zip_has = |name: &str| -> bool {
        std::fs::File::open(&path)
            .ok()
            .and_then(|file| zip::ZipArchive::new(file).ok())
            .map(|mut archive| archive.by_name(name).is_ok())
            .unwrap_or(false)
    };
    let is_db = zip_has("bir_data.db");
    let is_profile = zip_has("profile.json");

    if is_db {
        // Painted flow (import_export.rs): safety-backup the live file, then
        // extract the archive over it. Drop the cached connection first — it
        // holds the owner lock and an open handle on the file being replaced.
        let db_path = db::app_database_path();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        if db_path.exists()
            && let Some(parent) = db_path.parent()
        {
            let _ = std::fs::copy(
                &db_path,
                parent.join(format!("bir_data.safety-{timestamp}.db")),
            );
        }
        reset_database();
        return match bir_core::import::extract_database_zip(&path, &db_path) {
            Ok(()) => Ok(HostObject::new().field("kind", "database").into()),
            Err(error) => Err(HostError::new(error.to_string())),
        };
    }

    if is_profile {
        return with_database(
            |db| match bir_core::import::import_profile_data(db, &path) {
                Ok(()) => Ok(HostObject::new().field("kind", "profile").into()),
                Err(error) => Err(HostError::new(error.to_string())),
            },
        );
    }

    Ok(HostObject::new().field("kind", "invalid").into())
}

/// Tax deadlines feed for the calendar page.
fn list_deadlines() -> HostResult {
    with_database(|db| {
        db.list_tax_deadlines()
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(|deadlines| serialize(&deadlines))
    })
}

/// The module the app's JavaScript imports as `"bir"`.
pub fn module() -> HostModule {
    let module = HostModule::new("bir")
        .function("system_info", |_| system_info())
        .function("list_profiles", |_| list_profiles())
        .function("forms_catalog", |_| forms_catalog())
        .function("get_setting", get_setting)
        .function("set_setting", set_setting)
        .function("profile_detail", profile_detail)
        .function("save_profile", save_profile)
        .function("set_profile_archived", set_profile_archived)
        .function("delete_profile", delete_profile)
        .function("set_profile_pin", set_profile_pin)
        .function("set_profile_totp", set_profile_totp)
        .function("auth_state", |_| auth_state())
        .function("taxpayer_auth", taxpayer_auth)
        .function("set_taxpayer_pin", set_taxpayer_pin)
        .function("set_taxpayer_totp", set_taxpayer_totp)
        .function("verify_taxpayer_pin", verify_taxpayer_pin)
        .function("verify_taxpayer_totp", verify_taxpayer_totp)
        .function("verify_admin_pin", verify_admin_pin)
        .function("verify_admin_totp", verify_admin_totp)
        .function("set_admin_pin", set_admin_pin)
        .function("set_admin_totp", set_admin_totp)
        .function("setup_solo", setup_solo)
        .function("setup_helpdesk", setup_helpdesk)
        .function("change_owner", change_owner)
        .function("change_to_helpdesk", change_to_helpdesk)
        .function("create_recovery_key", |_| create_recovery_key())
        .function("use_recovery_key", use_recovery_key)
        .async_function("mac_login", mac_login)
        .function("set_taxpayer_archived", set_taxpayer_archived)
        .function("delete_taxpayer", delete_taxpayer)
        .function("generate_totp_secret", generate_totp_secret)
        .function("delete_setting", delete_setting)
        .function("list_drafts", list_drafts)
        .function("get_form_draft", get_form_draft)
        .function("save_form_draft", save_form_draft)
        .function("list_queued", |_| list_queued())
        .function("queue_form_draft", queue_form_draft)
        .function("cancel_queued", cancel_queued)
        .function("list_deadlines", |_| list_deadlines())
        .function("import_archive", import_archive)
        .function("form_print_html", form_print_html)
        .function("list_background_jobs", |_| list_background_jobs())
        .function("delete_background_job", delete_background_job)
        .function("archive_background_job", archive_background_job)
        .function("purge_archived_jobs", |_| purge_archived_jobs())
        .function("job_log_tail", |_| job_log_tail());
    let module = crate::tax_profile_module::register(module);
    let module = crate::screen_form_module::register(module);
    let module = crate::dashboard_module::register(module);
    module.declarations(format!(
        "{BASE_DECLARATIONS}{}{}{}",
        crate::tax_profile_module::DECLARATIONS,
        crate::screen_form_module::DECLARATIONS,
        crate::dashboard_module::DECLARATIONS,
    ))
}

/// TypeScript for the functions registered in this file. The tax-profile and
/// screen-form files append their own (`declarations` replaces, so `module`
/// concatenates them).
const BASE_DECLARATIONS: &str = r#"
export interface SystemInfo {
    os: string;
    arch: string;
    app_version: string;
    database_path: string;
    runtime: string;
}
export interface ProfileSummary {
    id: number;
    tin: string;
    full_name: string;
    rdo_code: string;
    email: string;
    taxpayer_type: string;
    is_vat_registered: boolean;
    is_archived: boolean;
    default_form_type: string;
    applicable_forms: string[];
}
export interface FormCatalogEntry {
    code: string;
    title: string;
    category: string;
    frequency: string;
    requires_employees: boolean;
    is_deprecated: boolean;
}
export function system_info(): SystemInfo;
export function list_profiles(): ProfileSummary[];
export function forms_catalog(): FormCatalogEntry[];
export function get_setting(key: string): string | null;
export function set_setting(key: string, value: string): null;

export interface AuthState {
    /** null on a fresh install that has not chosen yet. */
    mode: "solo" | "helpdesk" | null;
    /** solo: the owner's TIN root; helpdesk: "000000000"; null if unset. */
    admin_root: string | null;
    /** solo: the owner taxpayer's PIN; helpdesk: the admin PIN setting. */
    has_admin_pin: boolean;
    has_admin_totp: boolean;
    has_recovery_key: boolean;
    /** Touch ID / account password (macOS) or Windows Hello can confirm the admin. */
    mac_login_available: boolean;
}
export interface ProfileAuth {
    has_pin: boolean;
    has_totp: boolean;
}
export interface TotpSecret {
    secret: string;
    qr_path: string;
}
export interface RecoveryKey {
    /** Shown once, e.g. "7K3M-Q9TB-0XWD-H2RE"; only its hash is kept. */
    recovery_key: string;
}
export function profile_detail(tin: string): object;
export function save_profile(profile: object): object;
export function set_profile_archived(tin: string, archived: boolean): object;
export function delete_profile(tin: string): null;
/** Sets the PIN of the branch's taxpayer (one PIN per TIN root). */
export function set_profile_pin(tin: string, pin: string | null): null;
/** Sets the authenticator of the branch's taxpayer; setting needs a current code. */
export function set_profile_totp(tin: string, secret: string | null, token?: string): null;
export function auth_state(): AuthState;
/** PIN and authenticator of a taxpayer (TIN root), shared by every branch. */
export function taxpayer_auth(tin_root: string): ProfileAuth;
/** pin: exactly 4 digits, or null to clear. */
export function set_taxpayer_pin(tin_root: string, pin: string | null): null;
/** Set (with a current code from the app) or clear (null) a taxpayer's authenticator. */
export function set_taxpayer_totp(tin_root: string, secret: string | null, code?: string): null;
export function verify_taxpayer_pin(tin_root: string, pin: string): boolean;
export function verify_taxpayer_totp(tin_root: string, code: string): boolean;
/** solo: the owner taxpayer's PIN; helpdesk: the admin PIN. false when not set up. */
export function verify_admin_pin(pin: string): boolean;
export function verify_admin_totp(code: string): boolean;
/** Help desk refuses to clear the PIN while there is no admin authenticator. */
export function set_admin_pin(pin: string | null): null;
/** Setting needs a current code; help desk refuses to clear it while there is no admin PIN. */
export function set_admin_totp(secret: string | null, code?: string): null;
/** First launch, after creating the owner's taxpayer. Refused once set up. */
export function setup_solo(tin_root: string): null;
/** First launch for a help desk: admin PIN plus a new recovery key. Refused once set up. */
export function setup_helpdesk(pin: string): RecoveryKey;
/** Admin only: make this taxpayer the owner (solo), from solo or help desk. Needs the current admin PIN or code; the taxpayer needs a PIN or authenticator and an active branch. Help desk's admin PIN and authenticator are removed. Answers a new recovery key. */
export function change_owner(tin_root: string, pin: string | null, code: string | null): RecoveryKey;
/** Admin only: switch from solo to help desk with a new admin PIN. Needs the current admin PIN or code. Answers a new recovery key. */
export function change_to_helpdesk(pin: string | null, code: string | null, new_pin: string): RecoveryKey;
/** Replace the admin recovery key. Needs the app set up. */
export function create_recovery_key(): RecoveryKey;
/** Single use: true (and the key is forgotten) on a match; case, dashes and spaces ignored. */
export function use_recovery_key(key: string): boolean;
/** Touch ID / account password (Windows Hello on Windows); false elsewhere or when cancelled. */
export function mac_login(reason: string): Promise<boolean>;
/** Archive or unarchive every branch. Archiving the admin taxpayer is refused. */
export function set_taxpayer_archived(tin_root: string, archived: boolean): null;
/** Delete the taxpayer and every branch. Refused for the admin taxpayer. */
export function delete_taxpayer(tin_root: string): null;
export function generate_totp_secret(account: string): TotpSecret;
export function delete_setting(key: string): null;

export interface DraftSummary {
    id: number;
    tin: string;
    form_code: string;
    taxable_year: number;
    quarter: number | null;
    month: number | null;
    status: "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid";
    updated_at: string;
}
export function list_drafts(tin: string, year: number): DraftSummary[];
export function get_form_draft(tin: string, form_code: string, year: number, quarter: number | null): object | null;
export function save_form_draft(tin: string, form_code: string, year: number, quarter: number | null, status: string, data: object): number;
export function list_queued(): DraftSummary[];
export function queue_form_draft(tin: string, form_code: string, year: number, quarter: number | null): number;
export function cancel_queued(tin: string, form_code: string, year: number, quarter: number | null): null;
export interface TaxDeadline {
    id: number | null;
    form_type: string;
    due_date: string;
    description: string;
}
export function list_deadlines(): TaxDeadline[];
export interface ImportResult {
    kind: "database" | "profile" | "invalid" | "cancelled";
}
export function import_archive(path?: string): ImportResult;
export interface PrintPreview {
    path: string;
}
export function form_print_html(tin: string, form_code: string, year: number, quarter: number | null): PrintPreview;
export interface BackgroundJob {
    kind: "job" | "submission";
    id: number | null;
    tin?: string;
    form_code?: string;
    taxable_year?: number;
    quarter?: number | null;
    month?: number | null;
    name: string;
    job_type: string;
    cron_expr: string | null;
    status: string;
    retries: number;
    last_run_at: string | null;
    next_run_at: string | null;
    created_at: string;
    output_log: string | null;
}
export function list_background_jobs(): BackgroundJob[];
export function delete_background_job(id: number): null;
export function archive_background_job(id: number): null;
export function purge_archived_jobs(): null;
export interface JobLogTail {
    file_name: string;
    lines: number;
    content: string;
}
export function job_log_tail(): JobLogTail;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::profile::TaxpayerType;
    use bir_core::tax_profile::{BranchFacts, NewTaxpayer};
    use gpui_shell::HostArguments;

    const TIN: &str = "12345678800000";

    fn setup() {
        use_ephemeral_database();
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: "123456788".into(),
                name: "Example Trading Corp".into(),
                taxpayer_type: TaxpayerType::Corporation,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "040".into(),
                    registered_address: "1 Example Street, Pasig".into(),
                    zip_code: "1600".into(),
                    phone: "0280000000".into(),
                    email: "office@example.com".into(),
                },
            })
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn args(values: Vec<HostValue>) -> HostArguments {
        HostArguments::new(values)
    }

    fn has_totp() -> bool {
        with_database(|db| Ok(taxpayer_credentials(db, ROOT)?.totp_secret.is_some())).unwrap()
    }

    fn current_code(secret: &str) -> String {
        use totp_rs::{Algorithm, Secret, TOTP};
        let bytes = Secret::Encoded(secret.to_string()).to_bytes().unwrap();
        TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, String::new())
            .unwrap()
            .generate_current()
            .unwrap()
    }

    const ROOT: &str = "123456788";
    const BRANCH: &str = "12345678800001";
    const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

    fn s(value: &str) -> HostValue {
        HostValue::Str(value.into())
    }

    fn add_branch() {
        with_database(|db| {
            db.create_branch(
                ROOT,
                bir_core::tax_profile::NewBranch {
                    branch_code: "00001".into(),
                    name: "Makati branch".into(),
                    fields: Default::default(),
                },
            )
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn profile_credentials(tin: &str) -> (Option<String>, Option<String>) {
        with_database(|db| {
            let profile = get_profile_by_tin(db, tin)?;
            Ok((
                profile.profile_pin_hash.clone(),
                profile.totp_secret.clone(),
            ))
        })
        .unwrap()
    }

    fn truthy(result: HostResult) -> bool {
        result.unwrap() == HostValue::Bool(true)
    }

    fn state() -> serde_json::Value {
        to_json(&auth_state().unwrap())
    }

    fn set(key: &str, value: &str) {
        with_database(|db| db.set_setting(key, value).map_err(db_error)).unwrap();
    }

    #[test]
    fn set_taxpayer_totp_needs_a_matching_code_and_clears_with_null() {
        setup();
        let wrong = set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s("000000")]));
        if current_code(SECRET) != "000000" {
            assert!(wrong.unwrap_err().message().contains("does not match"));
            assert!(!has_totp());
        }
        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
        assert!(truthy(verify_taxpayer_totp(&args(vec![
            s(ROOT),
            s(&current_code(SECRET))
        ]))));
        set_taxpayer_totp(&args(vec![s(ROOT), HostValue::Null])).unwrap();
        assert!(!has_totp());
    }

    #[test]
    fn credentials_are_stored_once_per_taxpayer_not_on_branches() {
        setup();
        add_branch();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        assert_eq!(profile_credentials(TIN), (None, None));
        assert_eq!(profile_credentials(BRANCH), (None, None));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1234")
        ]))));
        assert_eq!(
            to_json(&taxpayer_auth(&args(vec![s(ROOT)])).unwrap()),
            serde_json::json!({"has_pin": true, "has_totp": false})
        );

        // The legacy per-branch setter writes the same taxpayer PIN.
        set_profile_pin(&args(vec![s(BRANCH), s("2222")])).unwrap();
        assert_eq!(profile_credentials(BRANCH), (None, None));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("2222")]))));

        assert!(set_taxpayer_pin(&args(vec![s(ROOT), s("12345")])).is_err());
        set_taxpayer_pin(&args(vec![s(ROOT), HostValue::Null])).unwrap();
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("2222")
        ]))));
    }

    #[test]
    fn six_wrong_pins_make_even_the_right_one_wait() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        for _ in 0..5 {
            assert!(!truthy(verify_taxpayer_pin(&args(vec![
                s(ROOT),
                s("1111")
            ]))));
        }
        let sixth = verify_taxpayer_pin(&args(vec![s(ROOT), s("1111")])).unwrap_err();
        assert!(
            sixth.to_string().contains("Too many wrong tries"),
            "{sixth}"
        );
        assert!(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).is_err());
    }

    #[test]
    fn auth_state_is_unset_until_first_launch_setup() {
        setup();
        assert_eq!(
            state(),
            serde_json::json!({
                "mode": null,
                "admin_root": null,
                "has_admin_pin": false,
                "has_admin_totp": false,
                "has_recovery_key": false,
                "mac_login_available": mac_login_available(),
            })
        );
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(set_admin_pin(&args(vec![s("0817")])).is_err());
        assert!(create_recovery_key().is_err());
    }

    const OTHER_ROOT: &str = "987654321";

    fn add_other_taxpayer() {
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: OTHER_ROOT.into(),
                name: "Maria Santos".into(),
                taxpayer_type: TaxpayerType::Individual,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "043".into(),
                    registered_address: "2 Sample Road, Makati".into(),
                    zip_code: "1200".into(),
                    phone: String::new(),
                    email: String::new(),
                },
            })
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn null() -> HostValue {
        HostValue::Null
    }

    fn message(result: HostResult) -> String {
        result.unwrap_err().message().to_owned()
    }

    fn key_of(result: HostResult) -> String {
        to_json(&result.unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn change_owner_moves_the_admin_to_another_taxpayer() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let old_key = key_of(create_recovery_key());

        let to_other = |pin: &str| change_owner(&args(vec![s(OTHER_ROOT), s(pin), null()]));
        assert!(message(to_other("1111")).contains("not the admin"));
        assert!(message(to_other("0817")).contains("no PIN or authenticator"));
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        with_database(|db| db.set_taxpayer_archived(OTHER_ROOT, true).map_err(db_error)).unwrap();
        assert!(message(to_other("0817")).contains("archived"));
        with_database(|db| {
            db.set_taxpayer_archived(OTHER_ROOT, false)
                .map_err(db_error)
        })
        .unwrap();
        assert!(change_owner(&args(vec![s("000000000"), s("0817"), null()])).is_err());
        assert!(message(change_owner(&args(vec![s(ROOT), s("0817"), null()]))).contains("already"));
        assert_eq!(state()["admin_root"], ROOT, "refusals change nothing");

        let new_key = key_of(to_other("0817"));
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], OTHER_ROOT);
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        // The old owner keeps their own PIN and is now an ordinary taxpayer.
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        assert!(set_taxpayer_archived(&args(vec![s(OTHER_ROOT), HostValue::Bool(true)])).is_err());
        // The previous admin's recovery key is dead; the new one works once.
        assert!(!truthy(use_recovery_key(&args(vec![s(&old_key)]))));
        assert!(truthy(use_recovery_key(&args(vec![s(&new_key)]))));
    }

    #[test]
    fn change_to_helpdesk_and_back_to_an_owner() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();

        assert!(
            message(change_to_helpdesk(&args(vec![
                s("1111"),
                null(),
                s("4321")
            ])))
            .contains("not the admin")
        );
        assert!(change_to_helpdesk(&args(vec![s("0817"), null(), s("12")])).is_err());
        assert_eq!(state()["mode"], "solo", "refusals change nothing");

        // Confirming with the authenticator code works as well as the PIN.
        let key = key_of(change_to_helpdesk(&args(vec![
            null(),
            s(&current_code(SECRET)),
            s("4321"),
        ])));
        assert_eq!(key.len(), 19);
        assert_eq!(state()["mode"], "helpdesk");
        assert_eq!(state()["admin_root"], "000000000");
        assert_eq!(
            state()["has_admin_totp"],
            false,
            "no authenticator carried over"
        );
        assert!(truthy(verify_admin_pin(&args(vec![s("4321")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        // The former owner is no longer protected from archiving.
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(false)])).unwrap();
        assert!(
            message(change_to_helpdesk(&args(vec![
                s("4321"),
                null(),
                s("5555")
            ])))
            .contains("already")
        );

        // Back to solo with another taxpayer as the owner.
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        assert!(
            change_owner(&args(vec![s(OTHER_ROOT), s("0817"), null()])).is_err(),
            "help desk PIN needed"
        );
        change_owner(&args(vec![s(OTHER_ROOT), s("4321"), null()])).unwrap();
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], OTHER_ROOT);
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));
        let leftover = with_database(|db| Ok(setting(db, ADMIN_PIN_HASH)?.is_some())).unwrap();
        assert!(!leftover, "the help desk admin PIN is removed");
    }

    #[test]
    fn solo_admin_is_the_owner_taxpayer() {
        setup();
        assert!(setup_solo(&args(vec![s("000000000")])).is_err());
        assert!(
            setup_solo(&args(vec![s("999999999")])).is_err(),
            "no such taxpayer"
        );
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        assert!(setup_solo(&args(vec![s(ROOT)])).is_err(), "only once");
        assert!(setup_helpdesk(&args(vec![s("1111")])).is_err(), "only once");
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], ROOT);

        // One code: the owner's taxpayer PIN is the admin PIN.
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        assert_eq!(state()["has_admin_pin"], true);
        assert!(truthy(verify_admin_pin(&args(vec![s("0817")]))));
        set_admin_pin(&args(vec![s("4321")])).unwrap();
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("4321")]))));
        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
        assert!(truthy(verify_admin_totp(&args(vec![s(&current_code(
            SECRET
        ))]))));
        // The help desk settings play no part in solo mode.
        set(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin("9999"));
        assert!(!truthy(verify_admin_pin(&args(vec![s("9999")]))));

        // Clearing the owner's PIN and authenticator is allowed in solo.
        set_admin_pin(&args(vec![HostValue::Null])).unwrap();
        set_admin_totp(&args(vec![HostValue::Null])).unwrap();
        assert_eq!(state()["has_admin_pin"], false);
        assert_eq!(state()["has_admin_totp"], false);
    }

    #[test]
    fn helpdesk_admin_uses_the_settings_and_keeps_one_way_in() {
        setup();
        assert!(setup_helpdesk(&args(vec![s("12")])).is_err());
        let created = to_json(&setup_helpdesk(&args(vec![s("0817")])).unwrap());
        assert_eq!(created["recovery_key"].as_str().unwrap().len(), 19);
        let st = state();
        assert_eq!(st["mode"], "helpdesk");
        assert_eq!(st["admin_root"], "000000000");
        assert_eq!(st["has_admin_pin"], true);
        assert_eq!(st["has_admin_totp"], false);
        assert_eq!(st["has_recovery_key"], true);
        assert!(truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("1234")]))));

        // A taxpayer's PIN is not the admin PIN in help desk mode.
        set_taxpayer_pin(&args(vec![s(ROOT), s("5555")])).unwrap();
        assert!(!truthy(verify_admin_pin(&args(vec![s("5555")]))));

        // The only way in cannot be removed.
        let refused = set_admin_pin(&args(vec![HostValue::Null])).unwrap_err();
        assert!(refused.message().contains("at least one way"));
        assert_eq!(state()["has_admin_pin"], true);

        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(truthy(verify_admin_totp(&args(vec![s(&current_code(
            SECRET
        ))]))));
        set_admin_pin(&args(vec![HostValue::Null])).unwrap();
        assert_eq!(state()["has_admin_pin"], false);
        let refused = set_admin_totp(&args(vec![HostValue::Null])).unwrap_err();
        assert!(refused.message().contains("at least one way"));
        assert_eq!(state()["has_admin_totp"], true);
    }

    #[test]
    fn a_recovery_key_works_once_and_a_new_one_replaces_it() {
        setup();
        let first = to_json(&setup_helpdesk(&args(vec![s("0817")])).unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_string();
        let second = to_json(&create_recovery_key().unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            !truthy(use_recovery_key(&args(vec![s(&first)]))),
            "replaced"
        );
        let typed = second.to_lowercase().replace('-', " ");
        assert!(truthy(use_recovery_key(&args(vec![s(&typed)]))));
        assert_eq!(state()["has_recovery_key"], false);
        assert!(
            !truthy(use_recovery_key(&args(vec![s(&second)]))),
            "single use"
        );
    }

    #[test]
    fn the_admin_taxpayer_cannot_be_archived_or_deleted() {
        setup();
        add_branch();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        for result in [
            set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])),
            delete_taxpayer(&args(vec![s(ROOT)])),
            delete_taxpayer(&args(vec![s("000000000")])),
        ] {
            assert!(result.unwrap_err().message().contains("admin account"));
        }
        // One branch may go, but not the last one.
        delete_profile(&args(vec![s(BRANCH)])).unwrap();
        assert!(delete_profile(&args(vec![s(TIN)])).is_err());
        assert!(set_profile_archived(&args(vec![s(TIN), HostValue::Bool(true)])).is_err());
        with_database(|db| {
            assert!(db.get_taxpayer(ROOT).map_err(db_error)?.is_some());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn other_taxpayers_archive_and_delete_across_every_branch() {
        setup();
        add_branch();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        assert!(profile_detail_archived(TIN) && profile_detail_archived(BRANCH));
        with_database(|db| {
            assert!(
                db.get_taxpayer(ROOT)
                    .map_err(db_error)?
                    .unwrap()
                    .taxpayer
                    .archived
            );
            Ok(())
        })
        .unwrap();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(false)])).unwrap();
        assert!(!profile_detail_archived(TIN));
        delete_taxpayer(&args(vec![s(ROOT)])).unwrap();
        with_database(|db| {
            assert!(db.get_taxpayer(ROOT).map_err(db_error)?.is_none());
            assert!(db.get_profile_by_tin(BRANCH).map_err(db_error)?.is_none());
            Ok(())
        })
        .unwrap();
    }

    fn profile_detail_archived(tin: &str) -> bool {
        with_database(|db| Ok(get_profile_by_tin(db, tin)?.is_archived)).unwrap()
    }

    #[test]
    fn mac_login_answers_without_blocking_the_caller_off_macos() {
        if mac_login_available() {
            return; // would show a real OS prompt
        }
        let future = module()
            .begin("mac_login", &args(vec![s("unlock")]))
            .expect("sync half");
        let answer = futures::executor::block_on(future).unwrap();
        assert_eq!(answer, HostValue::Bool(false));
    }
}
