//! What the screens say about a return's send: one pure rule ([`send_state`])
//! over the facts a stored return keeps ([`SendFacts`]), built from each
//! storage shape by one helper ([`facts_of_lifecycle`], [`facts_of_1601c`],
//! [`facts_of_2551q`]). `screen_form_open` and `list_returns` answer it as
//! `send` ([`stored_send`]).

use bir_core::db::Database;
use bir_core::forms::form_1601c::Form1601CDraft;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_core::forms::queueable::{QueueableKind, SubmissionLifecycle};
use bir_core::forms::{FilingStatus, FormDraftSummary};
use serde::Serialize;
use serde_json::Value;

/// The line of a return the worker is sending now.
pub const SENDING_LINE: &str = "Sending now. Keep eBIRForms open.";
/// The line of a Queued return with no reason kept.
pub const QUEUED_LINE: &str = "Queued to send.";
/// The line of a dry-run send: the app's one sentence for a test send, the
/// one the OS notice says too (bir-core).
pub const TEST_SENT_LINE: &str = bir_core::background_cron::TEST_SENT_LINE;
/// The line of a send the Buwiz relay gave a receipt for.
pub const SENT_LINE: &str =
    "Sent to the BIR through Buwiz. Waiting for the BIR's confirmation email.";
/// The line of a claimed return whose relay send the worker is asking Buwiz about.
pub const CHECKING_LINE: &str =
    "Checking with Buwiz whether this return was sent. Keep eBIRForms open.";

/// What one stored return says about its send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFacts<'a> {
    pub status: &'a FilingStatus,
    /// A network claim is held (its token or its time is set).
    pub claimed: bool,
    /// The claim's PUT started (`submission_put_started_at`).
    pub put_started: bool,
    /// This process's submission worker holds the return's job key now.
    pub in_flight: bool,
    pub attempts: u32,
    /// The stored reason: `submission_error`, or 2551Q's `last_error`.
    pub reason: Option<&'a str>,
    pub next_retry_at: Option<&'a str>,
    pub submitted_dry_run: bool,
    pub has_relay_receipt: bool,
    /// The worker keeps a relay send it did not see end (`relay_pending`): it
    /// asks Buwiz how it ended.
    pub relay_pending: bool,
}

/// What the screens show about a return's send (`send` in `screen_form_open`
/// and in each `list_returns` row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SendState {
    /// queued | sending | checking | waiting | held | needs_attention | refused | sent | test_sent
    pub stage: &'static str,
    /// The sentence the person reads.
    pub line: String,
    /// The code the reason ended in (`tin_not_active`), refused only.
    pub code: Option<String>,
    /// When the worker tries again (held and waiting).
    pub next_try_at: Option<String>,
    /// The person may answer whether the BIR's email came (`resolve_unknown_send`).
    pub can_resolve: bool,
}

impl SendState {
    fn new(stage: &'static str, line: impl Into<String>) -> Self {
        Self {
            stage,
            line: line.into(),
            code: None,
            next_try_at: None,
            can_resolve: false,
        }
    }
}

/// A stored reason split into its text and its trailing ` (code)` (lower case
/// letters and `_` in final brackets, after a space or alone), the way
/// `bir_submit::wire::TinRefusal::from_reason` reads it.
pub fn split_reason(reason: &str) -> (&str, Option<&str>) {
    let trimmed = reason.trim();
    let coded = (|| {
        let inner = trimmed.strip_suffix(')')?;
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
    })();
    match coded {
        Some((message, code)) => (message, Some(code)),
        None => (trimmed, None),
    }
}

/// A stored reason as the person reads it. The Buwiz server's sentences are
/// written for the person and pass through; the worker's own notes are not
/// (they say "claim", "PUT", "SFTP", "relay"), so the ones it can leave on a
/// Draft or an unclaimed Queued return are said in plain words here.
pub fn plain_reason(text: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    const NOTES: [(&str, &str); 5] = [
        (
            "Claim lease expired before PUT started",
            "Sending was interrupted before it started.",
        ),
        (
            "BIR SFTP session failed before upload",
            "Could not connect to send this return.",
        ),
        ("Submission outcome pending", "Sending was interrupted."),
        ("the relay is unavailable", "The test relay did not answer."),
        (
            "the relay refused the submission",
            "The test relay refused this return.",
        ),
    ];
    for (note, plain) in NOTES {
        if text.starts_with(note) {
            return Cow::Borrowed(plain);
        }
    }
    Cow::Borrowed(text)
}

