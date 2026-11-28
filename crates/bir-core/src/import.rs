use crate::db::{Database, DbError, Submission};
use crate::integration::providers::ProviderConfig;
use crate::private_temp::{PrivateTempDir, unique_name};
use crate::profile::{ProfileCredentials, TaxpayerProfile};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::info;
use zip::ZipArchive;

/// Parse a semver string like "0.0.1" into (major, minor, patch).
/// Returns (0, 0, 0) for unparseable strings.
fn parse_semver(s: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = s.trim().split('.').collect();
    let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// Apply sequential JSON-level migrations to a profile Value before deserialization.
///
/// Each migration step transforms the JSON structure to match the current schema.
/// The `from_version` is the semver tuple from the archive's manifest.json.
///
/// When you bump the Cargo package version AND change the export schema, add a new
/// `if from_version < (M, N, P)` block here with the corresponding field transformations.
fn migrate_profile_json(
    _value: &mut serde_json::Value,
    from_version: (u32, u32, u32),
) -> Result<(), DbError> {
    // Example for future use:
    // if from_version < (0, 2, 0) {
    //     // < 0.2.0: renamed `rdo_code` → `rdo`
    //     if let Some(old) = value.get("rdo_code").cloned() {
    //         value["rdo"] = old;
    //         value.as_object_mut().unwrap().remove("rdo_code");
    //     }
    // }
    if from_version < (0, 0, 1) {
        // Pre-0.0.1 (legacy unversioned): no structural changes needed.
        // All newer fields have #[serde(default)].
    }
    Ok(())
}

/// Apply sequential JSON-level migrations to a submission Value before deserialization.
fn migrate_submission_json(
    _value: &mut serde_json::Value,
    from_version: (u32, u32, u32),
) -> Result<(), DbError> {
    if from_version < (0, 0, 1) {
        // Pre-0.0.1: no changes needed
    }
    Ok(())
}

/// The period key the v5 backfill derives for a row that has none: monthly
/// forms their M{month}, annual forms their A, open-ended forms their O{n},
/// everything else its quarter.
fn derive_period_key(form_code: &str, quarter: Option<i64>) -> String {
    // The registry's own mapping, never a hand list: a monthly form derives
    // an M key whatever its month, an annual A, an open-ended entry O{n} —
    // the same key `period_from_row` wins with when it reads the row back.
    crate::db::drafts::period_from_legacy_slot(
        form_code,
        quarter.and_then(|value| u8::try_from(value).ok()),
        1,
    )
    .to_period_key()
}

/// Monotonic filing status rank. Unknown legacy text is weakest: it may never
/// displace a known state, while any known state may displace it.
fn filing_status_rank(status: &str) -> u8 {
    match status.to_ascii_lowercase().as_str() {
        "paid" => 10,
        "confirmed" => 8,
        "submitted" | "filed" => 6,
        "queued" => 4,
        "draft" => 2,
        _ => 0,
    }
}

/// The rank a row competes with, dry runs one notch under a real send: a
/// dry-run Submitted never displaces a real submission, and a real one
/// displaces a dry run by rank. The flag lives in the row's data, not its
/// status.
fn filing_status_rank_with_data(status: &str, data_json: &str) -> u8 {
    let rank = filing_status_rank(status);
    // A dry run is not a send, only a rehearsal: it ranks as a Draft, never
    // one notch under a real send, so it can never outrank real work.
    if rank == 6
        && serde_json::from_str::<serde_json::Value>(data_json)
            .ok()
            .and_then(|value| value.get("submitted_dry_run").and_then(|v| v.as_bool()))
            .unwrap_or(false)
    {
        filing_status_rank("Draft")
    } else {
        rank
    }
}

/// The status an imported row may take. An import is never a send: the file's
/// Queued — a send nobody pressed here — comes in as a Draft.
fn import_safe_status(status: &str) -> String {
    if status.eq_ignore_ascii_case("queued") {
        "Draft".to_string()
    } else {
        status.to_string()
    }
}

/// Whether an incoming row may replace an existing row. A lower status never
/// does. A higher status is newer by itself. At the same status the
/// timestamps decide — as timestamps, not strings. A file that cannot be
/// shown newer than the stored row — a legacy archive has no timestamps — is
/// older by rule and never replaces at the same status.
fn import_may_replace(
    existing_status: &str,
    existing_updated_at: Option<&str>,
    incoming_status: &str,
    incoming_updated_at: Option<&str>,
) -> bool {
    let existing = filing_status_rank(existing_status);
    let incoming = filing_status_rank(incoming_status);
    if incoming < existing {
        return false;
    }
    if incoming > existing {
        // Further along is newer by itself — but never at the price of newer
        // work: a device row dated strictly newer than the file row stays,
        // even in front of a filed row. An undated device row cannot be
        // newer than anything, so the rank decides there as before.
        if let (Some(existing_stamp), Some(incoming_stamp)) =
            (existing_updated_at, incoming_updated_at)
        {
            if let (Some(existing_time), Some(incoming_time)) = (
                crate::db::taxpayers::parse_stored_timestamp(existing_stamp),
                crate::db::taxpayers::parse_stored_timestamp(incoming_stamp),
            ) {
                if existing_time > incoming_time {
                    return false;
                }
            }
        }
        return true;
    }
    match (existing_updated_at, incoming_updated_at) {
        (Some(existing), Some(incoming)) => match (
            crate::db::taxpayers::parse_stored_timestamp(existing),
            crate::db::taxpayers::parse_stored_timestamp(incoming),
        ) {
            (Some(existing), Some(incoming)) => incoming > existing,
            // An unreadable timestamp cannot show the file is newer.
            _ => false,
        },
        // An undated row (a legacy archive) counts as older than any existing
        // row: it never replaces at the same status, so nothing is lowered
        // and no newer work is overwritten. Where there is no row yet, or the
        // row is undated too, the import still applies.
        (Some(_), None) => false,
        // The device row undated: a dated file row is newer than anything
        // legacy, but both undated is a tie, and a tie keeps the device row.
        (None, Some(_)) => true,
        (None, None) => false,
    }
}

/// What an import did, counted in rows and records: `restored` written
/// (inserted or replaced), `kept_newer` left as they are because this
/// device's version is the newer or further-along one, `unmatched` the file
/// could not be matched or read.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportCounts {
    pub restored: u32,
    pub kept_newer: u32,
    pub unmatched: u32,
    /// File rows kept out because they were in the middle of a send where
    /// they were exported: never inserted, never matched, the claim never
    /// cleared. Counted into the page's skipped part with this reason.
    pub skipped_mid_send: u32,
}

/// The lifecycle rewrite the app's own revert performs
/// (`SubmissionLifecycle::revert_to_draft`), at the JSON level because an
/// import is form-generic: the stored status goes back to Draft and the
/// send's claim, retry and filing fields are cleared, so the return can be
/// edited, queued and cancelled again.
fn revert_json_to_draft(data_json: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(data_json) else {
        return data_json.to_string();
    };
    let Some(object) = value.as_object_mut() else {
        return data_json.to_string();
    };
    object.insert("status".to_string(), serde_json::json!("Draft"));
    object.insert("profile_baseline".to_string(), serde_json::json!({}));
    for key in [
        "submitted_at",
        "relay_receipt",
        "confirmed_at",
        "receipt_id",
        "submission_filename",
        "submitted_payload",
        "queued_submission_fingerprint",
        "queue_authorization",
        "submission_claim_token",
        "submission_claimed_at",
        "submission_claim_lease_until",
        "submission_put_started_at",
        "relay_pending",
        "submission_error",
        "next_retry_at",
    ] {
        object.insert(key.to_string(), serde_json::Value::Null);
    }
    object.insert("submitted_dry_run".to_string(), serde_json::json!(false));
    object.insert("submitted_via_relay".to_string(), serde_json::json!(false));
    object.insert("submission_attempts".to_string(), serde_json::json!(0));
    serde_json::to_string(&value).unwrap_or_else(|_| data_json.to_string())
}

/// The same-rank comparison on its own, for a pairing where status rank
/// must not decide (an amendment begun here against the file's original).
fn import_timestamp_allows_draft(
    existing_updated_at: Option<&str>,
    incoming_updated_at: Option<&str>,
) -> bool {
    match (existing_updated_at, incoming_updated_at) {
        (Some(existing), Some(incoming)) => match (
            crate::db::taxpayers::parse_stored_timestamp(existing),
            crate::db::taxpayers::parse_stored_timestamp(incoming),
        ) {
            (Some(existing), Some(incoming)) => incoming > existing,
            _ => false,
        },
        (Some(_), None) => false,
        (None, Some(_)) => true,
        (None, None) => false,
    }
}

