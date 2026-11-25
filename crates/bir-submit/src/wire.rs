//! The small pieces of the sister repository's `bir-core` this client uses,
//! copied verbatim with their doc comments (eBIRForms/eBIRForms at
//! 14a0f63), so this crate depends on no `bir-*` crate:
//!
//! - [`TinKey`], from `crates/bir-core/src/forms/taxform.rs`;
//! - [`TinRefusal`] and its helpers, from `crates/bir-core/src/tin_refusal.rs`,
//!   with the seventh refusal, `tin_test_only`;
//! - [`FailureClass`], [`Blame`], the hold, limit and final codes and the
//!   server-time helpers, from `crates/bir-core/src/smart_queue.rs`.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

// ------------------------------------------ bir-core/src/forms/taxform.rs

/// The one key two TINs are compared by: the 9-digit root and the 5-digit
/// branch. A 9-digit TIN is its head office (`00000`), a 12-digit one has a
/// 3-digit branch. Its `Debug` never shows the digits.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TinKey {
    root: String,
    branch: String,
}

impl TinKey {
    /// The key of a TIN with 9, 12 or 14 digits (anything else between the
    /// digits, such as dashes, is ignored).
    pub fn parse(tin: &str) -> Option<Self> {
        let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
        let branch = match digits.len() {
            9 => "00000".to_owned(),
            12 => format!("00{}", &digits[9..]),
            14 => digits[9..].to_owned(),
            _ => return None,
        };
        Some(Self {
            root: digits[..9].to_owned(),
            branch,
        })
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }
}

impl std::fmt::Debug for TinKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TinKey(***-***-***-{})", self.branch)
    }
}

// ----------------------------------------------- bir-core/src/tin_refusal.rs
//
// Why Buwiz would not send a return for its TIN. The server ends every
// refusal with its code in brackets (`"{message} ({code})"`), and the queue
// stores that text as the return's reason; [`TinRefusal::from_reason`] is
// the one reader of that code, for live errors and stored reasons alike.
// Nothing here matches on the English of a message.

/// A refusal because of the TIN's state on the Buwiz account, or of the
/// account itself (banned, or a practice account): nothing reached BIR, and
/// sending again cannot help until the person fixes it on the account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TinRefusal {
    /// `tin_not_active`: not on this account (or the server says no more).
    NotActive,
    /// `tin_unverified`: on the account, not verified yet.
    Unverified,
    /// `tin_pending`: its papers are being checked.
    Pending,
    /// `tin_needs_fix`: staff sent it back for a fix.
    NeedsFix,
    /// `tin_paused`: staff paused sending for it.
    Paused,
    /// `tin_test_only`: a test TIN (root `000-000-000`): Buwiz never sends
    /// its returns to the BIR.
    TestOnly,
    /// `account_banned`: the account can send nothing.
    AccountBanned,
    /// `account_practice`: a practice account (the kind app-store reviewers
    /// use). It can prepare returns; Buwiz never sends them to the BIR.
    AccountPractice,
}

/// What the server's `tin_needs_fix` message begins with, before the fixed
/// reason sentences.
const FIX_PREFIX: &str = "Sent back for a fix: ";

/// Where the queue says a reason came from; [`TinRefusal::fix_reasons`]
/// takes them off.
const SOURCE_PREFIXES: [&str; 3] = [
    "Needs attention: ",
    "Not sent (nothing reached BIR): ",
    "Submission session failed before upload (no PUT attempted): ",
];

impl TinRefusal {
    pub const ALL: [TinRefusal; 8] = [
        TinRefusal::NotActive,
        TinRefusal::Unverified,
        TinRefusal::Pending,
        TinRefusal::NeedsFix,
        TinRefusal::Paused,
        TinRefusal::TestOnly,
        TinRefusal::AccountBanned,
        TinRefusal::AccountPractice,
    ];

