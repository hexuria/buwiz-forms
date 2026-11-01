//! Whether a payment is recorded for a return.
//!
//! The mini app records payments (`mini_app::payments`, agent builds only)
//! into the database's settings. Every build reads them: a paid return opens
//! without stale penalties whichever app recorded the payment.

use bir_core::db::Database;
use serde::Deserialize;

/// Settings key of every recorded payment (a JSON list).
pub(crate) const PAYMENTS_KEY: &str = "mini_app.payments";

/// The fields of a stored payment that name its return.
#[derive(Deserialize)]
struct PaymentTarget {
    /// The branch's full TIN, digits only.
    tin: String,
    form: String,
    period: String,
}

/// Whether a payment is recorded for the return `tin` (9 or 14 digits)
/// files as `form` for `period` (its canonical text, `2026-Q3`).
pub(crate) fn recorded_for(db: &Database, tin: &str, form: &str, period: &str) -> bool {
    let branch = |tin: &str| {
        let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
        if digits.len() == 9 {
            format!("{digits}00000")
        } else {
            digits
        }
    };
    let tin = branch(tin);
    let Ok(Some(stored)) = db.get_setting(PAYMENTS_KEY) else {
        return false;
    };
    serde_json::from_str::<Vec<PaymentTarget>>(&stored).is_ok_and(|payments| {
        payments.iter().any(|payment| {
            branch(&payment.tin) == tin
                && payment.form.eq_ignore_ascii_case(form)
                && payment.period == period
        })
    })
}
