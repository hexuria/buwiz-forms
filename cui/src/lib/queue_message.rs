//! What a person reads when a cancel-queue is refused, said once for every screen that offers the button.
//! The submission worker sends a queued return within a second or two, so a Cancel queue pressed on a stale
//! card or page is refused by the host: the person reads that the return was already sent. Pure.

use super::errors::error_text;

/// What a person reads when the return had been sent (or claimed by the worker) before the cancel got there.
pub const ALREADY_SENT: &str = "This return was already sent.";

/// Whether the host's refusal is "only an unclaimed queued snapshot can be canceled".
pub fn is_already_sent(message: &str) -> bool {
    let text = error_text(message).to_lowercase();
    text.contains("unclaimed queued") && text.contains("can be canceled")
}

/// The sentence a failed cancel-queue shows: the plain one for a return that was sent, else the host's own
/// words without its "Other error:" label.
pub fn cancel_queue_message(message: &str) -> String {
    if is_already_sent(message) {
        ALREADY_SENT.to_owned()
    } else {
        error_text(message)
            .trim_start_matches("Other error: ")
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_cancel_reads_as_already_sent() {
        let host = "Other error: Only an unclaimed queued 1601C snapshot can be canceled";
        assert!(is_already_sent(host));
        assert_eq!(cancel_queue_message(host), ALREADY_SENT);
        assert_eq!(
            cancel_queue_message("Other error: the database is locked"),
            "the database is locked"
        );
    }
}