/// The send state of a return, or `None` where the screens keep what they
/// show today (a Draft with no reason, a Submitted return without a relay
/// receipt, Confirmed, Paid). The rules, in order:
/// 1. Draft with a reason: `refused` (except the reason a person's own
///    "It did not arrive" leaves, which is no refusal: `None`).
/// 2. Queued, claimed, in flight: `sending`.
/// 3. Queued, claimed, not in flight: `checking` while the worker keeps a
///    relay send it did not see end (it asks Buwiz; nothing for a person to
///    answer yet); else `needs_attention`, resolvable, when its upload started
///    (the BIR may have it); `queued` when it never did (nothing left this
///    computer: the worker takes it again by itself when the claim's time runs
///    out, and there is nothing for a person to answer).
/// 4. Queued, unclaimed, a reason, no try counted: `held`.
/// 5. Queued, unclaimed, a reason, tries counted: `waiting`.
/// 6. Queued, unclaimed, no reason: `queued`.
/// 7. Submitted as a dry run: `test_sent`.
/// 8. Submitted with a relay receipt: `sent`.
pub fn send_state(facts: &SendFacts) -> Option<SendState> {
    let reason = facts.reason.map(str::trim).filter(|text| !text.is_empty());
    let next_try_at = facts.next_retry_at.map(str::to_string);
    match facts.status {
        FilingStatus::Draft => {
            let reason = reason?;
            if reason
                == bir_core::db::abandoned_claim_audit(bir_core::db::ABANDONED_CLAIM_RELEASE_REASON)
            {
                return None;
            }
            let (line, code) = split_reason(reason);
            let mut state = SendState::new("refused", plain_reason(line));
            state.code = code.map(str::to_string);
            Some(state)
        }
        FilingStatus::Queued if facts.claimed => Some(if facts.in_flight {
            SendState::new("sending", SENDING_LINE)
        } else if facts.relay_pending {
            SendState::new("checking", CHECKING_LINE)
        } else if !facts.put_started {
            SendState::new("queued", QUEUED_LINE)
        } else {
            let mut state = SendState::new(
                "needs_attention",
                bir_core::background_cron::OUTCOME_UNKNOWN,
            );
            state.can_resolve = true;
            state
        }),
        FilingStatus::Queued => Some(match reason {
            None => SendState::new("queued", QUEUED_LINE),
            Some(reason) => {
                let (text, _) = split_reason(reason);
                let text = plain_reason(text);
                let text = text.as_ref();
                let mut state = if facts.attempts == 0 {
                    SendState::new("held", text)
                } else {
                    let stop = if text.ends_with(['.', '!', '?']) {
                        ""
                    } else {
                        "."
                    };
                    SendState::new(
                        "waiting",
                        format!("Not sent yet: {text}{stop} It tries again by itself."),
                    )
                };
                state.next_try_at = next_try_at;
                state
            }
        }),
        FilingStatus::Submitted if facts.submitted_dry_run => {
            Some(SendState::new("test_sent", TEST_SENT_LINE))
        }
        FilingStatus::Submitted if facts.has_relay_receipt => {
            Some(SendState::new("sent", SENT_LINE))
        }
        _ => None,
    }
}

/// The facts of a return on the generic submission path.
pub fn facts_of_lifecycle(lifecycle: &SubmissionLifecycle, in_flight: bool) -> SendFacts<'_> {
    SendFacts {
        status: &lifecycle.status,
        claimed: !lifecycle.is_unclaimed(),
        put_started: lifecycle.submission_put_started_at.is_some(),
        in_flight,
        attempts: lifecycle.submission_attempts,
        reason: lifecycle.submission_error.as_deref(),
        next_retry_at: lifecycle.next_retry_at.as_deref(),
        submitted_dry_run: lifecycle.submitted_dry_run,
        has_relay_receipt: lifecycle.relay_receipt.is_some(),
        relay_pending: lifecycle.relay_pending.is_some(),
    }
}

/// The facts of a 1601C (its lifecycle is inline).
pub fn facts_of_1601c(draft: &Form1601CDraft, in_flight: bool) -> SendFacts<'_> {
    SendFacts {
        status: &draft.status,
        claimed: draft.submission_claim_token.is_some() || draft.submission_claimed_at.is_some(),
        put_started: draft.submission_put_started_at.is_some(),
        in_flight,
        attempts: draft.submission_attempts,
        reason: draft.submission_error.as_deref(),
        next_retry_at: draft.next_retry_at.as_deref(),
        submitted_dry_run: draft.submitted_dry_run,
        has_relay_receipt: draft.relay_receipt.is_some(),
        relay_pending: draft.relay_pending.is_some(),
    }
}

