//! What a failed Google request may say in an error or a log line: the HTTP status and Google's short error code.
//!
//! Google's error answers carry free text (`error_description`, `message`, an HTML page from a proxy), and the
//! consent link carries the sign-in's one-time `state` and PKCE challenge. None of that goes into an error or a log
//! line: a log sent to support, or a message on screen, needs only the status and the code to say what went wrong.

/// Google's error code as a short, plain word: only `[A-Za-z0-9_]`, at most 40 characters, `unknown` when nothing
/// is left. It names the reason, never anything about the person.
pub(crate) fn plain_code(error: &str) -> String {
    let code: String = error
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(40)
        .collect();
    if code.is_empty() {
        "unknown".into()
    } else {
        code
    }
}

/// The short error code in a Google error answer, if it has one.
///
/// The OAuth endpoints answer `{"error": "invalid_grant", ...}`; the REST APIs (Calendar, Gmail) answer
/// `{"error": {"status": "PERMISSION_DENIED", "errors": [{"reason": "rateLimitExceeded"}], ...}}`, where the reason
/// is the more precise of the two.
pub(crate) fn error_code(body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = parsed.get("error")?;
    let code = match error.as_str() {
        Some(code) => code,
        None => error
            .pointer("/errors/0/reason")
            .and_then(|v| v.as_str())
            .or_else(|| error.get("status").and_then(|v| v.as_str()))?,
    };
    (!code.trim().is_empty()).then(|| plain_code(code))
}

/// "<what> (HTTP <status>, <code>)", or without the code when Google's answer has none. Never the body itself.
pub(crate) fn failed_request(what: &str, status: impl std::fmt::Display, body: &str) -> String {
    match error_code(body) {
        Some(code) => format!("{what} (HTTP {status}, {code})"),
        None => format!("{what} (HTTP {status})"),
    }
}

/// The browser could not be opened for a Google sign-in. Only the kind of failure: the `open` crate's error text
/// names the command it ran, and that command line holds the consent link.
pub(crate) fn browser_not_opened(error: &std::io::Error) -> String {
    format!(
        "Could not open the browser for the Google sign-in ({:?})",
        error.kind()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "PRIVATE-MARKER-7c1e";

    #[test]
    fn an_oauth_error_gives_its_code_and_never_its_description() {
        let body = format!(r#"{{"error":"invalid_grant","error_description":"{MARKER}"}}"#);
        let text = failed_request("Google sign-in failed", 400, &body);
        assert_eq!(text, "Google sign-in failed (HTTP 400, invalid_grant)");
    }

    #[test]
    fn a_rest_api_error_gives_its_reason_or_its_status_and_never_its_message() {
        let with_reason = format!(
            r#"{{"error":{{"code":403,"message":"{MARKER}","status":"PERMISSION_DENIED","errors":[{{"reason":"rateLimitExceeded","message":"{MARKER}"}}]}}}}"#
        );
        assert_eq!(
            error_code(&with_reason).as_deref(),
            Some("rateLimitExceeded")
        );
        let status_only =
            format!(r#"{{"error":{{"code":404,"message":"{MARKER}","status":"NOT_FOUND"}}}}"#);
        assert_eq!(error_code(&status_only).as_deref(), Some("NOT_FOUND"));
        assert!(!failed_request("Calendar", 403, &with_reason).contains(MARKER));
    }

    #[test]
    fn a_body_that_is_not_googles_error_format_gives_the_status_alone() {
        for body in [
            format!("<html>{MARKER}</html>"),
            String::new(),
            format!(r#"{{"message":"{MARKER}"}}"#),
            r#"{"error":"   "}"#.to_string(),
            MARKER.repeat(1000),
        ] {
            assert_eq!(
                failed_request("Calendar", 502, &body),
                "Calendar (HTTP 502)",
                "{body:.40}"
            );
        }
    }

    #[test]
    fn a_code_is_cut_to_a_short_plain_word() {
        let body = format!(r#"{{"error":"bad code\n{}"}}"#, "x".repeat(100));
        let code = error_code(&body).unwrap();
        assert!(
            code.len() <= 40 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "{code}"
        );
        assert_eq!(plain_code("<>"), "unknown");
    }

    #[test]
    fn a_browser_failure_names_only_its_kind() {
        let link =
            "https://accounts.google.com/o/oauth2/v2/auth?client_id=x&code_challenge=c&state=s";
        let error = std::io::Error::other(format!("Launcher \"xdg-open\" \"{link}\" failed"));
        let text = browser_not_opened(&error);
        assert!(
            !text.contains("state=")
                && !text.contains("code_challenge=")
                && !text.contains("xdg-open"),
            "{text}"
        );
        assert!(text.contains("Other"), "{text}");
    }
}
