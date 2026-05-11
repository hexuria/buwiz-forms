//! Backward compatibility adapter.
//!
//! Wraps the new `TemporalEngine` behind the old `applicable_forms_for_profile()` API
//! so existing dashboard code works unchanged.

use crate::profile::TaxpayerProfile;
use crate::temporal::engine::TemporalEngine;
use chrono::Datelike;

/// Drop-in replacement for the old `applicable_forms_for_profile()`.
///
/// Uses the temporal engine with `target_year = current year`.
pub fn applicable_forms_temporal(profile: &TaxpayerProfile) -> Vec<String> {
    let engine = TemporalEngine::default();
    let current_year = chrono::Local::now().year() as u16;
    engine.visible_form_codes(profile, current_year)
}
