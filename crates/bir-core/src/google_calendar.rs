//! Google Calendar synchronization for profile-specific BIR filing deadlines.

use crate::calendar_rules::{DeadlineKind, DeadlinePeriod, DeadlineResolver};
use crate::db::{CalendarEventLink, Database, ProfileCalendarLink};
use crate::forms::{FilingStatus, FormDraftSummary};
use crate::profile::TaxpayerProfile;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use thiserror::Error;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
const CALENDAR_API: &str = "https://www.googleapis.com/calendar/v3";
const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar.app.created https://www.googleapis.com/auth/userinfo.email";
const KEYCHAIN_SERVICE: &str = "dev.goldcoders.ebirforms.google-calendar";
const KEYCHAIN_ACCOUNT: &str = "global-oauth";
const CONNECTED_EMAIL_SETTING: &str = "google_calendar_connected_email";
const FORM_SELECTION_SETTING_PREFIX: &str = "google_calendar_form_selection:";

#[derive(Debug, Error)]
#[error("Google Calendar resource was not found")]
struct CalendarResourceNotFound;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCalendarCredential {
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoogleCalendarConnection {
    pub configured: bool,
    pub connected_email: Option<String>,
}

impl GoogleCalendarConnection {
    /// The per-profile Calendar UI is useful only after the release contains
    /// Google OAuth credentials and the user has connected a Calendar account
    /// from Settings. Email-receipt OAuth is a separate integration and must
    /// not make the Calendar tab appear.
    pub fn profile_calendar_available(&self) -> bool {
        self.configured
            && self
                .connected_email
                .as_deref()
                .is_some_and(|email| !email.trim().is_empty())
    }
}

/// Which Forms Set entries feed a profile's Google calendar by default.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarFormPreset {
    /// Every active form in the yearly Forms Sets (current behavior).
    #[default]
    AllForms,
    /// Only forms whose official filing frequency recurs (monthly, quarterly,
    /// annual). Event-driven forms such as one-time registrations are left
    /// off the calendar unless manually included.
    RecurringOnly,
}

impl CalendarFormPreset {
    fn allows(self, form_code: &str) -> bool {
        match self {
            CalendarFormPreset::AllForms => true,
            CalendarFormPreset::RecurringOnly => crate::forms::registry::find_form(form_code)
                .map(|definition| {
                    !matches!(
                        definition.frequency,
                        crate::forms::FilingFrequency::OpenEnded
                    )
                })
                // Custom codes have no official frequency; fail closed under
                // the recurring preset and let a manual include rescue them.
                .unwrap_or(false),
        }
    }
}

/// Per-profile choice of which forms produce Google Calendar events.
///
/// The preset supplies the default; manual lists override it per form code.
/// An exclude always wins over an include.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarFormSelection {
    #[serde(default)]
    pub preset: CalendarFormPreset,
    #[serde(default)]
    pub manual_include: Vec<String>,
    #[serde(default)]
    pub manual_exclude: Vec<String>,
}

impl CalendarFormSelection {
    pub fn allows(&self, form_code: &str) -> bool {
        if self.manual_exclude.iter().any(|code| code == form_code) {
            return false;
        }
        if self.manual_include.iter().any(|code| code == form_code) {
            return true;
        }
        self.preset.allows(form_code)
    }

    pub fn is_overridden(&self, form_code: &str) -> bool {
        self.manual_include.iter().any(|code| code == form_code)
            || self.manual_exclude.iter().any(|code| code == form_code)
    }

    /// Flip one form's effective state, recording a manual override only when
    /// the desired state differs from what the preset already produces.
    pub fn toggle(&mut self, form_code: &str) {
        let turn_on = !self.allows(form_code);
        self.manual_include.retain(|code| code != form_code);
        self.manual_exclude.retain(|code| code != form_code);
        let preset_allows = self.preset.allows(form_code);
        if turn_on && !preset_allows {
            self.manual_include.push(form_code.to_string());
            self.manual_include.sort();
        } else if !turn_on && preset_allows {
            self.manual_exclude.push(form_code.to_string());
            self.manual_exclude.sort();
        }
    }

    /// Switch presets and drop overrides made redundant by the new default.
    pub fn set_preset(&mut self, preset: CalendarFormPreset) {
        self.preset = preset;
        self.manual_include.retain(|code| !preset.allows(code));
        self.manual_exclude.retain(|code| preset.allows(code));
    }

    pub fn clear_overrides(&mut self) {
        self.manual_include.clear();
        self.manual_exclude.clear();
    }
}

