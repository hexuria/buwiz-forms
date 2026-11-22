//! Form 0605's field picks on a new return (form-fields PR 1): Items 4 and
//! 7 keep three month/day/year boxes whose draft dates start empty; a part
//! typed into them lands part by part, the parts of one batch are judged as
//! one date, a cleared part shows blank rather than the official zero, and
//! Items 6 and 8 take their ATC and tax type from the official lists.

use super::*;

/// Item 4 Due Date's boxes, in page order.
const DUE: [&str; 3] = [
    "frm0605:txtDueDateMonth",
    "frm0605:txtDueDateDay",
    "frm0605:txtDueDateYear",
];
/// Item 7 Return Period's boxes.
const RETURN_PERIOD: [&str; 3] = [
    "frm0605:txtReturnPeriodMonth",
    "frm0605:txtReturnPeriodDay",
    "frm0605:txtReturnPeriodYear",
];

/// One key's edit, as the page sends it.
fn one_edit(key: &str, value: &str) -> Value {
    let mut edits = Map::new();
    edits.insert(key.to_string(), json!(value));
    Value::Object(edits)
}

/// What the page shows in `key`'s box after `fill`.
fn shown(filled: &Value, key: &str) -> String {
    filled["values"][key]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The draft's `field` date as (year, month, day), or None.
fn date_of(draft: &Value, field: &str) -> Option<(u64, u64, u64)> {
    let date = &draft[field];
    if date.is_null() {
        return None;
    }
    Some((
        date["year"].as_u64().unwrap_or_default(),
        date["month"].as_u64().unwrap_or_default(),
        date["day"].as_u64().unwrap_or_default(),
    ))
}

/// The due date and the return period in one batch land the whole dates on
/// a new return; the official field map shows the same parts the page does.
#[test]
fn date_parts_in_one_batch_land_the_whole_dates() {
    setup();
    let opened = open("0605", 1);
    let draft = &opened["draft"];
    assert!(draft["due_date"].is_null(), "a new return has no due date");
    assert!(
        draft["return_period"].is_null(),
        "a new return has no return period"
    );

    let mut edits = Map::new();
    for (key, value) in [
        (DUE[0], "08"),
        (DUE[1], "17"),
        (DUE[2], "2023"),
        (RETURN_PERIOD[0], "12"),
        (RETURN_PERIOD[1], "31"),
        (RETURN_PERIOD[2], "2023"),
    ] {
        edits.insert(key.to_string(), json!(value));
    }
    let filled = fill("0605", draft, Value::Object(edits));
    assert!(
        filled["rejected"].as_object().unwrap().is_empty(),
        "nothing is refused: {:?}",
        filled["rejected"]
    );
    let draft = &filled["draft"];
    assert_eq!(date_of(draft, "due_date"), Some((2023, 8, 17)));
    assert_eq!(date_of(draft, "return_period"), Some((2023, 12, 31)));
    let map = agent::field_map(kind_of("0605").expect("0605 kind"), draft).expect("0605 field map");
    assert_eq!(map["frm0605:txtDueDateMonth"], "08");
    assert_eq!(map["frm0605:txtDueDateDay"], "17");
    assert_eq!(map["frm0605:txtDueDateYear"], "2023");
    assert_eq!(map["frm0605:txtReturnPeriodMonth"], "12");
    assert_eq!(map["frm0605:txtReturnPeriodDay"], "31");
    assert_eq!(map["frm0605:txtReturnPeriodYear"], "2023");
}

/// The same parts sent one key per call land the same end state, nothing
/// rejected at any step; after the month alone the date is still empty and
/// only the month box shows anything.
#[test]
fn date_parts_sent_one_key_at_a_time_land_the_same() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();
    for (key, value) in [(DUE[0], "08"), (DUE[1], "17"), (DUE[2], "2023")] {
        let filled = fill("0605", &draft, one_edit(key, value));
        assert!(
            filled["rejected"][key].is_null(),
            "{key} takes {value}: {:?}",
            filled["rejected"]
        );
        draft = filled["draft"].clone();
    }
    assert_eq!(date_of(&draft, "due_date"), Some((2023, 8, 17)));

    // After the month alone: no whole date, the month shows, the rest blank.
    let mut draft = open("0605", 1)["draft"].clone();
    let filled = fill("0605", &draft, one_edit(DUE[0], "08"));
    assert!(
        filled["rejected"][DUE[0]].is_null(),
        "{:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert!(date_of(&draft, "due_date").is_none());
    assert_eq!(shown(&filled, DUE[0]), "08");
    for key in [DUE[1], DUE[2]] {
        assert_eq!(shown(&filled, key), "", "{key} shows blank, not zero");
    }
}

/// On a complete date, clearing the day keeps the rest and shows it blank;
/// clearing every part clears the date and blanks every box.
#[test]
fn cleared_date_parts_stay_blank() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();
    let mut edits = Map::new();
    for (key, value) in [(DUE[0], "08"), (DUE[1], "17"), (DUE[2], "2023")] {
        edits.insert(key.to_string(), json!(value));
    }
    let filled = fill("0605", &draft, Value::Object(edits));
    draft = filled["draft"].clone();
    assert_eq!(date_of(&draft, "due_date"), Some((2023, 8, 17)));

    // The day cleared alone: the date is incomplete, month and year stay.
    let filled = fill("0605", &draft, one_edit(DUE[1], ""));
    assert!(
        filled["rejected"][DUE[1]].is_null(),
        "{:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert!(date_of(&draft, "due_date").is_none());
    assert_eq!(shown(&filled, DUE[0]), "08");
    assert_eq!(shown(&filled, DUE[1]), "");
    assert_eq!(shown(&filled, DUE[2]), "2023");

    // Every part cleared: the date is gone and every box is blank.
    let mut edits = Map::new();
    for key in DUE {
        edits.insert(key.to_string(), json!(""));
    }
    let filled = fill("0605", &draft, Value::Object(edits));
    draft = filled["draft"].clone();
    assert!(date_of(&draft, "due_date").is_none());
    for key in DUE {
        assert_eq!(shown(&filled, key), "", "{key} shows blank, not zero");
    }
}

/// An impossible complete date arriving in one batch is refused whole —
/// keyed to the month box only — and the draft is unchanged.
#[test]
fn an_impossible_date_batch_is_refused_on_the_month_box() {
    setup();
    let draft = open("0605", 1)["draft"].clone();
    let mut edits = Map::new();
    for (key, value) in [(DUE[0], "02"), (DUE[1], "30"), (DUE[2], "2023")] {
        edits.insert(key.to_string(), json!(value));
    }
    let filled = fill("0605", &draft, Value::Object(edits));
    assert!(
        filled["rejected"][DUE[0]].is_string(),
        "the month box carries the refusal: {:?}",
        filled["rejected"]
    );
    for key in [DUE[1], DUE[2]] {
        assert!(
            filled["rejected"][key].is_null(),
            "{key} is not separately refused: {:?}",
            filled["rejected"]
        );
    }
    assert!(date_of(&filled["draft"], "due_date").is_none());
    assert!(
        filled["draft"]["due_date_typed"].is_null()
            || filled["draft"]["due_date_typed"] == json!({}),
        "no part of the refused date lands: {:?}",
        filled["draft"]["due_date_typed"]
    );
}

/// A partial date is not a fill refusal — the parts land — and validation
/// reports the date as incomplete on the item.
#[test]
fn a_partial_date_validates_as_incomplete() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();
    let filled = fill("0605", &draft, one_edit(DUE[0], "08"));
    assert!(
        filled["rejected"][DUE[0]].is_null(),
        "{:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert!(date_of(&draft, "due_date").is_none());

    let checked = call_async(
        "screen_form_validate",
        vec!["0605".into(), json(draft.clone())],
    )
    .expect("0605 validates");
    let on_date = checked["errors"]
        .as_array()
        .expect("errors array")
        .iter()
        .any(|error| {
            error["message"]
                .as_str()
                .is_some_and(|message| message.to_lowercase().contains("date"))
                || error["keys"]
                    .as_array()
                    .is_some_and(|keys| keys.iter().any(|k| k.as_str() == Some(DUE[0])))
        });
    assert!(
        on_date,
        "the incomplete date is reported on Item 4: {}",
        checked["errors"]
    );
}

/// The same writes through the agent door the assistant and the probe use
/// (`fill_as_user`), one box at a time: the typed writer, not `set_path`.
#[test]
fn the_agent_write_path_takes_the_same_parts() {
    setup();
    let kind = kind_of("0605").expect("0605 kind");
    let mut draft = open("0605", 1)["draft"].clone();
    for (key, value) in [
        (DUE[0], "08"),
        (DUE[1], "17"),
        (DUE[2], "2023"),
        (RETURN_PERIOD[0], "12"),
        (RETURN_PERIOD[1], "31"),
        (RETURN_PERIOD[2], "2023"),
    ] {
        agent::fill_as_user(kind, &mut draft, key, &json!(value))
            .unwrap_or_else(|error| panic!("{key} takes {value}: {error}"));
    }
    assert_eq!(date_of(&draft, "due_date"), Some((2023, 8, 17)));
    assert_eq!(date_of(&draft, "return_period"), Some((2023, 12, 31)));
}

/// Item 6's ATC and Item 8's tax type are picked from the official lists: a
/// listed code stores the whole indexed code the official map writes as its
/// `AtcCode<n>`/`TaxTypeCode<n>` radio, a blank clears it, and a code the
/// list does not have is refused without touching the draft.
#[test]
fn atc_and_tax_type_are_picked_from_the_official_lists() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();

    // FP010 is the first ATC of FORM_0605_ATCS (xml index 1); DO the
    // fourth tax type (xml index 4).
    let filled = fill("0605", &draft, json!({ "txtATCCode": "FP010" }));
    assert!(
        filled["rejected"]["txtATCCode"].is_null(),
        "FP010 is on the list: {:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert_eq!(draft["atc"]["code"], "FP010");
    assert_eq!(draft["atc"]["xml_index"], 1);
    assert_eq!(shown(&filled, "txtATCCode"), "FP010");

    let filled = fill("0605", &draft, json!({ "txtTaxTypeCode": "DO" }));
    assert!(
        filled["rejected"]["txtTaxTypeCode"].is_null(),
        "DO is on the list: {:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert_eq!(draft["tax_type"]["code"], "DO");
    assert_eq!(draft["tax_type"]["xml_index"], 4);

    // The official map files the picks as their popup radios.
    let map =
        agent::field_map(kind_of("0605").expect("0605 kind"), &draft).expect("0605 field map");
    assert_eq!(map["txtATCCode"], "FP010");
    assert_eq!(map["AtcCode1"], "true");
    assert_eq!(map["txtTaxTypeCode"], "DO");
    assert_eq!(map["TaxTypeCode4"], "true");

    // A code the list does not have is refused, the draft untouched.
    let filled = fill("0605", &draft, json!({ "txtATCCode": "ZZ999" }));
    assert_eq!(
        filled["rejected"]["txtATCCode"].as_str(),
        Some("Pick an ATC from the list."),
        "{:?}",
        filled["rejected"]
    );
    assert_eq!(filled["draft"]["atc"]["code"], "FP010");

    // An unlisted tax type is refused the same way.
    let filled = fill("0605", &draft, json!({ "txtTaxTypeCode": "ZZ" }));
    assert_eq!(
        filled["rejected"]["txtTaxTypeCode"].as_str(),
        Some("Pick a tax type from the list."),
        "{:?}",
        filled["rejected"]
    );
    assert_eq!(filled["draft"]["tax_type"]["code"], "DO");

    // Blank clears the pick.
    let filled = fill("0605", &draft, json!({ "txtATCCode": "" }));
    assert!(
        filled["rejected"]["txtATCCode"].is_null(),
        "{:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert!(draft["atc"].is_null());
}

/// The ATC list follows the tax type — the PT ATCs are offered only under
/// PT, and choosing a tax type the held ATC is not offered under clears it.
#[test]
fn the_atc_list_follows_the_tax_type() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();
    let filled = fill("0605", &draft, json!({ "txtTaxTypeCode": "PT" }));
    assert!(
        filled["rejected"]["txtTaxTypeCode"].is_null(),
        "{:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert_eq!(draft["tax_type"]["code"], "PT");

    // A non-PT ATC is not offered under PT.
    let filled = fill("0605", &draft, json!({ "txtATCCode": "FP010" }));
    assert!(
        filled["rejected"]["txtATCCode"].is_string(),
        "FP010 is not offered under PT: {:?}",
        filled["rejected"]
    );
    assert!(filled["draft"]["atc"].is_null());

    // A PT ATC is.
    let filled = fill("0605", &draft, json!({ "txtATCCode": "PT010" }));
    assert!(
        filled["rejected"]["txtATCCode"].is_null(),
        "PT010 is offered under PT: {:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert_eq!(draft["atc"]["code"], "PT010");

    // Moving the tax type off PT clears the ATC it no longer offers.
    let filled = fill("0605", &draft, json!({ "txtTaxTypeCode": "DO" }));
    assert!(
        filled["rejected"]["txtTaxTypeCode"].is_null(),
        "{:?}",
        filled["rejected"]
    );
    draft = filled["draft"].clone();
    assert_eq!(draft["tax_type"]["code"], "DO");
    assert!(
        draft["atc"].is_null(),
        "PT010 is not offered under DO: {:?}",
        draft["atc"]
    );
}

/// An imported pair that is not a reviewed pick still opens and shows its
/// code — the official map reads the code, not the evidence.
#[test]
fn an_imported_atc_still_shows_its_code() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();
    let filled = fill("0605", &draft, json!({ "txtATCCode": "FP010" }));
    draft = filled["draft"].clone();
    assert!(
        draft["atc"].is_object(),
        "FP010 picked from the list: {:?}",
        filled["rejected"]
    );
    // The official page writes the ATC radios only once a tax type is
    // chosen; an imported return carries both.
    let filled = fill("0605", &draft, json!({ "txtTaxTypeCode": "DO" }));
    draft = filled["draft"].clone();
    draft["atc"]["evidence"] = json!("ImportedExact");
    let map =
        agent::field_map(kind_of("0605").expect("0605 kind"), &draft).expect("0605 field map");
    assert_eq!(map["txtATCCode"], "FP010");
    assert_eq!(map["AtcCode1"], "true");
}

/// The official lists reach the page as the boxes' options: every tax type
/// is offered, the ATC list follows the picked tax type (no PT ATC before
/// PT, only PT ATCs under it), and the options the fill answers carry are
/// computed on the draft the batch leaves.
#[test]
fn the_official_lists_reach_the_page_as_options() {
    setup();
    let opened = open("0605", 1);
    let tax_options = opened["options"]["txtTaxTypeCode"]
        .as_array()
        .expect("txtTaxTypeCode options");
    for code in ["DO", "PT", "IT", "VT"] {
        assert!(
            tax_options.iter().any(|o| o["value"] == code),
            "{code} on the tax type list"
        );
    }
    // No tax type picked: the non-PT ATCs are offered, the PT ones not.
    let atc_options = opened["options"]["txtATCCode"]
        .as_array()
        .expect("txtATCCode options");
    assert!(atc_options.iter().any(|o| o["value"] == "FP010"));
    assert!(!atc_options.iter().any(|o| o["value"] == "PT010"));

    // Under PT the fill's own options answer holds only PT ATCs.
    let draft = opened["draft"].clone();
    let filled = fill("0605", &draft, json!({ "txtTaxTypeCode": "PT" }));
    let atc_options = filled["options"]["txtATCCode"]
        .as_array()
        .expect("txtATCCode options under PT");
    assert!(!atc_options.is_empty());
    assert!(
        atc_options
            .iter()
            .all(|o| o["value"].as_str().is_some_and(|v| v.starts_with("PT"))),
        "only PT ATCs under PT: {atc_options:?}"
    );
    // And a PT ATC is on it.
    assert!(atc_options.iter().any(|o| o["value"] == "PT010"));
}

/// B20: the official map uppercases the line-of-business text (capital()), so
/// the page can take the host's casing when the box is left; the draft keeps
/// what was typed, the field map and the print show the capitals.
#[test]
fn the_line_of_business_prints_in_capitals() {
    setup();
    let mut draft = open("0605", 1)["draft"].clone();
    draft["line_of_business"] = json!("Retail");
    let map =
        agent::field_map(kind_of("0605").expect("0605 kind"), &draft).expect("0605 field map");
    assert_eq!(map["frm0605:txtLineBus"], "RETAIL");
}

/// Review F1: a day typed "0" in Item 4 is refused at its box and no part of
/// the date lands.
#[test]
fn a_zero_day_in_item_4_is_refused() {
    setup();
    let draft = open("0605", 1)["draft"].clone();
    let mut edits = Map::new();
    for (key, value) in [(DUE[0], "08"), (DUE[1], "0"), (DUE[2], "2023")] {
        edits.insert(key.to_string(), json!(value));
    }
    let filled = fill("0605", &draft, Value::Object(edits));
    let rejected = filled["rejected"].as_object().expect("rejected map");
    assert!(
        rejected.contains_key(DUE[0]) || rejected.contains_key(DUE[1]),
        "the zero day is refused: {:?}",
        filled["rejected"]
    );
    assert!(date_of(&filled["draft"], "due_date").is_none());
}
