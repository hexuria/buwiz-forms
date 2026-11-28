//! Shared IMAP fetch + parse pipeline.
//!
//! Both App Password and Google OAuth2 authenticate, then hand off to this
//! module for the actual search → fetch → parse → confirm workflow.

use crate::db::{Database, SubmissionReceipt};
use crate::lock_timing;
use crate::profile::{EmailAuthMethod, TaxpayerProfile, inbox_emails_match};
use crate::receipt::parse_bir_receipt_email;
use chrono::Datelike;

use super::auth_oauth::{GOOGLE_NOT_IN_THIS_VERSION, GoogleOAuthAuth, google_sign_in_available};
use super::auth_password::AppPasswordAuth;
use super::inbox::{plan_email_poll_for_address, resolve_inbox_fetch_profile};

/// How long one read or write on the IMAP connection may wait. A stalled server ends the poll with an error
/// instead of holding the poll (and the database handle it was given) for ever.
const IMAP_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// The TCP connection to an IMAP server, with [`IMAP_IO_TIMEOUT`] on connect, reads and writes, before TLS.
fn imap_tcp_stream(host: &str, port: u16) -> Result<std::net::TcpStream, anyhow::Error> {
    use std::net::ToSocketAddrs;
    let mut last = None;
    for address in (host, port).to_socket_addrs()? {
        match std::net::TcpStream::connect_timeout(&address, IMAP_IO_TIMEOUT) {
            Ok(stream) => {
                stream.set_read_timeout(Some(IMAP_IO_TIMEOUT))?;
                stream.set_write_timeout(Some(IMAP_IO_TIMEOUT))?;
                return Ok(stream);
            }
            Err(error) => last = Some(error),
        }
    }
    Err(match last {
        Some(error) => error.into(),
        None => anyhow::anyhow!("{host} has no address"),
    })
}

/// What `imap::connect` does (TCP, TLS, the greeting), on a stream with timeouts.
fn imap_connect(
    host: &str,
    tls: &native_tls::TlsConnector,
) -> Result<imap::Client<native_tls::TlsStream<std::net::TcpStream>>, anyhow::Error> {
    let stream = imap_tcp_stream(host, 993)?;
    let tls_stream = tls
        .connect(host, stream)
        .map_err(|error| anyhow::anyhow!("TLS to {host}: {error}"))?;
    let mut client = imap::Client::new(tls_stream);
    client.read_greeting()?;
    Ok(client)
}

/// Trait that both auth backends implement.
pub trait ImapAuthenticator {
    fn authenticate(
        &self,
        client: imap::Client<native_tls::TlsStream<std::net::TcpStream>>,
    ) -> Result<
        (
            imap::Session<native_tls::TlsStream<std::net::TcpStream>>,
            Option<String>,
        ),
        anyhow::Error,
    >;

    /// The IMAP server hostname to connect to.
    fn host(&self) -> &str;
}

/// Fetch and process BIR confirmation emails for the given profile.
///
/// Automatically selects the correct auth strategy based on `email_auth_method`.
/// When several profiles share the inbox, Google OAuth credentials come from
/// the inbox-keyed grant (or a sibling with a refresh token), not list order.
pub fn fetch_and_process_emails(
    profile: &TaxpayerProfile,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    fetch_and_process_emails_until(profile, db, &|| false)
}

/// [`fetch_and_process_emails`] that stops between messages once `stop` says so.
fn fetch_and_process_emails_until(
    profile: &TaxpayerProfile,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    if !profile.is_email_tracking_active() {
        return Ok(vec![]);
    }

    let fetch_profile = overlay_shared_inbox_credentials(profile, &db);
    fetch_with_resolved_profile(&fetch_profile, db, stop)
}

fn overlay_shared_inbox_credentials(
    profile: &TaxpayerProfile,
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
) -> TaxpayerProfile {
    let Ok(db_guard) = lock_timing::lock(db) else {
        return profile.clone();
    };
    match resolve_inbox_fetch_profile(&db_guard, profile.inbox_email()) {
        Ok(Some(resolved)) => resolved,
        _ => profile.clone(),
    }
}