pub fn calendar_form_selection(db: &Database, tin: &str) -> CalendarFormSelection {
    db.get_setting(&format!("{FORM_SELECTION_SETTING_PREFIX}{tin}"))
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_calendar_form_selection(
    db: &Database,
    tin: &str,
    selection: &CalendarFormSelection,
) -> Result<(), anyhow::Error> {
    db.set_setting(
        &format!("{FORM_SELECTION_SETTING_PREFIX}{tin}"),
        &serde_json::to_string(selection)?,
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarSyncReport {
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
    pub unchanged: usize,
    pub excluded_undated: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesiredCalendarEvent {
    pub obligation_key: String,
    pub taxable_year: u16,
    pub form_code: String,
    pub period_label: String,
    pub content_hash: String,
    pub body: Value,
}

pub fn google_calendar_configuration() -> GoogleCalendarConnection {
    GoogleCalendarConnection {
        configured: google_oauth_is_configured(),
        connected_email: load_credentials().ok().map(|credentials| credentials.email),
    }
}

pub fn google_calendar_connection_from_db(db: &Database) -> GoogleCalendarConnection {
    GoogleCalendarConnection {
        configured: google_oauth_is_configured(),
        connected_email: db
            .get_setting(CONNECTED_EMAIL_SETTING)
            .ok()
            .flatten()
            .filter(|email| !email.trim().is_empty()),
    }
}

pub fn connect_google_calendar_account(db: Arc<Mutex<Database>>) -> Result<String, anyhow::Error> {
    let credentials = start_calendar_oauth_flow()?;
    save_credentials(&credentials)?;
    let persist_result = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock poisoned"))
        .and_then(|db| {
            db.set_setting(CONNECTED_EMAIL_SETTING, &credentials.email)
                .map_err(Into::into)
        });
    if let Err(error) = persist_result {
        let rollback = delete_credentials();
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "Failed to save the connected Google account ({error}); credential rollback also failed ({rollback_error})"
            )),
        };
    }
    Ok(credentials.email)
}

pub fn disconnect_google_calendar_account(db: Arc<Mutex<Database>>) -> Result<(), anyhow::Error> {
    let credentials = load_credentials().ok();
    delete_credentials()?;
    let delete_result = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock poisoned"))
        .and_then(|db| {
            db.delete_setting(CONNECTED_EMAIL_SETTING)
                .map_err(Into::into)
        });
    match delete_result {
        Ok(()) => Ok(()),
        Err(error) => {
            let rollback = credentials
                .as_ref()
                .map(save_credentials)
                .transpose()
                .map(|_| ());
            match rollback {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(anyhow::anyhow!(
                    "Failed to clear the connected Google account ({error}); credential rollback also failed ({rollback_error})"
                )),
            }
        }
    }
}

pub fn default_profile_calendar_name(profile: &TaxpayerProfile) -> String {
    let tin = profile.tin.full();
    let suffix = tin
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("eBIRForms - {} (...{})", profile.full_name, suffix)
}

pub fn create_profile_calendar(
    db: Arc<Mutex<Database>>,
    tin: &str,
    requested_name: Option<&str>,
) -> Result<CalendarSyncReport, anyhow::Error> {
    let profile = {
        let db = db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
        if db.get_profile_calendar_link(tin)?.is_some() {
            return Err(anyhow::anyhow!(
                "This profile already has a linked Google Calendar"
            ));
        }
        db.get_profile(tin)?
            .ok_or_else(|| anyhow::anyhow!("Profile not found"))?
    };
    let calendar_name = requested_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_profile_calendar_name(&profile));

    let mut client = GoogleCalendarClient::new()?;
    let calendar_id = client.create_calendar(&calendar_name)?;
    let link = ProfileCalendarLink {
        profile_tin: tin.to_string(),
        google_calendar_id: calendar_id,
        calendar_name,
        enabled: true,
        last_synced_at: None,
        last_error: None,
    };
    let save_result = {
        let db = db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
        db.save_profile_calendar_link(&link)
    };
    if let Err(error) = save_result {
        let rollback = client.delete_calendar(&link.google_calendar_id);
        return match rollback {
            Ok(()) => Err(error.into()),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "Failed to save the calendar link ({error}); also failed to remove the newly created Google calendar ({rollback_error})"
            )),
        };
    }
    match sync_profile_calendar(db.clone(), tin) {
        Ok(report) => Ok(report),
        Err(error) => {
            let rollback_remote = client.delete_calendar(&link.google_calendar_id);
            let rollback_local = db
                .lock()
                .map_err(|_| anyhow::anyhow!("Database lock poisoned"))
                .and_then(|db| db.delete_profile_calendar_link(tin).map_err(Into::into));
            match (rollback_remote, rollback_local) {
                (Ok(()), Ok(())) => Err(error),
                (remote, local) => Err(anyhow::anyhow!(
                    "Initial calendar sync failed ({error}); rollback was incomplete (remote: {}; local: {})",
                    rollback_result_label(&remote),
                    rollback_result_label(&local)
                )),
            }
        }
    }
}