/// The facts of a 2551Q (its lifecycle is inline; its reason is `last_error`).
pub fn facts_of_2551q(draft: &Form2551QDraft, in_flight: bool) -> SendFacts<'_> {
    SendFacts {
        status: &draft.status,
        claimed: draft.submission_claim_token.is_some() || draft.submission_claimed_at.is_some(),
        put_started: draft.submission_put_started_at.is_some(),
        in_flight,
        attempts: draft.submission_attempts,
        reason: draft.last_error.as_deref(),
        next_retry_at: draft.next_retry_at.as_deref(),
        submitted_dry_run: draft.submitted_dry_run,
        has_relay_receipt: draft.relay_receipt.is_some(),
        relay_pending: draft.relay_pending.is_some(),
    }
}

/// Whether this process's submission worker is sending the return now: it
/// holds the return's job key ([`bir_core::background_cron::submission_job_key`]).
pub fn in_flight(summary: &FormDraftSummary) -> bool {
    let key = bir_core::background_cron::submission_job_key(
        &summary.form_code,
        &summary.tin,
        summary.taxable_year,
        summary.month,
        summary.quarter,
    );
    bir_core::background_cron::get_active_jobs()
        .lock()
        .map(|jobs| jobs.contains(&key))
        .unwrap_or(false)
}

/// The send state of a stored return (its summary and stored JSON). `None`
/// for a form that is never queued and for stored JSON its shape cannot read.
pub fn send_of_stored(summary: &FormDraftSummary, json: &str) -> Option<SendState> {
    // Rule 9 needs no read of the JSON.
    if matches!(summary.status, FilingStatus::Confirmed | FilingStatus::Paid) {
        return None;
    }
    let flying = in_flight(summary);
    match summary.form_code.as_str() {
        "2551Q" => crate::screen_form_module::out_of_line(|| {
            let draft: Form2551QDraft = serde_json::from_str(json).ok()?;
            send_state(&facts_of_2551q(&draft, flying))
        }),
        "1601C" => crate::screen_form_module::out_of_line(|| {
            let draft: Form1601CDraft = serde_json::from_str(json).ok()?;
            send_state(&facts_of_1601c(&draft, flying))
        }),
        code => {
            QueueableKind::from_form_code(code)?;
            // Every generic form flattens its lifecycle into the stored JSON.
            let lifecycle: SubmissionLifecycle = serde_json::from_str(json).ok()?;
            send_state(&facts_of_lifecycle(&lifecycle, flying))
        }
    }
}

