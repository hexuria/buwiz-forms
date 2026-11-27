//! Dummy-data sample payloads for the queueable forms.
//!
//! `tests/sample-payloads/` holds what our serializers produce for a fake
//! taxpayer: the plaintext BIR pseudo-XML and the encrypted IAF file that is
//! uploaded (`dump_xml <file>` decrypts it back). No real taxpayer data.
//!
//! `<stem>.official.xml` is what the official eBIRForms page submits when a
//! filer enters `<stem>.steps.json`: `tools/official-xml/runtime.js` runs the
//! official HTA with its own scripts, so the official handlers compute and
//! format every derived amount, then its `saveXMLsubmit()` loop writes the
//! plaintext. Our plaintext must equal it byte for byte, which checks our
//! calculations, value formatting and serialization together. Regenerating our
//! samples never changes it.
//!
//! Regenerate after an intentional serializer change with
//! `UPDATE_SAMPLE_PAYLOADS=1 cargo test -p bir-core --test sample_payloads_test`.

use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::official_xml;
use bir_core::profile::TaxpayerProfile;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// One module per form: its dummy draft and its sample test.
mod sample_payloads;

pub(crate) fn dummy_profile(default_form_type: &str, taxpayer_type: &str) -> TaxpayerProfile {
    // 123-456-788 passes the official check digit; every value is fictitious.
    serde_json::from_value(serde_json::json!({
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
    .expect("dummy profile must deserialize")
}

pub(crate) fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-payloads")
}

/// `official_encrypt_sha256` is the SHA-256 of `<stem>.plain.xml` encrypted by the
/// official eBIRForms `Encrypt.exe` (sha256 `429337f4…4d2c`, unchanged from
/// 7.9.6.0 through 7.9.6.2.1) run under emulation; our `compress_and_encrypt`
/// must match it byte for byte. Regenerating samples can never change it.
pub(crate) fn check_sample(
    stem: &str,
    form_id: &str,
    plaintext: &str,
    official_encrypt_sha256: &str,
) {
    let layout = official_xml::layout(form_id).expect("official layout");
    check_sample_with_layout(stem, layout, plaintext, official_encrypt_sha256);
}

/// [`check_sample`] for a form whose page adds rows at run time: the sample
/// follows the form's own extension of the generated layout.
pub(crate) fn check_sample_with_layout(
    stem: &str,
    layout: &official_xml::OfficialLayout,
    plaintext: &str,
    official_encrypt_sha256: &str,
) {
    let fields = official_xml::read(layout, plaintext).expect("sample follows the official layout");
    assert!(!fields.is_empty());

    let encrypted = compress_and_encrypt(plaintext.as_bytes(), BIR_IAF_PASSPHRASE)
        .expect("sample payload must encrypt");
    let decrypted =
        decrypt_and_decompress(&encrypted, BIR_IAF_PASSPHRASE).expect("sample must decrypt");
    assert_eq!(
        decrypted,
        plaintext.as_bytes(),
        "encryption must round-trip"
    );

    let dir = samples_dir();
    let plain_path = dir.join(format!("{stem}.plain.xml"));
    let iaf_path = dir.join(format!("{stem}.iaf.xml"));
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&plain_path, plaintext).unwrap();
        std::fs::write(&iaf_path, &encrypted).unwrap();
    }

    let official_plain_path = dir.join(format!("{stem}.official.xml"));
    let official_plain = std::fs::read_to_string(&official_plain_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", official_plain_path.display()));
    assert_eq!(
        plaintext, official_plain,
        "{stem}: our plaintext differs from the official upload (saveEncryptedProfile) output"
    );

    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, official_encrypt_sha256,
        "{stem}: compress_and_encrypt no longer matches the official Encrypt.exe output"
    );

    if updating {
        return;
    }
    let committed = std::fs::read_to_string(&plain_path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", plain_path.display()));
    assert_eq!(
        committed, plaintext,
        "{stem}: serializer output changed; rerun with UPDATE_SAMPLE_PAYLOADS=1 if intended"
    );
    assert_eq!(
        std::fs::read(&iaf_path).unwrap(),
        encrypted,
        "{stem}: encrypted sample is stale"
    );
}

/// Parse `<stem>.official.xml` (`<div>key=valuekey=</div>`) tolerantly so
/// extended-layout samples (added rows) are covered without needing each
/// dynamic layout.
fn parse_official_xml_fields(xml: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let mut rest = xml;
    while let Some(open) = rest.find("<div>") {
        rest = &rest[open + 5..];
        let Some(close) = rest.find("</div>") else {
            break;
        };
        let body = &rest[..close];
        rest = &rest[close + 6..];
        let Some(eq) = body.find('=') else {
            continue;
        };
        let key = &body[..eq];
        let after_eq = &body[eq + 1..];
        let suffix = format!("{key}=");
        let raw_val = after_eq.strip_suffix(&suffix).unwrap_or(after_eq);
        let decoded = js_unescape_official(raw_val);
        map.insert(key.to_string(), decoded);
    }
    map
}

fn js_unescape_official(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 5 < bytes.len() && bytes[i + 1] == b'u' {
                if let Ok(code) = u32::from_str_radix(&s[i + 2..i + 6], 16) {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                        i += 6;
                        continue;
                    }
                }
            } else if i + 2 < bytes.len() {
                if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn frozen_slug_for_sample(stem: &str) -> &'static str {
    let prefix = stem.split('-').next().unwrap_or(stem);
    match prefix {
        "0605" => "0605-1999",
        "1600PT" => "1600-pt-2018",
        "1600VT" => "1600-vt-2018",
        "1601C" => "1601c-2018",
        "1601EQ" => "1601eq-2019",
        "1601FQ" => "1601-fq-2020",
        "1602Q" => "1602q-2019",
        "1603Q" => "1603q-2018",
        "1604C" => "1604c-2018",
        "1604E" => "1604e-2018",
        "1604F" => "1604f-2018",
        "1606" => "1606-2018",
        "1700" => "1700-2018",
        "1701" => "1701-2018",
        "1701A" => "1701a-2018",
        "1701MS" => "1701ms-2024",
        "1701Q" => "1701q-2018",
        "1702EX" => "1702ex-2018",
        "1702MX" => "1702mx-2018c",
        "1702Q" => "1702q-2018",
        "1702RT" => "1702rt-2018c",
        "1706" => "1706-2018",
        "1707" => "1707-2021",
        "1707A" => "1707a-2021",
        "1800" => "1800-2018",
        "1801" => "1801-2018",
        "2000" => "2000-dst-2018",
        "2000OT" => "2000-ot-2018",
        "2200A" => "2200a-2020",
        "2200AN" => "2200an-2018",
        "2200C" => "2200c-2018",
        "2200M" => "2200m-2018",
        "2200P" => "2200p-2020",
        "2200S" => "2200s-2018",
        "2200T" => "2200t-2022",
        "2550M" => "2550m-2007",
        "2550Q" => "2550q-2024",
        "2551Q" => "2551q-2018",
        "2552" => "2552-2018",
        "2553" => "2553-1999",
        other => panic!("unmapped sample stem prefix {other} ({stem})"),
    }
}

fn joined_keys_for_slug(slug: &str) -> std::collections::BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../html-frozen")
        .join(slug)
        .join("writer-cells.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing {}: {e}", path.display()));
    let json: serde_json::Value = serde_json::from_str(&raw).expect("valid writer-cells.json");
    let mut keys = std::collections::BTreeSet::new();
    for section in ["joins", "money_joins", "date_joins"] {
        if let Some(arr) = json.get(section).and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(k) = item.get("writer_key").and_then(|v| v.as_str()) {
                    if !k.starts_with("derived:") {
                        keys.insert(k.to_string());
                    }
                }
            }
        }
    }
    keys
}