    /// The refusal a server code names, exactly; `None` for any other code.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|refusal| refusal.code() == code)
    }

    /// The server's code.
    pub fn code(self) -> &'static str {
        match self {
            TinRefusal::NotActive => "tin_not_active",
            TinRefusal::Unverified => "tin_unverified",
            TinRefusal::Pending => "tin_pending",
            TinRefusal::NeedsFix => "tin_needs_fix",
            TinRefusal::Paused => "tin_paused",
            TinRefusal::TestOnly => "tin_test_only",
            TinRefusal::AccountBanned => "account_banned",
            TinRefusal::AccountPractice => "account_practice",
        }
    }

    /// The one decoder: the trailing ` (code)` of a live or stored reason,
    /// matched against the closed set. `None` when the text does not end in
    /// one of the eight codes.
    pub fn from_reason(reason: &str) -> Option<Self> {
        trailing_code(reason).and_then(|(_, code)| Self::from_code(code))
    }

    /// `reason` with the text before its trailing code cut to `max_chars`
    /// characters (an ellipsis marks the cut), the code kept; text without
    /// a code is cut the same way.
    pub fn capped(reason: &str, max_chars: usize) -> String {
        let cut = |text: &str| match text.char_indices().nth(max_chars) {
            Some((end, _)) => format!("{}\u{2026}", &text[..end]),
            None => text.to_owned(),
        };
        match trailing_code(reason) {
            Some(("", code)) => format!("({code})"),
            Some((message, code)) => format!("{} ({code})", cut(message)),
            None => cut(reason),
        }
    }

    /// The fixed reason sentences of a `tin_needs_fix` reason, when the
    /// server named any (`Sent back for a fix: <sentences> (tin_needs_fix)`);
    /// `None` for any other reason.
    pub fn fix_reasons(reason: &str) -> Option<String> {
        if Self::from_reason(reason) != Some(TinRefusal::NeedsFix) {
            return None;
        }
        let mut rest = reason.trim();
        for prefix in SOURCE_PREFIXES {
            if let Some(after) = rest.strip_prefix(prefix) {
                rest = after.trim();
            }
        }
        let (message, _) = trailing_code(rest)?;
        let sentences = message.strip_prefix(FIX_PREFIX)?.trim();
        (!sentences.is_empty()).then(|| sentences.to_owned())
    }
}

/// `"{message} ({code})"` split at its trailing code, when it has one: the
/// text ends in `)`, the last `(` opens the text or follows whitespace (an
/// empty message reads `" (code)"`, which is `"(code)"` trimmed), and the
/// brackets hold only lowercase letters and `_`. A prefix the queue adds in
/// front never changes the answer.
fn trailing_code(reason: &str) -> Option<(&str, &str)> {
    let reason = reason.trim();
    let inner = reason.strip_suffix(')')?;
    let open = inner.rfind('(')?;
    let message = &inner[..open];
    if !(message.is_empty() || message.ends_with(char::is_whitespace)) {
        return None;
    }
    let code = &inner[open + 1..];
    let is_code = !code.is_empty()
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_');
    is_code.then(|| (message.trim_end(), code))
}

// ---------------------------------------------- bir-core/src/smart_queue.rs

/// The longest any time the server names (`wait_until`, `Retry-After`) is
/// believed: anything further reads as this far.
pub const MAX_SERVER_WAIT_S: i64 = 7 * 86_400;

/// `until`, read as at most [`MAX_SERVER_WAIT_S`] after `now`.
pub fn capped_wait_until(until: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    until.min(now + Duration::seconds(MAX_SERVER_WAIT_S))
}

/// The server's `class` on a failure: whose side it failed on. An unknown
/// word reads as `bir` (public contract).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureClass {
    Bir,
    UserNetwork,
    Ours,
}

impl FailureClass {
    pub fn from_wire(word: &str) -> Self {
        match word {
            "ours" => Self::Ours,
            "user_network" => Self::UserNetwork,
            _ => Self::Bir,
        }
    }
}

/// The `503` codes with which Buwiz holds a ticket. Nothing reached the
/// BIR; a hold is never a try.
pub const HOLD_CODES: [&str; 4] = ["bir_offline", "bir_busy", "relay_paused", "restarting"];

