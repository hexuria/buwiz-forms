//! The RDO box offers the RDO catalog on every form. 1601C's schema
//! carried the official page's unevaluated option script
//! (`["000", "\" + rdo.description + \""]`) and 1701Q's an empty-array
//! literal (`["[]"]`); the host offered its catalog only where the schema
//! list was empty, so a filer whose profile has no RDO could pick only
//! the dummy "000" (or "[]"). `catalogs::schema_list` now reads such a
//! list as no list.

use super::*;

/// Template text the official page's script left unevaluated.
fn template(text: &str) -> bool {
    text.starts_with('"') || text.contains("\" +") || text.contains("+ \"") || text == "[]"
}

/// A box bound to an RDO code (the binding `catalogs::field_options`
/// answers with the catalog).
fn rdo_bound(kind: AgentFormKind, key: &str) -> bool {
    matches!(
        kind.definition().bindings.get(key).map(|b| &b.binding),
        Some(Binding::Text { path, .. })
            if path
                .rsplit('/')
                .next()
                .is_some_and(|leaf| leaf == "rdo_code" || leaf.ends_with("_rdo_code"))
    )
}

/// What the page lists for a `sel` box (session.ts `listedOptions`): the
/// host's options, else the schema's `o`, each read as `optionOf` reads it
/// (template text starting with `"` is dropped). `(value, label)` pairs.
fn offered(opened: &Value, field: &Value, key: &str) -> Vec<(String, String)> {
    let rows = opened["options"][key]
        .as_array()
        .or_else(|| field["o"].as_array())
        .cloned()
        .unwrap_or_default();
    rows.iter()
        .filter_map(|o| {
            if o.is_object() {
                return Some((
                    o["value"].as_str().unwrap_or_default().to_string(),
                    o["label"].as_str().unwrap_or_default().to_string(),
                ));
            }
            let text = o.as_str()?;
            if text.starts_with('"') {
                return None;
            }
            let value = text
                .split_once(" - ")
                .filter(|(code, label)| {
                    !code.is_empty() && !code.contains(char::is_whitespace) && !label.is_empty()
                })
                .map_or(text, |(code, _)| code);
            Some((value.to_string(), text.to_string()))
        })
        .collect()
}

/// Every `sel` field of a form's schema, bound or not: `(key, field)`.
fn schema_sels(code: &str) -> Vec<(String, Value)> {
    let form = schema_entry(code).unwrap();
    let mut out = Vec::new();
    for item in form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
    {
        for f in item["f"].as_array().into_iter().flatten() {
            if f["t"] == "sel"
                && let Some(key) = f["k"].as_str()
            {
                out.push((key.to_string(), f.clone()));
            }
        }
        for row in item["keys"].as_array().into_iter().flatten() {
            for (col, key) in row.as_array().into_iter().flatten().enumerate() {
                let field = &item["cols"][col];
                if field["t"] == "sel"
                    && let Some(key) = key.as_str()
                {
                    out.push((key.to_string(), field.clone()));
                }
            }
        }
    }
    out
}

fn catalog() -> BTreeSet<String> {
    bir_core::reference::get_all_rdos()
        .into_iter()
        .map(|r| r.code)
        .collect()
}