pub fn unlink_profile_calendar(db: Arc<Mutex<Database>>, tin: &str) -> Result<(), anyhow::Error> {
    let db = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
    db.delete_profile_calendar_link(tin)?;
    Ok(())
}

pub fn delete_profile_calendar(db: Arc<Mutex<Database>>, tin: &str) -> Result<(), anyhow::Error> {
    let link = {
        let db = db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
        db.get_profile_calendar_link(tin)?
            .ok_or_else(|| anyhow::anyhow!("No linked Google Calendar"))?
    };
    let mut client = GoogleCalendarClient::new()?;
    if let Err(error) = client.delete_calendar(&link.google_calendar_id)
        && !is_resource_not_found(&error)
    {
        return Err(error);
    }
    unlink_profile_calendar(db, tin)
}

pub fn sync_all_profile_calendars(
    db: Arc<Mutex<Database>>,
) -> Vec<(String, Result<CalendarSyncReport, String>)> {
    let tins = match db.lock() {
        Ok(db) => db
            .list_profile_calendar_links()
            .unwrap_or_default()
            .into_iter()
            .map(|link| link.profile_tin)
            .collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };

    tins.into_iter()
        .map(|tin| {
            let result = sync_profile_calendar(db.clone(), &tin).map_err(|error| error.to_string());
            (tin, result)
        })
        .collect()
}

pub fn sync_profile_calendar(
    db: Arc<Mutex<Database>>,
    tin: &str,
) -> Result<CalendarSyncReport, anyhow::Error> {
    let (link, desired, existing, excluded_undated) = {
        let db_guard = db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
        let link = db_guard
            .get_profile_calendar_link(tin)?
            .ok_or_else(|| anyhow::anyhow!("No linked Google Calendar"))?;
        let profile = db_guard
            .get_profile(tin)?
            .ok_or_else(|| anyhow::anyhow!("Profile not found"))?;
        let (desired, excluded) = desired_events_for_sync(&db_guard, &profile)?;
        let existing = db_guard.list_calendar_event_links(tin)?;
        (link, desired, existing, excluded)
    };

    let result = sync_snapshot(&db, &link, desired, existing, excluded_undated);
    if let Ok(db_guard) = db.lock() {
        let error = result.as_ref().err().map(|error| error.to_string());
        let _ = db_guard.set_profile_calendar_sync_result(tin, error.as_deref());
    }
    result
}

fn desired_events_for_sync(
    db: &Database,
    profile: &TaxpayerProfile,
) -> Result<(Vec<DesiredCalendarEvent>, usize), anyhow::Error> {
    if profile.is_archived {
        Ok((Vec::new(), 0))
    } else {
        build_desired_events(db, profile)
    }
}

fn sync_snapshot(
    db: &Arc<Mutex<Database>>,
    link: &ProfileCalendarLink,
    desired: Vec<DesiredCalendarEvent>,
    existing: Vec<CalendarEventLink>,
    excluded_undated: usize,
) -> Result<CalendarSyncReport, anyhow::Error> {
    let mut client = GoogleCalendarClient::new()?;
    let desired_by_key: BTreeMap<_, _> = desired
        .into_iter()
        .map(|event| (event.obligation_key.clone(), event))
        .collect();
    let existing_by_key: BTreeMap<_, _> = existing
        .into_iter()
        .map(|event| (event.obligation_key.clone(), event))
        .collect();
    let mut report = CalendarSyncReport {
        inserted: 0,
        updated: 0,
        deleted: 0,
        unchanged: 0,
        excluded_undated,
    };

    for (key, event) in &desired_by_key {
        if let Some(stored) = existing_by_key.get(key) {
            if stored.content_hash == event.content_hash {
                report.unchanged += 1;
                continue;
            }
            match client.update_event(
                &link.google_calendar_id,
                &stored.google_event_id,
                &event.body,
            ) {
                Ok(()) => {
                    save_event_mapping(db, &link.profile_tin, event, &stored.google_event_id)?;
                    report.updated += 1;
                }
                Err(error) if is_resource_not_found(&error) => {
                    let google_event_id =
                        client.create_event(&link.google_calendar_id, &event.body)?;
                    save_event_mapping(db, &link.profile_tin, event, &google_event_id)?;
                    report.updated += 1;
                }
                Err(error) => return Err(error),
            }
        } else {
            let google_event_id = client.create_event(&link.google_calendar_id, &event.body)?;
            save_event_mapping(db, &link.profile_tin, event, &google_event_id)?;
            report.inserted += 1;
        }
    }

    for (key, stored) in &existing_by_key {
        if desired_by_key.contains_key(key) {
            continue;
        }
        if let Err(error) = client.delete_event(&link.google_calendar_id, &stored.google_event_id)
            && !is_resource_not_found(&error)
        {
            return Err(error);
        }
        let db = db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
        db.delete_calendar_event_link(&link.profile_tin, key)?;
        report.deleted += 1;
    }

    Ok(report)
}

