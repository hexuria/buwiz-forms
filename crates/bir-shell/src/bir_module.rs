//! The `bir` host module: the JavaScript-facing bridge into bir-core.
//!
//! Everything here crosses the boundary as `HostValue` — JSON-shaped records,
//! never Rust types. Functions are synchronous (main thread, no engine
//! re-entry); slow work (SFTP, exports) gets an `async_function` in a later
//! slice. The database opens lazily through `open_live_database` so
//! `BIR_DATABASE_PATH` keeps working for tests exactly like painted `bir`.

use std::cell::RefCell;

use bir_core::db::{self, Database};
use bir_core::forms::queueable::QueueableForm as _;
use bir_core::forms::FORM_REGISTRY;
use gpui_shell::{HostError, HostModule, HostObject, HostResult, HostValue};

thread_local! {
    static DATABASE: RefCell<Option<Database>> = const { RefCell::new(None) };
}

/// One connection for the process — the same owner-lock discipline as painted
/// `bir` (a second app instance cannot take it while we hold it).
fn with_database<R>(f: impl FnOnce(&Database) -> Result<R, HostError>) -> Result<R, HostError> {
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

/// `serde_json::Value` → `HostValue`. bir-core serializes cleanly; the
/// boundary keeps JSON's six shapes exactly.
fn json(value: serde_json::Value) -> HostValue {
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

fn serialize<T: serde::Serialize>(value: &T) -> Result<HostValue, HostError> {
    serde_json::to_value(value)
        .map(json)
        .map_err(|error| HostError::new(error.to_string()))
}

/// `HostValue` → `serde_json::Value`: the inverse of `json`, for functions
/// that take structured arguments (a whole profile record, a draft).
fn to_json(value: &HostValue) -> serde_json::Value {
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

fn get_profile_by_tin(db: &Database, tin: &str) -> Result<bir_core::profile::TaxpayerProfile, HostError> {
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
        Ok(HostValue::Bool(profile.profile_pin_hash.as_deref() == Some(hashed.as_str())))
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
    let hashed = bir_core::crypto::hash_pin(&pin);
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
                            .field(
                                "month",
                                row.month.map_or(HostValue::Null, HostValue::from),
                            )
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
fn optional_quarter(arguments: &gpui_shell::HostArguments, index: usize) -> Result<Option<u8>, HostError> {
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
                        serde_json::to_value(&d)
                            .map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            "1601C" => {
                let month = quarter.unwrap_or(1);
                db.get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d)
                            .map_err(|error| HostError::new(error.to_string()))
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
                let base =
                    bir_core::forms::Form1601CDraft::new_from_profile(&profile, year, month);
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
        let queued: Result<i64, HostError> = (|| {
            match form_code.as_str() {
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
                    let month = quarter.ok_or_else(|| {
                        HostError::new("1601C drafts are monthly: pass the month")
                    })?;
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
                    let kind = QueueableKind::from_form_code(&form_code).ok_or_else(|| {
                        HostError::new(format!("Form {form_code} is not queueable"))
                    })?;
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
        let result: Result<(), HostError> = (|| {
            match form_code.as_str() {
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
                    let month = quarter.ok_or_else(|| {
                        HostError::new("1601C drafts are monthly: pass the month")
                    })?;
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
                    let kind = QueueableKind::from_form_code(&form_code).ok_or_else(|| {
                        HostError::new(format!("Form {form_code} is not queueable"))
                    })?;
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
            }
        })();
        result.map(|_| HostValue::Null)
    })
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
        if db_path.exists() {
            if let Some(parent) = db_path.parent() {
                let _ =
                    std::fs::copy(&db_path, parent.join(format!("bir_data.safety-{timestamp}.db")));
            }
        }
        reset_database();
        return match bir_core::import::extract_database_zip(&path, &db_path) {
            Ok(()) => Ok(HostObject::new().field("kind", "database").into()),
            Err(error) => Err(HostError::new(error.to_string())),
        };
    }

    if is_profile {
        return with_database(|db| match bir_core::import::import_profile_data(db, &path) {
            Ok(()) => Ok(HostObject::new().field("kind", "profile").into()),
            Err(error) => Err(HostError::new(error.to_string())),
        });
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

/// BIR announcements feed — the notifications drawer content.
fn list_announcements() -> HostResult {
    with_database(|db| {
        db.list_announcements()
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(|announcements| serialize(&announcements))
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
    HostModule::new("bir")
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
        .function("list_announcements", |_| list_announcements())
        .function("import_archive", import_archive)
        .declarations(
            r#"
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
export interface Announcement {
    id: number | null;
    source: string;
    title: string;
    content: string;
    published_at: string;
    read_status: boolean;
}
export function list_announcements(): Announcement[];
export interface ImportResult {
    kind: "database" | "profile" | "invalid" | "cancelled";
}
export function import_archive(path?: string): ImportResult;
"#,
        )
}