pub fn is_hold_code(code: &str) -> bool {
    HOLD_CODES.contains(&code)
}

/// The `429` codes that say "not now" about Buwiz's own limits: nothing
/// reached the BIR, so never a try; the answer's `Retry-After` is the wait.
pub const LIMIT_CODES: [&str; 3] = ["rate_limited", "quota_exceeded", "try_anyway_limited"];

pub fn is_limit_code(code: &str) -> bool {
    LIMIT_CODES.contains(&code)
}

/// The code with which Buwiz says the sealed return does not match its
/// ticket. Final, like a TIN refusal: the return goes back to Draft.
pub const RETURN_MISMATCH: &str = "return_mismatch";

/// A reason that ends in a code no retry can change: a TIN refusal
/// (`tin_*`, `account_banned`) or [`RETURN_MISMATCH`]. Such a reason sends
/// the return back to Draft whatever class the server gave it.
pub fn is_final_reason(reason: &str) -> bool {
    TinRefusal::from_reason(reason).is_some() || is_mismatch_reason(reason)
}

/// The reason ends in `(return_mismatch)`.
pub fn is_mismatch_reason(reason: &str) -> bool {
    reason
        .trim()
        .strip_suffix(')')
        .and_then(|inner| inner.rsplit_once('('))
        .is_some_and(|(message, code)| {
            code == RETURN_MISMATCH
                && (message.is_empty() || message.ends_with(char::is_whitespace))
        })
}

/// Where a counted try failed, which decides who waits for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blame {
    /// The try never got past the BIR's front desk (every connection it
    /// opened toward the BIR was refused or timed out, no upload stream
    /// opened, or the front desk failed). It says nothing about this
    /// return: the whole queue backs off (`PathBackoff`), and the device
    /// guard hears it.
    Path,
    /// An upload stream opened and the send broke after it (a write, the
    /// session). This return's own ladder; the device guard hears it.
    Upload,
    /// About this return: a refused login, a validation refusal, Buwiz's
    /// answer to its ticket. Its own ladder; not the device guard. Three
    /// of the same kind in a row stop its automatic tries.
    Return,
    /// The BIR has no upload service for this TIN: like `Return`.
    NoService,
}

impl Blame {
    /// Whether the device guard hears this failure: only those that say
    /// something about this device's way to the BIR.
    pub fn heard_by_guard(self) -> bool {
        matches!(self, Self::Path | Self::Upload)
    }
}