fn save_event_mapping(
    db: &Arc<Mutex<Database>>,
    tin: &str,
    event: &DesiredCalendarEvent,
    google_event_id: &str,
) -> Result<(), anyhow::Error> {
    let db = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
    db.save_calendar_event_link(&CalendarEventLink {
        profile_tin: tin.to_string(),
        obligation_key: event.obligation_key.clone(),
        google_event_id: google_event_id.to_string(),
        content_hash: event.content_hash.clone(),
        taxable_year: event.taxable_year,
        form_code: event.form_code.clone(),
        period_label: event.period_label.clone(),
    })?;
    Ok(())
}

pub fn build_desired_events(
    db: &Database,
    profile: &TaxpayerProfile,
) -> Result<(Vec<DesiredCalendarEvent>, usize), anyhow::Error> {
    let mut desired = Vec::new();
    let mut excluded_undated = 0;
    let tin = profile.tin.full();
    let years = db.list_forms_set_years(&tin)?;
    let global_overrides = db.get_deadline_overrides();
    let selection = calendar_form_selection(db, &tin);

    for year in years {
        // The user's calendar form selection decides which Forms Set entries
        // become events; deselected forms also stay out of the undated count.
        let form_codes = db
            .active_form_codes_for_year(&tin, year)?
            .into_iter()
            .filter(|code| selection.allows(code))
            .collect::<Vec<_>>();
        let mut overrides = global_overrides.clone();
        overrides.extend(crate::integration::profile_deadline_overrides_for_year(
            profile, year,
        ));
        let summaries = db.list_draft_summaries(&tin, year).unwrap_or_default();
        let deadlines = DeadlineResolver::deadlines_for_forms(&form_codes, year as i32, &overrides);
        let dated_codes = deadlines
            .iter()
            .map(|deadline| deadline.form_code.clone())
            .collect::<BTreeSet<_>>();
        excluded_undated += form_codes
            .iter()
            .filter(|code| !dated_codes.contains(code.as_str()))
            .count();

        for deadline in deadlines {
            if !crate::integration::deadline_applies_to_profile(profile, &deadline) {
                continue;
            }
            let DeadlineKind::Dated {
                original_deadline,
                final_deadline,
            } = deadline.deadline
            else {
                excluded_undated += 1;
                continue;
            };
            let paid = deadline_is_paid(&deadline.period, &deadline.form_code, &summaries);
            let period_key = period_key(&deadline.period);
            let obligation_key = format!("{}:{}:{}", year, deadline.form_code, period_key);
            let period_label = concise_period_label(&deadline.period);
            let title_prefix = if paid { "[Filed] " } else { "" };
            let masked_tin = mask_tin(&tin);
            let mut description = format!(
                "Tax profile: {} ({})\nForm: {} - {}\n{}\nFinal deadline: {}\nStatus: {}",
                profile.full_name,
                masked_tin,
                deadline.display_form_no,
                deadline.form_name,
                deadline.period.label(),
                final_deadline,
                deadline.status.label(),
            );
            if original_deadline != final_deadline {
                description.push_str(&format!("\nOriginal deadline: {original_deadline}"));
            }
            if let Some(source) = &deadline.source_reference {
                description.push_str(&format!("\nSource: {source}"));
            }
            if !deadline.description.is_empty() {
                description.push_str(&format!("\n\n{}", deadline.description));
            }

            let reminders = if paid {
                json!({"useDefault": false, "overrides": []})
            } else {
                json!({
                    "useDefault": false,
                    "overrides": [
                        {"method": "email", "minutes": 10080},
                        {"method": "email", "minutes": 1440}
                    ]
                })
            };
            let body = json!({
                "summary": format!("{title_prefix}[BIR] {} - {}", deadline.display_form_no, period_label),
                "description": description,
                "start": {"date": final_deadline.format("%Y-%m-%d").to_string()},
                "end": {"date": (final_deadline + Duration::days(1)).format("%Y-%m-%d").to_string()},
                "transparency": "transparent",
                "reminders": reminders,
                "extendedProperties": {
                    "private": {
                        "ebirformsManaged": "true",
                        "profileKey": profile_key(&tin),
                        "obligationKey": obligation_key
                    }
                }
            });
            let content_hash = hex::encode(Sha256::digest(serde_json::to_vec(&body)?));
            desired.push(DesiredCalendarEvent {
                obligation_key,
                taxable_year: year,
                form_code: deadline.form_code,
                period_label,
                content_hash,
                body,
            });
        }
    }
    desired.sort_by(|a, b| a.obligation_key.cmp(&b.obligation_key));
    Ok((desired, excluded_undated))
}