fn fetch_with_resolved_profile(
    profile: &TaxpayerProfile,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    let email = profile.inbox_email();

    // Build the right authenticator
    let (authenticator, host): (Box<dyn ImapAuthenticator>, String) = match profile
        .email_auth_method
    {
        EmailAuthMethod::AppPassword => {
            let host = profile
                .imap_host
                .as_deref()
                .unwrap_or("imap.gmail.com")
                .to_string();
            let auth =
                AppPasswordAuth::new(email, profile.imap_app_password.as_deref().unwrap_or(""))?;
            (Box::new(auth), host)
        }
        EmailAuthMethod::GoogleOAuth => {
            let access = profile.oauth_access_token.as_deref().unwrap_or("");
            let refresh = profile.oauth_refresh_token.as_deref().unwrap_or("");
            let auth = GoogleOAuthAuth::new(email, access, refresh)?;
            let host = "imap.gmail.com".to_string();
            (Box::new(auth), host)
        }
    };

    fetch_with_auth(authenticator.as_ref(), &host, db, profile, stop)
}

/// Test that a connection can be established and authenticated.
/// Returns `Ok(())` on success or an error describing the failure.
pub fn test_connection(profile: &TaxpayerProfile) -> Result<Option<String>, anyhow::Error> {
    let email = profile.inbox_email();

    let (authenticator, host): (Box<dyn ImapAuthenticator>, String) = match profile
        .email_auth_method
    {
        EmailAuthMethod::AppPassword => {
            let host = profile
                .imap_host
                .as_deref()
                .unwrap_or("imap.gmail.com")
                .to_string();
            let auth =
                AppPasswordAuth::new(email, profile.imap_app_password.as_deref().unwrap_or(""))?;
            (Box::new(auth), host)
        }
        EmailAuthMethod::GoogleOAuth => {
            let access = profile.oauth_access_token.as_deref().unwrap_or("");
            let refresh = profile.oauth_refresh_token.as_deref().unwrap_or("");
            let auth = GoogleOAuthAuth::new(email, access, refresh)?;
            let host = "imap.gmail.com".to_string();
            (Box::new(auth), host)
        }
    };

    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap_connect(&host, &tls)?;
    let (mut session, new_access_token) = authenticator.authenticate(client)?;
    session.select("INBOX")?;
    session.logout()?;
    Ok(new_access_token)
}

// ── Private ──────────────────────────────────────────────────────────────────

