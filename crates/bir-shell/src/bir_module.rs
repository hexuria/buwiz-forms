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
        db.delete_profile(tin)
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

fn set_profile_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let pin = arguments.value(1)?.as_str().map(str::to_owned); // null clears
    with_database(|db| {
        let mut profile = get_profile_by_tin(db, &tin)?;
        profile.profile_pin_hash = pin.map(|pin| bir_core::crypto::hash_pin(&pin));
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .map(|_| HostValue::Null)
    })
}

/// Set or clear a taxpayer's authenticator. Setting needs a current code
/// from the authenticator app, checked against the new secret, so a
/// mistyped enrolment cannot lock the taxpayer out. `null` clears it.
fn set_profile_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let secret = arguments.value(1)?.as_str().map(str::to_owned);
    let token = arguments
        .get(2)
        .and_then(HostValue::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Some(secret) = &secret
        && !bir_core::crypto::validate_totp(secret, &token)
    {
        return Err(HostError::new(
            "That code does not match. Type the 6-digit code your authenticator app shows now.",
        ));
    }
    with_database(|db| {
        let mut profile = get_profile_by_tin(db, &tin)?;
        profile.totp_secret = secret;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .map(|_| HostValue::Null)
    })
}

/// App-wide auth toggles the lock screen and profile unlock gate read once
/// per render (same settings keys as the painted app reads at startup).
fn auth_state() -> HostResult {
    with_database(|db| {
        let flag = |key: &str| {
            db.get_setting(key)
                .ok()
                .flatten()
                .is_some_and(|v| v == "true")
        };
        let set = |key: &str| db.get_setting(key).ok().flatten().is_some();
        Ok(HostObject::new()
            .field("enable_profile_pins", flag("enable_profile_pins"))
            .field("has_admin_totp", set("app_totp_secret"))
            .field("has_admin_pin", set("app_lock_pin_hash"))
            .field("hide_tax_profiles", flag("hide_tax_profiles"))
            .into())
    })
}

fn profile_auth(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        let profile = get_profile_by_tin(db, tin)?;
        Ok(HostObject::new()
            .field("has_pin", profile.profile_pin_hash.is_some())
            .field("has_totp", profile.totp_secret.is_some())
            .into())
    })
}

fn verify_profile_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let pin = arguments.string(1)?;
    let hashed = bir_core::crypto::hash_pin(pin);
    with_database(|db| {
        let profile = get_profile_by_tin(db, tin)?;
        Ok(HostValue::Bool(
            profile.profile_pin_hash.as_deref() == Some(hashed.as_str()),
        ))
    })
}

fn verify_profile_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    let token = arguments.string(1)?;
    with_database(|db| {
        let profile = get_profile_by_tin(db, tin)?;
        let ok = profile
            .totp_secret
            .as_deref()
            .is_some_and(|secret| bir_core::crypto::validate_totp(secret, token));
        Ok(HostValue::Bool(ok))
    })
}

fn verify_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = arguments.string(0)?;
    let hashed = bir_core::crypto::hash_pin(pin);
    with_database(|db| {
        let ok = db
            .get_setting("app_lock_pin_hash")
            .ok()
            .flatten()
            .is_some_and(|stored| stored == hashed);
        Ok(HostValue::Bool(ok))
    })
}

fn verify_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let token = arguments.string(0)?;
    with_database(|db| {
        let ok = db
            .get_setting("app_totp_secret")
            .ok()
            .flatten()
            .is_some_and(|secret| bir_core::crypto::validate_totp(&secret, token));
        Ok(HostValue::Bool(ok))
    })
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
            .map_err(|error| HostError::new(error.to_string()))
    })
}

// ---------- taxpayer credentials (one PIN per TIN root) ----------
//
// A PIN and an authenticator belong to the taxpayer, not the branch: every
// branch of a TIN root shares them. They are still stored on each branch's
// profile row (that is where `profile_pin_hash` lives), so writes go to every
// branch and reads resolve the head office first, then the other branches.