fn deadline_is_paid(
    period: &DeadlinePeriod,
    form_code: &str,
    summaries: &[FormDraftSummary],
) -> bool {
    summaries.iter().any(|summary| {
        summary.form_code == form_code
            && summary.status == FilingStatus::Paid
            && match period {
                DeadlinePeriod::Monthly { month, .. } => summary.month == Some(*month),
                DeadlinePeriod::Quarterly { quarter, .. } => summary.quarter == Some(*quarter),
                DeadlinePeriod::Annual { .. } => true,
                DeadlinePeriod::EventBased => false,
            }
    })
}

fn period_key(period: &DeadlinePeriod) -> String {
    match period {
        DeadlinePeriod::Monthly { month, .. } => format!("m{month:02}"),
        DeadlinePeriod::Quarterly { quarter, .. } => format!("q{quarter}"),
        DeadlinePeriod::Annual { .. } => "annual".to_string(),
        DeadlinePeriod::EventBased => "event".to_string(),
    }
}

fn concise_period_label(period: &DeadlinePeriod) -> String {
    match period {
        DeadlinePeriod::Monthly {
            taxable_year,
            month,
        } => format!("{taxable_year}-{month:02}"),
        DeadlinePeriod::Quarterly {
            taxable_year,
            quarter,
        } => format!("{taxable_year} Q{quarter}"),
        DeadlinePeriod::Annual { taxable_year } => format!("{taxable_year} Annual"),
        DeadlinePeriod::EventBased => "Event-based".to_string(),
    }
}

fn mask_tin(tin: &str) -> String {
    let suffix = tin
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("TIN ending {suffix}")
}

fn profile_key(tin: &str) -> String {
    hex::encode(Sha256::digest(tin.as_bytes()))
}

struct GoogleCalendarClient {
    http: reqwest::blocking::Client,
    credentials: GoogleCalendarCredential,
}

impl GoogleCalendarClient {
    fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {
            http: reqwest::blocking::Client::new(),
            credentials: load_credentials()?,
        })
    }

    fn create_calendar(&mut self, name: &str) -> Result<String, anyhow::Error> {
        let body = json!({
            "summary": name,
            "description": "BIR filing deadlines managed by eBIRForms.",
            "timeZone": "Asia/Manila"
        });
        let response = self.send("POST", &format!("{CALENDAR_API}/calendars"), Some(&body))?;
        response
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Google did not return a calendar ID"))
    }

    fn delete_calendar(&mut self, calendar_id: &str) -> Result<(), anyhow::Error> {
        self.send_empty(
            "DELETE",
            &format!(
                "{CALENDAR_API}/calendars/{}",
                urlencoding::encode(calendar_id)
            ),
        )
    }

    fn create_event(&mut self, calendar_id: &str, body: &Value) -> Result<String, anyhow::Error> {
        let response = self.send(
            "POST",
            &format!(
                "{CALENDAR_API}/calendars/{}/events",
                urlencoding::encode(calendar_id)
            ),
            Some(body),
        )?;
        response
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Google did not return an event ID"))
    }

    fn update_event(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        body: &Value,
    ) -> Result<(), anyhow::Error> {
        self.send(
            "PUT",
            &format!(
                "{CALENDAR_API}/calendars/{}/events/{}",
                urlencoding::encode(calendar_id),
                urlencoding::encode(event_id)
            ),
            Some(body),
        )?;
        Ok(())
    }

    fn delete_event(&mut self, calendar_id: &str, event_id: &str) -> Result<(), anyhow::Error> {
        self.send_empty(
            "DELETE",
            &format!(
                "{CALENDAR_API}/calendars/{}/events/{}",
                urlencoding::encode(calendar_id),
                urlencoding::encode(event_id)
            ),
        )
    }

    fn send(
        &mut self,
        method: &str,
        url: &str,
        body: Option<&Value>,
    ) -> Result<Value, anyhow::Error> {
        let text = self.send_text(method, url, body)?;
        if text.trim().is_empty() {
            Ok(Value::Null)
        } else {
            Ok(serde_json::from_str(&text)?)
        }
    }

    fn send_empty(&mut self, method: &str, url: &str) -> Result<(), anyhow::Error> {
        self.send_text(method, url, None)?;
        Ok(())
    }

    fn send_text(
        &mut self,
        method: &str,
        url: &str,
        body: Option<&Value>,
    ) -> Result<String, anyhow::Error> {
        if self.credentials.expires_at_unix <= Utc::now().timestamp() + 60 {
            self.refresh()?;
        }
        let mut retried = false;
        loop {
            let method = reqwest::Method::from_bytes(method.as_bytes())?;
            let mut request = self
                .http
                .request(method, url)
                .bearer_auth(&self.credentials.access_token);
            if let Some(body) = body {
                request = request
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(serde_json::to_vec(body)?);
            }
            let response = request.send()?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED && !retried {
                self.refresh()?;
                retried = true;
                continue;
            }
            let status = response.status();
            let text = response.text()?;
            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(CalendarResourceNotFound.into());
            }
            if !status.is_success() {
                return Err(anyhow::anyhow!(
                    "Google Calendar API error ({status}): {text}"
                ));
            }
            return Ok(text);
        }
    }

    fn refresh(&mut self) -> Result<(), anyhow::Error> {
        let client_id = google_client_id()?;
        let client_secret = google_client_secret()?;
        let response = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", self.credentials.refresh_token.as_str()),
            ])
            .send()?;
        let status = response.status();
        let text = response.text()?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Google token refresh failed ({status}): {text}"
            ));
        }
        let body: Value = serde_json::from_str(&text)?;
        self.credentials.access_token = body
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Token refresh omitted access_token"))?
            .to_string();
        self.credentials.expires_at_unix = Utc::now().timestamp()
            + body
                .get("expires_in")
                .and_then(Value::as_i64)
                .unwrap_or(3600);
        save_credentials(&self.credentials)?;
        Ok(())
    }
}