/// `send` of the stored return `draft_id`: the JSON of its [`SendState`], or
/// null.
pub fn stored_send(db: &Database, draft_id: i64) -> Result<Value, bir_core::db::DbError> {
    Ok(db
        .draft_summary_and_json(draft_id)?
        .and_then(|(summary, json)| send_of_stored(&summary, &json))
        .map_or(Value::Null, |state| {
            serde_json::to_value(state).unwrap_or(Value::Null)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::background_cron::OUTCOME_UNKNOWN;

    const UNTIL: &str = "2026-11-24T07:40:00+00:00";

    fn facts(status: &FilingStatus) -> SendFacts<'_> {
        SendFacts {
            status,
            claimed: false,
            put_started: false,
            in_flight: false,
            attempts: 0,
            reason: None,
            next_retry_at: None,
            submitted_dry_run: false,
            has_relay_receipt: false,
            relay_pending: false,
        }
    }

    fn state(
        stage: &'static str,
        line: &str,
        code: Option<&str>,
        next: Option<&str>,
        resolve: bool,
    ) -> Option<SendState> {
        Some(SendState {
            stage,
            line: line.to_string(),
            code: code.map(str::to_string),
            next_try_at: next.map(str::to_string),
            can_resolve: resolve,
        })
    }

    #[test]
    fn every_rule_and_the_boundaries_between_them() {
        use FilingStatus::*;
        let released =
            bir_core::db::abandoned_claim_audit(bir_core::db::ABANDONED_CLAIM_RELEASE_REASON);
        let unknown = format!("{OUTCOME_UNKNOWN} (the upload timed out)");
        type Case<'a> = (&'a str, SendFacts<'a>, Option<SendState>);
        let rows: Vec<Case> = vec![
            // 1. Draft.
            ("draft, no reason", facts(&Draft), None),
            (
                "draft, blank reason",
                SendFacts {
                    reason: Some("  "),
                    ..facts(&Draft)
                },
                None,
            ),
            (
                "draft refused with a code",
                SendFacts {
                    reason: Some("This TIN is not on your Buwiz account. (tin_not_active)"),
                    ..facts(&Draft)
                },
                state(
                    "refused",
                    "This TIN is not on your Buwiz account.",
                    Some("tin_not_active"),
                    None,
                    false,
                ),
            ),
            (
                "draft refused without a code",
                SendFacts {
                    reason: Some("The file does not match its name."),
                    ..facts(&Draft)
                },
                state(
                    "refused",
                    "The file does not match its name.",
                    None,
                    None,
                    false,
                ),
            ),
            (
                "draft: brackets that are no code stay in the line",
                SendFacts {
                    reason: Some("Refused (HTTP 403)"),
                    ..facts(&Draft)
                },
                state("refused", "Refused (HTTP 403)", None, None, false),
            ),
            (
                "draft: a code glued to the word is no code",
                SendFacts {
                    reason: Some("Refused(tin_not_active)"),
                    ..facts(&Draft)
                },
                state("refused", "Refused(tin_not_active)", None, None, false),
            ),
            (
                "draft a person released after no email came",
                SendFacts {
                    reason: Some(&released),
                    ..facts(&Draft)
                },
                None,
            ),
            (
                "draft: a stale claim or count does not matter",
                SendFacts {
                    reason: Some("No. (tin_needs_fix)"),
                    claimed: true,
                    attempts: 3,
                    ..facts(&Draft)
                },
                state("refused", "No.", Some("tin_needs_fix"), None, false),
            ),
            // 2 and 3. Queued and claimed.
            (
                "claimed, in flight",
                SendFacts {
                    claimed: true,
                    put_started: true,
                    in_flight: true,
                    reason: Some("Submission outcome pending."),
                    ..facts(&Queued)
                },
                state("sending", SENDING_LINE, None, None, false),
            ),
            (
                "claimed, in flight before PUT",
                SendFacts {
                    claimed: true,
                    in_flight: true,
                    ..facts(&Queued)
                },
                state("sending", SENDING_LINE, None, None, false),
            ),
            (
                "claimed, not in flight: unknown outcome",
                SendFacts {
                    claimed: true,
                    put_started: true,
                    reason: Some(&unknown),
                    next_retry_at: Some(UNTIL),
                    attempts: 2,
                    ..facts(&Queued)
                },
                state("needs_attention", OUTCOME_UNKNOWN, None, None, true),
            ),
            (
                "claimed, not in flight, a relay send noted: Buwiz is asked, nothing to answer",
                SendFacts {
                    claimed: true,
                    put_started: true,
                    relay_pending: true,
                    reason: Some(&unknown),
                    ..facts(&Queued)
                },
                state("checking", CHECKING_LINE, None, None, false),
            ),
            (
                "claimed, in flight with a relay send noted: sending",
                SendFacts {
                    claimed: true,
                    put_started: true,
                    relay_pending: true,
                    in_flight: true,
                    ..facts(&Queued)
                },
                state("sending", SENDING_LINE, None, None, false),
            ),
            (
                "unclaimed: a stale noted send says nothing",
                SendFacts {
                    relay_pending: true,
                    ..facts(&Queued)
                },
                state("queued", QUEUED_LINE, None, None, false),
            ),
            (
                "claimed, not in flight, no PUT: nothing left this computer, nothing to answer",
                SendFacts {
                    claimed: true,
                    reason: Some("Submission outcome pending. Automatic retry is disabled"),
                    ..facts(&Queued)
                },
                state("queued", QUEUED_LINE, None, None, false),
            ),
            (
                "claimed, not in flight, PUT started, no reason",
                SendFacts {
                    claimed: true,
                    put_started: true,
                    ..facts(&Queued)
                },
                state("needs_attention", OUTCOME_UNKNOWN, None, None, true),
            ),
            (
                "unclaimed after an interrupted claim: the worker's note in plain words",
                SendFacts {
                    reason: Some(
                        "Claim lease expired before PUT started; retrying the same authorized task",
                    ),
                    next_retry_at: Some(UNTIL),
                    ..facts(&Queued)
                },
                state(
                    "held",
                    "Sending was interrupted before it started.",
                    None,
                    Some(UNTIL),
                    false,
                ),
            ),
            (
                "a Draft after five failed connections: plain words, no jargon",
                SendFacts {
                    reason: Some(
                        "BIR SFTP session failed before upload (no PUT attempted): timeout",
                    ),
                    ..facts(&Draft)
                },
                state(
                    "refused",
                    "Could not connect to send this return.",
                    None,
                    None,
                    false,
                ),
            ),
            // 4, 5 and 6. Queued and unclaimed.
            (
                "held, no try counted",
                SendFacts {
                    reason: Some("Buwiz is holding sends for now. (send_held)"),
                    next_retry_at: Some(UNTIL),
                    ..facts(&Queued)
                },
                state(
                    "held",
                    "Buwiz is holding sends for now.",
                    None,
                    Some(UNTIL),
                    false,
                ),
            ),
            (
                "held without a time",
                SendFacts {
                    reason: Some("Sign in to send returns."),
                    ..facts(&Queued)
                },
                state("held", "Sign in to send returns.", None, None, false),
            ),
            (
                "waiting after one try, the reason with its full stop",
                SendFacts {
                    reason: Some("BIR is not accepting returns right now. (bir_offline)"),
                    attempts: 1,
                    next_retry_at: Some(UNTIL),
                    ..facts(&Queued)
                },
                state(
                    "waiting",
                    "Not sent yet: BIR is not accepting returns right now. It tries again by itself.",
                    None,
                    Some(UNTIL),
                    false,
                ),
            ),
            (
                "waiting, a full stop put after the reason",
                SendFacts {
                    reason: Some("BIR's upload server did not answer"),
                    attempts: 4,
                    next_retry_at: Some(UNTIL),
                    ..facts(&Queued)
                },
                state(
                    "waiting",
                    "Not sent yet: BIR's upload server did not answer. It tries again by itself.",
                    None,
                    Some(UNTIL),
                    false,
                ),
            ),
            (
                "queued, nothing kept",
                SendFacts {
                    next_retry_at: Some(UNTIL),
                    ..facts(&Queued)
                },
                state("queued", QUEUED_LINE, None, None, false),
            ),
            (
                "queued, a blank reason is none",
                SendFacts {
                    reason: Some(""),
                    attempts: 2,
                    ..facts(&Queued)
                },
                state("queued", QUEUED_LINE, None, None, false),
            ),
            (
                "queued, in flight before the claim",
                SendFacts {
                    in_flight: true,
                    ..facts(&Queued)
                },
                state("queued", QUEUED_LINE, None, None, false),
            ),
            // 7, 8 and 9. Submitted and later.
            (
                "dry run",
                SendFacts {
                    submitted_dry_run: true,
                    ..facts(&Submitted)
                },
                state("test_sent", TEST_SENT_LINE, None, None, false),
            ),
            (
                "dry run with a receipt is still a test",
                SendFacts {
                    submitted_dry_run: true,
                    has_relay_receipt: true,
                    ..facts(&Submitted)
                },
                state("test_sent", TEST_SENT_LINE, None, None, false),
            ),
            (
                "sent with a receipt",
                SendFacts {
                    has_relay_receipt: true,
                    ..facts(&Submitted)
                },
                state("sent", SENT_LINE, None, None, false),
            ),
            ("submitted without a receipt", facts(&Submitted), None),
            (
                "confirmed",
                SendFacts {
                    has_relay_receipt: true,
                    submitted_dry_run: true,
                    ..facts(&Confirmed)
                },
                None,
            ),
            (
                "paid",
                SendFacts {
                    has_relay_receipt: true,
                    reason: Some("x (code)"),
                    ..facts(&Paid)
                },
                None,
            ),
        ];
        for (name, facts, want) in rows {
            assert_eq!(send_state(&facts), want, "{name}");
        }
    }

    #[test]
    fn a_reason_splits_into_its_text_and_its_trailing_code() {
        assert_eq!(
            split_reason("Text. (tin_unverified)"),
            ("Text.", Some("tin_unverified"))
        );
        assert_eq!(
            split_reason("(account_banned)"),
            ("", Some("account_banned"))
        );
        assert_eq!(split_reason("  Text (a_b) "), ("Text", Some("a_b")));
        assert_eq!(split_reason("Text (Tin)"), ("Text (Tin)", None));
        assert_eq!(split_reason("Text (tin 2)"), ("Text (tin 2)", None));
        assert_eq!(split_reason("Text ()"), ("Text ()", None));
        assert_eq!(split_reason("Text"), ("Text", None));
    }

    #[test]
    fn the_answer_has_the_declared_keys() {
        let value = serde_json::to_value(
            send_state(&SendFacts {
                claimed: true,
                put_started: true,
                ..facts(&FilingStatus::Queued)
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "stage": "needs_attention",
                "line": OUTCOME_UNKNOWN,
                "code": null,
                "next_try_at": null,
                "can_resolve": true,
            })
        );
    }
}
