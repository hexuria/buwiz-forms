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