/// Every form's RDO select offers exactly the RDO catalog: never "000",
/// never template text. "000" is not a catalog code.
#[test]
fn every_rdo_select_offers_the_rdo_catalog() {
    setup();
    let catalog = catalog();
    assert!(!catalog.contains("000"), "000 is not an RDO");
    let mut failures = Vec::new();
    let mut seen = Vec::new();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let opened = open(code, page_period_of(code));
        for (key, field) in schema_sels(code) {
            if !rdo_bound(kind, &key) {
                continue;
            }
            seen.push(format!("{code} {key}"));
            let listed = offered(&opened, &field, &key);
            let values: BTreeSet<String> = listed.iter().map(|(v, _)| v.clone()).collect();
            if values != catalog {
                failures.push(format!(
                    "{code} {key}: {} offered, not the catalog's {}: {:?}",
                    values.len(),
                    catalog.len(),
                    values.iter().take(4).collect::<Vec<_>>()
                ));
            }
            if listed
                .iter()
                .any(|(v, l)| v == "000" || template(v) || template(l))
            {
                failures.push(format!("{code} {key}: offers 000 or template text"));
            }
        }
    }
    eprintln!("RDO selects swept ({}): {seen:#?}", seen.len());
    assert!(failures.is_empty(), "{failures:#?}");
    for expected in [
        "1601C frm1601c:txtRDOCode",
        "1701Q frm1701q:txtRDOCode",
        "1701Q frm1701q:txtSpouseRDOCode",
        "0619E frm0619E:txtRDOCode",
    ] {
        assert!(seen.iter().any(|s| s == expected), "{expected} swept");
    }
    assert!(seen.len() >= 30, "{} RDO selects", seen.len());
}

/// No `sel` box on any form offers template text, and the only schema
/// lists read as placeholders (and replaced by the host's list) are the
/// RDO boxes'. Every other schema list is offered as it stands.
#[test]
fn no_select_offers_template_text_and_only_rdo_lists_are_placeholders() {
    setup();
    let mut placeholder = Vec::new();
    let mut templated = Vec::new();
    let mut changed = Vec::new();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let opened = open(code, page_period_of(code));
        for (key, field) in schema_sels(code) {
            let has_o = field["o"].as_array().is_some_and(|o| !o.is_empty());
            if has_o && catalogs::schema_list(&field).is_none() {
                placeholder.push(format!("{code} {key}"));
            }
            if offered(&opened, &field, &key)
                .iter()
                .any(|(v, l)| template(v) || template(l))
            {
                templated.push(format!("{code} {key}"));
            }
            // A real schema list stays authoritative: no host list over it.
            if catalogs::schema_list(&field).is_some() && opened["options"].get(&key).is_some() {
                changed.push(format!("{code} {key}"));
            }
        }
    }
    eprintln!("placeholder-only schema lists: {placeholder:#?}");
    assert_eq!(
        placeholder,
        [
            "1601C frm1601c:txtRDOCode",
            "1701Q frm1701q:txtRDOCode",
            "1701Q frm1701q:txtSpouseRDOCode",
        ]
    );
    assert!(
        templated.is_empty(),
        "template text offered: {templated:#?}"
    );
    assert!(
        changed.is_empty(),
        "host list over a schema list: {changed:#?}"
    );
}