fn norm_numeric(s: &str) -> Option<f64> {
    let t = s.trim().replace([',', ' '], "");
    if t.is_empty() {
        return None;
    }
    let cleaned = if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        format!("-{inner}")
    } else {
        t
    };
    cleaned.parse::<f64>().ok()
}

fn official_matches_print_exact(official: &str, printed: &str) -> bool {
    let off = official.trim();
    let prt = printed.trim();
    if off == prt {
        return true;
    }
    if (off == "true" && (prt == "true" || prt == "X"))
        || (off == "false" && (prt == "false" || prt.is_empty()))
    {
        return true;
    }
    // form-review/RULES.md section 7: an untouched amount box stays blank on
    // paper even though the upload carries zero; typed or computed zero may print.
    if norm_numeric(off) == Some(0.0) && prt.is_empty() {
        return true;
    }
    if let (Some(a), Some(b)) = (norm_numeric(off), norm_numeric(prt)) {
        if (a - b).abs() < 1e-6 {
            return true;
        }
    }
    false
}

/// Permanent exact print-map parity test over all 73 official samples:
/// every joined key in `html-frozen/<slug>/writer-cells.json` that holds a
/// non-empty value in `<stem>.official.xml` must appear with the exact same
/// value (case-sensitive, normalized only for numeric commas/decimals and
/// checkbox truth marks) in the form's print field map.
#[test]
fn every_official_sample_joined_key_matches_official_xml_exact() {
    use bir_core::forms::queueable::QueueableForm;

    // Explicit commented exception list (no other key may be skipped):
    // 1. Email keys (`*email*`): the official upload uppercases email on
    //    1700/1800/1801 (`SAMPLE.TAXPAYER@EXAMPLE.COM`), whereas paper keeps
    //    email addresses in the case held by the draft (`sample.taxpayer@example.com`).
    // 2. `frm1702RT:drpPg1I5AtcOther`: when "Other ATC" is NOT elected
    //    (`other_selected == false`), the official HTML `<select>` still
    //    serializes its disabled default `"IC010"`, while `to_print_field_map`
    //    intentionally prints `""` so an unselected box stays blank on paper
    //    (see `form_1702rt_official::tests::other_atc_prints_the_election_and_uploads_the_official_default`).
    // 3. `frm1604c:txtYear` and `frm1604e:txtYear`: the frozen sheet preprints
    //    `"2 0"` before the two year slots, so `to_print_field_map` emits the
    //    two-digit suffix (`"25"`) while the upload holds the 4-digit year (`"2025"`).
    // 4. `frm1701:txtPg2I2SpouseRDOCode`: when the spouse column is closed the
    //    upload writes `"000"` while `spouse_rdo_shown()` intentionally prints `""`.
    // 5. Unused zero-valued schedule rows on 1601C (`frm1601c:sched1:*`) and
    //    2551Q (`txtATCAmt{2..6}`, `txtATCDue{2..6}`): upload writes `"0.00"`
    //    while paper leaves unused rows blank.
    let is_allowed_exception = |key: &str, official: &str, printed: &str| -> bool {
        if key.to_ascii_lowercase().contains("email") && official.eq_ignore_ascii_case(printed) {
            return true;
        }
        if key == "frm1702RT:drpPg1I5AtcOther" && official == "IC010" && printed.is_empty() {
            return true;
        }
        if matches!(key, "frm1604c:txtYear" | "frm1604e:txtYear")
            && official.len() == 4
            && printed == &official[2..]
        {
            return true;
        }
        if key == "frm1701:txtPg2I2SpouseRDOCode" && official == "000" && printed.is_empty() {
            return true;
        }
        if (key.starts_with("frm1601c:sched1:")
            || key.starts_with("txtATCAmt")
            || key.starts_with("txtATCDue"))
            && official == "0.00"
            && printed.is_empty()
        {
            return true;
        }
        false
    };

    let samples: Vec<(&str, std::collections::BTreeMap<String, String>)> = vec![
        (
            "1600PT-062025",
            sample_payloads::form_1600pt::sample_1600pt().to_print_field_map(),
        ),
        (
            "1600PT-072025",
            sample_payloads::form_1600pt::sample_1600pt_private().to_print_field_map(),
        ),
        (
            "1600PT-082025",
            sample_payloads::form_1600pt::sample_1600pt_other_atcs().to_print_field_map(),
        ),
        (
            "1600VT-062025",
            sample_payloads::form_1600vt::sample_1600vt().to_print_field_map(),
        ),
        (
            "1600VT-072025",
            sample_payloads::form_1600vt::sample_1600vt_private().to_print_field_map(),
        ),
        (
            "1600VT-082025",
            sample_payloads::form_1600vt::sample_1600vt_other_atcs().to_print_field_map(),
        ),
        (
            "1601C-062025",
            sample_payloads::form_1601c::sample_1601c().to_print_field_map(),
        ),
        (
            "1601EQ-2025Q1",
            sample_payloads::form_1601eq::sample_1601eq().to_print_field_map(),
        ),
        (
            "1601FQ-2025Q2",
            sample_payloads::form_1601fq::sample_1601fq().to_print_field_map(),
        ),
        (
            "1602Q-2025Q3",
            sample_payloads::form_1602q::sample_1602q().to_print_field_map(),
        ),
        (
            "1603Q-2025Q3",
            sample_payloads::form_1603q::sample_1603q().to_print_field_map(),
        ),
        (
            "1604C-2025",
            sample_payloads::form_1604c::sample_1604c().to_print_field_map(),
        ),
        (
            "1604E-2025",
            sample_payloads::form_1604e::sample_1604e().to_print_field_map(),
        ),
        (
            "1604F-2025",
            sample_payloads::form_1604f::sample_1604f().to_print_field_map(),
        ),
        (
            "1606-06152025_T123456",
            sample_payloads::form_1606::sample_1606().to_print_field_map(),
        ),
        (
            "1700-2025",
            sample_payloads::form_1700::sample_1700().to_print_field_map(),
        ),
        (
            "1700-2025-joint",
            sample_payloads::form_1700::sample_1700_joint().to_print_field_map(),
        ),
        (
            "1700-2025-others",
            sample_payloads::form_1700::sample_1700_others().to_print_field_map(),
        ),
        (
            "1701A-122025",
            sample_payloads::form_1701a::sample_1701a().to_print_field_map(),
        ),
        (
            "1701A-122025-joint",
            sample_payloads::form_1701a::sample_1701a_joint().to_print_field_map(),
        ),
        (
            "1701A-122025-joint-many",
            sample_payloads::form_1701a::sample_1701a_joint_many().to_print_field_map(),
        ),
        (
            "1701A-122025-joint-more",
            sample_payloads::form_1701a::sample_1701a_joint_more().to_print_field_map(),
        ),
        (
            "1701A-122025-more",
            sample_payloads::form_1701a::sample_1701a_more().to_print_field_map(),
        ),
        (
            "1701MS-122025",
            sample_payloads::form_1701ms::sample_1701ms().to_print_field_map(),
        ),
        (
            "1701MS-122025-special",
            sample_payloads::form_1701ms::sample_1701ms_special().to_print_field_map(),
        ),
        (
            "1702EX-122025",
            sample_payloads::form_1702ex::sample_1702ex().to_print_field_map(),
        ),
        (
            "1702EX-122025-popups",
            sample_payloads::form_1702ex::sample_1702ex_popups().to_print_field_map(),
        ),
        (
            "1702MX-0625-fiscal",
            sample_payloads::form_1702mx::sample_1702mx_fiscal().to_print_field_map(),
        ),
        (
            "1702MX-1225",
            sample_payloads::form_1702mx::sample_1702mx().to_print_field_map(),
        ),
        (
            "1702MX-1225-sched6",
            sample_payloads::form_1702mx::sample_1702mx_schedule_6_rows().to_print_field_map(),
        ),
        (
            "1702Q-2025Q2",
            sample_payloads::form_1702q::sample_1702q().to_print_field_map(),
        ),
        (
            "1702RT-122025-more",
            sample_payloads::form_1702rt::sample_1702rt_more_rows().to_print_field_map(),
        ),
        (
            "1706-03152025_T123456",
            sample_payloads::form_1706::sample_1706().to_print_field_map(),
        ),
        (
            "1707-06302025",
            sample_payloads::form_1707::sample_1707().to_print_field_map(),
        ),
        (
            "1707-07152025",
            sample_payloads::form_1707::sample_1707_with_popups().to_print_field_map(),
        ),
        (
            "1707-08292025",
            sample_payloads::form_1707::sample_1707_many_rows().to_print_field_map(),
        ),
        (
            "1707A-12312023",
            sample_payloads::form_1707a::sample_1707a_many_rows().to_print_field_map(),
        ),
        (
            "1707A-12312024",
            sample_payloads::form_1707a::sample_1707a_with_popup().to_print_field_map(),
        ),
        (
            "1707A-12312025",
            sample_payloads::form_1707a::sample_1707a().to_print_field_map(),
        ),
        (
            "1800-03102025",
            sample_payloads::form_1800::sample_1800().to_print_field_map(),
        ),
        (
            "1800-03112025",
            sample_payloads::form_1800::sample_1800_many_rows().to_print_field_map(),
        ),
        (
            "1801-06052025",
            sample_payloads::form_1801::sample_1801().to_print_field_map(),
        ),
        (
            "1801-09152025",
            sample_payloads::form_1801::sample_1801_with_tax_and_added_rows().to_print_field_map(),
        ),
        (
            "1801-09162025",
            sample_payloads::form_1801::sample_1801_many_rows().to_print_field_map(),
        ),
        (
            "2000-062025",
            sample_payloads::form_2000::sample_2000().to_print_field_map(),
        ),
        (
            "2000OT-06152025",
            sample_payloads::form_2000ot::sample_2000ot().to_print_field_map(),
        ),
        (
            "2000OT-06162025",
            sample_payloads::form_2000ot::sample_2000ot_shares().to_print_field_map(),
        ),
        (
            "2200A-03142025",
            sample_payloads::form_2200a::sample_2200a().to_print_field_map(),
        ),
        (
            "2200AN-06302025",
            sample_payloads::form_2200an::sample_2200an().to_print_field_map(),
        ),
        (
            "2200C-06302025",
            sample_payloads::form_2200c::sample_2200c().to_print_field_map(),
        ),
        (
            "2200M-03132025",
            sample_payloads::form_2200m::sample_2200m_many_rows().to_print_field_map(),
        ),
        (
            "2200M-03142025",
            sample_payloads::form_2200m::sample_2200m().to_print_field_map(),
        ),
        (
            "2200P-03132025",
            sample_payloads::form_2200p::sample_2200p_many_rows().to_print_field_map(),
        ),
        (
            "2200P-03142025",
            sample_payloads::form_2200p::sample_2200p().to_print_field_map(),
        ),
        (
            "2200S-06292025",
            sample_payloads::form_2200s::sample_2200s_many_rows().to_print_field_map(),
        ),
        (
            "2200S-06302025",
            sample_payloads::form_2200s::sample_2200s().to_print_field_map(),
        ),
        (
            "2200T-03142025",
            sample_payloads::form_2200t::sample_2200t().to_print_field_map(),
        ),
        (
            "2550M-062022",
            sample_payloads::form_2550m::sample_2550m().print_field_map(),
        ),
        (
            "2551Q-122025Q1",
            sample_payloads::form_2551q::sample_2551q().to_print_field_map(),
        ),
        (
            "2552-03142025",
            sample_payloads::form_2552::sample_2552().print_field_map(),
        ),
        (
            "2553-122025Q1",
            sample_payloads::form_2553::sample_2553().to_print_field_map(),
        ),
        (
            "0605-12312025090203",
            sample_payloads::form_0605::sample_0605().print_field_map(),
        ),
        (
            "1701-122025",
            sample_payloads::form_1701::sample_1701().to_print_field_map(),
        ),
        (
            "1701-122025-joint",
            sample_payloads::form_1701::sample_1701_joint().to_print_field_map(),
        ),
        (
            "1701Q-2025Q2",
            sample_payloads::form_1701q::sample_1701q().to_print_field_map(),
        ),
        (
            "1702RT-122025",
            sample_payloads::form_1702rt::sample_1702rt().to_print_field_map(),
        ),
        (
            "1702RT-122025-small-loss",
            sample_payloads::form_1702rt::sample_1702rt_small_loss().to_print_field_map(),
        ),
        (
            "2550M-072022",
            sample_payloads::form_2550m::sample_2550m_schedule_1().print_field_map(),
        ),
        (
            "2550M-082022",
            sample_payloads::form_2550m::sample_2550m_capital_goods().print_field_map(),
        ),
        (
            "2550M-092022",
            sample_payloads::form_2550m::sample_2550m_credits().print_field_map(),
        ),
        (
            "2550Q-122025Q1",
            sample_payloads::form_2550q::sample_2550q().to_print_field_map(),
        ),
        (
            "2550Q-122025Q1-schedules",
            sample_payloads::form_2550q::sample_2550q_schedules().to_print_field_map(),
        ),
        (
            "2550Q-122025Q1-specify",
            sample_payloads::form_2550q::sample_2550q_specify().to_print_field_map(),
        ),
    ];

    let mut slug_cache: std::collections::BTreeMap<&str, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    let mut failures = Vec::new();
    let mut total_checked = 0usize;

    for (stem, print_map) in &samples {
        let slug = frozen_slug_for_sample(stem);
        let joined = slug_cache
            .entry(slug)
            .or_insert_with(|| joined_keys_for_slug(slug));
        let xml_path = samples_dir().join(format!("{stem}.official.xml"));
        let xml = std::fs::read_to_string(&xml_path)
            .unwrap_or_else(|e| panic!("missing {}: {e}", xml_path.display()));
        let official = parse_official_xml_fields(&xml);

        for key in joined.iter() {
            let Some(off_val) = official.get(key) else {
                continue;
            };
            let off_trim = off_val.trim();
            if off_trim.is_empty() || off_trim == "false" {
                continue;
            }
            total_checked += 1;
            let prt_val = print_map.get(key).map(String::as_str).unwrap_or("");
            if official_matches_print_exact(off_trim, prt_val)
                || is_allowed_exception(key, off_trim, prt_val)
            {
                continue;
            }
            let kind = if prt_val.trim().is_empty() {
                "HELD BUT NOT PRINTED"
            } else if off_trim.eq_ignore_ascii_case(prt_val.trim()) {
                "CASE DIFFERS ONLY"
            } else {
                "DIFFERENT VALUE"
            };
            failures.push(format!(
                "{stem} ({slug}) {key}: {kind} (official={off_trim:?}, print={prt_val:?})"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Official-sample print parity failed ({} mismatches across {} checked joined keys over {} samples):\n{}",
        failures.len(),
        total_checked,
        samples.len(),
        failures.join("\n")
    );
}