/// Branch profiles of a taxpayer, head office first. Branches without a
/// profile row are skipped.
fn taxpayer_profiles(
    db: &Database,
    tin_root: &str,
) -> Result<Vec<bir_core::profile::TaxpayerProfile>, HostError> {
    let taxpayer = db
        .get_taxpayer(tin_root)
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new(format!("No taxpayer with TIN {tin_root}")))?;
    let mut profiles = Vec::new();
    for branch in &taxpayer.branches {
        if let Some(profile) = db
            .get_profile_by_tin(&branch.tin)
            .map_err(|error| HostError::new(error.to_string()))?
        {
            profiles.push(profile);
        }
    }
    if profiles.is_empty() {
        return Err(HostError::new(format!(
            "no taxpayer profile with TIN {tin_root}"
        )));
    }
    Ok(profiles)
}

/// The taxpayer's PIN hash and authenticator secret: the head office's when
/// set, otherwise the first branch's that has one.
fn taxpayer_credentials(
    profiles: &[bir_core::profile::TaxpayerProfile],
) -> (Option<String>, Option<String>) {
    let pin = profiles.iter().find_map(|p| p.profile_pin_hash.clone());
    let totp = profiles.iter().find_map(|p| p.totp_secret.clone());
    (pin, totp)
}

/// Write one PIN hash and authenticator secret to every branch of a taxpayer.
/// Returns how many branch rows changed.
fn write_taxpayer_credentials(
    db: &Database,
    profiles: Vec<bir_core::profile::TaxpayerProfile>,
    pin: Option<String>,
    totp: Option<String>,
) -> Result<usize, HostError> {
    let mut changed = 0;
    for mut profile in profiles {
        if profile.profile_pin_hash == pin && profile.totp_secret == totp {
            continue;
        }
        profile.profile_pin_hash = pin.clone();
        profile.totp_secret = totp.clone();
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))?;
        changed += 1;
    }
    Ok(changed)
}

fn taxpayer_auth(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    with_database(|db| {
        let (pin, totp) = taxpayer_credentials(&taxpayer_profiles(db, tin_root)?);
        Ok(HostObject::new()
            .field("has_pin", pin.is_some())
            .field("has_totp", totp.is_some())
            .into())
    })
}

/// Set (`pin`) or clear (`null`) the PIN of every branch of a taxpayer.
fn set_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?.to_owned();
    let pin = arguments.value(1)?.as_str().map(bir_core::crypto::hash_pin);
    with_database(|db| {
        let profiles = taxpayer_profiles(db, &tin_root)?;
        let (_, totp) = taxpayer_credentials(&profiles);
        write_taxpayer_credentials(db, profiles, pin, totp).map(|_| HostValue::Null)
    })
}

/// Set or clear a taxpayer's authenticator; setting needs a current code, as
/// in `set_profile_totp`.
fn set_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?.to_owned();
    let secret = arguments.value(1)?.as_str().map(str::to_owned);
    let token = arguments
        .get(2)
        .and_then(HostValue::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Some(secret) = &secret
        && !bir_core::crypto::validate_totp(secret, &token)
    {
        return Err(HostError::new(
            "That code does not match. Type the 6-digit code your authenticator app shows now.",
        ));
    }
    with_database(|db| {
        let profiles = taxpayer_profiles(db, &tin_root)?;
        let (pin, _) = taxpayer_credentials(&profiles);
        write_taxpayer_credentials(db, profiles, pin, secret).map(|_| HostValue::Null)
    })
}

fn verify_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    let hashed = bir_core::crypto::hash_pin(arguments.string(1)?);
    with_database(|db| {
        let (pin, _) = taxpayer_credentials(&taxpayer_profiles(db, tin_root)?);
        Ok(HostValue::Bool(pin.as_deref() == Some(hashed.as_str())))
    })
}

fn verify_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = arguments.string(0)?;
    let token = arguments.string(1)?;
    with_database(|db| {
        let (_, totp) = taxpayer_credentials(&taxpayer_profiles(db, tin_root)?);
        Ok(HostValue::Bool(totp.as_deref().is_some_and(|secret| {
            bir_core::crypto::validate_totp(secret, token)
        })))
    })
}

