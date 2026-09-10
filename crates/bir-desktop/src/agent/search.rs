//! Shared taxpayer search used by the Command Palette and the agent host.
//!
//! Ranking is the palette contract: exact TIN (digits or formatted) beats
//! name/substring matches; a create suggestion appears only when the query
//! is non-empty and not an exact TIN or exact name.

use bir_core::profile::TaxpayerProfile;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct PaletteSearch {
    pub matches: Vec<TaxpayerProfile>,
    pub can_create: bool,
    pub create_query: Option<String>,
}

/// Outer-agent prompt shape. Does not include PIN hashes or TOTP secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileHit {
    pub tin: String,
    pub name: String,
    pub last4: String,
    pub selected: bool,
    pub archived: bool,
}

impl ProfileHit {
    pub fn from_profile(profile: &TaxpayerProfile, selected_tin: Option<&str>) -> Self {
        let tin = profile.tin.full();
        Self {
            last4: tin_last4(&tin),
            selected: selected_tin == Some(tin.as_str()),
            tin,
            name: profile.full_name.clone(),
            archived: profile.is_archived,
        }
    }

    pub fn from_listed(tin: &str, name: &str, selected: bool, archived: bool) -> Self {
        Self {
            last4: tin_last4(tin),
            tin: tin.to_string(),
            name: name.to_string(),
            selected,
            archived,
        }
    }
}

pub fn tin_last4(tin: &str) -> String {
    let digits: String = tin.chars().filter(|c| c.is_ascii_digit()).collect();
    digits
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn query_matches_exact_tin(profile: &TaxpayerProfile, q: &str) -> bool {
    let trimmed = q.trim();
    profile.tin.full() == trimmed || profile.tin.formatted().to_lowercase() == trimmed
}

/// Command Palette ranking. `hide_tax_profiles` only allows an exact TIN hit
/// (same as the Cmd+K UI). Non-hidden search is case-insensitive name or TIN
/// substring and is capped at 5 rows.
pub fn search_profiles_for_palette(
    all: &[TaxpayerProfile],
    query: &str,
    hide_tax_profiles: bool,
) -> PaletteSearch {
    let q = query.to_lowercase();
    let filtered: Vec<TaxpayerProfile> = if hide_tax_profiles {
        if q.is_empty() {
            Vec::new()
        } else {
            all.iter()
                .filter(|profile| query_matches_exact_tin(profile, &q))
                .cloned()
                .collect()
        }
    } else {
        all.iter()
            .filter(|profile| {
                q.is_empty()
                    || profile.full_name.to_lowercase().contains(&q)
                    || profile.tin.full().contains(&q)
                    || profile.tin.formatted().to_lowercase().contains(&q)
            })
            .take(5)
            .cloned()
            .collect()
    };

    let exact_tin_match = all
        .iter()
        .any(|profile| query_matches_exact_tin(profile, &q));
    let exact_name_match = all
        .iter()
        .any(|profile| profile.full_name.to_lowercase() == q.trim());
    let create_query = if !query.trim().is_empty() && !exact_tin_match && !exact_name_match {
        Some(query.trim().to_string())
    } else {
        None
    };

    PaletteSearch {
        matches: filtered,
        can_create: create_query.is_some(),
        create_query,
    }
}

/// Agent `profile.search`: TIN substring or name, case-insensitive.
/// Empty `q` lists every profile. Does not create and does not cap at 5.
pub fn search_listed(profiles: &[(String, String)], query: &str) -> Vec<(String, String)> {
    let q = query.trim().to_lowercase();
    profiles
        .iter()
        .filter(|(tin, name)| {
            q.is_empty() || tin.to_lowercase().contains(&q) || name.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, tin: &str) -> TaxpayerProfile {
        let digits: String = tin.chars().filter(|c| c.is_ascii_digit()).collect();
        let padded = format!("{:0<14}", digits);
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": name,
            "tin": {
                "segment1": &padded[0..3],
                "segment2": &padded[3..6],
                "segment3": &padded[6..9],
                "branch": &padded[9..14],
            },
            "rdo_code": "018",
            "line_of_business": "Retail",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "09170000000",
            "email": "a@example.com",
            "default_form_type": "1601Cv2018",
            "taxpayer_type": "Corporation",
        }))
        .expect("profile")
    }

    #[test]
    fn palette_caps_and_offers_create_when_no_exact_match() {
        let all = vec![
            profile("Alpha Store", "11111111100000"),
            profile("Beta Shop", "22222222200000"),
        ];
        let ranked = search_profiles_for_palette(&all, "store", false);
        assert_eq!(ranked.matches.len(), 1);
        assert!(ranked.can_create);
        assert_eq!(ranked.create_query.as_deref(), Some("store"));
    }

    #[test]
    fn agent_search_empty_query_lists_all() {
        let listed = vec![
            ("11111111100000".into(), "Alpha".into()),
            ("22222222200000".into(), "Beta".into()),
        ];
        assert_eq!(search_listed(&listed, "").len(), 2);
        assert_eq!(search_listed(&listed, "BETA").len(), 1);
        assert!(search_listed(&listed, "zzz").is_empty());
    }
}