fn start_calendar_oauth_flow() -> Result<GoogleCalendarCredential, anyhow::Error> {
    use crate::email::oauth_server;
    use data_encoding::BASE64URL_NOPAD;
    use rand::RngExt;

    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    let verifier = BASE64URL_NOPAD.encode(&bytes);
    let challenge = BASE64URL_NOPAD.encode(&Sha256::digest(verifier.as_bytes()));
    let state_bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    let state = BASE64URL_NOPAD.encode(&state_bytes);
    let (port, rx) = oauth_server::start_callback_server(state.clone())?;
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let client_id = google_client_id()?;
    let auth_url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(CALENDAR_SCOPE),
        urlencoding::encode(&challenge),
        urlencoding::encode(&state),
    );
    open::that(&auth_url)?;
    let code = rx
        .recv()
        .map_err(|_| anyhow::anyhow!("OAuth callback was cancelled"))?
        .map_err(anyhow::Error::msg)?;
    let client_secret = google_client_secret()?;
    let http = reqwest::blocking::Client::new();
    let response = http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ])
        .send()?;
    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        return Err(anyhow::anyhow!("Google OAuth failed ({status}): {text}"));
    }
    let tokens: Value = serde_json::from_str(&text)?;
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Google OAuth omitted access_token"))?
        .to_string();
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Google OAuth omitted refresh_token"))?
        .to_string();
    let userinfo_text = http
        .get(USERINFO_URL)
        .bearer_auth(&access_token)
        .send()?
        .error_for_status()?
        .text()?;
    let userinfo: Value = serde_json::from_str(&userinfo_text)?;
    let email = userinfo
        .get("email")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Google userinfo omitted email"))?
        .to_string();
    Ok(GoogleCalendarCredential {
        email,
        access_token,
        refresh_token,
        expires_at_unix: Utc::now().timestamp()
            + tokens
                .get("expires_in")
                .and_then(Value::as_i64)
                .unwrap_or(3600),
    })
}

fn google_client_id() -> Result<String, anyhow::Error> {
    std::env::var("GOOGLE_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("GOOGLE_CLIENT_ID").map(String::from))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_ID is not configured"))
}

fn google_client_secret() -> Result<String, anyhow::Error> {
    std::env::var("GOOGLE_CLIENT_SECRET")
        .ok()
        .or_else(|| option_env!("GOOGLE_CLIENT_SECRET").map(String::from))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("GOOGLE_CLIENT_SECRET is not configured"))
}

fn google_oauth_is_configured() -> bool {
    google_client_id().is_ok() && google_client_secret().is_ok()
}

fn is_resource_not_found(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CalendarResourceNotFound>().is_some()
}

fn rollback_result_label<T>(result: &Result<T, anyhow::Error>) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => error.to_string(),
    }
}

fn save_credentials(credentials: &GoogleCalendarCredential) -> Result<(), anyhow::Error> {
    let serialized = serde_json::to_string(credentials)?;
    keychain_set(&serialized)
}

