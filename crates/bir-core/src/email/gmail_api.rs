//! The Gmail API reader for BIR confirmation mail, with the read-only scope.
//!
//! Gmail sign-in asks Google for `gmail.readonly` only: it can list and read messages, never send, change or delete
//! them (IMAP's `https://mail.google.com/` could). The device reads the mail itself; nothing from the mailbox leaves it.
//!
//! The reader searches one sender, `ebirforms-noreply@bir.gov.ph`, across All Mail (a filter that archives the BIR's
//! mail still finds it), returns each message's raw RFC 822 bytes for the existing receipt parser, and names the
//! mailbox with `users.getProfile` (no `userinfo.email` scope needed). The access token travels only in the
//! `Authorization` header; errors carry an HTTP status, never a response body.

use crate::cloud_account::{CloudError, Http, HttpRequest};

/// The one scope Gmail sign-in asks for.
pub const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

/// The only sender the reader searches.
pub const BIR_SENDER: &str = "ebirforms-noreply@bir.gov.ph";

const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

/// Ids per page (Gmail's maximum is 500; 100 keeps each answer small).
const PAGE_SIZE: u32 = 100;

/// At most this many pages per search (2,000 messages): a search that would go further stops with an error instead
/// of reading without end.
const MAX_PAGES: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum GmailError {
    /// 401: the access token is expired or revoked. The caller refreshes once, then asks to reconnect.
    Unauthorized,
    /// 403 and the like: for example a grant without `gmail.readonly`, or the Gmail API turned off for the project.
    Refused { status: u16 },
    /// Any other answer that is not a success.
    Api { status: u16 },
    /// The request did not complete.
    Network(String),
    /// A success whose body is not what the Gmail API sends.
    Malformed(&'static str),
    /// More messages than [`MAX_PAGES`] pages.
    TooManyMessages,
    /// The caller asked the reader to stop (the queue worker is ending).
    Stopped,
}

impl std::fmt::Display for GmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "Google sign-in has expired or was removed"),
            Self::Refused { status } => write!(f, "Gmail refused the request (HTTP {status})"),
            Self::Api { status } => write!(f, "Gmail answered HTTP {status}"),
            Self::Network(error) => write!(f, "Gmail could not be reached: {error}"),
            Self::Malformed(what) => write!(f, "Gmail sent an unexpected answer: {what}"),
            Self::TooManyMessages => write!(f, "too many BIR messages to read in one check"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

impl std::error::Error for GmailError {}

/// The Gmail search for the BIR's confirmations sent on or after `since`.
///
/// Gmail's `after:` reads its date in Pacific time and is exclusive, so the search starts the day before `since`: a
/// message from early on `since` in Manila is never missed. A message read twice is harmless (receipts are unique by
/// their BIR filename).
pub fn bir_search_query(since: chrono::NaiveDate) -> String {
    let after = since.pred_opt().unwrap_or(since);
    format!("from:{BIR_SENDER} after:{}", after.format("%Y/%m/%d"))
}

/// The ids of every message matching `query`, newest first, following Gmail's pages.
pub fn list_message_ids(
    http: &mut dyn Http,
    access_token: &str,
    query: &str,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<String>, GmailError> {
    let mut ids = Vec::new();
    let mut page_token: Option<String> = None;
    for _ in 0..MAX_PAGES {
        if stop() {
            return Err(GmailError::Stopped);
        }
        let mut url = format!(
            "{API}/messages?q={}&maxResults={PAGE_SIZE}&includeSpamTrash=false",
            urlencoding::encode(query)
        );
        if let Some(token) = &page_token {
            url.push_str("&pageToken=");
            url.push_str(&urlencoding::encode(token));
        }
        let body = get_json(http, access_token, url)?;
        if let Some(messages) = body.get("messages") {
            let messages = messages
                .as_array()
                .ok_or(GmailError::Malformed("messages is not a list"))?;
            for message in messages {
                let id = message
                    .get("id")
                    .and_then(|id| id.as_str())
                    .ok_or(GmailError::Malformed("a message without an id"))?;
                if !is_message_id(id) {
                    return Err(GmailError::Malformed(
                        "a message id with unexpected characters",
                    ));
                }
                ids.push(id.to_string());
            }
        }
        match body.get("nextPageToken").and_then(|token| token.as_str()) {
            Some(token) if !token.is_empty() => page_token = Some(token.to_string()),
            _ => return Ok(ids),
        }
    }
    Err(GmailError::TooManyMessages)
}

/// One message's raw RFC 822 bytes (`format=raw`), for the receipt parser.
pub fn raw_message(
    http: &mut dyn Http,
    access_token: &str,
    id: &str,
) -> Result<Vec<u8>, GmailError> {
    if !is_message_id(id) {
        return Err(GmailError::Malformed(
            "a message id with unexpected characters",
        ));
    }
    let body = get_json(
        http,
        access_token,
        format!("{API}/messages/{id}?format=raw"),
    )?;
    let raw = body
        .get("raw")
        .and_then(|raw| raw.as_str())
        .ok_or(GmailError::Malformed("a message without raw content"))?;
    decode_base64url(raw).ok_or(GmailError::Malformed("raw content is not base64url"))
}

/// Every BIR confirmation sent on or after `since`, as raw messages, newest first.
pub fn read_bir_messages(
    http: &mut dyn Http,
    access_token: &str,
    since: chrono::NaiveDate,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, GmailError> {
    let ids = list_message_ids(http, access_token, &bir_search_query(since), stop)?;
    let mut messages = Vec::with_capacity(ids.len());
    for id in ids {
        if stop() {
            return Err(GmailError::Stopped);
        }
        messages.push(raw_message(http, access_token, &id)?);
    }
    Ok(messages)
}

/// The connected mailbox's address (`users.getProfile`).
pub fn profile_email(http: &mut dyn Http, access_token: &str) -> Result<String, GmailError> {
    let body = get_json(http, access_token, format!("{API}/profile"))?;
    body.get("emailAddress")
        .and_then(|address| address.as_str())
        .filter(|address| address.contains('@'))
        .map(str::to_string)
        .ok_or(GmailError::Malformed("a profile without an email address"))
}

fn get_json(
    http: &mut dyn Http,
    access_token: &str,
    url: String,
) -> Result<serde_json::Value, GmailError> {
    let response = http
        .exchange(&HttpRequest {
            method: "GET",
            url,
            content_type: "application/json",
            bearer: Some(access_token.to_string()),
            body: String::new(),
        })
        .map_err(|error| match error {
            CloudError::Network(error) => GmailError::Network(error),
            CloudError::TimedOut => GmailError::Network("timed out".into()),
            _ => GmailError::Network("the request failed".into()),
        })?;
    match response.status {
        200..=299 => {
            serde_json::from_str(&response.body).map_err(|_| GmailError::Malformed("not JSON"))
        }
        401 => Err(GmailError::Unauthorized),
        403 => Err(GmailError::Refused { status: 403 }),
        status => Err(GmailError::Api { status }),
    }
}

/// Gmail message ids are hexadecimal; anything else never goes into a URL path.
fn is_message_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Gmail's `raw` is base64url, with or without padding.
fn decode_base64url(raw: &str) -> Option<Vec<u8>> {
    data_encoding::BASE64URL_NOPAD
        .decode(raw.trim().trim_end_matches('=').as_bytes())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_account::HttpResponse;
    use std::collections::VecDeque;

    /// Answers requests in order and keeps what was asked.
    struct Scripted {
        answers: VecDeque<(u16, String)>,
        seen: Vec<HttpRequest>,
    }

    impl Scripted {
        fn new(answers: &[(u16, &str)]) -> Self {
            Self {
                answers: answers
                    .iter()
                    .map(|(status, body)| (*status, body.to_string()))
                    .collect(),
                seen: Vec::new(),
            }
        }
    }

    impl Http for Scripted {
        fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
            self.seen.push(request.clone());
            let (status, body) = self
                .answers
                .pop_front()
                .expect("an answer for every request");
            Ok(HttpResponse { status, body })
        }
    }

    struct Offline;

    impl Http for Offline {
        fn exchange(&mut self, _request: &HttpRequest) -> Result<HttpResponse, CloudError> {
            Err(CloudError::Network("no route".into()))
        }
    }

    const TOKEN: &str = "ya29.access-token-fixture";

    fn never() -> bool {
        false
    }

    fn date(text: &str) -> chrono::NaiveDate {
        chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn b64url(bytes: &[u8]) -> String {
        data_encoding::BASE64URL_NOPAD.encode(bytes)
    }

    #[test]
    fn the_search_names_only_the_bir_sender_and_starts_the_day_before() {
        assert_eq!(
            bir_search_query(date("2026-11-01")),
            "from:ebirforms-noreply@bir.gov.ph after:2026/10/31"
        );
        assert_eq!(
            bir_search_query(date("2026-01-01")),
            "from:ebirforms-noreply@bir.gov.ph after:2025/12/31"
        );
    }

    #[test]
    fn the_scope_is_read_only() {
        assert_eq!(
            GMAIL_READONLY_SCOPE,
            "https://www.googleapis.com/auth/gmail.readonly"
        );
    }

    #[test]
    fn ids_are_collected_across_pages_and_the_page_token_is_sent() {
        let mut http = Scripted::new(&[
            (
                200,
                r#"{"messages":[{"id":"18c1a","threadId":"t1"},{"id":"18c1b","threadId":"t1"}],"nextPageToken":"page/2+"}"#,
            ),
            (
                200,
                r#"{"messages":[{"id":"18c1c","threadId":"t2"}],"resultSizeEstimate":1}"#,
            ),
        ]);
        let ids = list_message_ids(
            &mut http,
            TOKEN,
            &bir_search_query(date("2026-11-01")),
            &never,
        )
        .unwrap();
        assert_eq!(ids, ["18c1a", "18c1b", "18c1c"]);
        assert_eq!(http.seen.len(), 2);
        let first = &http.seen[0].url;
        assert!(first.starts_with("https://gmail.googleapis.com/gmail/v1/users/me/messages?q="));
        assert!(first.contains("from%3Aebirforms-noreply%40bir.gov.ph%20after%3A2026%2F10%2F31"));
        assert!(!first.contains("pageToken"));
        assert!(http.seen[1].url.ends_with("&pageToken=page%2F2%2B"));
    }

    #[test]
    fn the_token_travels_only_in_the_authorization_header() {
        let mut http = Scripted::new(&[(200, r#"{"resultSizeEstimate":0}"#)]);
        list_message_ids(&mut http, TOKEN, "from:x", &never).unwrap();
        let request = &http.seen[0];
        assert_eq!(request.method, "GET");
        assert_eq!(request.bearer.as_deref(), Some(TOKEN));
        assert!(!request.url.contains(TOKEN));
        assert!(request.body.is_empty());
    }

    #[test]
    fn no_matches_is_an_empty_list() {
        let mut http = Scripted::new(&[(200, r#"{"resultSizeEstimate":0}"#)]);
        assert!(
            list_message_ids(&mut http, TOKEN, "from:x", &never)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn an_endless_search_stops_with_an_error() {
        let page = r#"{"messages":[{"id":"aa"}],"nextPageToken":"more"}"#;
        let pages: Vec<(u16, &str)> = (0..MAX_PAGES).map(|_| (200, page)).collect();
        let mut http = Scripted::new(&pages);
        assert_eq!(
            list_message_ids(&mut http, TOKEN, "from:x", &never),
            Err(GmailError::TooManyMessages)
        );
        assert_eq!(http.seen.len(), MAX_PAGES);
    }

    #[test]
    fn statuses_map_to_what_the_caller_acts_on() {
        for (status, want) in [
            (401, GmailError::Unauthorized),
            (403, GmailError::Refused { status: 403 }),
            (429, GmailError::Api { status: 429 }),
            (500, GmailError::Api { status: 500 }),
        ] {
            let mut http = Scripted::new(&[(status, r#"{"error":{"message":"private detail"}}"#)]);
            let error = list_message_ids(&mut http, TOKEN, "from:x", &never).unwrap_err();
            assert!(
                !error.to_string().contains("private detail"),
                "no response body in the error"
            );
            assert_eq!(error, want);
        }
        assert!(matches!(
            list_message_ids(&mut Offline, TOKEN, "from:x", &never),
            Err(GmailError::Network(_))
        ));
    }

    #[test]
    fn a_raw_message_is_decoded_padded_or_not() {
        let mail = b"From: ebirforms-noreply@bir.gov.ph\r\nSubject: x\r\n\r\nThis confirms receipt of your submission??";
        let unpadded = b64url(mail);
        let padded = data_encoding::BASE64URL.encode(mail);
        assert_ne!(unpadded, padded, "the fixture exercises padding");
        for raw in [unpadded, padded] {
            let mut http = Scripted::new(&[(200, &format!(r#"{{"id":"18c1a","raw":"{raw}"}}"#))]);
            assert_eq!(raw_message(&mut http, TOKEN, "18c1a").unwrap(), mail);
            assert_eq!(
                http.seen[0].url,
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/18c1a?format=raw"
            );
        }
    }

    #[test]
    fn a_message_id_never_shapes_the_url() {
        for id in ["", "../profile", "18c1a?format=full", "a b", "18c1a/x"] {
            let mut http = Scripted::new(&[]);
            assert!(
                matches!(
                    raw_message(&mut http, TOKEN, id),
                    Err(GmailError::Malformed(_))
                ),
                "{id:?}"
            );
            assert!(http.seen.is_empty());
        }
        let mut http = Scripted::new(&[(200, r#"{"messages":[{"id":"../profile"}]}"#)]);
        assert!(matches!(
            list_message_ids(&mut http, TOKEN, "from:x", &never),
            Err(GmailError::Malformed(_))
        ));
    }

    #[test]
    fn malformed_answers_are_errors_not_panics() {
        for body in [
            "not json",
            r#"{"messages":"x"}"#,
            r#"{"messages":[{"threadId":"t"}]}"#,
        ] {
            let mut http = Scripted::new(&[(200, body)]);
            assert!(
                matches!(
                    list_message_ids(&mut http, TOKEN, "from:x", &never),
                    Err(GmailError::Malformed(_))
                ),
                "{body}"
            );
        }
        for body in [r#"{"id":"aa"}"#, r#"{"raw":"***"}"#] {
            let mut http = Scripted::new(&[(200, body)]);
            assert!(
                matches!(
                    raw_message(&mut http, TOKEN, "aa"),
                    Err(GmailError::Malformed(_))
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn reading_fetches_each_listed_message_in_order() {
        let first = b64url(b"first");
        let second = b64url(b"second");
        let mut http = Scripted::new(&[
            (200, r#"{"messages":[{"id":"aa"},{"id":"bb"}]}"#),
            (200, &format!(r#"{{"raw":"{first}"}}"#)),
            (200, &format!(r#"{{"raw":"{second}"}}"#)),
        ]);
        let messages = read_bir_messages(&mut http, TOKEN, date("2026-11-01"), &never).unwrap();
        assert_eq!(messages, [b"first".to_vec(), b"second".to_vec()]);
        assert!(http.seen[1].url.ends_with("/messages/aa?format=raw"));
        assert!(http.seen[2].url.ends_with("/messages/bb?format=raw"));
    }

    #[test]
    fn stopping_ends_the_read_between_messages() {
        let calls = std::cell::Cell::new(0);
        let stop = || {
            calls.set(calls.get() + 1);
            calls.get() > 2
        };
        let raw = b64url(b"one");
        let mut http = Scripted::new(&[
            (200, r#"{"messages":[{"id":"aa"},{"id":"bb"}]}"#),
            (200, &format!(r#"{{"raw":"{raw}"}}"#)),
        ]);
        assert_eq!(
            read_bir_messages(&mut http, TOKEN, date("2026-11-01"), &stop),
            Err(GmailError::Stopped)
        );
        assert_eq!(http.seen.len(), 2, "the second message is never fetched");
    }

    #[test]
    fn the_profile_names_the_mailbox() {
        let mut http = Scripted::new(&[(
            200,
            r#"{"emailAddress":"juan@buwiz.com","messagesTotal":3}"#,
        )]);
        assert_eq!(profile_email(&mut http, TOKEN).unwrap(), "juan@buwiz.com");
        assert_eq!(
            http.seen[0].url,
            "https://gmail.googleapis.com/gmail/v1/users/me/profile"
        );
        let mut http = Scripted::new(&[(200, r#"{"messagesTotal":3}"#)]);
        assert!(matches!(
            profile_email(&mut http, TOKEN),
            Err(GmailError::Malformed(_))
        ));
    }
}
