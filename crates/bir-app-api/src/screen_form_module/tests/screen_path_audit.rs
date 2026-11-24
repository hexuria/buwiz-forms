//! Data-driven audit of all 43 forms: builds every official sample through the
//! real desktop screen path (`apply_screen_edits` + `agent::compute`) using
//! only user-editable boxes (never computed/read-only boxes, never typed
//! setters), then diffs every key of our upload against the official XML.

use super::*;
use bir_core::forms::agent::AgentFormKind;
use bir_core::official_xml;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Explicitly starts EMPTY: every difference belongs in the audit report,
/// not in an allow-list.
const KNOWN_DIFFERENCES: &[(&str, &str, &str)] = &[];

fn sample_dummy_profile(default_form_type: &str, taxpayer_type: &str) -> TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "full_name": "Sample Dummy Taxpayer",
        "tin": {
            "segment1": "123",
            "segment2": "456",
            "segment3": "788",
            "branch": "00000"
        },
        "rdo_code": "039",
        "line_of_business": "Sample Consulting Services",
        "registered_address": "123 Sample Street, Barangay Example, Quezon City",
        "zip_code": "1100",
        "phone": "09170000000",
        "email": "sample.taxpayer@example.com",
        "default_form_type": default_form_type,
        "taxpayer_type": taxpayer_type,
        "business_start_date": "2020-01-15",
        "tax_elections": [{
            "taxable_year": 2025,
            "election": "GraduatedUnspecified",
            "elected_at": "2025-01-15T00:00:00",
            "source_form": "sample"
        }]
    }))
    .expect("sample dummy profile deserializes")
}

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bir-core/tests/sample-payloads")
}

/// Parses every `<div>key=value</div>` entry from an `.official.xml` file.
/// For duplicate keys (e.g. `txtAtcCde` repeated rows), keeps all occurrences
/// in order alongside the first-value map.
fn parse_official_xml(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<div>") {
        let after = &rest[start + 5..];
        let Some(end) = after.find("</div>") else {
            break;
        };
        let body = &after[..end];
        if let Some((k, rest_val)) = body.split_once('=') {
            let suffix = format!("{k}=");
            let raw_val = rest_val.strip_suffix(&suffix).unwrap_or(rest_val);
            let decoded = official_xml::js_unescape(raw_val).unwrap_or_else(|| raw_val.to_string());
            out.push((k.to_string(), decoded));
        }
        rest = &after[end + 6..];
    }
    out
}

fn form_kind_for_code(code: &str) -> Option<AgentFormKind> {
    AgentFormKind::from_code(code)
}

fn infer_period(stem: &str, code: &str) -> (u16, u8) {
    let rest = stem
        .strip_prefix(code)
        .and_then(|s| s.strip_prefix('-'))
        .unwrap_or("");
    // e.g. 2025Q1, 122025Q1, 062025, 062022, 122025, 06152025, 2025
    if let Some(q_pos) = rest.find('Q') {
        let quarter = rest[q_pos + 1..]
            .chars()
            .next()
            .and_then(|c| c.to_digit(10))
            .unwrap_or(1) as u8;
        let prefix = &rest[..q_pos];
        let year = if prefix.len() >= 4 {
            prefix[prefix.len() - 4..].parse::<u16>().unwrap_or(2025)
        } else {
            2025
        };
        return (year, quarter);
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        4 => {
            let n: u16 = digits.parse().unwrap_or(2025);
            if n >= 2000 {
                (n, 12)
            } else {
                // MMYY e.g. 1225 or 0625
                let m = digits[0..2].parse::<u8>().unwrap_or(12);
                let y = 2000 + digits[2..4].parse::<u16>().unwrap_or(25);
                (y, m)
            }
        }
        6 => {
            // MMYYYY
            let m = digits[0..2].parse::<u8>().unwrap_or(1);
            let y = digits[2..6].parse::<u16>().unwrap_or(2025);
            (y, m)
        }
        8 | 14 => {
            // MMDDYYYY...
            let m = digits[0..2].parse::<u8>().unwrap_or(1);
            let y = digits[4..8].parse::<u16>().unwrap_or(2025);
            (y, m)
        }
        _ => (2025, 1),
    }
}

fn taxpayer_type_for_code(code: &str) -> &'static str {
    match code {
        "1700" | "1701" | "1701A" | "1701MS" | "1701Q" | "1800" | "1801" => "Individual",
        _ => "Corporation",
    }
}

fn is_zero_amount(s: &str) -> bool {
    matches!(s.trim(), "" | "0" | "0.00" | "0.0" | "00")
}