pub fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------ TinKey

    #[test]
    fn a_tin_key_is_one_root_and_branch_for_9_12_and_14_digits() {
        let key = |tin: &str| {
            TinKey::parse(tin).map(|key| (key.root().to_owned(), key.branch().to_owned()))
        };
        let head = Some(("000123453".to_owned(), "00000".to_owned()));
        assert_eq!(key("000123453"), head);
        assert_eq!(key("000-123-453"), head);
        assert_eq!(key("000123453000"), head);
        assert_eq!(key("000-123-453-000"), head);
        assert_eq!(key("00012345300000"), head);
        assert_eq!(key("000-123-453-00000"), head);
        let branch = Some(("000123453".to_owned(), "00002".to_owned()));
        assert_eq!(key("000-123-453-002"), branch);
        assert_eq!(key("000-123-453-00002"), branch);
        // A head office given as its 9-digit root is the 14-digit row.
        assert_eq!(
            TinKey::parse("000123453"),
            TinKey::parse("000-123-453-00000")
        );
        assert_ne!(
            TinKey::parse("000123453"),
            TinKey::parse("000-123-453-00002")
        );
        for bad in [
            "",
            "2026",
            "0001234530",
            "000-123-453-0000",
            "000123453000000",
        ] {
            assert_eq!(TinKey::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_tin_key_never_shows_its_digits_in_debug() {
        let key = TinKey::parse("000-123-453-00002").unwrap();
        let shown = format!("{key:?}");
        assert!(!shown.contains("123") && !shown.contains("453"), "{shown}");
    }

    // -------------------------------------------------------- TinRefusal

    const CODES: [&str; 8] = [
        "tin_not_active",
        "tin_unverified",
        "tin_pending",
        "tin_needs_fix",
        "tin_paused",
        "tin_test_only",
        "account_banned",
        "account_practice",
    ];

    #[test]
    fn each_code_names_one_refusal_and_back() {
        for (refusal, code) in TinRefusal::ALL.into_iter().zip(CODES) {
            assert_eq!(refusal.code(), code);
            assert_eq!(TinRefusal::from_code(code), Some(refusal), "{code}");
        }
        for other in [
            "",
            "already_submitted",
            "relay_refused",
            "TIN_NOT_ACTIVE",
            "tin_not_active ",
            "tin",
        ] {
            assert_eq!(TinRefusal::from_code(other), None, "{other:?}");
        }
    }

    #[test]
    fn a_live_or_stored_reason_decodes_by_its_trailing_code() {
        for (refusal, code) in TinRefusal::ALL.into_iter().zip(CODES) {
            for reason in [
                format!("Some sentence the server chose. ({code})"),
                format!("Not sent (nothing reached BIR): another wording entirely ({code})"),
                format!("Submission session failed before upload (no PUT attempted): x ({code})"),
                format!("  trailing space ({code})  "),
            ] {
                assert_eq!(TinRefusal::from_reason(&reason), Some(refusal), "{reason}");
            }
        }
    }

    /// The server's fixed sentence for a test TIN (`TEST_ONLY_MESSAGE` in
    /// buwiz-com's `src/filing/gate.rs`), as the app sees it, is the
    /// seventh refusal, and final.
    #[test]
    fn a_test_tin_is_its_own_refusal() {
        let reason = "Not sent (nothing reached BIR): This is a test TIN: you can prepare returns for it, but Buwiz never sends them to the BIR. (tin_test_only)";
        assert_eq!(TinRefusal::from_reason(reason), Some(TinRefusal::TestOnly));
        assert_eq!(
            TinRefusal::from_code("tin_test_only"),
            Some(TinRefusal::TestOnly)
        );
        assert_eq!(TinRefusal::TestOnly.code(), "tin_test_only");
        assert_eq!(TinRefusal::fix_reasons(reason), None);
        assert!(is_final_reason(reason));
    }

    #[test]
    fn an_old_stored_reason_still_decodes() {
        assert_eq!(
            TinRefusal::from_reason(
                "Not sent (nothing reached BIR): this TIN is not active on your account (tin_not_active)"
            ),
            Some(TinRefusal::NotActive)
        );
    }

    #[test]
    fn a_message_with_parentheses_is_not_a_code() {
        for reason in [
            "the relay closed early (relay_closed)",
            "the connection ended (see the log)",
            "BIR may have it (tin_not_active) so check your email",
            "Not sent (nothing reached BIR): ",
            "(tin_not_active",
            "tin_not_active",
            "x(tin_not_active)",
            "x (TIN_NOT_ACTIVE)",
            "x (tin_not_active )",
            "",
        ] {
            assert_eq!(TinRefusal::from_reason(reason), None, "{reason:?}");
        }
    }

    /// An empty server message displays as `" (code)"`, and a relay error
    /// can be just `"(code)"`: the live text and the stored reason built
    /// from it decode alike.
    #[test]
    fn a_live_error_and_its_stored_reason_decode_alike() {
        for live in [
            " (tin_paused)",
            "(tin_paused)",
            "\t(tin_paused)",
            "Sending is paused. (tin_paused)",
            "x(tin_paused)",
            "(tin_paused",
            "",
            "the relay closed early",
        ] {
            for prefix in SOURCE_PREFIXES {
                let stored = format!("{prefix}{live}");
                assert_eq!(
                    TinRefusal::from_reason(live),
                    TinRefusal::from_reason(&stored),
                    "{live:?} / {stored:?}"
                );
            }
        }
        for live in [" (tin_paused)", "(tin_paused)", "\t(tin_paused)"] {
            assert_eq!(
                TinRefusal::from_reason(live),
                Some(TinRefusal::Paused),
                "{live:?}"
            );
        }
    }

    #[test]
    fn a_capped_reason_keeps_its_code_and_short_text_whole() {
        let long = format!(
            "Not sent (nothing reached BIR): {} (tin_paused)",
            "y".repeat(500)
        );
        let capped = TinRefusal::capped(&long, 40);
        assert_eq!(
            capped,
            format!(
                "Not sent (nothing reached BIR): {}\u{2026} (tin_paused)",
                "y".repeat(8)
            )
        );
        assert_eq!(TinRefusal::from_reason(&capped), Some(TinRefusal::Paused));
        let short = "Not sent (nothing reached BIR): Paused. (tin_paused)";
        assert_eq!(TinRefusal::capped(short, 40), short);
        assert_eq!(
            TinRefusal::capped(" (tin_paused)", 40),
            "(tin_paused)",
            "an empty message stays empty"
        );
        assert_eq!(TinRefusal::capped("abcdef", 3), "abc\u{2026}");
    }

    #[test]
    fn fix_reasons_are_only_the_fixed_sentences_after_the_prefix() {
        assert_eq!(
            TinRefusal::fix_reasons("Not sent (nothing reached BIR): Sent back for a fix: A photo is too blurry to read. The name does not match. (tin_needs_fix)").as_deref(),
            Some("A photo is too blurry to read. The name does not match.")
        );
        // The server's sentence when it named no reason.
        assert_eq!(
            TinRefusal::fix_reasons(
                "Not sent (nothing reached BIR): Sent back for a fix. See what to change on My TINs at account.buwiz.com. (tin_needs_fix)"
            ),
            None
        );
        assert_eq!(
            TinRefusal::fix_reasons(
                "Not sent (nothing reached BIR): Sent back for a fix:  (tin_needs_fix)"
            ),
            None
        );
        // Another code's text is never read as reasons.
        assert_eq!(
            TinRefusal::fix_reasons("Sent back for a fix: x (tin_paused)"),
            None
        );
        assert_eq!(TinRefusal::fix_reasons("Sent back for a fix: x"), None);
    }

    // ------------------------------------------------------- smart_queue

    /// From `holds_sign_outs_and_ours_are_not_tries`, without its
    /// `TryClass` lines (the queue's, not ported).
    #[test]
    fn holds_sign_outs_and_ours_are_not_tries() {
        for code in ["bir_offline", "bir_busy", "relay_paused", "restarting"] {
            assert!(is_hold_code(code));
        }
        for code in [
            "tin_not_active",
            "rate_limited",
            "internal",
            "try_anyway_limited",
            "",
        ] {
            assert!(!is_hold_code(code), "{code}");
        }
        assert_eq!(
            FailureClass::from_wire("something_new"),
            FailureClass::Bir,
            "an unknown class is bir"
        );
    }

    /// From `a_wait_until_beyond_seven_days_reads_as_seven_days`, its
    /// `capped_wait_until` line (the rest is the queue's `next_try_at`).
    #[test]
    fn a_wait_until_beyond_seven_days_reads_as_seven_days() {
        let t0 = parse_time("2026-11-11T07:00:00Z").unwrap();
        let far = t0 + Duration::days(365);
        assert_eq!(capped_wait_until(far, t0), t0 + Duration::days(7));
    }

    #[test]
    fn final_codes_are_refusals_and_return_mismatch() {
        assert!(is_final_reason("Not sent: x (tin_paused)"));
        assert!(is_final_reason(
            "Not sent (nothing reached BIR): did not match (return_mismatch)"
        ));
        assert!(is_final_reason("(return_mismatch)"));
        assert!(!is_final_reason("x(return_mismatch)"));
        assert!(!is_final_reason("the relay closed early (relay_closed)"));
        for code in ["rate_limited", "quota_exceeded", "try_anyway_limited"] {
            assert!(is_limit_code(code) && !is_hold_code(code));
        }
    }
}