fn load_credentials() -> Result<GoogleCalendarCredential, anyhow::Error> {
    Ok(serde_json::from_str(&keychain_get()?)?)
}

fn delete_credentials() -> Result<(), anyhow::Error> {
    keychain_delete()
}

#[cfg(target_os = "macos")]
fn keychain_set(value: &str) -> Result<(), anyhow::Error> {
    let output = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
            value,
            "-U",
        ])
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to save Google Calendar credentials: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(target_os = "macos")]
fn keychain_get() -> Result<String, anyhow::Error> {
    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
        ])
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err(anyhow::anyhow!("Google Calendar account is not connected"))
    }
}

#[cfg(target_os = "macos")]
fn keychain_delete() -> Result<(), anyhow::Error> {
    let output = std::process::Command::new("security")
        .args([
            "delete-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
        ])
        .output()?;
    if output.status.success()
        || String::from_utf8_lossy(&output.stderr).contains("could not be found")
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to remove Google Calendar credentials: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_set(value: &str) -> Result<(), anyhow::Error> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?.set_password(value)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn keychain_get() -> Result<String, anyhow::Error> {
    Ok(keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?.get_password()?)
}

#[cfg(not(target_os = "macos"))]
fn keychain_delete() -> Result<(), anyhow::Error> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar_rules::DeadlinePeriod;
    use crate::forms::{FilingPeriod, FormSetSource, PerYearFormsSet};
    use crate::naming::Tin;
    use crate::profile::{TaxpayerProfile, TaxpayerType};
    use tempfile::NamedTempFile;

    #[test]
    fn default_selection_allows_every_form() {
        let selection = CalendarFormSelection::default();
        assert!(selection.allows("2551Q"));
        assert!(selection.allows("0605"));
        assert!(selection.allows("CUSTOM_FORM"));
    }

    #[test]
    fn recurring_preset_drops_event_driven_and_unknown_forms() {
        let selection = CalendarFormSelection {
            preset: CalendarFormPreset::RecurringOnly,
            ..Default::default()
        };
        assert!(selection.allows("2551Q"));
        assert!(selection.allows("1601C"));
        // 0605 is an event-driven payment form (OpenEnded frequency).
        assert!(!selection.allows("0605"));
        // Custom codes have no official frequency: fail closed.
        assert!(!selection.allows("CUSTOM_FORM"));
    }

    #[test]
    fn manual_overrides_beat_the_preset_and_exclude_beats_include() {
        let mut selection = CalendarFormSelection {
            preset: CalendarFormPreset::RecurringOnly,
            ..Default::default()
        };
        selection.toggle("0605"); // preset denies -> becomes manual include
        assert!(selection.allows("0605"));
        assert!(selection.is_overridden("0605"));

        selection.toggle("2551Q"); // preset allows -> becomes manual exclude
        assert!(!selection.allows("2551Q"));

        // Toggling back removes the override instead of stacking lists.
        selection.toggle("0605");
        assert!(!selection.allows("0605"));
        assert!(!selection.is_overridden("0605"));

        // Exclude wins if both lists ever contain the same code.
        let conflicted = CalendarFormSelection {
            preset: CalendarFormPreset::AllForms,
            manual_include: vec!["2550Q".to_string()],
            manual_exclude: vec!["2550Q".to_string()],
        };
        assert!(!conflicted.allows("2550Q"));
    }

    #[test]
    fn switching_presets_drops_redundant_overrides() {
        let mut selection = CalendarFormSelection {
            preset: CalendarFormPreset::RecurringOnly,
            manual_include: vec!["0605".to_string()],
            manual_exclude: vec!["2551Q".to_string()],
        };
        // Under AllForms the 0605 include is redundant; the 2551Q exclude
        // still differs from the preset and must survive.
        selection.set_preset(CalendarFormPreset::AllForms);
        assert!(selection.manual_include.is_empty());
        assert_eq!(selection.manual_exclude, vec!["2551Q".to_string()]);
        assert!(selection.allows("0605"));
        assert!(!selection.allows("2551Q"));
    }

    #[test]
    fn selection_round_trips_through_settings() {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        let tin = "123456789000";

        // Absent -> default.
        assert_eq!(
            calendar_form_selection(&db, tin),
            CalendarFormSelection::default()
        );

        let mut selection = CalendarFormSelection {
            preset: CalendarFormPreset::RecurringOnly,
            ..Default::default()
        };
        selection.toggle("0605");
        save_calendar_form_selection(&db, tin, &selection).unwrap();
        assert_eq!(calendar_form_selection(&db, tin), selection);

        // Corrupt JSON falls back to default instead of erroring.
        db.set_setting(&format!("{FORM_SELECTION_SETTING_PREFIX}{tin}"), "not json")
            .unwrap();
        assert_eq!(
            calendar_form_selection(&db, tin),
            CalendarFormSelection::default()
        );
    }

    fn test_profile() -> TaxpayerProfile {
        let mut profile = TaxpayerProfile {
            id: None,
            full_name: "Calendar Test Corp".into(),
            tin: Tin {
                segment1: "123".into(),
                segment2: "456".into(),
                segment3: "789".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Software".into(),
            registered_address: "Quezon City".into(),
            zip_code: "1100".into(),
            phone: "09170000000".into(),
            email: "calendar@example.com".into(),
            default_form_type: "0619E".into(),
            taxpayer_type: TaxpayerType::Corporation,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: true,
            atc_codes: Vec::new(),
            excise_tax_categories: Vec::new(),
            tax_elections: Vec::new(),
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: Default::default(),
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: true,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
            profile_versions: Vec::new(),
            compliance_source_mode: Default::default(),
            per_year_forms: Default::default(),
        };
        profile.ensure_profile_version_ledger();
        profile.per_year_forms.insert(
            2026,
            PerYearFormsSet::from_codes(2026, ["0619E"], FormSetSource::Manual),
        );
        profile
    }

    fn test_database() -> (NamedTempFile, Database) {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        let forms = PerYearFormsSet::from_codes(2026, ["0619E"], FormSetSource::Manual);
        db.save_per_year_forms("123456789000", 2026, &forms)
            .unwrap();
        (file, db)
    }

    #[test]
    fn period_keys_are_stable() {
        assert_eq!(
            period_key(&DeadlinePeriod::Monthly {
                taxable_year: 2026,
                month: 2
            }),
            "m02"
        );
        assert_eq!(
            period_key(&DeadlinePeriod::Quarterly {
                taxable_year: 2026,
                quarter: 3
            }),
            "q3"
        );
        assert_eq!(
            period_key(&DeadlinePeriod::Annual { taxable_year: 2026 }),
            "annual"
        );
    }

    #[test]
    fn profile_calendar_requires_build_configuration_and_connected_account() {
        let configured_and_connected = GoogleCalendarConnection {
            configured: true,
            connected_email: Some("calendar@example.com".into()),
        };
        assert!(configured_and_connected.profile_calendar_available());

        let not_built_with_oauth = GoogleCalendarConnection {
            configured: false,
            connected_email: Some("calendar@example.com".into()),
        };
        assert!(!not_built_with_oauth.profile_calendar_available());

        let not_connected = GoogleCalendarConnection {
            configured: true,
            connected_email: None,
        };
        assert!(!not_connected.profile_calendar_available());

        let blank_connection_marker = GoogleCalendarConnection {
            configured: true,
            connected_email: Some("   ".into()),
        };
        assert!(!blank_connection_marker.profile_calendar_available());
    }

    #[test]
    fn mask_tin_only_exposes_suffix() {
        assert_eq!(mask_tin("123456789000"), "TIN ending 9000");
        assert!(!mask_tin("123456789000").contains("12345678"));
    }

    #[test]
    fn build_desired_events_uses_authoritative_forms_set() {
        let (_file, db) = test_database();

        let (events, excluded) = build_desired_events(&db, &test_profile()).unwrap();

        assert_eq!(events.len(), 12);
        assert_eq!(excluded, 0);
        assert!(events.iter().all(|event| event.form_code == "0619E"));
    }

    #[test]
    fn paid_filing_disables_reminders_and_marks_event_filed() {
        let (_file, db) = test_database();
        db.save_form_draft_v2(
            "123456789000",
            "0619E",
            2026,
            &FilingPeriod::Monthly(1),
            &FilingStatus::Paid,
            &json!({"status": "paid"}),
        )
        .unwrap();

        let (events, _) = build_desired_events(&db, &test_profile()).unwrap();
        let january = events
            .iter()
            .find(|event| event.obligation_key.ends_with(":m01"))
            .unwrap();

        assert!(
            january.body["summary"]
                .as_str()
                .is_some_and(|summary| summary.starts_with("[Filed]"))
        );
        assert_eq!(
            january.body["reminders"]["overrides"],
            Value::Array(Vec::new())
        );
    }

    #[test]
    fn archived_profile_has_no_desired_events() {
        let (_file, db) = test_database();
        let mut profile = test_profile();
        profile.is_archived = true;

        let (events, excluded) = desired_events_for_sync(&db, &profile).unwrap();

        assert!(events.is_empty());
        assert_eq!(excluded, 0);
    }

    #[test]
    fn resource_not_found_error_is_recognized_through_anyhow() {
        let error = anyhow::Error::new(CalendarResourceNotFound);

        assert!(is_resource_not_found(&error));
    }
}