/// Whether the stored row is an amendment of the return — a later filing
/// generation than the original the file may carry.
fn json_is_amended(data_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(data_json)
        .ok()
        .and_then(|value| value.get("is_amended").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

pub fn import_profile_data(db: &Database, import_file: &Path) -> Result<ImportCounts, DbError> {
    let file = fs::File::open(import_file)?;
    let mut archive = ZipArchive::new(file).map_err(|e| DbError::Other(e.to_string()))?;

    let mut base_dirs = Vec::new();
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name();
            if name.ends_with("profile.json") {
                let Some(base_dir) = name.strip_suffix("profile.json") else {
                    continue;
                };
                base_dirs.push(base_dir.to_string());
            }
        }
    }

    if base_dirs.is_empty() {
        return Err(DbError::Other(
            "No profile.json found in archive".to_string(),
        ));
    }

    // The whole archive writes under one transaction: a failure partway —
    // a later profile's malformed JSON, a full disk — must not leave some
    // profiles imported and some not. `save_profile` rides this transaction
    // (see its nested-transaction note); COMMIT lands all of it, ROLLBACK
    // none of it.
    db.conn.execute_batch("BEGIN IMMEDIATE")?;
    let mut counts = ImportCounts::default();
    let result = (|| -> Result<(), DbError> {
        for base_dir in base_dirs {
            // Helper to read file from archive
            let mut read_from_zip = |name: &str| -> Result<String, DbError> {
                let full_name = format!("{}{}", base_dir, name);
                let mut file = archive
                    .by_name(&full_name)
                    .map_err(|e| DbError::Other(e.to_string()))?;
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                Ok(content)
            };

            // 0. Read manifest.json for export version (defaults to (0,0,0) for legacy archives)
            let export_version = match read_from_zip("manifest.json") {
                Ok(manifest_str) => {
                    let m: serde_json::Value = serde_json::from_str(&manifest_str)?;
                    let ver_str = m
                        .get("export_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("0.0.0");
                    let ver = parse_semver(ver_str);
                    info!(
                        "Import archive manifest: export_version={}.{}.{}",
                        ver.0, ver.1, ver.2
                    );
                    ver
                }
                Err(_) => {
                    info!("No manifest.json found — treating as legacy (0.0.0) export");
                    (0, 0, 0)
                }
            };

            // 1. Read profile.json — parse as Value, migrate, then deserialize
            let profile_json = read_from_zip("profile.json")?;
            let mut profile_value: serde_json::Value = serde_json::from_str(&profile_json)?;
            migrate_profile_json(&mut profile_value, export_version)?;
            let mut profile: TaxpayerProfile = serde_json::from_value(profile_value)?;
            let tin = profile.tin.full();
            // The file's row id and credentials are never used. An id names a
            // row in the database that wrote the file; here it can be another
            // taxpayer's row, which the save would overwrite, re-keying that
            // taxpayer's records onto this TIN. Credentials stay with the
            // database they were set in. Both come from this database's row with
            // the same TIN, or are none when there is no such row.
            let (id, credentials) = match db.get_profile_by_tin(&tin)? {
                Some(mut stored) => (stored.id, stored.take_credentials()),
                None => (None, ProfileCredentials::default()),
            };
            let new_taxpayer = id.is_none();
            profile.id = id;
            profile.put_credentials(credentials);
            // A restore: versions confirmed where the export was made come in
            // as recorded when this database does not have the taxpayer yet.
            db.save_restored_profile(profile)?;
            if new_taxpayer {
                // The profile itself is a restored item: a brand-new
                // taxpayer's archive never reads "nothing was restored".
                counts.restored += 1;
            }

            // 2. Read submissions.json — deduplicate via (tin, form_type, period, submitted_at)
            if let Ok(submissions_json) = read_from_zip("submissions.json") {
                let mut submissions: Vec<serde_json::Value> =
                    serde_json::from_str(&submissions_json)?;
                for sub_value in &mut submissions {
                    migrate_submission_json(sub_value, export_version)?;
                }
                let typed: Vec<Submission> = submissions
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<_, _>>()?;

                for sub in typed {
                    let json_data = serde_json::to_string(&sub.form_data)?;
                    let submitted_at_key = sub.submitted_at.clone().unwrap_or_default();

                    // Check if a matching submission already exists (using COALESCE to match the index)
                    let exists: bool = db
                        .conn
                        .query_row(
                            "SELECT 1 FROM submissions
                     WHERE tin = ?1 AND form_type = ?2 AND period = ?3
                       AND COALESCE(submitted_at, '') = ?4",
                            rusqlite::params![tin, sub.form_type, sub.period, submitted_at_key],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    // An import is never a send: the file's Queued is a Draft here.
                    let status = import_safe_status(&sub.status);
                    if exists {
                        let (existing_status, existing_updated_at): (String, Option<String>) =
                            db.conn.query_row(
                                "SELECT status, updated_at FROM submissions
                                 WHERE tin = ?1 AND form_type = ?2 AND period = ?3
                                   AND COALESCE(submitted_at, '') = ?4",
                                rusqlite::params![tin, sub.form_type, sub.period, submitted_at_key],
                                |row| Ok((row.get(0)?, row.get(1)?)),
                            )?;
                        if import_may_replace(
                            &existing_status,
                            existing_updated_at.as_deref(),
                            &status,
                            sub.updated_at.as_deref(),
                        ) {
                            db.conn.execute(
                                "UPDATE submissions SET status = ?1, form_data = ?2, filename = ?3,
                                    updated_at = COALESCE(?4, datetime('now'))
                             WHERE tin = ?5 AND form_type = ?6 AND period = ?7
                               AND COALESCE(submitted_at, '') = ?8",
                                rusqlite::params![
                                    status,
                                    json_data,
                                    sub.filename,
                                    sub.updated_at,
                                    tin,
                                    sub.form_type,
                                    sub.period,
                                    submitted_at_key
                                ],
                            )?;
                            counts.restored += 1;
                        } else {
                            counts.kept_newer += 1;
                        }
                    } else {
                        db.conn.execute(
                        "INSERT INTO submissions (tin, form_type, period, status, form_data, submitted_at, filename, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, datetime('now')))",
                        rusqlite::params![
                            tin, sub.form_type, sub.period, status,
                            json_data, sub.submitted_at, sub.filename, sub.updated_at
                        ],
                    )?;
                        counts.restored += 1;
                    }
                }
            }

            // 3. Read drafts.json
            if let Ok(drafts_json) = read_from_zip("drafts.json") {
                let drafts: Vec<serde_json::Value> = serde_json::from_str(&drafts_json)?;

                let mut inserted_row_ids: Vec<i64> = Vec::new();
                for draft in drafts {
                    if let (Some(form_code), Some(taxable_year), Some(status), Some(data_json)) = (
                        draft.get("form_code").and_then(|v| v.as_str()),
                        draft.get("taxable_year").and_then(|v| v.as_i64()),
                        draft.get("status").and_then(|v| v.as_str()),
                        draft.get("data_json").and_then(|v| v.as_str()),
                    ) {
                        // The file's own quarter is kept for the insert:
                        // queueable annual forms (1700, 1701, 1702…) live and
                        // are read at quarter 0, open-ended forms at NULL.
                        // Quarter 0 and NULL are the same period only in the
                        // match, never a second row beside the other.
                        let raw_quarter = draft.get("quarter").and_then(|v| v.as_i64());
                        let quarter = raw_quarter;
                        let incoming_updated_at = draft.get("updated_at").and_then(|v| v.as_str());
                        // N8: a file row in the middle of its send where it
                        // was exported — any of the five claim fields in its
                        // data — is kept out entirely: never inserted, never
                        // matched, its claim never cleared. The origin
                        // device's send may still be in flight, and a cleared
                        // editable copy invites a re-queue that files twice.
                        let file_data: serde_json::Value =
                            serde_json::from_str(data_json).unwrap_or_default();
                        // Mid-send is the claim itself: the lifecycle clears
                        // the token, the claim time, the lease, the PUT start
                        // and the relay bookkeeping in one `clear_claim`, so a
                        // genuinely in-flight row always carries the token.
                        // Lease, PUT or relay fields WITHOUT it are remnants
                        // of a settled or dead send — the Queued revert clears
                        // them like any other, they are not a send in flight.
                        let file_mid_send = ["submission_claim_token", "submission_claimed_at"]
                            .into_iter()
                            .any(|key| file_data.get(key).is_some_and(|v| !v.is_null()));
                        if file_mid_send && status.eq_ignore_ascii_case("queued") {
                            counts.skipped_mid_send += 1;
                            continue;
                        }
                        // An import is never a send: the file's Queued comes
                        // in as a Draft, and its data is put back the way the
                        // app's own revert puts a send back — the lifecycle
                        // rewrite of `revert_to_draft` — so the return can be
                        // edited, queued and cancelled again, and no carried
                        // claim makes later imports skip it.
                        let was_queued = status.eq_ignore_ascii_case("queued");
                        let status = import_safe_status(status);
                        let data_json = if was_queued {
                            revert_json_to_draft(data_json)
                        } else {
                            data_json.to_string()
                        };
                        let data_json = data_json.as_str();
                        // Open-ended returns store quarter NULL and tell one
                        // period from the next by period_key, so the match is
                        // the form, the year and the period key — a NULL
                        // quarter normalized — never the quarter alone.
                        let period_key = draft.get("period_key").and_then(|v| v.as_str());
                        // The file's period key scopes the match. A device row
                        // with a NULL key (written before keys existed) is a
                        // candidate too — adopted, never duplicated — but an
                        // exact key always outranks it. A legacy file row
                        // without a key can mean any period of the year, so
                        // the candidates are all of them and a plurality is
                        // never guessed.
                        let key_clause = if period_key.is_some() {
                            "AND (period_key IS NULL OR period_key = ?5)"
                        } else {
                            ""
                        };
                        let mut statement = db.conn.prepare(&format!(
                            "SELECT id, status, updated_at, data_json, period_key FROM form_drafts
                             WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3
                               AND COALESCE(NULLIF(quarter, 0), -1) = COALESCE(NULLIF(?4, 0), -1)
                               {key_clause}
                             ORDER BY id",
                        ))?;
                        let read_row = |row: &rusqlite::Row<'_>| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, Option<String>>(4)?,
                            ))
                        };
                        type Candidate = (i64, String, Option<String>, String, Option<String>);
                        let candidates: Vec<Candidate> = if period_key.is_some() {
                            statement
                                .query_map(
                                    rusqlite::params![
                                        tin,
                                        form_code,
                                        taxable_year,
                                        quarter,
                                        period_key
                                    ],
                                    read_row,
                                )?
                                .collect::<Result<Vec<_>, _>>()?
                        } else {
                            statement
                                .query_map(
                                    rusqlite::params![tin, form_code, taxable_year, quarter],
                                    read_row,
                                )?
                                .collect::<Result<Vec<_>, _>>()?
                        };
                        // Rows this import itself inserted are never
                        // candidates: the file's rows are independent of one
                        // another, and the second keyless open-ended entry
                        // must not "match" the row the first just wrote.
                        let candidates: Vec<Candidate> = candidates
                            .into_iter()
                            .filter(|row| !inserted_row_ids.contains(&row.0))
                            .collect();
                        // Exact keys first; a NULL-keyed row only where no
                        // exact candidate exists. Several of either can never
                        // be told apart: nothing is written, the row counts
                        // as unmatched — the first id is never fallen through
                        // to.
                        let existing = if period_key.is_some() {
                            let exact: Vec<_> = candidates
                                .iter()
                                .filter(|row| row.4.as_deref() == period_key)
                                .collect();
                            if exact.len() > 1 {
                                counts.unmatched += 1;
                                continue;
                            } else if exact.len() == 1 {
                                Some(exact[0].clone())
                            } else {
                                let nulls: Vec<_> =
                                    candidates.iter().filter(|row| row.4.is_none()).collect();
                                if nulls.len() > 1 {
                                    counts.unmatched += 1;
                                    continue;
                                }
                                nulls.first().map(|row| (*row).clone())
                            }
                        } else {
                            // A keyless open-ended row can never be told
                            // apart from ANY candidate: its periods are
                            // numbered, and adopting whichever row sits at
                            // NULL quarter is exactly the collapse that loses
                            // an entry. Any candidate at all — unmatched.
                            let open_ended = matches!(
                                crate::db::drafts::period_from_legacy_slot(form_code, None, 1),
                                crate::forms::FilingPeriod::OpenEnded(_)
                            );
                            if open_ended && !candidates.is_empty() {
                                counts.unmatched += 1;
                                continue;
                            }
                            if candidates.len() > 1 {
                                counts.unmatched += 1;
                                continue;
                            }
                            candidates.into_iter().next()
                        };
                        // The key written on insert or update: the file's own,
                        // or the one the v5 backfill would derive for a legacy
                        // row — a device row is never duplicated over a key it
                        // simply never had, and a legacy insert reads back in
                        // the same period it wrote.
                        let key_to_write = period_key.map(str::to_string).unwrap_or_else(|| {
                            // An open-ended row carries its entry number
                            // in the key: number after the largest O key
                            // already stored — two entries of one archive
                            // must never both land on O1, or the next
                            // save overwrites the first.
                            if matches!(
                                crate::db::drafts::period_from_legacy_slot(form_code, None, 1),
                                crate::forms::FilingPeriod::OpenEnded(_)
                            ) {
                                match db.next_open_ended_period_number(
                                    &tin,
                                    form_code,
                                    taxable_year as u16,
                                ) {
                                    Ok(next) => format!("O{next}"),
                                    Err(_) => derive_period_key(form_code, raw_quarter),
                                }
                            } else {
                                derive_period_key(form_code, raw_quarter)
                            }
                        });
                        match existing {
                            None => {
                                // A unique violation here is a key conflict on
                                // one period — corruption in one of the two
                                // copies, never a guess to make: the row counts
                                // as unmatched and the rest of the archive
                                // still imports.
                                let inserted = db.conn.execute(
                                    "INSERT INTO form_drafts
                                        (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, datetime('now')))",
                                    rusqlite::params![
                                        tin,
                                        form_code,
                                        taxable_year,
                                        quarter,
                                        key_to_write,
                                        status,
                                        data_json,
                                        incoming_updated_at
                                    ],
                                );
                                match inserted {
                                    Ok(_) => {
                                        inserted_row_ids.push(db.conn.last_insert_rowid());
                                        counts.restored += 1;
                                    }
                                    Err(rusqlite::Error::SqliteFailure(error, _))
                                        if error.code
                                            == rusqlite::ErrorCode::ConstraintViolation =>
                                    {
                                        counts.unmatched += 1;
                                    }
                                    Err(error) => return Err(error.into()),
                                }
                            }
                            Some((
                                id,
                                existing_status,
                                existing_updated_at,
                                existing_data,
                                existing_key,
                            )) => {
                                // A row in the middle of being sent — any of
                                // the five claim fields in its data — is never
                                // replaced, whatever the file carries.
                                let stored: serde_json::Value =
                                    serde_json::from_str(&existing_data).unwrap_or_default();
                                let mid_send = [
                                    "submission_claim_token",
                                    "submission_claimed_at",
                                    "submission_claim_lease_until",
                                    "submission_put_started_at",
                                    "relay_pending",
                                ]
                                .into_iter()
                                .any(|key| stored.get(key).is_some_and(|v| !v.is_null()));
                                // Across an amendment rank must not beat time:
                                // the file's row needs rank at least the
                                // device's AND a strictly newer stamp, so a
                                // re-stamped old original never displaces a
                                // newer in-place amendment, whatever its
                                // status. Only the same filing generation
                                // compares by rank alone.
                                let cross_amendment =
                                    json_is_amended(&existing_data) != json_is_amended(data_json);
                                let existing_rank =
                                    filing_status_rank_with_data(&existing_status, &existing_data);
                                let incoming_rank =
                                    filing_status_rank_with_data(&status, data_json);
                                let newer = import_timestamp_allows_draft(
                                    existing_updated_at.as_deref(),
                                    incoming_updated_at,
                                );
                                // A higher rank is newer by itself — except
                                // in front of a device row dated strictly
                                // newer: an older filed row never eats a
                                // newer Draft or a real Queued. BOTH rows
                                // must be dated for that: an undated filed
                                // row (every v0.2.0 archive) carries no time
                                // that could lose, so the rank still decides.
                                let device_dated_strictly_newer =
                                    match (existing_updated_at.as_deref(), incoming_updated_at) {
                                        (Some(existing_stamp), Some(incoming_stamp)) => match (
                                            crate::db::taxpayers::parse_stored_timestamp(
                                                existing_stamp,
                                            ),
                                            crate::db::taxpayers::parse_stored_timestamp(
                                                incoming_stamp,
                                            ),
                                        ) {
                                            (Some(existing_time), Some(incoming_time)) => {
                                                existing_time > incoming_time
                                            }
                                            _ => false,
                                        },
                                        _ => false,
                                    };
                                let may = !mid_send
                                    && if cross_amendment {
                                        incoming_rank >= existing_rank && newer
                                    } else if incoming_rank < existing_rank {
                                        false
                                    } else if incoming_rank > existing_rank {
                                        !device_dated_strictly_newer
                                    } else {
                                        newer
                                    };
                                if may {
                                    db.conn.execute(
                                        "UPDATE form_drafts
                                         SET status = ?1, data_json = ?2,
                                             period_key = COALESCE(?5, period_key),
                                             updated_at = COALESCE(?3, datetime('now'))
                                         WHERE id = ?4",
                                        rusqlite::params![
                                            status,
                                            data_json,
                                            incoming_updated_at,
                                            id,
                                            if existing_key.is_none() {
                                                Some(&key_to_write)
                                            } else {
                                                None
                                            },
                                        ],
                                    )?;
                                    counts.restored += 1;
                                } else {
                                    counts.kept_newer += 1;
                                }
                            }
                        }
                    } else {
                        counts.unmatched += 1;
                    }
                }
            }

            // 4. Read receipts
            if let Ok(manifest_json) = read_from_zip("receipts_manifest.json") {
                let manifest: Vec<String> = serde_json::from_str(&manifest_json)?;

                for filename in manifest {
                    let txt_name = format!("Receipts/{}.txt", filename);
                    let html_name = format!("Receipts/{}.html", filename);

                    if let Ok(raw_text) = read_from_zip(&txt_name) {
                        let raw_html = read_from_zip(&html_name).ok();

                        let mut form_type = "Unknown".to_string();
                        let mut period = "Unknown".to_string();
                        let mut received_date = "Unknown".to_string();
                        let mut received_time = "Unknown".to_string();

                        if let Ok(parsed) =
                            crate::receipt::parse_bir_receipt_email(&raw_text, raw_html.clone())
                        {
                            received_date = parsed.date_received.to_string();
                            received_time = parsed.time_received.to_string();
                        }

                        if let Some(split) = crate::receipt::split_bir_filename(&filename) {
                            form_type = split.1;
                            period = split.2;
                        }

                        let _ = db.conn.execute(
                        "INSERT INTO submission_receipts (filename, tin, form_type, period, received_date, received_time, source_from, raw_text, raw_html)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                         ON CONFLICT(filename) DO UPDATE SET raw_text = excluded.raw_text, raw_html = excluded.raw_html",
                        rusqlite::params![
                            filename,
                            tin,
                            form_type,
                            period,
                            received_date,
                            received_time,
                            "Import",
                            raw_text,
                            raw_html
                        ],
                    );
                    }
                }
            }

            // 6. The branch's tax years (added with the year facts): each
            // year's forms list, hand-picked (source Manual), and its facts
            // record. An archive without them imports as before, and the
            // forms list is never generated from the profile.
            if let Ok(yearly_json) = read_from_zip("yearly_forms.json") {
                let yearly_meta: BTreeMap<String, serde_json::Value> =
                    read_from_zip("yearly_forms_meta.json")
                        .ok()
                        .and_then(|meta| serde_json::from_str(&meta).ok())
                        .unwrap_or_default();
                let yearly: BTreeMap<String, serde_json::Value> =
                    serde_json::from_str(&yearly_json)?;
                for (year, entry) in yearly {
                    let Ok(year) = year.parse::<u16>() else {
                        continue;
                    };
                    // Both shapes: the legacy bare array of codes, and the
                    // object of `codes` and `updated_at`. The list's timestamp
                    // comes from yearly_forms_meta.json, which older readers
                    // never write.
                    // An entry whose codes do not parse is SKIPPED, never
                    // treated as an empty list: an empty list written over a
                    // year empties it.
                    let codes = match &entry {
                        serde_json::Value::Array(_) => {
                            match serde_json::from_value::<Vec<String>>(entry.clone()) {
                                Ok(codes) => codes,
                                Err(_) => continue,
                            }
                        }
                        serde_json::Value::Object(_) => match entry.get("codes").and_then(|codes| {
                            serde_json::from_value::<Vec<String>>(codes.clone()).ok()
                        }) {
                            Some(codes) => codes,
                            None => continue,
                        },
                        _ => continue,
                    };
                    let updated_at = yearly_meta
                        .get(&year.to_string())
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    crate::db::taxpayers::restore_yearly_forms(
                        &db.conn,
                        &tin,
                        year,
                        &codes,
                        updated_at.as_deref(),
                    )
                    .map_err(|error| DbError::Other(error.to_string()))?;
                }
            }
            if let Ok(facts_json) = read_from_zip("year_facts.json") {
                let records: BTreeMap<String, serde_json::Value> =
                    serde_json::from_str(&facts_json)?;
                for (year, entry) in records {
                    let Ok(year) = year.parse::<u16>() else {
                        continue;
                    };
                    // The record's `updated_at` travels as a key beside the
                    // facts; it is not part of YearFacts.
                    let updated_at = entry
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    let mut facts_value = entry.clone();
                    if let Some(object) = facts_value.as_object_mut() {
                        object.remove("updated_at");
                    }
                    let Ok(facts) =
                        serde_json::from_value::<crate::tax_profile::YearFacts>(facts_value)
                    else {
                        continue;
                    };
                    crate::db::taxpayers::restore_year_facts(
                        &db.conn,
                        &tin,
                        year,
                        &facts,
                        updated_at.as_deref(),
                    )?;
                }
            }

            // 5. Read data_providers.json (added in export v1): a provider's
            // provider_id and name only, never its credentials or row id. One is
            // created, with no credentials, only when this TIN has no provider
            // with that provider_id yet, so an existing provider is never
            // touched and a re-import adds no duplicate.
            if let Ok(providers_json) = read_from_zip("data_providers.json") {
                let providers: Vec<serde_json::Value> = serde_json::from_str(&providers_json)?;
                let mut known = provider_ids(db, &tin)?;
                for provider in providers {
                    let (Some(provider_id), Some(name)) = (
                        provider.get("provider_id").and_then(|v| v.as_str()),
                        provider.get("name").and_then(|v| v.as_str()),
                    ) else {
                        continue;
                    };
                    if known.insert(provider_id.to_string()) {
                        db.save_data_provider(ProviderConfig {
                            id: None,
                            profile_tin: tin.clone(),
                            provider_id: provider_id.to_string(),
                            name: name.to_string(),
                            credentials: BTreeMap::new(),
                        })?;
                    }
                }
            }
        }

        Ok(())
    })();
    match result {
        Ok(()) => {
            db.conn.execute_batch("COMMIT")?;
            Ok(counts)
        }
        Err(error) => {
            let _ = db.conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// The provider ids `tin` already has, read without their credentials.
fn provider_ids(db: &Database, tin: &str) -> Result<BTreeSet<String>, DbError> {
    let mut statement = db
        .conn
        .prepare("SELECT provider_id FROM data_providers WHERE profile_tin = ?1")?;
    let ids = statement
        .query_map([tin], |row| row.get::<_, String>(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// A database taken out of a backup archive, checked, and not yet installed.
///
/// Staging touches nothing live: the archive's `bir_data.db` is written to a
/// temporary file and must pass [`check_restorable_database`]. Only
/// [`StagedDatabase::install`] replaces the live file, so a caller can do its
/// own work between the two (a safety copy, closing its connection) knowing
/// the archive is good. Dropping it, installed or not, removes the temporary
/// file.
#[derive(Debug)]
pub struct StagedDatabase {
    staged: PathBuf,
    target: PathBuf,
    /// The private folder the SQLCipher build stages in (the plain build
    /// stages beside the target and has none). Dropped after the file is
    /// removed, and removed with whatever SQLite made beside it.
    folder: Option<PrivateTempDir>,
}

impl StagedDatabase {
    /// The temporary file the archive's database was extracted to.
    pub fn path(&self) -> &Path {
        &self.staged
    }

    /// Replace the database at the target path with the staged one.
    ///
    /// The stale `-wal`, `-shm` and `-journal` files of the replaced database
    /// go first (a log left behind would be replayed into the new file), then
    /// one rename puts the new file over the live one, so the live path holds
    /// the old database or the new one and never a half-written file. The
    /// SQLCipher build first encrypts the staged file into a temporary file
    /// beside the live one and checks that it opens with the current key. A
    /// failure at any step leaves the old database where it was.
    pub fn install(self) -> Result<(), DbError> {
        install_staged_database(&self.staged, &self.target)
    }
}

impl Drop for StagedDatabase {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = fs::remove_file(with_suffix(&self.staged, suffix));
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut raw = path.as_os_str().to_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

/// A temporary file named for `out_db_path` and placed beside it, unique per
/// call, so the rename that puts it over `out_db_path` never crosses a file
/// system.
fn sibling_temporary_path(out_db_path: &Path, what: &str) -> PathBuf {
    let name = out_db_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bir_data.db".to_string());
    out_db_path.with_file_name(format!("{name}.{what}-{}.tmp", unique_name()))
}

/// Where an archive's database is extracted, unique per call. The SQLCipher
/// build stages the plain file in a private folder of its own in the temp
/// folder (see [`PrivateTempDir`]): never with the temp folder's default
/// permissions. The plain build stages next to the file it will replace (the
/// live database is plain there too), so the final rename never crosses a
/// file system.
fn staging_place(out_db_path: &Path) -> Result<(PathBuf, Option<PrivateTempDir>), DbError> {
    #[cfg(feature = "sqlcipher")]
    {
        let _ = out_db_path;
        let folder = PrivateTempDir::new("bir_unencrypted_import")?;
        Ok((folder.path().join("bir_data.db"), Some(folder)))
    }
    #[cfg(not(feature = "sqlcipher"))]
    {
        Ok((sibling_temporary_path(out_db_path, "import"), None))
    }
}

/// Extract the database of the backup archive at `zip_path` and check it,
/// without touching `out_db_path`. See [`StagedDatabase`].
pub fn stage_database_zip(zip_path: &Path, out_db_path: &Path) -> Result<StagedDatabase, DbError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| DbError::Other(e.to_string()))?;

    let mut db_file_in_zip = archive
        .by_name("bir_data.db")
        .map_err(|e| DbError::Other(e.to_string()))?;

    // From here on every early return drops the guard, which removes the file
    // (and the private folder it is in).
    let (path, folder) = staging_place(out_db_path)?;
    let staged = StagedDatabase {
        staged: path,
        target: out_db_path.to_path_buf(),
        folder,
    };
    let mut temp_file = match &staged.folder {
        Some(folder) => folder.create_file("bir_data.db")?.1,
        None => {
            if let Some(parent) = staged.staged.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            fs::File::create(&staged.staged)?
        }
    };
    std::io::copy(&mut db_file_in_zip, &mut temp_file)?;
    // The bytes are on disk before anything depends on them.
    temp_file.sync_all()?;
    drop(temp_file);

    check_restorable_database(&staged.staged)?;
    Ok(staged)
}

/// Restore the database of the backup archive at `zip_path` over
/// `out_db_path`: stage and check it first, replace only then.
pub fn extract_database_zip(zip_path: &Path, out_db_path: &Path) -> Result<(), DbError> {
    stage_database_zip(zip_path, out_db_path)?.install()
}

/// The tables every database this app has ever written has (its first schema
/// creates them all). An archive without them is some other SQLite file.
const CORE_TABLES: [&str; 4] = ["profiles", "submissions", "form_drafts", "settings"];

/// What a plain SQLite file must be before it may replace the live database:
/// the plain header, a clean `PRAGMA integrity_check`, the app's core tables,
/// and a schema version this build can open (not newer than the one it
/// migrates to). The file is only read (no create, no write).
fn check_restorable_database(path: &Path) -> Result<(), DbError> {
    fn refuse(why: impl std::fmt::Display) -> DbError {
        DbError::Other(format!(
            "The backup's database {why}, so it was not restored. The current database was left as it is."
        ))
    }
    if crate::db::database_file_kind(path)? != crate::db::DatabaseFileKind::PlainSqlite {
        return Err(refuse("is not a plain SQLite file"));
    }
    let verdict = (|| -> Result<(), DbError> {
        let conn =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|error| refuse(format!("cannot be opened ({error})")))?;
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|error| refuse(format!("did not pass the integrity check ({error})")))?;
        if integrity != "ok" {
            return Err(refuse(format!(
                "did not pass the integrity check ({integrity})"
            )));
        }
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| refuse(format!("has no readable schema version ({error})")))?;
        let supported = crate::db::current_schema_version()?;
        if version > supported {
            return Err(refuse(format!(
                "was made by a newer version of the app (schema version {version}; this one opens up to {supported})"
            )));
        }
        let mut statement = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .map_err(|error| refuse(format!("has no readable tables ({error})")))?;
        let tables: BTreeSet<String> = statement
            .query_map([], |row| row.get::<_, String>(0))
            .and_then(|rows| rows.collect())
            .map_err(|error| refuse(format!("has no readable tables ({error})")))?;
        let missing: Vec<&str> = CORE_TABLES
            .into_iter()
            .filter(|table| !tables.contains(*table))
            .collect();
        if !missing.is_empty() {
            return Err(refuse(format!(
                "is not an eBIRForms database (no {} table)",
                missing.join(", ")
            )));
        }
        Ok(())
    })();
    // A read-only open of a database in WAL mode may have made these.
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(with_suffix(path, suffix));
    }
    verdict
}