fn fetch_with_auth(
    auth: &dyn ImapAuthenticator,
    host: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
    profile: &TaxpayerProfile,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap_connect(host, &tls)?;
    let (mut session, new_access_token) = auth.authenticate(client)?;

    // Save the new access token if it was refreshed. Fan-out so siblings and
    // the inbox-keyed row do not keep a stale access token beside a live refresh.
    if let Some(token) = new_access_token
        && let Ok(db_guard) = lock_timing::lock(&db)
    {
        let _ = db_guard.update_inbox_oauth_access_token(profile.inbox_email(), &token);
    }

    session.select("INBOX")?;

    // Search ALL BIR confirmation emails from the last 30 days.
    //
    // IMPORTANT: We intentionally do NOT use `UNSEEN` here. If the user reads
    // a confirmation email on their phone or in Gmail web before our background
    // poller runs, the `UNSEEN` filter would silently skip it, causing the
    // draft to stay stuck in "Submitted" forever. Instead, we search ALL
    // matching emails and rely on the `submission_receipts` table's
    // `UNIQUE(filename)` constraint for deduplication — processing the same
    // email twice is harmless (ON CONFLICT DO UPDATE is a no-op for identical data).
    let since_date = {
        let now = chrono::Utc::now().naive_utc().date();
        let since = now - chrono::Duration::days(30);
        since.format("%d-%b-%Y").to_string()
    };
    let search_query = format!("FROM \"ebirforms-noreply@bir.gov.ph\" SINCE {}", since_date);
    let seqs = session.search(&search_query)?;

    let mut processed = Vec::new();

    for seq in seqs {
        if stop() {
            tracing::info!("Email polling stopped early: the queue worker is ending");
            break;
        }
        let messages = session.fetch(seq.to_string(), "RFC822")?;
        for msg in messages.iter() {
            if let Some(body) = msg.body()
                && let Some(parsed_mail) = mail_parser::MessageParser::default().parse(body)
            {
                let mut text_content = parsed_mail
                    .body_text(0)
                    .map(|s| s.into_owned())
                    .unwrap_or_default();
                let html_content = parsed_mail.body_html(0).map(|s| s.into_owned());

                // Sanitize HTML
                let safe_html =
                    html_content.map(|html| ammonia::Builder::default().clean(&html).to_string());

                // If text_content is empty but we have HTML, use html2text to render it
                if text_content.is_empty()
                    && let Some(html) = &safe_html
                {
                    text_content = html2text::from_read(html.as_bytes(), 80).unwrap_or_default();
                }

                // When the message arrived, from its Date header. BIR's own
                // received stamp is parsed from the body separately.
                let email_received_at = parsed_mail.date().map(|date| date.to_rfc3339());

                match parse_bir_receipt_email(&text_content, safe_html) {
                    Ok(receipt) => {
                        let mut pending_notice: Option<(String, String)> = None;
                        if let Ok(db_guard) = lock_timing::lock(&db)
                            && let Ok((submission_receipt, _is_new)) = db_guard
                                .save_submission_receipt_with_email_date(
                                    &receipt,
                                    email_received_at.as_deref(),
                                )
                        {
                            // Always attempt confirm. A previous poll may have
                            // saved the receipt while Submitted→Confirmed was
                            // ignored (clock skew, 1601-C matcher gap). UNIQUE
                            // filename makes a second save a no-op; confirm
                            // of an already-Confirmed row is Ignored.
                            let confirmed = match db_guard
                                .confirm_submission_from_receipt(&submission_receipt)
                            {
                                Ok(crate::db::ReceiptConfirmationOutcome::Confirmed) => true,
                                Ok(crate::db::ReceiptConfirmationOutcome::Ignored) => false,
                                Err(error) => {
                                    tracing::warn!(
                                        "Receipt {} was saved but could not confirm a submission: {}",
                                        submission_receipt.filename,
                                        error
                                    );
                                    false
                                }
                            };
                            if confirmed
                                && let Some((tin, form_type, period)) =
                                    crate::receipt::split_bir_filename(&submission_receipt.filename)
                            {
                                let form_code =
                                    crate::background_cron::form_code_from_form_type(&form_type)
                                        .to_string();
                                let period_label =
                                    if crate::filing_queue::is_audited_1601c_receipt_form_type(
                                        &form_type,
                                    ) {
                                        crate::filing_queue::parse_1601c_period(&period).map(
                                            |(year, month)| {
                                                crate::background_cron::monthly_period_label(
                                                    year, month,
                                                )
                                            },
                                        )
                                    } else {
                                        crate::db::parse_2551q_period(&period).map(
                                            |(year, quarter)| {
                                                crate::background_cron::quarterly_period_label(
                                                    year, quarter,
                                                )
                                            },
                                        )
                                    };
                                if let Some(period_label) = period_label {
                                    let taxpayer_name = db_guard
                                        .get_profile(&tin)
                                        .ok()
                                        .flatten()
                                        .map(|profile| profile.full_name.clone())
                                        .unwrap_or_else(|| tin.clone());
                                    let bir_filename = format!("{tin}-{form_type}-{period}.xml");
                                    let (title, body) = crate::background_cron::confirmation_notice(
                                        &taxpayer_name,
                                        &crate::naming::Tin::dashed_display(&tin),
                                        &form_code,
                                        &period_label,
                                        &bir_filename,
                                        &crate::background_cron::display_received_at(
                                            &submission_receipt.received_date,
                                            &submission_receipt.received_time,
                                        ),
                                        profile.inbox_email(),
                                    );
                                    crate::background_cron::record_confirmation_alert(
                                        &db_guard,
                                        &tin,
                                        &form_code,
                                        &period_label,
                                        &title,
                                        &body,
                                        &taxpayer_name,
                                    );
                                    // Sent after the lock below: the banner spawns a process.
                                    pending_notice = Some((title, body));
                                }
                            }
                            processed.push(submission_receipt);
                        }
                        if let Some((title, body)) = pending_notice {
                            crate::notification::send_notification(&title, &body);
                        }
                    }
                    Err(e) => {
                        // If it's truly an email from BIR but we failed to parse it, log it.
                        if text_content.contains("This confirms receipt of your submission") {
                            tracing::error!(
                                "Failed to parse BIR receipt email. Error: {:?}\nRaw Body snippet: {:.200}",
                                e,
                                text_content
                            );
                        }
                    }
                }
            }
        }
    }

    session.logout()?;
    Ok(processed)
}