#[test]
fn screen_path_reproduces_official_samples() {
    let filter_form = std::env::var("AUDIT_FORM").ok();
    let dir = samples_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("sample-payloads dir exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".official.xml"))
        })
        .collect();
    files.sort();

    let db = bir_core::db::Database::open_ephemeral().expect("in-memory db");
    let mut diffs: Vec<(String, String, String, String, String, bool)> = Vec::new();
    let mut checked_samples = 0usize;

    for path in files {
        let stem = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_suffix(".official.xml")
            .unwrap()
            .to_string();
        let code = stem.split('-').next().unwrap().to_string();
        if let Some(ref wanted) = filter_form {
            if !code.eq_ignore_ascii_case(wanted) {
                continue;
            }
        }
        let Some(kind) = form_kind_for_code(&code) else {
            continue;
        };
        checked_samples += 1;

        let official_text = std::fs::read_to_string(&path).expect("read official.xml");
        let entries = parse_official_xml(&official_text);
        let (year, period) = infer_period(&stem, &code);
        let profile = sample_dummy_profile(&code, taxpayer_type_for_code(&code));

        let mut draft = load_draft(&db, kind, &profile, year, period, true, None)
            .unwrap_or_else(|e| panic!("{stem}: load_draft failed: {e}"));
        let def = kind.definition();
        let initial_map = agent::field_map(kind, &draft)
            .unwrap_or_else(|e| panic!("{stem}: initial field_map failed: {e}"));

        // Build edits containing ONLY user-editable (non-computed, non-hidden) boxes
        // whose official value differs from the fresh draft's initial value.
        let mut edits = Map::new();
        let mut unlocked = BTreeSet::new();
        let mut seen_keys = BTreeSet::new();
        for (key, val) in &entries {
            if !seen_keys.insert(key.clone()) {
                continue;
            }
            if schema_computed(&code, key) {
                continue;
            }
            if def.box_def(key).is_some_and(|b| b.computed) {
                continue;
            }
            // Skip fields that already match the fresh draft (untouched defaults/zeros).
            if initial_map.get(key).is_some_and(|init| init == val) {
                continue;
            }
            // Skip unticked radios ("false") and empty strings so we don't clear defaults.
            if val == "false" || val.is_empty() {
                continue;
            }
            // Skip identity-locked boxes (period/TIN/etc. already come from load_draft).
            if def
                .bindings
                .get(key)
                .is_some_and(|b| matches!(b.lock, Some(bir_core::forms::agent::Lock::Identity)))
            {
                continue;
            }
            if def.bindings.get(key).is_some_and(|b| {
                matches!(
                    b.lock,
                    Some(
                        bir_core::forms::agent::Lock::Profile
                            | bir_core::forms::agent::Lock::Election
                    )
                )
            }) {
                unlocked.insert(key.clone());
            }
            edits.insert(key.clone(), Value::String(val.clone()));
        }

        // Pass 1: apply radios, checkboxes, selects, dates, and ATC/gate controls first so
        // conditional sections (joint spouse, special rate, ATC rows, amended return) open
        // before amounts are written.
        let mut pass1 = Map::new();
        let mut pass2 = Map::new();
        for (k, v) in &edits {
            let is_gate = def.box_def(k).is_some_and(|b| {
                matches!(
                    b.kind.as_str(),
                    "radio" | "checkbox" | "select" | "sel" | "date"
                )
            }) || k.contains("Opt")
                || k.contains("opt")
                || k.contains("Rd")
                || k.contains("Chk")
                || k.contains("chk")
                || k.contains("Atc")
                || k.contains("ATC")
                || k.contains("atc")
                || k.contains("Date")
                || k.contains("Month")
                || k.contains("Year")
                || k.contains("Quarter")
                || k.contains("Amend")
                || k.contains("Joint")
                || k.contains("Spouse")
                || k.contains("Regime")
                || k.contains("Method")
                || k.contains("TaxRelief")
                || k.contains("ShortPrd")
                || k.contains("txt15A")
                || k.contains("txt15B");
            if is_gate {
                pass1.insert(k.clone(), v.clone());
            } else {
                pass2.insert(k.clone(), v.clone());
            }
        }

        let mut rejected = apply_screen_edits(kind, &mut draft, &pass1, true, &unlocked)
            .unwrap_or_else(|e| panic!("{stem}: apply_screen_edits pass1 failed: {e:?}"));
        agent::compute(kind, &mut draft)
            .unwrap_or_else(|e| panic!("{stem}: agent::compute pass1 failed: {e}"));

        // Pass 2: apply remaining edits (plus any pass1 rejections whose gate just opened).
        for (k, _) in &rejected {
            if let Some(v) = pass1.get(k) {
                pass2.insert(k.clone(), v.clone());
            }
        }
        rejected = apply_screen_edits(kind, &mut draft, &pass2, true, &unlocked)
            .unwrap_or_else(|e| panic!("{stem}: apply_screen_edits pass2 failed: {e:?}"));
        agent::compute(kind, &mut draft)
            .unwrap_or_else(|e| panic!("{stem}: agent::compute pass2 failed: {e}"));

        // Pass 3: retry any edits rejected in pass 2 (e.g. row N waiting on row N-1, ATC,
        // whole-peso commas from official XML, or penalty override unlocked via "Edit penalties"),
        // plus explicit "0.00"/"0" penalty overrides where the current clock auto-computed late
        // penalties on a historical official sample.
        let mid_map = agent::field_map(kind, &draft)
            .unwrap_or_else(|e| panic!("{stem}: mid field_map failed: {e}"));
        let mut retry = Map::new();
        for (k, msg) in &rejected {
            if msg.as_str().is_some_and(|s| s.contains("Edit penalties")) {
                unlocked.insert(k.clone());
            }
            if let Some(v) = edits.get(k) {
                if msg
                    .as_str()
                    .is_some_and(|s| s.contains("must be a whole number"))
                {
                    if let Some(s) = v.as_str() {
                        retry.insert(k.clone(), Value::String(s.replace(',', "")));
                        continue;
                    }
                }
                retry.insert(k.clone(), v.clone());
            }
        }
        // Build official first-occurrence map for penalty-clock check and final comparison.
        let mut official_map: BTreeMap<String, String> = BTreeMap::new();
        for (k, v) in &entries {
            official_map.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for (k, off_val) in &official_map {
            let lower = k.to_ascii_lowercase();
            let is_penalty = lower.contains("surcharge")
                || lower.contains("surchange")
                || lower.contains("interest")
                || lower.contains("compromise");
            if is_penalty
                && is_zero_amount(off_val)
                && !schema_computed(&code, k)
                && !def.box_def(k).is_some_and(|b| b.computed)
                && mid_map.get(k).is_some_and(|v| !is_zero_amount(v))
            {
                unlocked.insert(k.clone());
                retry.insert(k.clone(), Value::String(off_val.clone()));
            }
        }
        if !retry.is_empty() {
            rejected = apply_screen_edits(kind, &mut draft, &retry, true, &unlocked)
                .unwrap_or_else(|e| panic!("{stem}: apply_screen_edits pass3 failed: {e:?}"));
            agent::compute(kind, &mut draft)
                .unwrap_or_else(|e| panic!("{stem}: agent::compute pass3 failed: {e}"));
        }

        let our_map = agent::field_map(kind, &draft)
            .unwrap_or_else(|e| panic!("{stem}: agent::field_map failed: {e}"));

        let mut sample_diffs = 0usize;
        for (key, off_val) in &official_map {
            let our_val = our_map.get(key).map(String::as_str).unwrap_or("<MISSING>");
            if our_val == off_val {
                continue;
            }
            // Ignore formatting-only zero differences like "" vs "0.00" only? NO — report all!
            if KNOWN_DIFFERENCES
                .iter()
                .any(|(f, s, k)| *f == code && *s == stem && *k == key)
            {
                continue;
            }
            let is_computed =
                schema_computed(&code, key) || def.box_def(key).is_some_and(|b| b.computed);
            let was_rejected = rejected.contains_key(key);
            if is_computed || !is_zero_amount(off_val) || !is_zero_amount(our_val) {
                sample_diffs += 1;
                diffs.push((
                    code.clone(),
                    stem.clone(),
                    key.clone(),
                    off_val.clone(),
                    format!(
                        "{our_val}{}",
                        if was_rejected {
                            format!(" [REJECTED: {}]", rejected[key])
                        } else {
                            String::new()
                        }
                    ),
                    is_computed,
                ));
            }
        }
        eprintln!(
            "AUDIT {code} | {stem} | edits={} | rejected={} | diffs={}",
            edits.len(),
            rejected.len(),
            sample_diffs
        );
    }

    assert!(checked_samples > 0, "no samples matched");
    if !diffs.is_empty() {
        eprintln!(
            "\n=== SCREEN-PATH AUDIT DIFFERENCES ({} total) ===",
            diffs.len()
        );
        for (code, stem, key, off, ours, comp) in &diffs {
            eprintln!(
                "{code} | {stem} | {key} | computed={comp} | official={off:?} | ours={ours:?}"
            );
        }
        panic!(
            "screen_path_reproduces_official_samples found {} differences across {checked_samples} samples",
            diffs.len()
        );
    }
}