/// A temporary file that is removed, with the sidecar files SQLite may have
/// made beside it, when this goes out of scope: after a rename it is already
/// gone, and on every other way out it must not be left behind.
#[cfg(feature = "sqlcipher")]
struct TemporaryDatabase(PathBuf);

#[cfg(feature = "sqlcipher")]
impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = fs::remove_file(with_suffix(&self.0, suffix));
        }
    }
}

/// SQLCipher build: encrypt the staged plain database and put it in place of
/// `out_db_path`, staged like the plain build does. The archive was checked
/// when it was staged. The export goes into a temporary file beside the live
/// one, keyed with the current key, and is checked to open with that key; only
/// then do the replaced database's sidecar files go and one rename put it
/// over the live file. A failure at any step leaves the current database as
/// it was, and no temporary file behind.
#[cfg(feature = "sqlcipher")]
fn install_staged_database(temp_db_path: &Path, out_db_path: &Path) -> Result<(), DbError> {
    let key_hex = zeroize::Zeroizing::new(Database::get_or_create_master_key()?);
    let encrypted = TemporaryDatabase(sibling_temporary_path(out_db_path, "encrypted"));

    // Open the unencrypted db, attach the new encrypted file and export to it.
    let conn = rusqlite::Connection::open(temp_db_path)?;
    let target = encrypted
        .0
        .to_str()
        .ok_or_else(|| DbError::Other("The database path is not valid text.".into()))?;
    conn.execute(
        &zeroize::Zeroizing::new(format!(
            "ATTACH DATABASE ?1 AS encrypted KEY \"x'{}'\";",
            key_hex.as_str()
        )),
        rusqlite::params![target],
    )?;
    let mut stmt = conn.prepare("SELECT sqlcipher_export('encrypted');")?;
    let _ = stmt.query([])?.next()?;
    drop(stmt);
    crate::export::copy_schema_version(&conn, "main", "encrypted")?;
    conn.execute("DETACH DATABASE encrypted;", [])?;

    // Explicitly close the connection to ensure all data is flushed
    drop(conn);

    // Small delay to ensure file system operations complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    check_exported_database(&encrypted.0, &key_hex)?;
    rename_over_live(&encrypted.0, out_db_path)
}

/// The file [`install_staged_database`] exported opens with the key it was
/// exported with and holds the app's core tables. The file is only read.
#[cfg(feature = "sqlcipher")]
fn check_exported_database(path: &Path, key_hex: &str) -> Result<(), DbError> {
    fn refuse(why: impl std::fmt::Display) -> DbError {
        DbError::Other(format!(
            "The backup's database could not be encrypted again ({why}), so it was not restored. The current database was left as it is."
        ))
    }
    let verdict = (|| -> Result<(), DbError> {
        let conn =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|error| refuse(format!("it cannot be opened ({error})")))?;
        conn.execute_batch(&zeroize::Zeroizing::new(format!(
            "PRAGMA key = \"x'{key_hex}'\";"
        )))
        .map_err(|error| refuse(format!("the key was refused ({error})")))?;
        let mut statement = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .map_err(|error| refuse(format!("it does not open with the key ({error})")))?;
        let tables: BTreeSet<String> = statement
            .query_map([], |row| row.get::<_, String>(0))
            .and_then(|rows| rows.collect())
            .map_err(|error| refuse(format!("it does not open with the key ({error})")))?;
        let missing: Vec<&str> = CORE_TABLES
            .into_iter()
            .filter(|table| !tables.contains(*table))
            .collect();
        if !missing.is_empty() {
            return Err(refuse(format!(
                "no {} table came across",
                missing.join(", ")
            )));
        }
        Ok(())
    })();
    // A read-only open of a database in WAL mode may have made these.
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(with_suffix(path, suffix));
    }
    verdict
}

/// Plain build: there is no key and no `sqlcipher_export`; the staged file
/// already is the database. It is renamed over the live one (atomic on the
/// same folder), after the replaced database's sidecar files are gone, so a
/// failure at any step leaves the current database as it was.
#[cfg(not(feature = "sqlcipher"))]
fn install_staged_database(staged: &Path, out_db_path: &Path) -> Result<(), DbError> {
    rename_over_live(staged, out_db_path)
}