/// One-time move to one PIN per taxpayer: every branch takes the head
/// office's PIN and authenticator (or, without one, the first branch's that
/// has one). Safe to run at every launch; returns how many rows changed.
fn unify_taxpayer_credentials() -> HostResult {
    with_database(|db| {
        let mut changed = 0;
        for entry in db
            .list_taxpayers()
            .map_err(|error| HostError::new(error.to_string()))?
        {
            let Ok(profiles) = taxpayer_profiles(db, &entry.taxpayer.tin_root) else {
                continue;
            };
            let (pin, totp) = taxpayer_credentials(&profiles);
            changed += write_taxpayer_credentials(db, profiles, pin, totp)?;
        }
        Ok(HostValue::Number(changed as f64))
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
        open_print_preview(slug, &form_code, year, &fields, receipt.as_ref())
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
) -> HostResult {
    let html = bir_print::frozen_html::filled_document_with_receipt(slug, fields, receipt)
        .map_err(HostError::new)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let path =
        std::env::temp_dir().join(format!("bir-{form_code}-{year}-{timestamp}-preview.html"));
    std::fs::write(&path, html).map_err(|error| HostError::new(error.to_string()))?;
    open_in_system_browser(&path)?;
    Ok(HostObject::new()
        .field("path", path.display().to_string())
        .into())
}

/// Launch the platform's default HTML viewer on `path` — detached, so the
/// host call returns immediately like the painted preview window does.
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

/// Admin PIN lives only as its hash (`app_lock_pin_hash`); hashing stays
/// host-side so the script never sees the stored digest.
fn set_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = arguments.value(0)?.as_str().map(str::to_owned); // null clears
    with_database(|db| {
        match pin {
            Some(pin) => db.set_setting("app_lock_pin_hash", &bir_core::crypto::hash_pin(&pin)),
            None => db.delete_setting("app_lock_pin_hash"),
        }
        .map(|_| HostValue::Null)
        .map_err(|error| HostError::new(error.to_string()))
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
        .function("auth_state", |_| auth_state())
        .function("profile_auth", profile_auth)
        .function("verify_profile_pin", verify_profile_pin)
        .function("verify_profile_totp", verify_profile_totp)
        .function("set_profile_totp", set_profile_totp)
        .function("taxpayer_auth", taxpayer_auth)
        .function("set_taxpayer_pin", set_taxpayer_pin)
        .function("set_taxpayer_totp", set_taxpayer_totp)
        .function("verify_taxpayer_pin", verify_taxpayer_pin)
        .function("verify_taxpayer_totp", verify_taxpayer_totp)
        .function("unify_taxpayer_credentials", |_| {
            unify_taxpayer_credentials()
        })
        .function("verify_admin_pin", verify_admin_pin)
        .function("verify_admin_totp", verify_admin_totp)
        .function("generate_totp_secret", generate_totp_secret)
        .function("delete_setting", delete_setting)
        .function("set_admin_pin", set_admin_pin)
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
    enable_profile_pins: boolean;
    has_admin_totp: boolean;
    has_admin_pin: boolean;
    hide_tax_profiles: boolean;
}
export interface ProfileAuth {
    has_pin: boolean;
    has_totp: boolean;
}
export interface TotpSecret {
    secret: string;
    qr_path: string;
}
export function profile_detail(tin: string): object;
export function save_profile(profile: object): object;
export function set_profile_archived(tin: string, archived: boolean): object;
export function delete_profile(tin: string): null;
export function set_profile_pin(tin: string, pin: string | null): null;
export function auth_state(): AuthState;
export function profile_auth(tin: string): ProfileAuth;
export function verify_profile_pin(tin: string, pin: string): boolean;
export function verify_profile_totp(tin: string, token: string): boolean;
/** Set (with a current code from the app) or clear (null) a taxpayer's authenticator. */
export function set_profile_totp(tin: string, secret: string | null, token?: string): null;
/** PIN and authenticator of a taxpayer (TIN root), shared by every branch. */
export function taxpayer_auth(tin_root: string): ProfileAuth;
export function set_taxpayer_pin(tin_root: string, pin: string | null): null;
export function set_taxpayer_totp(tin_root: string, secret: string | null, token?: string): null;
export function verify_taxpayer_pin(tin_root: string, pin: string): boolean;
export function verify_taxpayer_totp(tin_root: string, token: string): boolean;
/** Give every branch its taxpayer's PIN and authenticator; returns rows changed. */
export function unify_taxpayer_credentials(): number;
export function verify_admin_pin(pin: string): boolean;
export function verify_admin_totp(token: string): boolean;
export function generate_totp_secret(account: string): TotpSecret;
export function delete_setting(key: string): null;
export function set_admin_pin(pin: string | null): null;

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
        with_database(|db| Ok(get_profile_by_tin(db, TIN)?.totp_secret.is_some())).unwrap()
    }

    fn current_code(secret: &str) -> String {
        use totp_rs::{Algorithm, Secret, TOTP};
        let bytes = Secret::Encoded(secret.to_string()).to_bytes().unwrap();
        TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, String::new())
            .unwrap()
            .generate_current()
            .unwrap()
    }

    #[test]
    fn set_profile_totp_needs_a_matching_code_and_clears_with_null() {
        setup();
        let secret = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        let wrong = set_profile_totp(&args(vec![
            HostValue::Str(TIN.into()),
            HostValue::Str(secret.into()),
            HostValue::Str("000000".into()),
        ]));
        if current_code(secret) != "000000" {
            assert!(wrong.unwrap_err().message().contains("does not match"));
            assert!(!has_totp());
        }
        set_profile_totp(&args(vec![
            HostValue::Str(TIN.into()),
            HostValue::Str(secret.into()),
            HostValue::Str(current_code(secret)),
        ]))
        .unwrap();
        assert!(has_totp());
        set_profile_totp(&args(vec![HostValue::Str(TIN.into()), HostValue::Null])).unwrap();
        assert!(!has_totp());
    }

    const ROOT: &str = "123456788";
    const BRANCH: &str = "12345678800001";

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

    fn pin_hash(tin: &str) -> Option<String> {
        with_database(|db| Ok(get_profile_by_tin(db, tin)?.profile_pin_hash.clone())).unwrap()
    }

    fn set_branch_pin(tin: &str, pin: &str) {
        set_profile_pin(&args(vec![
            HostValue::Str(tin.into()),
            HostValue::Str(pin.into()),
        ]))
        .unwrap();
    }

    fn verify(pin: &str) -> bool {
        verify_taxpayer_pin(&args(vec![
            HostValue::Str(ROOT.into()),
            HostValue::Str(pin.into()),
        ]))
        .unwrap()
            == HostValue::Bool(true)
    }

    #[test]
    fn a_taxpayer_pin_is_written_to_every_branch_and_cleared_from_every_branch() {
        setup();
        add_branch();
        set_taxpayer_pin(&args(vec![
            HostValue::Str(ROOT.into()),
            HostValue::Str("0817".into()),
        ]))
        .unwrap();
        let hash = Some(bir_core::crypto::hash_pin("0817"));
        assert_eq!(pin_hash(TIN), hash);
        assert_eq!(pin_hash(BRANCH), hash);
        assert!(verify("0817"));
        assert!(!verify("1234"));

        set_taxpayer_pin(&args(vec![HostValue::Str(ROOT.into()), HostValue::Null])).unwrap();
        assert_eq!(pin_hash(TIN), None);
        assert_eq!(pin_hash(BRANCH), None);
        assert!(!verify("0817"));
    }

    #[test]
    fn unifying_gives_every_branch_the_head_office_pin() {
        setup();
        add_branch();
        set_branch_pin(TIN, "1111");
        set_branch_pin(BRANCH, "2222");
        assert!(verify("1111"), "the head office's PIN wins");
        assert!(!verify("2222"));
        assert_eq!(
            unify_taxpayer_credentials().unwrap(),
            HostValue::Number(1.0)
        );
        assert_eq!(pin_hash(BRANCH), Some(bir_core::crypto::hash_pin("1111")));
        assert_eq!(
            unify_taxpayer_credentials().unwrap(),
            HostValue::Number(0.0),
            "a second run changes nothing"
        );
    }

    #[test]
    fn a_pin_only_a_branch_had_becomes_the_taxpayer_pin() {
        setup();
        add_branch();
        set_branch_pin(BRANCH, "2222");
        assert!(verify("2222"));
        unify_taxpayer_credentials().unwrap();
        assert_eq!(pin_hash(TIN), Some(bir_core::crypto::hash_pin("2222")));
    }
}