/// 1601C, profile with an RDO: Item 7 shows the profile's code, locked
/// with the profile's note and Edit, over the whole catalog; "Save to
/// profile" is offered for it once edited. Profile with no RDO: the box is
/// open and blank, offers the whole catalog, takes a pick (the page sends
/// a box with no lock as open, `unlockedFor`; a plain fill is refused as a
/// profile box), and the pick stays on this return — the profile is not
/// written, and no "Save to profile" entry is offered for the box.
#[test]
fn form_1601c_rdo_is_locked_from_the_profile_or_open_with_the_catalog() {
    setup();
    let key = "frm1601c:txtRDOCode";
    let catalog = catalog();
    let codes = |opened: &Value| -> BTreeSet<String> {
        opened["options"][key]
            .as_array()
            .expect("host RDO list")
            .iter()
            .filter_map(|o| o["value"].as_str().map(str::to_string))
            .collect()
    };

    let full = open("1601C", 2);
    assert_eq!(full["values"][key], "039");
    assert_eq!(full["locks"][key], agent::Lock::Profile.reason());
    assert_eq!(full["lock_notes"][key], NOTE_PROFILE);
    assert!(strings(&full["profile"]).contains(key));
    assert!(strings(&full["profile_edit"]).contains(key));
    assert_eq!(full["profile_save"][key]["field"], "rdo_code");
    assert_eq!(codes(&full), catalog);

    let sparse = install_sparse_profile();
    let thin = open_for(sparse, "1601C", YEAR, 2);
    assert_eq!(thin["draft"]["rdo_code"], "");
    assert_eq!(thin["values"][key], "");
    assert!(
        thin["locks"].get(key).is_none(),
        "open: {:?}",
        thin["locks"][key]
    );
    assert!(!strings(&thin["profile"]).contains(key));
    assert!(thin["profile_save"].get(key).is_none());
    assert_eq!(codes(&thin), catalog);
    assert!(!codes(&thin).contains("000"));

    let refused = fill("1601C", &thin["draft"], json!({ key: "040" }));
    assert!(refused["rejected"].get(key).is_some());
    let picked = fill_unlocked("1601C", &thin["draft"], json!({ key: "040" }), &[key]);
    assert_eq!(picked["rejected"], json!({}));
    assert_eq!(picked["draft"]["rdo_code"], "040");
    assert_eq!(picked["values"][key], "040");
    save("1601C", &picked["draft"]);
    let profile = with_database(|db| {
        db.get_profile_by_tin(sparse)
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap()
    .unwrap();
    assert_eq!(profile.rdo_code, "", "the profile is not written");
}

/// "000" (the official dropdown's placeholder, not a catalog code) is no
/// RDO: 1601C's validation flags Item 7 with the official message, and
/// Queue refuses the saved return. Save itself keeps a draft as typed.
#[test]
fn form_1601c_refuses_000_as_its_rdo_on_validate_and_queue() {
    setup();
    let key = "frm1601c:txtRDOCode";
    let sparse = install_sparse_profile();
    let thin = open_for(sparse, "1601C", YEAR, 2);
    // The box takes what it is given (an RDO is not judged by its list:
    // older codes write through the same path); validation judges it.
    let typed = fill_unlocked("1601C", &thin["draft"], json!({ key: "000" }), &[key]);
    assert_eq!(typed["rejected"], json!({}));
    let mut draft = typed["draft"].clone();
    assert_eq!(draft["rdo_code"], "000");
    let rdo_error = |errors: &Value| {
        errors.as_array().unwrap().iter().any(|e| {
            e["message"] == bir_core::validation::OFFICIAL_INVALID_RDO_MESSAGE
                && strings(&e["keys"]).contains(key)
        })
    };
    assert!(rdo_error(&validated("1601C", &draft)["errors"]));
    let saved = save("1601C", &draft);
    assert_eq!(
        saved["draft"]["rdo_code"], "000",
        "a draft keeps what was typed"
    );
    draft = saved["draft"].clone();
    // The year's facts Queue asks for first (decision 3), so the refusal
    // is the return's own validation.
    with_database(|db| {
        db.set_year_facts(
            sparse,
            2026,
            &bir_core::tax_profile::YearFactsPatch {
                eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
                is_government_withholding_entity: Some(Some(false)),
                ..Default::default()
            },
        )
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
    let refused = call(
        "queue_form_draft",
        vec![sparse.into(), "1601C".into(), YEAR.into(), 2i64.into()],
    )
    .expect_err("Queue refuses 000");
    assert!(
        refused.message().contains(&format!(
            "rdo_code: {}",
            bir_core::validation::OFFICIAL_INVALID_RDO_MESSAGE
        )),
        "{}",
        refused.message()
    );
    assert_eq!(open_for(sparse, "1601C", YEAR, 2)["status"], "Draft");

    // A catalog code clears the RDO error.
    let fixed = fill_unlocked("1601C", &draft, json!({ key: "040" }), &[key]);
    assert!(!rdo_error(&validated("1601C", &fixed["draft"])["errors"]));
}

/// Every form's own RDO box (`/rdo_code`): "000" fails that form's
/// validation on the box. A spouse's RDO is judged only on a return with a
/// spouse, and 1801's Schedule 4 RDOs only on a row that exists, so those
/// are not swept here.
#[test]
fn every_form_flags_000_as_its_rdo() {
    setup();
    let mut failures = Vec::new();
    let mut seen = 0;
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let opened = open(code, page_period_of(code));
        for (key, _) in schema_sels(code) {
            if key.contains('{') || !rdo_bound(kind, &key) {
                continue;
            }
            let Some(Binding::Text { path, .. }) =
                kind.definition().bindings.get(&key).map(|b| &b.binding)
            else {
                continue;
            };
            if path != "/rdo_code" {
                continue;
            }
            seen += 1;
            let mut draft = opened["draft"].clone();
            let Some(slot) = draft.pointer_mut(path) else {
                failures.push(format!("{code} {key}: no draft field {path}"));
                continue;
            };
            *slot = json!("000");
            let errors = validated(code, &draft)["errors"].clone();
            let flagged = errors
                .as_array()
                .unwrap()
                .iter()
                .any(|e| strings(&e["keys"]).contains(&key));
            if !flagged {
                failures.push(format!("{code} {key} ({path}): 000 not flagged"));
            }
        }
    }
    eprintln!("RDO boxes given 000: {seen}");
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(seen >= 30, "{seen}");
}

/// The year's facts Queue asks for first (decision 3), so a refusal is the
/// return's own validation.
fn set_queue_facts(tin: &str) {
    with_database(|db| {
        db.set_year_facts(
            tin,
            2026,
            &bir_core::tax_profile::YearFactsPatch {
                eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
                is_government_withholding_entity: Some(Some(false)),
                atc_codes: Some(vec!["II012".to_string()]),
                ..Default::default()
            },
        )
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
}

/// Queue a saved return the way the page does: `screen_form_queue` for a
/// queueable form, `queue_form_draft` for the rest. Either runs the form's
/// `transition_to_queued`, which validates.
fn queue_saved(code: &str, period: i64, saved: &Value) -> Result<Value, HostError> {
    match kind_of(code).unwrap() {
        AgentFormKind::Queueable(_) => call(
            "screen_form_queue",
            vec![code.into(), json(saved["draft"].clone())],
        ),
        _ => call(
            "queue_form_draft",
            vec![TIN.into(), code.into(), YEAR.into(), period.into()],
        ),
    }
}

/// What validation says on `key`, and what Queue answers, for `draft`
/// saved over `base` (the stored return so far). Returns the messages on
/// the box, Queue's error (`None`: it queued) and the saved draft.
fn judged(
    code: &str,
    period: i64,
    key: &str,
    draft: &Value,
) -> (Vec<String>, Option<String>, Value) {
    let on_box: Vec<String> = validated(code, draft)["errors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| strings(&e["keys"]).contains(key))
        .map(|e| e["message"].as_str().unwrap_or_default().to_string())
        .collect();
    let saved = save(code, draft);
    let queued = queue_saved(code, period, &saved)
        .err()
        .map(|e| e.message().to_string());
    (on_box, queued, saved["draft"].clone())
}

/// A 1701Q, 1701A and 1701 return WITH a spouse (the gates that switch the
/// spouse's boxes on, and a valid spouse TIN: 1701A judges the RDO only
/// once the TIN is well formed), opened for the fixture taxpayer.
fn with_spouse(code: &str) -> Value {
    let mut draft = open(code, page_period_of(code))["draft"].clone();
    let gates = match code {
        "1701Q" => json!({"has_spouse": true, "spouse_tin": "000-123-453-00000",
            "spouse_name": "Example Spouse"}),
        "1701A" => json!({"civil_status": "married", "spouse_has_income": true,
            "filing_status": "joint"}),
        "1701" => json!({"civil_status": "Married", "spouse_has_income": true,
            "joint_filing_status": "Joint"}),
        _ => unreachable!("{code}"),
    };
    for (k, v) in gates.as_object().unwrap() {
        draft[k] = v.clone();
    }
    if code != "1701Q" {
        draft["spouse"]["enabled"] = json!(true);
        draft["spouse"]["tin"] = json!("000-123-453-00000");
        draft["spouse"]["name"] = json!("Example Spouse");
    }
    draft
}

/// The spouse RDO boxes: `(form, key, draft path, validation field,
/// message)`.
const SPOUSE_RDOS: [(&str, &str, &str, &str, &str); 3] = [
    (
        "1701Q",
        "frm1701q:txtSpouseRDOCode",
        "/spouse_rdo_code",
        "spouse_rdo_code",
        "Please enter a valid RDO Code on Item 18.",
    ),
    (
        "1701A",
        "frm1701A:txtSpouseRDOCode",
        "/spouse/rdo_code",
        "spouse.rdo_code",
        "Please enter a valid RDO Code on Item 67.",
    ),
    (
        "1701",
        "frm1701:txtPg2I2SpouseRDOCode",
        "/spouse_rdo_code",
        "spouse_rdo_code",
        "Please enter a valid RDO Code on Page 2 Part IV Item 2.",
    ),
];

/// On a return with a spouse, the spouse RDO "000" (the official
/// placeholder) and "[]" (what 1701Q's broken list offered) are refused:
/// validation names the box with the form's message, and Queue
/// (`transition_to_queued`) refuses with it. A catalog code clears it. The
/// spouse RDO left empty is refused the same way.
#[test]
fn a_spouse_rdo_off_the_catalog_or_empty_is_refused_on_validate_and_queue() {
    setup();
    set_queue_facts(TIN);
    for (code, key, path, field, message) in SPOUSE_RDOS {
        let period = page_period_of(code);
        let mut base = with_spouse(code);
        let queued_line = format!("{field}: {message}");
        for rdo in ["000", "[]", "", "040"] {
            let mut draft = base.clone();
            *draft.pointer_mut(path).unwrap() = json!(rdo);
            let (on_box, queued, saved) = judged(code, period, key, &draft);
            assert_eq!(saved.pointer(path), Some(&json!(rdo)), "{code} {rdo:?}");
            let queued = queued.unwrap_or_else(|| panic!("{code} {rdo:?} queued"));
            if rdo == "040" {
                assert!(on_box.is_empty(), "{code} {rdo:?}: {on_box:?}");
                assert!(!queued.contains(&queued_line), "{code} {rdo:?}: {queued}");
            } else {
                assert_eq!(on_box, [message], "{code} {rdo:?}");
                assert!(queued.contains(&queued_line), "{code} {rdo:?}: {queued}");
            }
            base = saved;
        }
    }
}

/// 1801 Schedule 4, row 1 with its other cells filled: an RDO of "000"
/// (the official dropdown's empty choice, which the field map also writes
/// for a blank) leaves the row incomplete; "[]" is no catalog code. Both
/// are named on the box and refused by Queue; a catalog code passes.
#[test]
fn form_1801_schedule_4_rdo_off_the_catalog_is_refused_on_validate_and_queue() {
    setup();
    let (code, key) = ("1801", "frm1801v2018:sched4Rdo_1");
    let period = page_period_of(code);
    let mut base = open(code, period)["draft"].clone();
    for (rdo, refusal) in [
        (
            "000",
            Some((
                "business_interests[0]",
                "Incomplete values on Schedule 4, Row 1.",
            )),
        ),
        (
            "[]",
            Some((
                "business_interests[0].rdo_code",
                "Please enter a valid RDO Code on Schedule 4, Row 1.",
            )),
        ),
        ("040", None),
    ] {
        let mut draft = base.clone();
        draft["business_interests"] = json!([{
            "name": "Example Store", "address": "1 Example Street", "rdo_code": rdo,
            "value": {"exclusive": 1000.0, "conjugal": 0.0},
        }]);
        let (on_box, queued, saved) = judged(code, period, key, &draft);
        assert_eq!(saved["business_interests"][0]["rdo_code"], rdo);
        let queued = queued.unwrap_or_else(|| panic!("{rdo:?} queued"));
        match refusal {
            Some((field, message)) => {
                assert_eq!(on_box, [message], "{rdo:?}");
                assert!(
                    queued.contains(&format!("{field}: {message}")),
                    "{rdo:?}: {queued}"
                );
            }
            None => {
                assert!(on_box.is_empty(), "{rdo:?}: {on_box:?}");
                assert!(!queued.contains("business_interests"), "{rdo:?}: {queued}");
            }
        }
        base = saved;
    }
}