/// Put `staged`, a finished database beside `out_db_path`, in place of the
/// live file: the replaced database's sidecar files go first (a log left
/// behind would be replayed into the new file), then one rename, so the live
/// path holds the old database or the new one and never a half-written file.
fn rename_over_live(staged: &Path, out_db_path: &Path) -> Result<(), DbError> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = with_suffix(out_db_path, suffix);
        match fs::remove_file(&sidecar) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(DbError::Io(error)),
        }
    }
    fs::rename(staged, out_db_path)?;
    // Make the rename itself durable (best effort; not every platform lets a folder be synced).
    #[cfg(unix)]
    {
        if let Some(folder) = out_db_path.parent().filter(|p| !p.as_os_str().is_empty())
            && let Ok(folder) = fs::File::open(folder)
        {
            let _ = folder.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, Submission};
    use crate::profile::{TaxpayerProfile, TaxpayerType};
    use tempfile::tempdir;

    #[test]
    fn test_profile_export_import_zip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        // Create mock profile
        let profile = TaxpayerProfile {
            id: None,
            full_name: "John Doe".to_string(),
            tin: crate::naming::Tin {
                segment1: "123".into(),
                segment2: "456".into(),
                segment3: "789".into(),
                branch: "000".into(),
            },
            rdo_code: "123".to_string(),
            line_of_business: "Tech".to_string(),
            registered_address: "123 Main St".to_string(),
            zip_code: "1000".to_string(),
            phone: "09123456789".to_string(),
            email: "test@example.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            per_year_forms: Default::default(),
            profile_pin_hash: None,
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        };
        db.save_profile(profile.clone()).unwrap();

        // Create mock submission
        let sub = Submission {
            id: None,
            tin: "123456789000".to_string(),
            form_type: "2551Q".to_string(),
            period: "2024Q1".to_string(),
            status: "Draft".to_string(),
            form_data: std::collections::BTreeMap::new(),
            submitted_at: None,
            filename: None,
            created_at: None,
            updated_at: None,
        };
        db.save_submission(sub.clone()).unwrap();

        // A return the worker sent in a dry run keeps saying so.
        let mut dry_run = crate::forms::Form1601CDraft::new_from_profile(&profile, 2024, 5);
        dry_run.status = crate::forms::FilingStatus::Queued;
        dry_run.transition_to_submitted(dry_run.default_submission_filename(), true);
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
                 VALUES ('123456789000', '1601C', 2024, 5, 'Submitted', ?1)",
                rusqlite::params![serde_json::to_string(&dry_run).unwrap()],
            )
            .unwrap();

        // Export to zip
        let export_zip = dir.path().join("export.zip");
        crate::export::export_profile_data(&db, "123456789000", &export_zip, None).unwrap();
        assert!(export_zip.exists());

        // Import into new DB
        let db2_path = dir.path().join("test2.db");
        let (db2, _) = Database::open_or_recreate(&db2_path).unwrap();
        import_profile_data(&db2, &export_zip).unwrap();

        // Verify
        let imported_profile = db2.get_profile_by_tin("123456789000").unwrap().unwrap();
        assert_eq!(imported_profile.full_name, "John Doe".to_string());

        let submissions = db2.list_submissions_for_tin("123456789000").unwrap();
        assert_eq!(submissions.len(), 1);
        assert_eq!(submissions[0].form_type, "2551Q");

        let imported: String = db2
            .conn
            .query_row(
                "SELECT data_json FROM form_drafts WHERE form_code = '1601C' AND tin = '123456789000'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let imported: crate::forms::Form1601CDraft = serde_json::from_str(&imported).unwrap();
        assert_eq!(imported.status, crate::forms::FilingStatus::Submitted);
        assert!(imported.submitted_dry_run);
    }

    #[test]
    fn test_database_export_import_zip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("original.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Jane Doe".to_string(),
            tin: crate::naming::Tin {
                segment1: "999".into(),
                segment2: "999".into(),
                segment3: "999".into(),
                branch: "000".into(),
            },
            rdo_code: "123".to_string(),
            line_of_business: "Tech".to_string(),
            registered_address: "123 Main St".to_string(),
            zip_code: "1000".to_string(),
            phone: "09123456789".to_string(),
            email: "test2@example.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            per_year_forms: Default::default(),
            profile_pin_hash: None,
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        };
        db.save_profile(profile).unwrap();
        db.checkpoint().unwrap();

        let backup_zip = dir.path().join("backup.db.zip");
        crate::export::export_database_zip(&db, &backup_zip).unwrap();
        assert!(backup_zip.exists());

        let restored_db_path = dir.path().join("restored.db");
        extract_database_zip(&backup_zip, &restored_db_path).unwrap();

        let (db2, _) = Database::open_or_recreate(&restored_db_path).unwrap();
        let profiles = db2.list_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].tin.full(), "999999999000");
    }

    #[test]
    fn test_legacy_profile_json_deserializes_with_defaults() {
        // A minimal JSON without any of the newer #[serde(default)] fields
        let legacy_json = r#"{
            "full_name": "Legacy User",
            "tin": {"segment1":"111","segment2":"222","segment3":"333","branch":"000"},
            "rdo_code": "039",
            "line_of_business": "IT",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "0912",
            "email": "legacy@test.com",
            "default_form_type": "2551Q"
        }"#;

        // Parse as Value, run migration, then deserialize
        let mut value: serde_json::Value = serde_json::from_str(legacy_json).unwrap();
        migrate_profile_json(&mut value, (0, 0, 0)).unwrap();
        let profile: TaxpayerProfile = serde_json::from_value(value).unwrap();

        assert_eq!(profile.full_name, "Legacy User");
        assert_eq!(profile.tax_classification, None);
        assert!(!profile.has_8_percent_election(2026));
        assert!(!profile.is_archived);
        assert!(!profile.email_tracking_enabled);
    }

    #[test]
    fn test_import_idempotent_no_duplicate_submissions() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_idem.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Idem User".to_string(),
            tin: crate::naming::Tin {
                segment1: "555".into(),
                segment2: "555".into(),
                segment3: "555".into(),
                branch: "000".into(),
            },
            rdo_code: "039".to_string(),
            line_of_business: "IT".to_string(),
            registered_address: "QC".to_string(),
            zip_code: "1103".to_string(),
            phone: "0999".to_string(),
            email: "idem@test.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            per_year_forms: Default::default(),
            profile_pin_hash: None,
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        };
        db.save_profile(profile).unwrap();

        let sub = Submission {
            id: None,
            tin: "555555555000".to_string(),
            form_type: "2551Q".to_string(),
            period: "2024Q1".to_string(),
            status: "Draft".to_string(),
            form_data: std::collections::BTreeMap::new(),
            submitted_at: None,
            filename: None,
            created_at: None,
            updated_at: None,
        };
        db.save_submission(sub).unwrap();

        // Export
        let export_zip = dir.path().join("idem_export.zip");
        crate::export::export_profile_data(&db, "555555555000", &export_zip, None).unwrap();

        // Import into a new DB twice
        let db2_path = dir.path().join("test_idem2.db");
        let (db2, _) = Database::open_or_recreate(&db2_path).unwrap();
        import_profile_data(&db2, &export_zip).unwrap();
        // Second import — should NOT create duplicate submissions
        import_profile_data(&db2, &export_zip).unwrap();

        let submissions = db2.list_submissions_for_tin("555555555000").unwrap();
        assert_eq!(
            submissions.len(),
            1,
            "Re-importing should not create duplicate submissions"
        );
    }

    #[test]
    fn test_parse_semver_valid() {
        assert_eq!(parse_semver("0.0.1"), (0, 0, 1));
        assert_eq!(parse_semver("1.2.3"), (1, 2, 3));
        assert_eq!(parse_semver("10.20.30"), (10, 20, 30));
    }

    #[test]
    fn test_parse_semver_edge_cases() {
        // Missing parts default to 0
        assert_eq!(parse_semver("1"), (1, 0, 0));
        assert_eq!(parse_semver("1.2"), (1, 2, 0));
        // Empty / garbage defaults to (0,0,0)
        assert_eq!(parse_semver(""), (0, 0, 0));
        assert_eq!(parse_semver("not-a-version"), (0, 0, 0));
        // Whitespace is trimmed
        assert_eq!(parse_semver("  1.2.3  "), (1, 2, 3));
    }

    #[test]
    fn test_manifest_contains_cargo_version() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_manifest.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Manifest Test".to_string(),
            tin: crate::naming::Tin {
                segment1: "777".into(),
                segment2: "777".into(),
                segment3: "777".into(),
                branch: "000".into(),
            },
            rdo_code: "039".to_string(),
            line_of_business: "IT".to_string(),
            registered_address: "BGC".to_string(),
            zip_code: "1634".to_string(),
            phone: "0917".to_string(),
            email: "manifest@test.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_versions: vec![],
            compliance_source_mode: Default::default(),
            per_year_forms: Default::default(),
            profile_pin_hash: None,
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        };
        db.save_profile(profile).unwrap();

        // Export
        let export_zip = dir.path().join("manifest_test.zip");
        crate::export::export_profile_data(&db, "777777777000", &export_zip, None).unwrap();

        // Read the ZIP and extract manifest.json directly
        let file = std::fs::File::open(&export_zip).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut manifest_file = archive.by_name("manifest.json").unwrap();
        let mut manifest_str = String::new();
        manifest_file.read_to_string(&mut manifest_str).unwrap();

        let manifest: serde_json::Value = serde_json::from_str(&manifest_str).unwrap();

        // export_version should be the Cargo package version string
        let version = manifest.get("export_version").unwrap();
        assert!(version.is_string(), "export_version should be a string");
        assert_eq!(
            version.as_str().unwrap(),
            env!("CARGO_PKG_VERSION"),
            "export_version should match CARGO_PKG_VERSION"
        );

        // exported_at should be present
        assert!(manifest.get("exported_at").is_some());

        // Verify the version parses correctly
        let ver = parse_semver(version.as_str().unwrap());
        assert_eq!(ver, parse_semver(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_semver_comparison_ordering() {
        // Verify that tuple comparison works correctly for migration gates
        assert!((0, 0, 0) < (0, 0, 1));
        assert!((0, 0, 1) < (0, 1, 0));
        assert!((0, 1, 0) < (1, 0, 0));
        assert!((0, 0, 1) < (0, 2, 0));
        assert!((0, 0, 1) >= (0, 0, 1));
        assert!((1, 0, 0) < (1, 0, 1));
    }

    // ---------- an archive's row id and credentials are never used ----------

    use crate::profile::PROFILE_CREDENTIAL_KEYS;

    const TIN_Y: &str = "11122233300000";
    const TIN_X: &str = "44455566600000";

    /// Distinctive values this database stores, in `PROFILE_CREDENTIAL_KEYS`
    /// order, so a leak or an overwrite cannot pass unnoticed.
    const STORED_CREDENTIALS: [&str; 5] = [
        "stored-totp-K7Q2",
        "stored-pass-W9X4",
        "stored-access-M3R8",
        "stored-refresh-T5V1",
        "stored-pin-hash-H6N3",
    ];

    /// What a crafted archive carries, in the same order.
    const FORGED_CREDENTIALS: [&str; 5] = [
        "forged-totp-J4P9",
        "forged-pass-C2L6",
        "forged-access-Z8D1",
        "forged-refresh-Y3F7",
        "forged-pin-hash-B5G2",
    ];

    const STORED_PROVIDER_KEY: &str = "stored-provider-key-R2S6";
    const FORGED_PROVIDER_KEY: &str = "forged-provider-key-U4K8";

    /// A profile for the 14-digit `tin`.
    fn branch_profile(full_name: &str, tin: &str) -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": full_name,
            "tin": {
                "segment1": &tin[0..3],
                "segment2": &tin[3..6],
                "segment3": &tin[6..9],
                "branch": &tin[9..],
            },
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Sample Street, Pasig",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "books@example.com",
            "default_form_type": "2551Q",
        }))
        .expect("profile fixture")
    }

    fn set_credentials(profile: &mut TaxpayerProfile, values: [&str; 5]) {
        let [totp, password, access, refresh, pin_hash] =
            values.map(|value| Some(value.to_string()));
        profile.totp_secret = totp;
        profile.imap_app_password = password;
        profile.oauth_access_token = access;
        profile.oauth_refresh_token = refresh;
        profile.profile_pin_hash = pin_hash;
    }

    /// `tin`'s five stored credentials, in `PROFILE_CREDENTIAL_KEYS` order.
    fn stored_credentials(db: &Database, tin: &str) -> [Option<String>; 5] {
        let mut profile = db
            .get_profile_by_tin(tin)
            .unwrap()
            .expect("a stored profile");
        [
            profile.totp_secret.take(),
            profile.imap_app_password.take(),
            profile.oauth_access_token.take(),
            profile.oauth_refresh_token.take(),
            profile.profile_pin_hash.take(),
        ]
    }

    /// Write a zip archive holding `entries` as (name, contents).
    fn write_archive(path: &Path, entries: &[(&str, String)]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, contents) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zip, contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    /// A crafted profile archive: `profile` with the five forged credentials
    /// and the row id `id`, and `providers` as its data_providers.json.
    fn crafted_archive(
        path: &Path,
        profile: &TaxpayerProfile,
        id: i64,
        providers: serde_json::Value,
    ) {
        let mut record = serde_json::to_value(profile).unwrap();
        record["id"] = serde_json::json!(id);
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(FORGED_CREDENTIALS) {
            record[key] = serde_json::json!(value);
        }
        let manifest = serde_json::json!({ "export_version": env!("CARGO_PKG_VERSION") });
        write_archive(
            path,
            &[
                ("manifest.json", manifest.to_string()),
                ("profile.json", record.to_string()),
                ("data_providers.json", providers.to_string()),
            ],
        );
    }

    /// Providers as a crafted data_providers.json carries them: each with a
    /// row id, another TIN, and credentials.
    fn forged_providers(provider_ids: &[&str]) -> serde_json::Value {
        provider_ids
            .iter()
            .enumerate()
            .map(|(index, provider_id)| {
                serde_json::json!({
                    "id": 900 + index,
                    "profile_tin": "99988877700000",
                    "provider_id": provider_id,
                    "name": format!("Forged {provider_id}"),
                    "credentials": { "api_key": FORGED_PROVIDER_KEY },
                })
            })
            .collect()
    }

    /// No forged value is stored where a credential can live: a profile row,
    /// a provider's credentials, or a taxpayer's PIN and authenticator. A
    /// failure names the key, never the value.
    fn assert_no_forged_value(db: &Database) {
        let column = |sql: &str| -> Vec<String> {
            let mut statement = db.conn.prepare(sql).unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        let stored = [
            column("SELECT data_json FROM profiles"),
            column("SELECT credentials_json FROM data_providers"),
            column("SELECT COALESCE(pin_hash, '') || COALESCE(totp_secret, '') FROM taxpayers"),
        ]
        .concat();
        for row in &stored {
            for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(FORGED_CREDENTIALS) {
                assert!(!row.contains(value), "the database stored the forged {key}");
            }
            assert!(
                !row.contains(FORGED_PROVIDER_KEY),
                "the database stored the forged provider credential"
            );
        }
    }

    /// An archive for a TIN this database has: the row keeps its own five
    /// credentials and its own row id, whether the file names that row's id
    /// or one that names no row here, and takes the file's other fields. The
    /// stored provider keeps its name and credentials, a provider id the TIN
    /// lacks is added without credentials, and nothing is duplicated.
    #[test]
    fn an_import_keeps_the_stored_rows_credentials_and_id() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let mut stored = branch_profile("Stored Name", TIN_Y);
        set_credentials(&mut stored, STORED_CREDENTIALS);
        let row_id = db.save_profile(stored).unwrap().id.expect("a row id");
        let provider_credentials =
            BTreeMap::from([("api_key".to_string(), STORED_PROVIDER_KEY.to_string())]);
        db.save_data_provider(ProviderConfig {
            id: None,
            profile_tin: TIN_Y.into(),
            provider_id: "generic".into(),
            name: "Books".into(),
            credentials: provider_credentials.clone(),
        })
        .unwrap();

        for (round, file_id) in [row_id, 4242].into_iter().enumerate() {
            let archive = dir.path().join(format!("crafted-{round}.zip"));
            let name = format!("Imported Name {round}");
            crafted_archive(
                &archive,
                &branch_profile(&name, TIN_Y),
                file_id,
                forged_providers(&["generic", "taxman"]),
            );
            import_profile_data(&db, &archive).unwrap();

            let imported = db.get_profile_by_tin(TIN_Y).unwrap().expect("the row");
            assert_eq!(imported.id, Some(row_id), "the row keeps its id");
            assert_eq!(
                imported.full_name, name,
                "the file's other fields are stored"
            );
            drop(imported);
            for ((key, value), expected) in PROFILE_CREDENTIAL_KEYS
                .into_iter()
                .zip(stored_credentials(&db, TIN_Y))
                .zip(STORED_CREDENTIALS)
            {
                assert!(
                    value.as_deref() == Some(expected),
                    "the import changed the stored {key}"
                );
            }
            assert_no_forged_value(&db);

            let providers = db.get_data_providers(TIN_Y).unwrap();
            assert_eq!(providers.len(), 2, "one provider per provider_id");
            let generic = providers
                .iter()
                .find(|provider| provider.provider_id == "generic")
                .expect("the stored provider");
            assert_eq!(generic.name, "Books", "the stored provider keeps its name");
            assert!(
                generic.credentials == provider_credentials,
                "the import changed the stored provider's credentials"
            );
            let taxman = providers
                .iter()
                .find(|provider| provider.provider_id == "taxman")
                .expect("the added provider");
            assert_eq!(taxman.name, "Forged taxman");
            assert!(
                taxman.credentials.is_empty(),
                "the added provider has credentials"
            );
        }
        assert_eq!(db.list_profiles().unwrap().len(), 1, "no second row");
    }

    /// An archive for a TIN this database lacks: the new row stores none of
    /// the file's five credentials, and its provider is added without
    /// credentials. Importing it again adds no second provider.
    #[test]
    fn an_import_for_a_new_tin_stores_no_credential() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let archive = dir.path().join("crafted.zip");
        crafted_archive(
            &archive,
            &branch_profile("New Taxpayer", TIN_X),
            4242,
            forged_providers(&["generic"]),
        );
        for _ in 0..2 {
            import_profile_data(&db, &archive).unwrap();
            for (key, value) in PROFILE_CREDENTIAL_KEYS
                .into_iter()
                .zip(stored_credentials(&db, TIN_X))
            {
                assert!(value.is_none(), "a new profile stored a {key}");
            }
            assert_no_forged_value(&db);
            let providers = db.get_data_providers(TIN_X).unwrap();
            assert_eq!(providers.len(), 1, "one provider per provider_id");
            assert_eq!(providers[0].name, "Forged generic");
            assert!(
                providers[0].credentials.is_empty(),
                "the added provider has credentials"
            );
        }
    }

    /// The regression test for a data-corruption bug. The file's `id`
    /// reached `save_profile`, which reads it as this database's row: here
    /// row 1 is taxpayer Y's, so importing taxpayer X's archive that says
    /// `"id": 1` overwrote Y's profile with X's and re-keyed Y's draft and
    /// submission onto X's TIN. Y must keep its row, draft and submission,
    /// and X must get a row of its own.
    #[test]
    fn an_archive_row_id_never_names_another_taxpayers_row() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let y = db
            .save_profile(branch_profile("Taxpayer Y", TIN_Y))
            .unwrap();
        assert_eq!(y.id, Some(1), "Y holds row 1");
        drop(y);
        let submission = |tin: &str, form_type: &str, period: &str, owner: &str| Submission {
            id: None,
            tin: tin.into(),
            form_type: form_type.into(),
            period: period.into(),
            status: "Draft".into(),
            form_data: BTreeMap::from([("owner".to_string(), owner.to_string())]),
            submitted_at: None,
            filename: None,
            created_at: None,
            updated_at: None,
        };
        db.save_submission(submission(TIN_Y, "2551Q", "2026Q1", "Y"))
            .unwrap();
        db.save_form_draft(
            TIN_Y,
            "1701Q",
            2026,
            Some(1),
            &crate::forms::FilingStatus::Draft,
            &serde_json::json!({ "owner": "Y" }),
        )
        .unwrap();

        let archive = dir.path().join("taxpayer-x.zip");
        let mut x = serde_json::to_value(branch_profile("Taxpayer X", TIN_X)).unwrap();
        x["id"] = serde_json::json!(1);
        let x_submissions = [submission(TIN_X, "2550Q", "2026Q2", "X")];
        write_archive(
            &archive,
            &[
                ("profile.json", x.to_string()),
                (
                    "submissions.json",
                    serde_json::to_string(&x_submissions).unwrap(),
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();

        let y = db
            .get_profile_by_tin(TIN_Y)
            .unwrap()
            .expect("Y's profile is still there");
        assert_eq!(y.id, Some(1), "Y keeps row 1");
        assert_eq!(y.full_name, "Taxpayer Y");
        let x = db
            .get_profile_by_tin(TIN_X)
            .unwrap()
            .expect("X is imported");
        assert!(x.id.is_some_and(|id| id != 1), "X gets a row of its own");
        assert_eq!(x.full_name, "Taxpayer X");
        assert_eq!(db.list_profiles().unwrap().len(), 2);

        let owners = |tin: &str| -> Vec<String> {
            db.list_submissions_for_tin(tin)
                .unwrap()
                .into_iter()
                .map(|submission| submission.form_data["owner"].clone())
                .collect()
        };
        assert_eq!(owners(TIN_Y), ["Y"], "Y's submission stays Y's");
        assert_eq!(owners(TIN_X), ["X"], "X has only its own submission");
        let draft = |tin: &str| -> Option<serde_json::Value> {
            db.get_form_draft(tin, "1701Q", 2026, Some(1)).unwrap()
        };
        assert_eq!(draft(TIN_Y).expect("Y's draft stays Y's")["owner"], "Y");
        assert!(draft(TIN_X).is_none(), "X has none of Y's drafts");
    }

    /// Export, then import into a fresh database: every profile field
    /// matches except the row id, which is the new database's own, and the
    /// five credentials, which are none. The provider comes back with its
    /// provider_id and name and no credentials.
    #[test]
    fn a_round_trip_keeps_every_profile_field_but_the_id_and_credentials() {
        use crate::profile::{
            ComplianceSourceMode, EmailAuthMethod, EoptTier, ExciseTaxCategory, IncomeTaxElection,
            TaxClassification, TaxElectionHistory,
        };
        let date = |year, month, day| chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap();
        let dir = tempdir().unwrap();
        let source = Database::open_ephemeral().unwrap();
        // Row 1 is another taxpayer's, so the exported profile is row 2: not
        // the id a fresh database gives the import.
        source
            .save_profile(branch_profile("Earlier Taxpayer", TIN_Y))
            .unwrap();
        let mut profile = branch_profile("Round Trip Corp", TIN_X);
        profile.is_vat_registered = true;
        profile.business_start_date = Some(date(2020, 1, 15));
        profile.birth_date = Some(date(1985, 6, 30));
        profile.tax_classification = Some(TaxClassification::SelfEmployed);
        profile.eopt_tier = Some(EoptTier::Small);
        profile.is_bmbe = true;
        profile.is_create_msme = true;
        profile.is_expanded_withholding_agent = true;
        profile.atc_codes = vec!["WI010".into(), "WC010".into()];
        profile.excise_tax_categories = vec![ExciseTaxCategory::Alcohol];
        profile.tax_elections = vec![TaxElectionHistory {
            taxable_year: 2026,
            election: IncomeTaxElection::EightPercent,
            elected_at: date(2026, 2, 1).and_hms_opt(9, 30, 0).unwrap(),
            source_form: "2551Q".into(),
        }];
        profile.email_tracking_enabled = true;
        profile.email_auth_method = EmailAuthMethod::GoogleOAuth;
        profile.imap_email = Some("receipts@example.com".into());
        profile.imap_host = Some("imap.example.com".into());
        profile.test_notification_enabled = true;
        profile.has_employees = true;
        profile.withholds_compensation = true;
        profile.withholds_expanded = true;
        profile.withholds_final = true;
        profile.is_top_withholding_agent = true;
        profile.compliance_source_mode = ComplianceSourceMode::TemporalSuggestion;
        set_credentials(&mut profile, STORED_CREDENTIALS);
        // A stored PIN hash gates the export, so the row's is the hash of
        // the PIN the call below passes. The file's still never imports.
        profile.profile_pin_hash = Some(crate::crypto::hash_pin("0817"));
        assert_eq!(source.save_profile(profile).unwrap().id, Some(2));
        source
            .save_data_provider(ProviderConfig {
                id: None,
                profile_tin: TIN_X.into(),
                provider_id: "generic".into(),
                name: "Books".into(),
                credentials: BTreeMap::from([(
                    "api_key".to_string(),
                    STORED_PROVIDER_KEY.to_string(),
                )]),
            })
            .unwrap();

        let archive = dir.path().join("round-trip.zip");
        crate::export::export_profile_data(&source, TIN_X, &archive, Some("0817")).unwrap();
        let target = Database::open_ephemeral().unwrap();
        import_profile_data(&target, &archive).unwrap();

        // Records without the credentials, so a failure prints none.
        let comparable = |db: &Database| -> serde_json::Value {
            let mut profile = db.get_profile_by_tin(TIN_X).unwrap().expect("the profile");
            let mut record = profile.record_without_credentials().unwrap();
            record.as_object_mut().unwrap().remove("id");
            record
        };
        assert_eq!(
            comparable(&target),
            comparable(&source),
            "a profile field changed on the round trip"
        );
        let imported = target.get_profile_by_tin(TIN_X).unwrap().unwrap();
        assert_eq!(imported.id, Some(1), "the import is this database's row");
        drop(imported);
        for (key, value) in PROFILE_CREDENTIAL_KEYS
            .into_iter()
            .zip(stored_credentials(&target, TIN_X))
        {
            assert!(value.is_none(), "the round trip stored a {key}");
        }

        let providers = target.get_data_providers(TIN_X).unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].provider_id, "generic");
        assert_eq!(providers[0].name, "Books");
        assert!(
            providers[0].credentials.is_empty(),
            "the provider came back with credentials"
        );
    }

    /// The Settings all-profiles backup puts each profile under
    /// `Profiles/<TIN>/`: the import reads every one, not just a top-level
    /// `profile.json`.
    #[test]
    fn a_combined_archive_imports_every_profile() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let archive = dir.path().join("all-profiles.zip");
        write_archive(
            &archive,
            &[
                (
                    &format!("Profiles/{TIN_Y}/profile.json"),
                    serde_json::to_string(&branch_profile("Combined One", TIN_Y)).unwrap(),
                ),
                (
                    &format!("Profiles/{TIN_X}/profile.json"),
                    serde_json::to_string(&branch_profile("Combined Two", TIN_X)).unwrap(),
                ),
            ],
        );

        import_profile_data(&db, &archive).unwrap();
        let mut names: Vec<String> = db
            .list_profiles()
            .unwrap()
            .iter()
            .map(|profile| profile.full_name.clone())
            .collect();
        names.sort();
        assert_eq!(names, ["Combined One", "Combined Two"]);
    }

    /// The whole archive writes under one transaction: a later profile's
    /// bad `profile.json` fails the import and rolls the earlier profile
    /// back — no half-imported state.
    /// A profile's version ledger, as it is stored.
    fn versions_of(profile: &TaxpayerProfile) -> serde_json::Value {
        serde_json::to_value(&profile.profile_versions).unwrap()
    }

    /// A taxpayer whose COR version was confirmed under review, as the app
    /// saves it.
    fn save_with_a_confirmed_cor(db: &Database, full_name: &str, tin: &str) -> TaxpayerProfile {
        use crate::profile::TaxProfileVersionSource;
        let mut profile = branch_profile(full_name, tin);
        profile.business_start_date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1);
        profile.ensure_profile_version_ledger();
        let version = &mut profile.profile_versions[0];
        version.id = "reviewed-cor".to_string();
        version.label = "Reviewed COR".to_string();
        version.source = TaxProfileVersionSource::ManualCor;
        let effective_from = version.effective_from.expect("a dated version");
        let plan = profile
            .profile_version_confirmation_plan("reviewed-cor", effective_from)
            .expect("a plan for the first confirmed version");
        db.save_profile_with_confirmation_plan(profile, &plan)
            .expect("the reviewed confirmation saves")
    }

    /// The owner's rule for a restore: an export of a taxpayer with a
    /// confirmed COR version comes into a database that does not have the
    /// taxpayer with that version confirmed, as recorded. Typing the same
    /// profile into the app without a review is still refused.
    #[test]
    fn a_restore_keeps_the_confirmed_versions_of_a_taxpayer_the_database_lacks() {
        let versions = versions_of;
        use crate::profile::{TaxProfileVersionSource, TaxProfileVersionStatus};
        let dir = tempdir().unwrap();
        let source = Database::open_ephemeral().unwrap();
        let exported = save_with_a_confirmed_cor(&source, "Confirmed Corp", TIN_X);
        let archive = dir.path().join("confirmed.zip");
        crate::export::export_profile_data(&source, TIN_X, &archive, None).unwrap();

        let target = Database::open_ephemeral().unwrap();
        import_profile_data(&target, &archive).expect("the export restores");
        let restored = target.get_profile_by_tin(TIN_X).unwrap().expect("restored");
        assert_eq!(versions(&restored), versions(&exported));
        let version = &restored.profile_versions[0];
        assert_eq!(version.status, TaxProfileVersionStatus::Confirmed);
        assert_eq!(version.source, TaxProfileVersionSource::ManualCor);

        // Restoring it again, now that the taxpayer is here, changes nothing.
        import_profile_data(&target, &archive).expect("the same export restores again");
        assert_eq!(
            versions(&target.get_profile_by_tin(TIN_X).unwrap().unwrap()),
            versions(&exported)
        );

        // The rule for a save from the app is as it was.
        let mut typed = exported.clone();
        typed.id = None;
        let error = Database::open_ephemeral()
            .unwrap()
            .save_profile(typed)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires an explicit reviewed confirmation plan"),
            "{error}"
        );
    }

    /// A taxpayer that is already here is not given confirmed versions by a
    /// file: the import is refused and the stored taxpayer is as it was.
    #[test]
    fn a_restore_does_not_confirm_versions_for_a_taxpayer_already_here() {
        let dir = tempdir().unwrap();
        let source = Database::open_ephemeral().unwrap();
        save_with_a_confirmed_cor(&source, "Confirmed Corp", TIN_X);
        let archive = dir.path().join("confirmed.zip");
        crate::export::export_profile_data(&source, TIN_X, &archive, None).unwrap();

        let target = Database::open_ephemeral().unwrap();
        target
            .save_profile(branch_profile("Already Here Corp", TIN_X))
            .unwrap();
        let before = target.get_profile_by_tin(TIN_X).unwrap().unwrap();
        let error = import_profile_data(&target, &archive).unwrap_err();
        assert!(
            error.to_string().contains("reviewed confirmation plan"),
            "{error}"
        );
        let after = target.get_profile_by_tin(TIN_X).unwrap().unwrap();
        assert_eq!(after.full_name, "Already Here Corp");
        assert_eq!(versions_of(&after), versions_of(&before));
    }

    #[test]
    fn a_failed_second_profile_rolls_back_the_whole_import() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let archive = dir.path().join("broken.zip");
        // The valid profile comes first in the archive, so without the
        // transaction it would already be saved when the second fails.
        write_archive(
            &archive,
            &[
                (
                    &format!("Profiles/{TIN_Y}/profile.json"),
                    serde_json::to_string(&branch_profile("Rolled Back", TIN_Y)).unwrap(),
                ),
                (
                    &format!("Profiles/{TIN_X}/profile.json"),
                    "{ not json".to_string(),
                ),
            ],
        );

        assert!(import_profile_data(&db, &archive).is_err());
        assert!(
            db.list_profiles().unwrap().is_empty(),
            "the first profile survived a rolled-back import"
        );
    }

    /// Plain build: a backup whose database is not plain SQLite (what an
    /// encrypted file looks like) is refused, and the current database, its
    /// bytes and its name, is not touched.
    #[cfg(not(feature = "sqlcipher"))]
    #[test]
    fn a_backup_that_is_not_plain_sqlite_is_refused_and_the_current_database_stays() {
        let dir = tempdir().unwrap();
        let current = dir.path().join("bir_data.db");
        {
            let db = Database::open(&current).unwrap();
            db.set_setting("kept", "yes").unwrap();
            db.close().unwrap();
        }
        let before = fs::read(&current).unwrap();
        let archive = dir.path().join("backup.zip");
        write_archive(
            &archive,
            &[("bir_data.db", "\u{1}\u{2}not-sqlite-".repeat(512))],
        );

        let error = extract_database_zip(&archive, &current).expect_err("must refuse");
        assert!(error.to_string().contains("not a plain SQLite"), "{error}");
        assert_eq!(
            fs::read(&current).unwrap(),
            before,
            "current database untouched"
        );
        let db = Database::open(&current).unwrap();
        assert_eq!(db.get_setting("kept").unwrap(), Some("yes".to_string()));
    }

    /// Plain build: a whole-database backup round-trips with no key and no
    /// `sqlcipher_export`, and the restored file is plain SQLite.
    #[cfg(not(feature = "sqlcipher"))]
    #[test]
    fn a_whole_database_backup_round_trips_as_plain_sqlite() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source.db");
        let restored = dir.path().join("restored.db");
        let archive = dir.path().join("backup.zip");
        {
            let db = Database::open(&source).unwrap();
            db.set_setting("round_trip", "plain").unwrap();
            crate::export::export_database_zip(&db, &archive).unwrap();
            db.close().unwrap();
        }
        // An older copy is replaced.
        fs::write(&restored, b"old").unwrap();
        extract_database_zip(&archive, &restored).unwrap();

        assert_eq!(
            crate::db::database_file_kind(&restored).unwrap(),
            crate::db::DatabaseFileKind::PlainSqlite
        );
        let db = Database::open(&restored).unwrap();
        assert_eq!(
            db.get_setting("round_trip").unwrap(),
            Some("plain".to_string())
        );
    }
    // ---- restoring a database backup: validate, then replace ----

    /// The files in `dir`, sorted.
    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// A backup archive whose `bir_data.db` is `bytes`.
    fn write_database_archive(path: &Path, bytes: &[u8]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        zip.start_file("bir_data.db", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, bytes).unwrap();
        zip.finish().unwrap();
    }

    /// The (plain) bytes of a real, migrated database that holds the setting
    /// `marker = value`: what a good backup carries. Built in a folder of its own.
    fn good_database_bytes(value: &str) -> Vec<u8> {
        let dir = tempdir().unwrap();
        let source = Database::open(dir.path().join("source.db")).unwrap();
        source.set_setting("marker", value).unwrap();
        let archive = dir.path().join("good.zip");
        crate::export::export_database_zip(&source, &archive).unwrap();
        source.close().unwrap();
        let mut zip = ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
        let mut bytes = Vec::new();
        zip.by_name("bir_data.db")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }

    /// The current database in a folder of its own: `kept = yes`, closed.
    fn current_database(dir: &Path) -> PathBuf {
        let current = dir.join("bir_data.db");
        let db = Database::open(&current).unwrap();
        db.set_setting("kept", "yes").unwrap();
        db.close().unwrap();
        current
    }

    fn assert_current_untouched(current: &Path, before: &[u8]) {
        assert_eq!(fs::read(current).unwrap(), before, "current database bytes");
        let db = Database::open(current).unwrap();
        assert_eq!(db.get_setting("kept").unwrap(), Some("yes".to_string()));
    }

    /// Every way an archive's database can be unfit to replace the live one is
    /// refused with the reason, before the live database is touched, and
    /// leaves no temporary file next to it. (Before the check ran first, the
    /// SQLCipher build deleted the live file and then failed to export; the
    /// plain build only looked at the header and `count(*)` of `sqlite_master`.)
    #[test]
    fn a_database_backup_that_is_unfit_never_costs_the_current_database() {
        let good = good_database_bytes("good");
        let page = {
            let size = u16::from_be_bytes([good[16], good[17]]);
            if size == 1 { 65536 } else { usize::from(size) }
        };
        assert!(good.len() >= page * 6, "a real database has several pages");

        let mut corrupt = good.clone();
        corrupt[page..page * 4].fill(0xAB);

        let scratch = tempdir().unwrap();
        let foreign = scratch.path().join("foreign.db");
        {
            let conn = rusqlite::Connection::open(&foreign).unwrap();
            conn.execute_batch("CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('x');")
                .unwrap();
        }
        let newer = scratch.path().join("newer.db");
        fs::write(&newer, &good).unwrap();
        {
            let conn = rusqlite::Connection::open(&newer).unwrap();
            conn.execute_batch("PRAGMA user_version = 9999;").unwrap();
        }

        let cases: [(&str, Vec<u8>, &str); 4] = [
            (
                "not sqlite at all",
                "\u{1}\u{2}not-sqlite-".repeat(512).into_bytes(),
                "not a plain SQLite",
            ),
            ("damaged pages", corrupt, "integrity check"),
            (
                "another program's database",
                fs::read(&foreign).unwrap(),
                "not an eBIRForms database",
            ),
            (
                "a newer schema",
                fs::read(&newer).unwrap(),
                "newer version of the app",
            ),
        ];
        for (name, bytes, reason) in cases {
            let dir = tempdir().unwrap();
            let current = current_database(dir.path());
            let before = fs::read(&current).unwrap();
            let listing = names(dir.path());
            let archive = scratch.path().join("backup.zip");
            write_database_archive(&archive, &bytes);

            let error = extract_database_zip(&archive, &current)
                .expect_err(&format!("{name}: must be refused"))
                .to_string();
            assert!(error.contains(reason), "{name}: {error}");
            assert!(error.contains("left as it is"), "{name}: {error}");
            assert_current_untouched(&current, &before);
            assert_eq!(
                names(dir.path()),
                listing,
                "{name}: nothing left beside the database"
            );
        }
    }

    /// A good backup replaces the current database, and what the replaced
    /// one left behind (its write-ahead log and shared memory) goes too, or it
    /// would be replayed into the new file.
    #[test]
    fn a_good_database_backup_replaces_the_current_one_and_leaves_nothing_behind() {
        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        for suffix in ["-wal", "-shm"] {
            fs::write(
                with_suffix(&current, suffix),
                b"stale bytes of the old file",
            )
            .unwrap();
        }
        let scratch = tempdir().unwrap();
        let archive = scratch.path().join("backup.zip");
        write_database_archive(&archive, &good_database_bytes("restored"));

        extract_database_zip(&archive, &current).unwrap();

        assert_eq!(
            names(dir.path()),
            ["bir_data.db"],
            "no log, no temporary file"
        );
        let db = Database::open(&current).unwrap();
        assert_eq!(
            db.get_setting("marker").unwrap(),
            Some("restored".to_string())
        );
        assert_eq!(
            db.get_setting("kept").unwrap(),
            None,
            "the old data is gone"
        );
    }

    /// The schema version a database file says, read without opening it as
    /// the app does (no migration).
    fn stored_schema_version(path: &Path) -> i32 {
        Database::open_existing_read_only(path)
            .unwrap()
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap()
    }

    /// The database in a backup archive, and the database a restore installs,
    /// say the schema version of the database they were made from. (The
    /// SQLCipher build's `sqlcipher_export` does not copy it: both said 0, and
    /// the next open ran every migration again over the person's data.)
    #[test]
    fn a_backup_and_a_restore_keep_the_schema_version() {
        let current_version = crate::db::current_schema_version().unwrap();
        assert!(current_version > 1);

        let scratch = tempdir().unwrap();
        let exported = scratch.path().join("exported.db");
        fs::write(&exported, good_database_bytes("versioned")).unwrap();
        let in_archive: i32 = rusqlite::Connection::open(&exported)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(in_archive, current_version, "the archive's database");

        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        let archive = scratch.path().join("backup.zip");
        write_database_archive(&archive, &fs::read(&exported).unwrap());
        extract_database_zip(&archive, &current).unwrap();
        assert_eq!(
            stored_schema_version(&current),
            current_version,
            "the restored database, before anything opens it"
        );

        // The read-only export of a database nobody has open.
        let read_only = scratch.path().join("read-only.zip");
        crate::export::export_existing_database_zip(&current, &read_only).unwrap();
        let mut zip = ZipArchive::new(fs::File::open(&read_only).unwrap()).unwrap();
        let mut bytes = Vec::new();
        zip.by_name("bir_data.db")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let out = scratch.path().join("read-only.db");
        fs::write(&out, bytes).unwrap();
        let read_only_version: i32 = rusqlite::Connection::open(&out)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(read_only_version, current_version, "the read-only export");
    }

    /// A backup archive an older encrypted build exported says version 0
    /// whatever its schema is. Restored and opened, it is not migrated again
    /// from the start: the forms the person set for a year are as they set
    /// them (the v8 and v9 backfills used to replace them).
    #[test]
    fn a_restored_backup_that_says_version_zero_keeps_what_the_person_set() {
        let scratch = tempdir().unwrap();
        let old_export = scratch.path().join("old-export.db");
        fs::write(&old_export, good_database_bytes("old export")).unwrap();
        {
            let conn = rusqlite::Connection::open(&old_export).unwrap();
            let profile = serde_json::json!({
                "id": 1,
                "full_name": "Juana Santos",
                "tin": { "segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000" },
                "rdo_code": "039",
                "line_of_business": "Consulting",
                "registered_address": "QC",
                "zip_code": "1100",
                "phone": "09170000000",
                "email": "test@example.com",
                "default_form_type": "2551Q",
                "taxpayer_type": "Individual",
                "is_vat_registered": false,
                "compliance_source_mode": "CorVersioned",
                "profile_versions": [{
                    "id": "v1",
                    "label": "Version 1",
                    "status": "Confirmed",
                    "source": "ManualCor",
                    "effective_from": "2026-01-01",
                    "effective_until": null,
                    "cor": {},
                    "registered_tax_types": ["IncomeTax", "PercentageTax"],
                    "taxpayer_type": "Individual",
                    "is_vat_registered": false,
                    "obligation_overrides": []
                }]
            })
            .to_string();
            conn.execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, ?2)",
                rusqlite::params!["000-123-453-00000", profile],
            )
            .unwrap();
            conn.execute_batch(
                "INSERT INTO per_year_forms (tin, taxable_year, form_code, frequency, active, source, custom, reason)
                 VALUES ('00012345300000', 2026, '2551Q', 'quarterly', 0, 'manual', 0, 'Turned off by hand');
                 PRAGMA user_version = 0;",
            )
            .unwrap();
        }
        let archive = scratch.path().join("backup.zip");
        write_database_archive(&archive, &fs::read(&old_export).unwrap());

        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        extract_database_zip(&archive, &current).unwrap();
        assert_eq!(stored_schema_version(&current), 0, "as the archive said");

        let db = Database::open(&current).unwrap();
        let forms: Vec<(String, i64, String)> = db
            .conn
            .prepare("SELECT form_code, active, source FROM per_year_forms ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            forms,
            [("2551Q".to_string(), 0, "manual".to_string())],
            "the form turned off by hand is still off, and nothing was generated"
        );
        drop(db);
        assert_eq!(
            stored_schema_version(&current),
            crate::db::current_schema_version().unwrap()
        );
    }

    /// The permission bits of `path`.
    #[cfg(all(unix, feature = "sqlcipher"))]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// The SQLCipher build stages the backup's plain database in the temp
    /// folder: in a folder of its own that only this user can open (0700),
    /// as a file only this user can read (0600), and the folder goes after an
    /// install and after a refusal. (It was a 0644 file straight in the temp
    /// folder.)
    #[cfg(all(unix, feature = "sqlcipher"))]
    #[test]
    fn a_restore_stages_the_plain_database_privately_and_leaves_nothing_behind() {
        let dir = tempdir().unwrap();
        let temp = dir.path().join("temp");
        fs::create_dir(&temp).unwrap();
        let live = dir.path().join("live");
        fs::create_dir(&live).unwrap();
        let current = current_database(&live);
        let archive = dir.path().join("backup.zip");
        write_database_archive(&archive, &good_database_bytes("private"));
        let unfit = dir.path().join("unfit.zip");
        write_database_archive(&unfit, &"not-sqlite-".repeat(512).into_bytes());

        crate::private_temp::with_root(&temp, || {
            let staged = stage_database_zip(&archive, &current).unwrap();
            let folder = staged.path().parent().unwrap().to_path_buf();
            assert_eq!(
                folder.parent(),
                Some(temp.as_path()),
                "staged in the temp folder"
            );
            assert_eq!(mode_of(&folder), 0o700, "the staging folder");
            assert_eq!(mode_of(staged.path()), 0o600, "the staged plain database");
            staged.install().unwrap();
            assert!(names(&temp).is_empty(), "nothing left after a restore");

            stage_database_zip(&unfit, &current).expect_err("an unfit backup is refused");
            assert!(names(&temp).is_empty(), "nothing left after a refusal");

            // Staged and dropped without an install (the caller's safety copy failed).
            drop(stage_database_zip(&archive, &current).unwrap());
            assert!(
                names(&temp).is_empty(),
                "nothing left after a staging that was dropped"
            );
        });
        let db = Database::open(&current).unwrap();
        assert_eq!(
            db.get_setting("marker").unwrap(),
            Some("private".to_string())
        );
    }

    /// Staging checks the archive and writes beside the target; it replaces
    /// nothing until `install`, and dropping a staged database removes it.
    #[test]
    fn a_staged_database_replaces_nothing_until_it_is_installed() {
        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        let before = fs::read(&current).unwrap();
        let scratch = tempdir().unwrap();
        let archive = scratch.path().join("backup.zip");
        write_database_archive(&archive, &good_database_bytes("staged"));

        let staged = stage_database_zip(&archive, &current).unwrap();
        let temporary = staged.path().to_path_buf();
        assert!(temporary.is_file());
        #[cfg(not(feature = "sqlcipher"))]
        assert_eq!(
            temporary.parent(),
            current.parent(),
            "staged beside the file it will replace, so the rename stays on one file system"
        );
        assert_current_untouched(&current, &before);
        drop(staged);
        assert!(!temporary.exists(), "dropping a staged database cleans up");
        assert_current_untouched(&current, &before);

        stage_database_zip(&archive, &current)
            .unwrap()
            .install()
            .unwrap();
        let db = Database::open(&current).unwrap();
        assert_eq!(
            db.get_setting("marker").unwrap(),
            Some("staged".to_string())
        );
    }

    /// When the replace itself fails, here because the old file's write-ahead
    /// log is something that cannot be removed, the current database is still
    /// there, byte for byte, and the temporary file is gone. (Before, the
    /// SQLCipher build deleted the live file first and the failure came after;
    /// it now stages like the plain build, so this runs in both.)
    #[test]
    fn a_replace_that_fails_midway_keeps_the_current_database() {
        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        let before = fs::read(&current).unwrap();
        fs::create_dir(with_suffix(&current, "-wal")).unwrap();
        let scratch = tempdir().unwrap();
        let archive = scratch.path().join("backup.zip");
        write_database_archive(&archive, &good_database_bytes("never"));

        extract_database_zip(&archive, &current).expect_err("the log cannot be removed");

        assert_eq!(
            fs::read(&current).unwrap(),
            before,
            "current database bytes"
        );
        assert_eq!(
            names(dir.path()),
            ["bir_data.db", "bir_data.db-wal"],
            "no temporary file"
        );
    }

    /// SQLCipher build: an export that fails (here the staged file is not a
    /// database at all) never costs the current database: it is still there,
    /// byte for byte, and nothing is left beside it. (Before, the live file
    /// was deleted before the export began.)
    #[cfg(feature = "sqlcipher")]
    #[test]
    fn an_export_that_fails_keeps_the_current_database() {
        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        let before = fs::read(&current).unwrap();
        let scratch = tempdir().unwrap();
        let staged = scratch.path().join("staged.db");
        fs::write(&staged, "\u{1}\u{2}not-sqlite-".repeat(512)).unwrap();

        install_staged_database(&staged, &current).expect_err("not a database: nothing to export");

        assert_current_untouched(&current, &before);
        assert_eq!(names(dir.path()), ["bir_data.db"], "no temporary file");
    }

    /// SQLCipher build: the replacement is the encrypted export of the
    /// staged database, made beside the live file and renamed over it. It
    /// does not open without the key, it does with it, and no temporary file
    /// or log stays behind.
    #[cfg(feature = "sqlcipher")]
    #[test]
    fn the_installed_database_is_encrypted_with_the_current_key() {
        let dir = tempdir().unwrap();
        let current = current_database(dir.path());
        let scratch = tempdir().unwrap();
        let archive = scratch.path().join("backup.zip");
        write_database_archive(&archive, &good_database_bytes("encrypted"));

        extract_database_zip(&archive, &current).unwrap();

        assert_eq!(names(dir.path()), ["bir_data.db"], "no temporary file");
        assert_eq!(
            crate::db::database_file_kind(&current).unwrap(),
            crate::db::DatabaseFileKind::Other,
            "not plain SQLite: it is encrypted"
        );
        let db = Database::open(&current).unwrap();
        assert_eq!(
            db.get_setting("marker").unwrap(),
            Some("encrypted".to_string())
        );
        assert_eq!(db.get_setting("kept").unwrap(), None);
    }

    // ---------- Round 3: the matching rule, one test per table row ----------

    /// A drafts.json row for a hand-written archive.
    fn file_draft(
        form_code: &str,
        year: i64,
        quarter: Option<i64>,
        status: &str,
        data: serde_json::Value,
        updated_at: Option<&str>,
        period_key: Option<&str>,
    ) -> serde_json::Value {
        serde_json::json!({
            "form_code": form_code,
            "taxable_year": year,
            "quarter": quarter,
            "status": status,
            "data_json": serde_json::to_string(&data).unwrap(),
            "updated_at": updated_at,
            "period_key": period_key,
        })
    }

    /// An archive carrying `drafts` for `tin` (and its profile).
    fn archive_with_drafts(path: &Path, tin: &str, drafts: &[serde_json::Value]) {
        let profile = serde_json::to_value(branch_profile("Archivist", tin)).unwrap();
        write_archive(
            path,
            &[
                ("profile.json", profile.to_string()),
                ("drafts.json", serde_json::to_string(drafts).unwrap()),
            ],
        );
    }

    /// The raw rows of one form/year, every quarter and key, oldest id first.
    fn rows_of(
        db: &Database,
        tin: &str,
        form_code: &str,
        year: i64,
    ) -> Vec<(i64, Option<i64>, Option<String>, String, String)> {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT id, quarter, period_key, status, data_json FROM form_drafts
                 WHERE tin = ?1 AND form_code = ?2 AND taxable_year = ?3 ORDER BY id",
            )
            .unwrap();
        stmt.query_map(rusqlite::params![tin, form_code, year], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    /// N1: a 1701 exported and imported onto a fresh database opens with its
    /// data in the real editor read path — the queueable read at quarter 0.
    #[test]
    fn import_insert_preserves_quarter_zero() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        let draft = crate::forms::form_1701::Form1701Draft::new_from_profile(
            &branch_profile("Juana", TIN_X),
            2024,
            0,
        );
        // Written as the queueable write path writes it: quarter 0, key "A".
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '1701', 2024, 0, 'A', 'Draft', ?2, '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X, serde_json::to_string(&serde_json::to_value(&draft).unwrap()).unwrap()],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        crate::export::export_profile_data(&db, TIN_X, &archive, None).unwrap();

        let fresh = Database::open_ephemeral().unwrap();
        import_profile_data(&fresh, &archive).unwrap();
        let read: Option<crate::forms::form_1701::Form1701Draft> =
            fresh.get_queueable_draft(TIN_X, 2024, 0).unwrap();
        assert!(
            read.is_some(),
            "an imported 1701 opens in the editor read path at quarter 0"
        );
        let rows = rows_of(&fresh, TIN_X, "1701", 2024);
        assert_eq!(rows.len(), 1, "exactly one row, never a duplicate");
        assert_eq!(rows[0].1, Some(0), "the file's own quarter is preserved");
    }

    /// N2: a legacy archive row without a period key is inserted with the
    /// key the v5 backfill would derive (a monthly form: M{month:02}).
    #[test]
    fn import_legacy_insert_derives_period_key() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let archive = dir.path().join("legacy.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "1601C",
                2024,
                Some(5),
                "Draft",
                serde_json::json!({"status": "Draft"}),
                Some("2024-06-01T00:00:00Z"),
                None,
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "1601C", 2024);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].2.as_deref(),
            Some("M05"),
            "the insert writes the derived period key"
        );
    }

    /// N1: a file row at quarter 0 matches a device row at NULL without a
    /// second row, and the update keeps the device's storage.
    #[test]
    fn import_quarter_zero_matches_null_without_duplicate() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json, updated_at)
                 VALUES (?1, '1701', 2024, NULL, 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "1701",
                2024,
                Some(0),
                "Draft",
                serde_json::json!({"status": "Draft", "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                Some("A"),
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "1701", 2024);
        assert_eq!(rows.len(), 1, "quarter 0 matches NULL: never a second row");
    }

    /// N2: a device row with a NULL key is adopted by the file's single
    /// dated candidate — matched, updated and stamped with the key.
    #[test]
    fn import_null_device_key_adopts_single_dated_candidate() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '1601C', 2024, 5, NULL, 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "1601C",
                2024,
                Some(5),
                "Draft",
                serde_json::json!({"status": "Draft", "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                Some("M05"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "1601C", 2024);
        assert_eq!(rows.len(), 1, "adopted, never duplicated");
        assert_eq!(
            rows[0].2.as_deref(),
            Some("M05"),
            "the update writes the key"
        );
        assert_eq!(counts.restored, 1);
    }

    /// N2: several NULL-keyed candidates can never be told apart — nothing
    /// is written and the row counts as unmatched. (Several candidates at one
    /// quarter exist only where the unique index cannot forbid them:
    /// open-ended rows at NULL quarters.)
    #[test]
    fn import_null_device_key_plural_candidates_unmatched() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        for marker in ["first", "second"] {
            db.conn
                .execute(
                    "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                     VALUES (?1, '0605', 2024, NULL, NULL, 'Draft', ?2, '2024-01-01T00:00:00Z')",
                    rusqlite::params![TIN_X, format!("{{\"status\":\"Draft\",\"mine\":\"{marker}\"}}")],
                )
                .unwrap();
        }
        let archive = dir.path().join("a.zip");
        // A legacy file row: no key, so it can mean any period of the year.
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "0605",
                2024,
                None,
                "Draft",
                serde_json::json!({"status": "Draft", "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                None,
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "0605", 2024);
        assert_eq!(rows.len(), 2, "nothing inserted, nothing overwritten");
        assert!(
            rows.iter()
                .all(|row| row.4.contains("first") || row.4.contains("second")),
            "both rows keep their own data"
        );
        assert_eq!(counts.restored, 0);
        assert_eq!(counts.unmatched, 1, "the plurality counts as unmatched");
    }

    /// A dated device row meeting a legacy file row over exactly one
    /// candidate: the match is made and the derived key is written.
    #[test]
    fn import_dated_device_legacy_file_single_candidate_writes_key() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '1601C', 2024, 5, NULL, 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "1601C",
                2024,
                Some(5),
                "Draft",
                serde_json::json!({"status": "Draft", "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                None,
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "1601C", 2024);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].2.as_deref(),
            Some("M05"),
            "the derived key is written on update"
        );
        assert!(
            !rows[0].4.contains("\"mine\":true"),
            "the newer file row replaced the data"
        );
    }

    /// Several exact-key candidates (pre-existing duplicates) are never
    /// fallen through to the first id.
    #[test]
    fn import_multiple_exact_candidates_never_picks_first() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        // Pre-existing duplicates: the unique index is over quarter, so use
        // a quarterly form's quarters… the index makes true duplicates of
        // (tin, form, year, quarter) impossible — except rows written before
        // the index or by other versions. Craft them with a direct insert,
        // dropping the index's protection the way old rows exist.
        for marker in ["first", "second"] {
            db.conn
                .execute(
                    "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                     VALUES (?1, '0605', 2024, NULL, 'O1', 'Draft', ?2, '2024-01-01T00:00:00Z')",
                    rusqlite::params![TIN_X, format!("{{\"status\":\"Draft\",\"mine\":\"{marker}\"}}")],
                )
                .unwrap();
        }
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "0605",
                2024,
                None,
                "Draft",
                serde_json::json!({"status": "Draft", "mine": "file"}),
                Some("2024-06-01T00:00:00Z"),
                Some("O1"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "0605", 2024);
        assert_eq!(rows.len(), 2, "nothing inserted");
        assert!(
            rows.iter()
                .all(|row| row.4.contains("first") || row.4.contains("second")),
            "neither candidate is overwritten by a guess"
        );
        assert_eq!(counts.unmatched, 1);
        assert_eq!(counts.restored, 0);
    }

    /// Keys in conflict on one quarter are never a second row and never
    /// roll the import back.
    #[test]
    fn import_conflicting_period_key_never_duplicates_return() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '1601C', 2024, 5, 'M05', 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[
                file_draft(
                    "1601C",
                    2024,
                    Some(5),
                    "Draft",
                    serde_json::json!({"status": "Draft", "mine": false}),
                    Some("2024-06-01T00:00:00Z"),
                    Some("M06"),
                ),
                // A second, independent row proves the import is not rolled back.
                file_draft(
                    "2551Q",
                    2024,
                    Some(1),
                    "Draft",
                    serde_json::json!({"status": "Draft"}),
                    Some("2024-06-01T00:00:00Z"),
                    Some("Q1"),
                ),
            ],
        );
        let result = import_profile_data(&db, &archive);
        assert!(
            result.is_ok(),
            "the import is never rolled back: {result:?}"
        );
        let rows = rows_of(&db, TIN_X, "1601C", 2024);
        assert_eq!(rows.len(), 1, "never a second row for one period");
        let quarterly = rows_of(&db, TIN_X, "2551Q", 2024);
        assert_eq!(quarterly.len(), 1, "the rest of the archive still imports");
    }

    /// The file's Queued becomes a Draft before any comparison: a newer
    /// Queued in the file never displaces the device's Submitted.
    #[test]
    fn import_file_queued_becomes_draft_before_compare() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Submitted', '{\"status\":\"Submitted\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Queued",
                serde_json::json!({"status": "Queued", "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].3, "Submitted", "never lowered by a file's Queued");
        assert_eq!(counts.kept_newer, 1);
    }

    /// Rank with dry runs: a newer dry-run Submitted never displaces the
    /// device's real submission.
    #[test]
    fn import_dry_run_never_displaces_real_submission() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Submitted', '{\"status\":\"Submitted\",\"submitted_dry_run\":false,\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Submitted",
                serde_json::json!({"status": "Submitted", "submitted_dry_run": true, "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert!(
            rows[0].4.contains("\"mine\":true"),
            "the real submission is never displaced by a dry run"
        );
        assert_eq!(counts.kept_newer, 1);
    }

    /// And the reverse: the device's dry run is displaced by the file's
    /// real submission, by rank.
    #[test]
    fn import_real_submission_displaces_dry_run() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Submitted', '{\"status\":\"Submitted\",\"submitted_dry_run\":true,\"mine\":true}', '2024-06-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Submitted",
                serde_json::json!({"status": "Submitted", "submitted_dry_run": false, "mine": false}),
                Some("2024-12-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        // The file is strictly newer here: rank may displace the rehearsal.
        // (An OLDER real submission never does — a dry run ranks as Draft,
        // and a newer device row is never eaten by an older filed one.)
        assert!(
            rows[0].4.contains("\"mine\":false"),
            "the real submission displaces the dry run by rank"
        );
    }

    /// Confirmed and Paid are never lowered, however new the file's row.
    #[test]
    fn import_never_lowers_confirmed() {
        never_lowers("Confirmed", "Submitted");
    }

    #[test]
    fn import_never_lowers_paid() {
        never_lowers("Paid", "Submitted");
    }

    fn never_lowers(device_status: &str, file_status: &str) {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', ?2, ?3, '2024-01-01T00:00:00Z')",
                rusqlite::params![
                    TIN_X,
                    device_status,
                    format!("{{\"status\":\"{device_status}\",\"mine\":true}}")
                ],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                file_status,
                serde_json::json!({"status": file_status, "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert_eq!(rows[0].3, device_status, "the status is never lowered");
    }

    /// An unparseable timestamp never wins at the same rank.
    #[test]
    fn import_unparseable_timestamp_never_wins_same_rank() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Draft",
                serde_json::json!({"status": "Draft", "mine": false}),
                Some("not a timestamp"),
                Some("Q1"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert!(rows[0].4.contains("\"mine\":true"));
        assert_eq!(counts.kept_newer, 1);
    }

    /// Across an amendment the file's row needs rank ≥ AND a strictly newer
    /// stamp: a re-stamped old original never displaces the device's newer
    /// amendment, whatever the amendment's status.
    #[test]
    fn import_cross_amendment_requires_rank_and_newer() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        // The device's in-place amendment, Draft, stamped now-ish.
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Submitted', '{\"status\":\"Submitted\",\"is_amended\":true,\"mine\":true}', '2024-06-01T00:00:00Z')",
                rusqlite::params![TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        // An old backup's Submitted original — but re-stamped newer, as an
        // open of the stale copy would stamp it.
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Draft",
                serde_json::json!({"status": "Draft", "is_amended": false, "mine": false}),
                Some("2024-07-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert!(
            rows[0].4.contains("\"mine\":true"),
            "the newer amendment stays: rank alone never beats time across an amendment"
        );
        assert_eq!(counts.kept_newer, 1);
    }

    /// A device row in the middle of a send — any of the five claim fields —
    /// is never replaced, whatever the file carries.
    #[test]
    fn import_never_replaces_any_mid_send_device_row() {
        for (field, value) in [
            ("submission_claim_token", serde_json::json!("tok")),
            (
                "submission_claimed_at",
                serde_json::json!("2024-05-01T00:00:00Z"),
            ),
            (
                "submission_claim_lease_until",
                serde_json::json!("2024-05-01T00:05:00Z"),
            ),
            (
                "submission_put_started_at",
                serde_json::json!("2024-05-01T00:01:00Z"),
            ),
            ("relay_pending", serde_json::json!("receipt")),
        ] {
            let dir = tempdir().unwrap();
            let db = Database::open_ephemeral().unwrap();
            db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
            let mut data = serde_json::json!({"status": "Queued", "mine": true});
            data[field] = value;
            db.conn
                .execute(
                    "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                     VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Queued', ?2, '2024-01-01T00:00:00Z')",
                    rusqlite::params![TIN_X, serde_json::to_string(&data).unwrap()],
                )
                .unwrap();
            let archive = dir.path().join("a.zip");
            archive_with_drafts(
                &archive,
                TIN_X,
                &[file_draft(
                    "2551Q",
                    2024,
                    Some(1),
                    "Submitted",
                    serde_json::json!({"status": "Submitted", "mine": false}),
                    Some("2024-06-01T00:00:00Z"),
                    Some("Q1"),
                )],
            );
            import_profile_data(&db, &archive).unwrap();
            let rows = rows_of(&db, TIN_X, "2551Q", 2024);
            assert!(
                rows[0].4.contains("\"mine\":true"),
                "a device row mid-send ({field}) is never replaced"
            );
        }
    }

    /// N8: a file row Queued in the middle of its send (claim fields in its
    /// data) is kept out entirely — never inserted, never matched — and
    /// counted with its own reason.
    #[test]
    fn import_file_mid_send_skipped_with_reason() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let archive = dir.path().join("a.zip");
        let mut mid_send = serde_json::json!({"status": "Queued"});
        mid_send["submission_claim_token"] = serde_json::json!("tok");
        mid_send["submission_claimed_at"] = serde_json::json!("2024-05-01T00:00:00Z");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Queued",
                mid_send,
                Some("2024-06-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert_eq!(rows.len(), 0, "a mid-send file row is never inserted");
        assert_eq!(counts.skipped_mid_send, 1, "counted with its own reason");
    }

    /// N1, through the room migration itself: a 1701 written at quarter 0
    /// (the queueable write path's storage) in a legacy database survives
    /// export_all_profiles_data -> import_profile_data and opens in the
    /// editor read path on the fresh room — the migration every user takes
    /// at the first sign-in after the room layout.
    #[test]
    fn import_room_migration_preserves_quarter_zero() {
        let dir = tempdir().unwrap();
        let legacy = Database::open_ephemeral().unwrap();
        legacy
            .save_profile(branch_profile("Migrant", TIN_X))
            .unwrap();
        let draft = crate::forms::form_1701::Form1701Draft::new_from_profile(
            &branch_profile("Migrant", TIN_X),
            2024,
            0,
        );
        legacy
            .conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '1701', 2024, 0, 'A', 'Draft', ?2, '2024-01-01T00:00:00Z')",
                rusqlite::params![TIN_X, serde_json::to_string(&serde_json::to_value(&draft).unwrap()).unwrap()],
            )
            .unwrap();
        let archive = dir.path().join("profiles.zip");
        crate::export::export_all_profiles_data(&legacy, &archive).unwrap();
        let room = Database::open_ephemeral().unwrap();
        import_profile_data(&room, &archive).unwrap();
        let opened: Option<crate::forms::form_1701::Form1701Draft> =
            room.get_queueable_draft(TIN_X, 2024, 0).unwrap();
        assert!(
            opened.is_some(),
            "a migrated 1701 opens in the editor read path at quarter 0"
        );
    }

    /// F6: the year facts keep the FILE's stamp, not the import time — a
    /// later import from another device must compare against when the facts
    /// were really written.
    #[test]
    fn import_year_facts_keep_the_file_stamp() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let profile = serde_json::to_value(branch_profile("Stamped", TIN_X)).unwrap();
        let archive = dir.path().join("a.zip");
        write_archive(
            &archive,
            &[
                ("profile.json", profile.to_string()),
                (
                    "year_facts.json",
                    serde_json::json!({
                        "2024": {"updated_at": "2020-03-01T10:00:00Z"}
                    })
                    .to_string(),
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();
        let stamp: String = db
            .conn
            .query_row(
                "SELECT updated_at FROM year_facts WHERE tin = ?1 AND taxable_year = 2024",
                [TIN_X],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            stamp, "2020-03-01T10:00:00Z",
            "the stored stamp is the file's, not the import time"
        );
    }

    /// F6, the branch path: a branch's archive carries the head office's
    /// effective facts; the head merge keeps the file's stamp too.
    #[test]
    fn import_branch_restore_keeps_the_file_stamp() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        // The head office already exists on this device.
        db.save_profile(branch_profile("Head Office", TIN_X))
            .unwrap();
        let branch_tin = "44455566600001";
        let profile = serde_json::to_value(branch_profile("Branch One", branch_tin)).unwrap();
        let archive = dir.path().join("a.zip");
        write_archive(
            &archive,
            &[
                ("profile.json", profile.to_string()),
                (
                    "year_facts.json",
                    serde_json::json!({
                        "2024": {"updated_at": "2020-03-01T10:00:00Z", "income_tax_election": "EightPercent"}
                    })
                    .to_string(),
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();
        let head_stamp: Option<String> = db
            .conn
            .query_row(
                "SELECT updated_at FROM year_facts WHERE tin = ?1 AND taxable_year = 2024",
                [TIN_X],
                |row| row.get(0),
            )
            .ok();
        assert_eq!(
            head_stamp.as_deref(),
            Some("2020-03-01T10:00:00Z"),
            "the head row written by the branch merge keeps the file's stamp"
        );
    }

    /// KEEP 2: the branch restore's head merge fills EMPTY fields only and
    /// never overwrites a newer head election; the head row's stamp never
    /// moves backward.
    #[test]
    fn restore_year_facts_branch_merge_never_overwrites_newer_head() {
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Head Office", TIN_X))
            .unwrap();
        let branch_tin = "44455566600001";
        db.save_profile(branch_profile("Branch One", branch_tin))
            .unwrap();
        // The head already made its 8% election, written recently.
        let head_facts: crate::tax_profile::YearFacts = serde_json::from_value(serde_json::json!({
            "income_tax_election": "EightPercent",
            "election_source": "2551Q Q1"
        }))
        .unwrap();
        crate::db::taxpayers::write_year_facts_stamped(
            &db.conn,
            TIN_X,
            2024,
            &head_facts,
            Some("2025-06-01T00:00:00Z"),
        )
        .unwrap();
        // An older branch archive carries the graduated election.
        let file_facts: crate::tax_profile::YearFacts = serde_json::from_value(serde_json::json!({
            "income_tax_election": "GraduatedOsd",
            "election_source": "old backup"
        }))
        .unwrap();
        crate::db::taxpayers::restore_year_facts(
            &db.conn,
            branch_tin,
            2024,
            &file_facts,
            Some("2024-01-01T00:00:00Z"),
        )
        .unwrap();
        let json: String = db
            .conn
            .query_row(
                "SELECT facts_json FROM year_facts WHERE tin = ?1 AND taxable_year = 2024",
                [TIN_X],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            json.contains("EightPercent"),
            "the head's newer election is never overwritten: {json}"
        );
        let stamp: String = db
            .conn
            .query_row(
                "SELECT updated_at FROM year_facts WHERE tin = ?1 AND taxable_year = 2024",
                [TIN_X],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            stamp, "2025-06-01T00:00:00Z",
            "the stamp never moves backward"
        );
    }

    /// D1: a higher-rank FILED row replaces unless BOTH rows are dated and
    /// the file's is older — an undated filed row (every v0.2.0 archive)
    /// still replaces a dated device Draft.
    #[test]
    fn import_undated_filed_row_replaces_dated_draft() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-06-01T00:00:00Z')",
                [TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Submitted",
                serde_json::json!({"status": "Submitted", "mine": false}),
                None,
                Some("Q1"),
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert!(
            rows[0].4.contains("\"mine\":false"),
            "the undated filed row replaces the dated Draft: {}",
            rows[0].4
        );
    }

    /// L1: two keyless open-ended entries number after the largest existing
    /// O key — DISTINCT keys, never two O1s.
    #[test]
    fn import_keyless_openended_inserts_get_distinct_keys() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[
                file_draft(
                    "0605",
                    2024,
                    None,
                    "Draft",
                    serde_json::json!({"status": "Draft", "which": "one"}),
                    Some("2024-01-01T00:00:00Z"),
                    None,
                ),
                file_draft(
                    "0605",
                    2024,
                    None,
                    "Draft",
                    serde_json::json!({"status": "Draft", "which": "two"}),
                    Some("2024-01-02T00:00:00Z"),
                    None,
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "0605", 2024);
        assert_eq!(rows.len(), 2);
        let mut keys: Vec<String> = rows.iter().filter_map(|row| row.2.clone()).collect();
        keys.sort();
        assert_eq!(keys.len(), 2, "two rows carry keys");
        assert_ne!(keys[0], keys[1], "the keys are DISTINCT: {keys:?}");
        assert_eq!(keys, vec!["O1".to_string(), "O2".to_string()], "{keys:?}");
    }

    /// L2: when the head row never existed and the branch archive fills
    /// nothing, no seed row is written — a later head archive is never
    /// refused against a seed's fresh stamp.
    #[test]
    fn restore_year_facts_no_seed_when_nothing_filled() {
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Head Office", TIN_X))
            .unwrap();
        let branch_tin = "44455566600001";
        db.save_profile(branch_profile("Branch One", branch_tin))
            .unwrap();
        // The archive's facts carry nothing the head could use.
        let empty_facts: crate::tax_profile::YearFacts =
            serde_json::from_value(serde_json::json!({})).unwrap();
        crate::db::taxpayers::restore_year_facts(
            &db.conn,
            branch_tin,
            2024,
            &empty_facts,
            Some("2020-01-01T00:00:00Z"),
        )
        .unwrap();
        let head_row: Option<i64> = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM year_facts WHERE tin = ?1 AND taxable_year = 2024",
                [TIN_X],
                |row| row.get(0),
            )
            .ok();
        assert_eq!(
            head_row,
            Some(0),
            "no seed row survives when nothing was filled"
        );
    }

    /// KEEP 1: the derived key comes from the registry's frequency, never a
    /// hand list — a May 1600PT (a monthly form) derives M05, not Q4.
    #[test]
    fn derive_period_key_1600pt_may_is_m05() {
        assert_eq!(derive_period_key("1600PT", Some(5)), "M05");
        assert_eq!(derive_period_key("1600VT", Some(5)), "M05");
    }

    /// KEEP 1: annual forms derive A even at the legacy slot 0 they live at.
    #[test]
    fn derive_period_key_1604c_is_annual() {
        assert_eq!(derive_period_key("1604C", Some(0)), "A");
        assert_eq!(derive_period_key("2316", Some(0)), "A");
    }

    /// KEEP 1: every form in the registry derives the key its frequency
    /// means — a monthly form an M key, annual A, quarterly Q, open-ended O.
    #[test]
    fn derive_period_key_walks_every_registry_form() {
        for form in crate::forms::registry::FORM_REGISTRY.iter() {
            for slot in [Some(0), Some(5), None] {
                let key = derive_period_key(form.code, slot);
                let prefix = &key[..1];
                match form.frequency {
                    crate::forms::FilingFrequency::Monthly => assert_eq!(
                        prefix, "M",
                        "{} slot {:?} derived {} — a monthly form derives an M key",
                        form.code, slot, key
                    ),
                    crate::forms::FilingFrequency::Annual => assert_eq!(
                        key, "A",
                        "{} slot {:?} derived {} — an annual form derives A",
                        form.code, slot, key
                    ),
                    crate::forms::FilingFrequency::Quarterly => assert_eq!(
                        prefix, "Q",
                        "{} slot {:?} derived {} — a quarterly form derives a Q key",
                        form.code, slot, key
                    ),
                    crate::forms::FilingFrequency::OpenEnded => assert_eq!(
                        prefix, "O",
                        "{} slot {:?} derived {} — an open-ended form derives an O key",
                        form.code, slot, key
                    ),
                }
            }
        }
    }

    /// KEEP 3: a higher rank never replaces a device row that is dated
    /// strictly newer — an older filed row never eats a newer Draft.
    #[test]
    fn import_higher_rank_never_replaces_a_dated_newer_device_row() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '2551Q', 2024, 1, 'Q1', 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-06-01T00:00:00Z')",
                [TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "2551Q",
                2024,
                Some(1),
                "Submitted",
                serde_json::json!({"status": "Submitted", "mine": false}),
                Some("2024-01-01T00:00:00Z"),
                Some("Q1"),
            )],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "2551Q", 2024);
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].4.contains("\"mine\":true"),
            "the newer device Draft stays: {}",
            rows[0].4
        );
    }

    /// KEEP 3: a dry run ranks as a Draft, never one notch under a real send.
    #[test]
    fn import_dry_run_ranks_as_draft() {
        let dry = serde_json::json!({"submitted_dry_run": true}).to_string();
        assert_eq!(
            filing_status_rank_with_data("Submitted", &dry),
            filing_status_rank("Draft"),
            "a dry-run Submitted ranks as Draft"
        );
    }

    /// KEEP 4: two keyless open-ended entries onto an empty device both
    /// insert — the second never matches the row the first just wrote.
    #[test]
    fn import_two_keyless_openended_entries_never_collapse() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[
                file_draft(
                    "0605",
                    2024,
                    None,
                    "Draft",
                    serde_json::json!({"status": "Draft", "which": "one"}),
                    Some("2024-01-01T00:00:00Z"),
                    None,
                ),
                file_draft(
                    "0605",
                    2024,
                    None,
                    "Draft",
                    serde_json::json!({"status": "Draft", "which": "two"}),
                    Some("2024-01-02T00:00:00Z"),
                    None,
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "0605", 2024);
        assert_eq!(rows.len(), 2, "both entries insert: {rows:?}");
    }

    /// KEEP 4: a keyless open-ended row with ANY candidate is never matched
    /// — unmatched, nothing written.
    #[test]
    fn import_keyless_openended_with_any_candidate_unmatched() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, '0605', 2024, NULL, 'O1', 'Draft', '{\"status\":\"Draft\",\"mine\":true}', '2024-01-01T00:00:00Z')",
                [TIN_X],
            )
            .unwrap();
        let archive = dir.path().join("a.zip");
        archive_with_drafts(
            &archive,
            TIN_X,
            &[file_draft(
                "0605",
                2024,
                None,
                "Draft",
                serde_json::json!({"status": "Draft", "mine": false}),
                Some("2024-06-01T00:00:00Z"),
                None,
            )],
        );
        let counts = import_profile_data(&db, &archive).unwrap();
        let rows = rows_of(&db, TIN_X, "0605", 2024);
        assert_eq!(rows.len(), 1, "nothing inserted over a candidate");
        assert!(
            rows[0].4.contains("\"mine\":true"),
            "the candidate untouched"
        );
        assert_eq!(counts.unmatched, 1);
    }

    /// KEEP 5: one bad element skips the YEAR — good codes of a malformed
    /// entry do NOT apply (the parse is all-or-nothing); what must never
    /// happen is the empty list that empties the year.
    #[test]
    fn import_yearly_forms_unparseable_code_never_empties_a_year() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        // The device already lists 2551Q for 2024.
        crate::db::taxpayers::restore_yearly_forms(
            &db.conn,
            TIN_X,
            2024,
            &["2551Q".to_string()],
            Some("2024-06-01T00:00:00Z"),
        )
        .unwrap();
        let archive = dir.path().join("a.zip");
        let profile = serde_json::to_value(branch_profile("Juana", TIN_X)).unwrap();
        write_archive(
            &archive,
            &[
                ("profile.json", profile.to_string()),
                (
                    "yearly_forms.json",
                    serde_json::json!({"2024": [{"not": "a string"}, 42]}).to_string(),
                ),
                (
                    "yearly_forms_meta.json",
                    serde_json::json!({"2024": "2025-01-01T00:00:00Z"}).to_string(),
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM per_year_forms WHERE tin = ?1 AND taxable_year = 2024 AND form_code = '2551Q' AND active = 1",
                [TIN_X],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "the year's list is never emptied by an unparseable entry"
        );
    }

    /// F2: the Queued revert clears the COMPLETE lifecycle claim at the raw
    /// JSON level — lease_until, put_started_at and relay_pending too — for
    /// a 1601C and a generic form.
    #[test]
    fn import_queued_revert_clears_complete_lifecycle_claim() {
        for form in ["1601C", "2551Q"] {
            let dir = tempdir().unwrap();
            let db = Database::open_ephemeral().unwrap();
            let archive = dir.path().join("a.zip");
            let mut data = serde_json::json!({"status": "Queued"});
            // A finished send's remnants: no live claim, but the lease and
            // relay bookkeeping of one that completed or expired.
            data["submission_claim_lease_until"] = serde_json::json!("2024-05-01T00:05:00Z");
            data["submission_put_started_at"] = serde_json::json!("2024-05-01T00:01:00Z");
            data["relay_pending"] = serde_json::json!("receipt");
            archive_with_drafts(
                &archive,
                TIN_X,
                &[file_draft(
                    form,
                    2024,
                    Some(if form == "1601C" { 5 } else { 1 }),
                    "Queued",
                    data,
                    Some("2024-06-01T00:00:00Z"),
                    Some(if form == "1601C" { "M05" } else { "Q1" }),
                )],
            );
            import_profile_data(&db, &archive).unwrap();
            let rows = rows_of(&db, TIN_X, form, 2024);
            assert_eq!(rows.len(), 1);
            let parsed: serde_json::Value = serde_json::from_str(&rows[0].4).unwrap();
            for key in [
                "submission_claim_token",
                "submission_claimed_at",
                "submission_claim_lease_until",
                "submission_put_started_at",
                "relay_pending",
            ] {
                assert!(
                    parsed.get(key).is_none_or(|v| v.is_null()),
                    "{form}: the revert clears {key}"
                );
            }
            assert_eq!(parsed["status"], "Draft");
        }
    }

    /// A legacy submission inserted from the file never enters as Queued.
    #[test]
    fn import_legacy_submission_insert_never_queues() {
        let dir = tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let profile = serde_json::to_value(branch_profile("Juana", TIN_X)).unwrap();
        let submissions = [serde_json::json!({
            "tin": TIN_X,
            "form_type": "2551Q",
            "period": "2024Q1",
            "status": "Queued",
            "form_data": {},
            "submitted_at": null,
            "filename": null,
            "created_at": null,
            "updated_at": "2024-01-01T00:00:00Z"
        })];
        let archive = dir.path().join("a.zip");
        write_archive(
            &archive,
            &[
                ("profile.json", profile.to_string()),
                (
                    "submissions.json",
                    serde_json::to_string(&submissions).unwrap(),
                ),
            ],
        );
        import_profile_data(&db, &archive).unwrap();
        let subs = db.list_submissions_for_tin(TIN_X).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].status, "Draft", "an import is never a send");
    }

    /// Both sides undated at the same rank is a tie: the device keeps its row.
    /// (The schema's updated_at is NOT NULL, so the case is reachable only in
    /// rows written before the constraint — the comparator is tested directly.)
    #[test]
    fn import_both_undated_same_rank_keeps_device() {
        assert!(!import_may_replace("Draft", None, "Draft", None));
        assert!(!import_timestamp_allows_draft(None, None));
        assert!(import_may_replace(
            "Draft",
            None,
            "Draft",
            Some("2024-01-01T00:00:00Z")
        ));
    }

    /// N5: a brand-new taxpayer's archive — profile, facts, forms lists and
    /// all — never says nothing was restored.
    #[test]
    fn import_new_taxpayer_counts_the_profile() {
        let dir = tempdir().unwrap();
        let source = Database::open_ephemeral().unwrap();
        source.save_profile(branch_profile("Juana", TIN_X)).unwrap();
        let archive = dir.path().join("a.zip");
        crate::export::export_profile_data(&source, TIN_X, &archive, None).unwrap();
        let db = Database::open_ephemeral().unwrap();
        let counts = import_profile_data(&db, &archive).unwrap();
        assert!(
            counts.restored >= 1,
            "a new taxpayer's profile counts as restored: {counts:?}"
        );
    }
}
