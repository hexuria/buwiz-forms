//! A new return starts with the answers of the previous period's return
//! (owner's decisions 9 and 10, `carry_forward.rs`), through
//! `screen_form_open`. These tests use their own taxpayer (the demo TIN),
//! so nothing they save reaches another test's returns.

use super::*;

/// Juana Santos, the demo taxpayer.
const JUANA: &str = "00012345300000";

fn install_juana() {
    let profile: TaxpayerProfile = serde_json::from_value(json!({
        "id": null,
        "full_name": "Juana Santos",
        "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
        "rdo_code": "039",
        "line_of_business": "Retail",
        "registered_address": "1 Sample Street, Quezon City",
        "zip_code": "1100",
        "phone": "0281234567",
        "email": "juana@example.com",
        "default_form_type": "1601C",
        "taxpayer_type": "Individual",
    }))
    .expect("the demo profile parses");
    with_database(|db| {
        db.save_profile(profile)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .expect("the demo profile saves");
}

fn set_rdo(rdo: &str) {
    with_database(|db| {
        let mut profile = db
            .get_profile_by_tin(JUANA)
            .map_err(|error| HostError::new(error.to_string()))?
            .expect("the demo profile");
        profile.rdo_code = rdo.to_string();
        db.save_profile(profile)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
}

fn open_at(code: &str, year: i64, period: i64) -> Value {
    call_async(
        "screen_form_open",
        vec![JUANA.into(), code.into(), year.into(), period.into()],
    )
    .unwrap_or_else(|error| panic!("{code} {year}/{period} opens: {error}"))
}

/// `fill`, asserting every edit was taken.
fn fill_all(code: &str, draft: &Value, edits: Value) -> Value {
    let filled = fill(code, draft, edits);
    assert_eq!(filled["rejected"], json!({}), "{code}: {filled}");
    filled
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|key| key.as_str().map(str::to_string))
        .collect()
}

/// A box that shows nothing: blank or zero.
fn empty(values: &Value, key: &str) -> bool {
    agent::carry::is_blank_value(values[key].as_str().unwrap_or(""))
}

/// The first month of 1601C, answered and saved; returns its row id.
fn first_month() -> i64 {
    let first = open_at("1601C", 2026, 1);
    let filled = fill_all(
        "1601C",
        &first["draft"],
        json!({
            "frm1601c:TaxWithheld_1": "true",
            "frm1601c:CatAgent_P": "true",
            "frm1601c:SpecialTax_1": "true",
            "frm1601c:selTreaty": "2",
            "frm1601c:txt20Other": "Rice subsidy",
            "frm1601c:txtTax14": "50000",
            "frm1601c:txtTax20": "1000",
        }),
    );
    stored_id(&save("1601C", &filled["draft"]))
}

#[test]
fn a_second_month_starts_with_the_first_months_answers() {
    setup();
    install_juana();
    // A first return has nothing to start from.
    let first = open_at("1601C", 2026, 1);
    assert_eq!(first["kept"], json!({}));
    assert_eq!(first["auto_stops"], json!([]));
    assert_eq!(first["stop_edits"], json!({}));
    let stoppable = keys(&first["stoppable"]);
    assert!(stoppable.contains("frm1601c:txtTax14"), "{stoppable:?}");
    assert!(!stoppable.contains("frm1601c:txtRDOCode"), "a profile box");
    // A radio item may be a stop under each radio the page draws, not under its group box.
    assert!(stoppable.contains("frm1601c:AmendedRtn_1"), "{stoppable:?}");
    assert!(!stoppable.contains("frm1601c:AmendedRtn"), "the page draws no group box");
    first_month();
    // The profile changes before the next month: the profile box follows it.
    set_rdo("040");

    let second = open_at("1601C", 2026, 2);
    let values = &second["values"];
    assert!(second["draft"]["id"].is_null(), "a new return");
    assert_eq!(values["frm1601c:txtATC"], "WW010");
    assert_eq!(values["frm1601c:SpecialTax_1"], "true");
    assert_eq!(values["frm1601c:selTreaty"], "2");
    assert_eq!(values["frm1601c:txt20Other"], "Rice subsidy");
    // Amounts start empty; Items 3 and 11 are asked again.
    for key in ["frm1601c:txtTax14", "frm1601c:txtTax20"] {
        assert!(empty(values, key), "{key}: {}", values[key]);
    }
    for key in ["frm1601c:TaxWithheld_1", "frm1601c:CatAgent_P"] {
        assert_ne!(values[key], "true", "{key} is left open");
    }
    assert_eq!(values["frm1601c:txtRDOCode"], "040", "the profile wins");
    assert_eq!(
        second["kept"],
        json!({
            // A radio item, by each radio the page draws.
            "frm1601c:SpecialTax_1": "last",
            "frm1601c:SpecialTax_2": "last",
            "frm1601c:selTreaty": "last",
            "frm1601c:txt20Other": "last",
            "frm1601c:txtATC": "last",
        })
    );
    let auto = keys(&second["auto_stops"]);
    assert_eq!(
        auto,
        BTreeSet::from([
            "frm1601c:txtTax14".to_string(),
            "frm1601c:txtTax20".to_string()
        ])
    );
    // Copied answers are no profile difference and no profile edit.
    assert_eq!(second["profile_differs"], json!([]));
    assert!(profile_edits(&second["draft"]).is_empty());

    // Saved and reopened, the return keeps its record.
    let saved = save("1601C", &second["draft"]);
    assert!(saved["draft"]["id"].is_i64(), "{saved}");
    let again = open_at("1601C", 2026, 2);
    assert_eq!(again["draft"]["id"], saved["draft"]["id"]);
    assert_eq!(again["kept"], second["kept"]);
    assert_eq!(again["auto_stops"], second["auto_stops"]);
}

#[test]
fn a_pinned_template_answer_wins_over_the_last_return() {
    setup();
    install_juana();
    first_month();
    // A later month, answered differently, becomes the template. It
    // started with January's answers; its description of Item 20 is cleared.
    let may = open_at("1601C", 2026, 5);
    assert_eq!(may["kept"]["frm1601c:txt20Other"], "last");
    let may = fill_all(
        "1601C",
        &may["draft"],
        json!({
            "frm1601c:selTreaty": "3",
            "frm1601c:txt20Other": "",
            "frm1601c:txt29Other": "Bank transfer",
        }),
    );
    let may = stored_id(&save("1601C", &may["draft"]));
    call("create_template_from_return", vec![may.into()]).unwrap();
    // A stop switched on in the template comes with the open.
    call(
        "set_template_stop",
        vec![
            JUANA.into(),
            "1601C".into(),
            "frm1601c:txtTax25".into(),
            true.into(),
        ],
    )
    .unwrap();

    let second = open_at("1601C", 2026, 2);
    let values = &second["values"];
    assert_eq!(values["frm1601c:selTreaty"], "3", "the template's pin");
    assert_eq!(values["frm1601c:txt29Other"], "Bank transfer");
    assert_eq!(
        values["frm1601c:txt20Other"], "Rice subsidy",
        "not pinned: from the last return"
    );
    assert_eq!(second["kept"]["frm1601c:selTreaty"], "template");
    assert_eq!(second["kept"]["frm1601c:txt29Other"], "template");
    assert_eq!(second["kept"]["frm1601c:txt20Other"], "last");
    assert_eq!(second["stop_edits"], json!({"frm1601c:txtTax25": true}));
}

#[test]
fn a_sample_and_an_amendment_take_nothing_from_the_last_return() {
    setup();
    install_juana();
    let january = first_month();
    let sample = call_async("screen_form_sample", vec![JUANA.into(), "1601C".into()]).unwrap();
    assert_eq!(sample["kept"], json!({}));
    assert_eq!(sample["auto_stops"], json!([]));
    assert!(empty(&sample["values"], "frm1601c:txt20Other"));

    // February is filed with its own answer, then amended: the amendment
    // is February's, never January's again.
    let february = open_at("1601C", 2026, 2);
    let february = fill_all(
        "1601C",
        &february["draft"],
        json!({"frm1601c:txt20Other": "Meal allowance"}),
    );
    let id = stored_id(&save("1601C", &february["draft"]));
    assert_ne!(id, january);
    raw_exec(
        "UPDATE form_drafts SET status = 'Submitted', data_json = json_set(data_json, '$.status', 'Submitted') WHERE id = ?1",
        &[&id.to_string()],
    );
    let amended = call_async(
        "screen_form_amend",
        vec![JUANA.into(), "1601C".into(), 2026.into(), 2.into()],
    )
    .unwrap();
    assert_eq!(amended["values"]["frm1601c:txt20Other"], "Meal allowance");
    assert_eq!(amended["kept"], json!({}));
    assert_eq!(amended["auto_stops"], json!([]));
}

#[test]
fn a_transaction_form_copies_nothing() {
    setup();
    install_juana();
    let kind = kind_of("1606").unwrap();
    let def = kind.definition();
    let first = open_at("1606", 2026, 1);
    // Some plain answer the filer types on the first transaction.
    let typed = agent::carry::stoppable(kind)
        .into_iter()
        .filter(|key| {
            matches!(
                def.bindings.get(*key).map(|bound| &bound.binding),
                Some(Binding::Text {
                    date: None,
                    part: None,
                    ..
                })
            )
        })
        .find_map(|key| {
            let filled = fill("1606", &first["draft"], json!({ key: "Sample answer" }));
            (filled["rejected"] == json!({})).then_some((key, filled))
        });
    let (key, filled) = typed.expect("1606 has a plain text answer");
    save("1606", &filled["draft"]);

    let next = open_at("1606", 2026, 2);
    assert!(next["draft"]["id"].is_null(), "a new transaction");
    assert_ne!(next["values"][key], "Sample answer", "{key}");
    assert_eq!(next["kept"], json!({}));
    assert_eq!(next["auto_stops"], json!([]));
}

#[test]
fn a_2551q_quarter_takes_the_last_quarters_atc_without_its_amount() {
    setup();
    install_juana();
    let first = open_at("2551Q", 2026, 1);
    let option = first["options"]["drpATC1"]
        .as_array()
        .and_then(|options| {
            options
                .iter()
                .map(|option| option["value"].as_str().unwrap_or("").to_string())
                .find(|value| !value.is_empty())
        })
        .expect("2551Q offers ATCs");
    let filled = fill_all(
        "2551Q",
        &first["draft"],
        json!({
            "drpATC1": option,
            "txtATCAmt1": "150000",
            "frm2551Qv2018:taxRate1": "true",
        }),
    );
    save("2551Q", &filled["draft"]);
    let second = open_at("2551Q", 2026, 2);
    assert_eq!(second["values"]["drpATC1"], option);
    assert!(empty(&second["values"], "txtATCAmt1"));
    assert_eq!(second["kept"]["drpATC1"], "last");
    assert!(keys(&second["auto_stops"]).contains("txtATCAmt1"));
    assert!(second["kept"].get("frm2551Qv2018:taxRate").is_none());
}