/// Fetch emails for a specific email address, across all profiles.
/// Returns (poll_success, still_pending_forms, error_message).
pub fn fetch_and_process_emails_for_address(
    email_address: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> (bool, bool, Option<String>) {
    fetch_and_process_emails_for_address_until(email_address, db, &|| false)
}

/// [`fetch_and_process_emails_for_address`] for the queue worker: it stops between messages once `stop` says so (the
/// worker is ending), and every read and write on the connection gives up after 30 seconds.
pub fn fetch_and_process_emails_for_address_until(
    email_address: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
    stop: &dyn Fn() -> bool,
) -> (bool, bool, Option<String>) {
    let (profile, still_pending) = {
        let db_guard = match lock_timing::lock(&db) {
            Ok(g) => g,
            // Still pending: a lock failure says nothing about the inbox, so the job keeps its place.
            Err(e) => return (false, true, Some(format!("DB lock failed: {}", e))),
        };
        let current_year = chrono::Utc::now().naive_utc().date().year() as u16;
        let plan = plan_email_poll_for_address(&db_guard, email_address, current_year);
        (plan.auth_profile, plan.still_pending)
    };

    if !still_pending {
        return (true, false, None);
    }

    if let Some(mut profile) = profile {
        // A Google grant in a build that cannot renew it: said once (one alert, one banner), and the poll stops rather
        // than failing the same way every interval. Only a grant that is really stored: after Disconnect the method
        // may still read GoogleOAuth with no grant, and that is not a connection to warn about.
        if google_grant_unusable(&profile, google_sign_in_available()) {
            return google_unavailable_poll(&db, &profile);
        }
        // Sibling Submitted drafts (Jane) must still be polled even when the
        // grant lives on Juan and Juan's tracking toggle is off.
        profile.email_tracking_enabled = true;
        match fetch_and_process_emails_until(&profile, db.clone(), stop) {
            Ok(_) => {
                let db_guard = match lock_timing::lock(&db) {
                    Ok(g) => g,
                    Err(_) => return (false, true, Some("DB lock failed after fetch".to_string())),
                };
                let mut remaining_pending = false;
                let current_year = chrono::Utc::now().naive_utc().date().year() as u16;
                let profiles = db_guard.list_profiles().unwrap_or_default();
                for p in profiles {
                    // A dry run waits for no email: only a real send keeps the poll on.
                    if inbox_emails_match(p.inbox_email(), email_address)
                        && db_guard
                            .has_return_awaiting_bir_email(&p.tin.full(), current_year)
                            .unwrap_or(false)
                    {
                        remaining_pending = true;
                    }
                }
                // The connection works, so clear any standing alert for every
                // TIN that shares this inbox. Done here rather than only when
                // the user clicks Reconnect, so a token that starts working
                // again for any reason stops nagging.
                let _ = db_guard.resolve_google_oauth_alerts_for_inbox(email_address);
                (true, remaining_pending, None)
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                tracing::warn!("Email polling failed for {}: {}", email_address, err_msg);

                // A missing refresh is "not connected", not a failed refresh.
                // Reconnect nags belong to a grant Google actually rejected.
                let recorded = if profile.has_usable_oauth_refresh() {
                    lock_timing::lock(&db)
                        .ok()
                        .and_then(|db_guard| record_poll_failure(&db_guard, &profile, &err_msg))
                } else {
                    None
                };
                // After the guard above is dropped: the banner can spawn a
                // process, and the UI thread waits on this lock.
                if let Some(outcome) = recorded {
                    crate::notification::notify_alert_if_newly_active(
                        outcome,
                        POLL_FAILURE_TITLE,
                        &err_msg,
                    );
                }
                (false, still_pending, Some(err_msg))
            }
        }
    } else {
        (
            false,
            false,
            Some(format!(
                "No profile found matching email: {}",
                email_address
            )),
        )
    }
}

const POLL_FAILURE_TITLE: &str = "Email confirmation checking has stopped";

/// A stored Google grant this build cannot renew (`available`: whether Google's sign-in is in the build).
fn google_grant_unusable(profile: &TaxpayerProfile, available: bool) -> bool {
    profile.email_auth_method == EmailAuthMethod::GoogleOAuth
        && profile.has_usable_oauth_refresh()
        && !available
}

/// The poll of an inbox connected to Google in a build that cannot renew the grant: one alert (with nothing to click:
/// Reconnect cannot work here), its banner only the first time, and nothing left pending, so the job ends instead of
/// failing every interval. Returns what `fetch_and_process_emails_for_address_until` returns.
fn google_unavailable_poll(
    db: &std::sync::Arc<std::sync::Mutex<Database>>,
    profile: &TaxpayerProfile,
) -> (bool, bool, Option<String>) {
    let recorded = lock_timing::lock(db)
        .ok()
        .and_then(|db_guard| record_google_unavailable(&db_guard, profile));
    if let Some(outcome) = recorded {
        crate::notification::notify_alert_if_newly_active(
            outcome,
            POLL_FAILURE_TITLE,
            GOOGLE_NOT_IN_THIS_VERSION,
        );
    }
    (false, false, Some(GOOGLE_NOT_IN_THIS_VERSION.to_string()))
}

/// The alert [`google_unavailable_poll`] records: the same kind and title as a failed poll, in plain words.
fn record_google_unavailable(
    db: &Database,
    profile: &TaxpayerProfile,
) -> Option<crate::db::AlertRecordOutcome> {
    db.record_alert(
        Some(&profile.tin.full()),
        crate::db::alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED,
        crate::db::AlertSeverity::Error,
        POLL_FAILURE_TITLE,
        GOOGLE_NOT_IN_THIS_VERSION,
        crate::db::AlertAction::None,
    )
    .ok()
}

/// Record that polling `profile`'s confirmation inbox failed. Returns what
/// the record did; the caller decides on the banner once it has let go of
/// `db`'s lock, because posting one can spawn a process.
fn record_poll_failure(
    db: &Database,
    profile: &TaxpayerProfile,
    err_msg: &str,
) -> Option<crate::db::AlertRecordOutcome> {
    db.record_alert(
        Some(&profile.tin.full()),
        crate::db::alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED,
        crate::db::AlertSeverity::Error,
        POLL_FAILURE_TITLE,
        err_msg,
        crate::db::AlertAction::ReconnectGoogleAccount,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AlertRecordOutcome;

    fn juana() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Juana Santos",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street, Pasig",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "juana@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Individual"
        }))
        .unwrap()
    }

    /// T4: recording a failed poll posts nothing itself. It hands the
    /// outcome back so the banner goes out after the guard is dropped, and
    /// only the first report of a standing failure is news.
    #[test]
    fn a_failed_poll_is_recorded_and_its_banner_left_to_the_caller() {
        let db = Database::open_ephemeral().unwrap();
        let profile = juana();
        assert_eq!(
            record_poll_failure(&db, &profile, "invalid_grant"),
            Some(AlertRecordOutcome::Created)
        );
        assert_eq!(
            record_poll_failure(&db, &profile, "invalid_grant"),
            Some(AlertRecordOutcome::StillActive)
        );
        let alerts = db.list_active_alerts(Some(&profile.tin.full())).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].title, POLL_FAILURE_TITLE);
        assert_eq!(alerts[0].detail, "invalid_grant");
        assert_eq!(alerts[0].occurrences, 2);
    }

    /// #504 review: only a stored grant is "a Google grant this build cannot renew". After Disconnect the method can
    /// still read GoogleOAuth with no grant; that profile gets no "disconnect" alert, and a build with Google gets none.
    #[test]
    fn only_a_stored_grant_is_unusable_without_google() {
        let mut profile = juana();
        profile.email_auth_method = EmailAuthMethod::GoogleOAuth;
        assert!(
            !google_grant_unusable(&profile, false),
            "disconnected: no grant"
        );
        profile.oauth_refresh_token = Some("refresh".to_string());
        assert!(google_grant_unusable(&profile, false));
        assert!(
            !google_grant_unusable(&profile, true),
            "a build with Google renews it"
        );
        profile.email_auth_method = EmailAuthMethod::AppPassword;
        assert!(
            !google_grant_unusable(&profile, false),
            "an app password is not a grant"
        );
    }

    /// A Google grant in a build without Google: one alert in plain words with nothing to click, recorded again only as
    /// a repeat (no second banner), and the poll reports nothing pending, so its job ends.
    #[test]
    fn a_google_grant_without_google_is_said_once_and_the_poll_ends() {
        let db = Database::open_ephemeral().unwrap();
        let profile = juana();
        assert_eq!(
            record_google_unavailable(&db, &profile),
            Some(AlertRecordOutcome::Created)
        );
        assert_eq!(
            record_google_unavailable(&db, &profile),
            Some(AlertRecordOutcome::StillActive)
        );
        let alerts = db.list_active_alerts(Some(&profile.tin.full())).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].detail, GOOGLE_NOT_IN_THIS_VERSION);
        assert!(!alerts[0].detail.contains("GOOGLE_CLIENT_ID"));

        let shared =
            std::sync::Arc::new(std::sync::Mutex::new(Database::open_ephemeral().unwrap()));
        let (ok, still_pending, message) = google_unavailable_poll(&shared, &profile);
        assert!(!ok);
        assert!(!still_pending);
        assert_eq!(message.as_deref(), Some(GOOGLE_NOT_IN_THIS_VERSION));
    }

    /// The IMAP connection waits at most 30 seconds on a read or a write (a loopback listener stands in for the server;
    /// nothing leaves this computer).
    #[test]
    fn the_imap_connection_has_read_and_write_timeouts() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stream = imap_tcp_stream("127.0.0.1", port).expect("connected");
        assert_eq!(stream.read_timeout().unwrap(), Some(IMAP_IO_TIMEOUT));
        assert_eq!(stream.write_timeout().unwrap(), Some(IMAP_IO_TIMEOUT));
        assert_eq!(IMAP_IO_TIMEOUT, std::time::Duration::from_secs(30));
    }
}
