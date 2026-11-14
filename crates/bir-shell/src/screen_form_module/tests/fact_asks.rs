//! Decisions 3, 5 and 8 on the form page: a return asks for the tax facts
//! it reads that are not set, Queue blocks only when one changes what is
//! filed, "Save to profile too" writes a corrected profile box through the
//! function owning it, and a filed return says when the profile changed.
//! The taxpayer is the demo individual Juana Santos.

use super::*;
use bir_core::tax_profile::{
    BranchFacts, BranchField, BranchUpdate, FieldSetting, NewBranch, NewTaxpayer, TaxpayerUpdate,
    YearFactsPatch,
};

const JUANA: &str = "00012345300000";
const ROOT: &str = "000123453";
const BRANCH: &str = "00012345300001";

/// Juana Santos with no facts set for 2026 but her flat profile's (no
/// birth date, no EOPT tier, no line of business, no ATC, no income type).
fn setup_juana() {
    use_ephemeral_database();
    clear_trigger_records();
    with_database(|db| {
        db.create_taxpayer(NewTaxpayer {
            tin_root: ROOT.into(),
            name: "Juana Santos".into(),
            taxpayer_type: bir_core::profile::TaxpayerType::Individual,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "039".into(),
                registered_address: "1 Sampaguita Street, Quezon City".into(),
                zip_code: "1100".into(),
                phone: "0281234567".into(),
                email: "juana@example.com".into(),
            },
        })
        .map_err(|error| HostError::new(error.to_string()))?;
        Ok(())
    })
    .unwrap();
}

fn set_facts(tin: &str, patch: YearFactsPatch) {
    with_database(|db| {
        db.set_year_facts(tin, 2026, &patch)
            .map(|_| ())
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
}

fn set_birth_date(date: &str) {
    with_database(|db| {
        db.update_taxpayer(
            ROOT,
            TaxpayerUpdate {
                birth_date: Some(Some(date.parse().unwrap())),
                ..TaxpayerUpdate::default()
            },
        )
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
}

fn open_as(tin: &str, code: &str, period: i64) -> Value {
    call_async(
        "screen_form_open",
        vec![tin.into(), code.into(), 2026i64.into(), period.into()],
    )
    .unwrap_or_else(|error| panic!("{code} opens: {}", error.message()))
}

fn at(day: (i32, u32, u32), run: impl FnOnce()) {
    bir_core::forms::penalty_items::with_clock(
        chrono::NaiveDate::from_ymd_opt(day.0, day.1, day.2).unwrap(),
        run,
    );
}

/// The printed item number of the page item that holds `key`.
fn item_of(code: &str, key: &str) -> String {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|part| part["items"].as_array().unwrap())
        .find(|item| {
            item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|field| field["k"] == key)
        })
        .and_then(|item| item["no"].as_str())
        .unwrap_or_else(|| panic!("{code}: {key} is on no numbered item"))
        .trim_start_matches('0')
        .to_string()
}

fn ask<'a>(opened: &'a Value, fact: &str) -> Option<&'a Value> {
    opened["asks"]
        .as_array()
        .unwrap_or_else(|| panic!("asks is a list: {}", opened["asks"]))
        .iter()
        .find(|ask| ask["fact"] == fact)
}

fn asked_at(opened: &Value, code: &str, fact: &str) -> String {
    let ask = ask(opened, fact).unwrap_or_else(|| panic!("{code} asks {fact}: {}", opened["asks"]));
    item_of(code, ask["key"].as_str().expect("placed"))
}

fn queue_check(code: &str, draft: &Value) -> Value {
    call(
        "screen_form_queue_check",
        vec![code.into(), json(draft.clone())],
    )
    .unwrap_or_else(|error| panic!("{code} queue check: {}", error.message()))
}

fn blocks_on(check: &Value, fact: &str) -> Option<Value> {
    check["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["field"] == format!("tax_profile.{fact}"))
        .cloned()
}

fn warns(check: &Value) -> Vec<String> {
    check["warns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|warn| warn.as_str().unwrap().to_string())
        .collect()
}

const EOPT_WARNING: &str =
    "EOPT tier is not set. It would set the penalty rates if this return were late.";

#[test]
fn each_fact_not_set_is_asked_at_the_item_that_uses_it() {
    setup_juana();
    set_facts(
        JUANA,
        YearFactsPatch {
            is_government_withholding_entity: Some(None),
            ..YearFactsPatch::default()
        },
    );
    let q = open_as(JUANA, "1701Q", 1);
    assert_eq!(
        asked_at(&q, "1701Q", "birth_date"),
        "11",
        "the date of birth"
    );
    assert_eq!(
        asked_at(&q, "1701Q", "atc_codes"),
        "8",
        "the first ATC item"
    );
    assert!(ask(&q, "eopt_tier").is_some(), "{}", q["asks"]);
    assert!(
        ask(&q, "income_tax_election").is_none(),
        "the rate is the form's own item"
    );
    let birth = ask(&q, "birth_date").unwrap();
    assert_eq!(birth["control"], "date");
    assert_eq!(
        birth["note"],
        "Juana Santos's date of birth is not in the tax profile; what you save here goes there for every later return."
    );
    for code in ["1700", "1701", "1701A"] {
        let opened = open_as(JUANA, code, 1);
        let key = ask(&opened, "birth_date")
            .unwrap_or_else(|| panic!("{code}: {}", opened["asks"]))["key"]
            .as_str()
            .unwrap_or_else(|| panic!("{code}: the birth date is placed"))
            .to_string();
        assert!(
            box_paths_of(code, &key)
                .iter()
                .any(|path| path.ends_with("birth_date") || path.ends_with("date_of_birth")),
            "{code}: {key}"
        );
    }
    assert_eq!(
        asked_at(&open_as(JUANA, "1701", 1), "1701", "atc_codes"),
        "7"
    );
    let e = open_as(JUANA, "0619E", 7);
    assert_eq!(
        asked_at(&e, "0619E", "is_government_withholding_entity"),
        "12"
    );
    let gov = ask(&e, "is_government_withholding_entity").unwrap();
    assert_eq!(gov["control"], "choice");
    assert_eq!(gov["choices"][1]["value"], true);
    assert_eq!(
        asked_at(
            &open_as(JUANA, "0619F", 7),
            "0619F",
            "is_government_withholding_entity"
        ),
        "11"
    );
    assert_eq!(
        ask(&e, "line_of_business").unwrap()["key"],
        key_bound_to("0619E", "/line_of_business"),
        "at its box"
    );
    let penalties = ask(&e, "eopt_tier").unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        penalties::computed_groups(kind_of("0619E").unwrap())
            .iter()
            .any(|group| group.keys.contains(&penalties)),
        "0619E: EOPT tier at its penalties, {penalties}"
    );
    assert_eq!(
        asked_at(&open_as(JUANA, "2550Q", 1), "2550Q", "eopt_tier"),
        "13",
        "2550Q's taxpayer classification"
    );
    let pt = open_as(JUANA, "2551Q", 1);
    assert_eq!(asked_at(&pt, "2551Q", "tax_classification"), "13");
    assert_eq!(asked_at(&pt, "2551Q", "business_start_date"), "13");
    assert_eq!(
        asked_at(
            &open_as(JUANA, "1702RT", 12),
            "1702RT",
            "business_start_date"
        ),
        "10"
    );
    assert_eq!(
        asked_at(
            &open_as(JUANA, "1702EX", 12),
            "1702EX",
            "business_start_date"
        ),
        "10"
    );
    let c = open_as(JUANA, "1601C", 2);
    assert_eq!(
        ask(&c, "line_of_business").unwrap()["key"],
        key_bound_to("1601C", "/line_of_business")
    );
    assert!(ask(&c, "birth_date").is_none(), "1601C reads no birth date");

    // A head-office fact asked on a branch return is set for all branches.
    with_database(|db| {
        db.create_branch(
            ROOT,
            NewBranch {
                branch_code: "00001".into(),
                name: "Branch 00001".into(),
                fields: BTreeMap::new(),
            },
        )
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
    let branch = open_as(BRANCH, "1601C", 2);
    assert!(
        ask(&branch, "eopt_tier").unwrap()["note"]
            .as_str()
            .unwrap()
            .ends_with("for every later return and for all branches."),
        "{}",
        branch["asks"]
    );
    assert!(
        ask(&branch, "line_of_business").unwrap()["note"]
            .as_str()
            .unwrap()
            .ends_with("for every later return."),
        "a branch's own fact"
    );
}

fn box_paths_of(code: &str, key: &str) -> Vec<String> {
    let def = kind_of(code).unwrap().definition();
    bound_paths(def, key)
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[test]
fn nothing_is_asked_once_set_on_a_sample_or_on_a_filed_return() {
    setup_juana();
    set_birth_date("1980-02-03");
    set_facts(
        JUANA,
        YearFactsPatch {
            eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
            line_of_business: Some(Some("Bookkeeping services".into())),
            atc_codes: Some(vec!["II012".into()]),
            ..YearFactsPatch::default()
        },
    );
    let opened = open_as(JUANA, "1701Q", 1);
    assert_eq!(opened["asks"], json!([]), "every fact is set");
    assert_eq!(opened["fact_errors"], json!([]));

    setup_juana();
    let sample = call_async("screen_form_sample", vec![JUANA.into(), "1701Q".into()]).unwrap();
    assert!(
        sample["asks"].as_array().is_none_or(Vec::is_empty),
        "{}",
        sample["asks"]
    );
    let opened = open_as(JUANA, "1601C", 2);
    assert!(ask(&opened, "eopt_tier").is_some());
    let draft = opened["draft"].clone();
    let id = {
        let mut draft = draft.clone();
        draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
        save("1601C", &draft)["id"].as_i64().unwrap()
    };
    let (_, raw) = raw_row(id);
    let filed = with_status(&serde_json::from_str(&raw).unwrap(), "Submitted");
    raw_exec(
        &format!("UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = {id}"),
        &[&serde_json::to_string(&filed).unwrap()],
    );
    let reopened = open_as(JUANA, "1601C", 2);
    assert_eq!(reopened["status"], "Submitted");
    assert_eq!(reopened["asks"], json!([]), "a filed return asks nothing");
}

#[test]
fn an_answer_saved_from_a_card_brings_the_return_up_to_date() {
    setup_juana();
    let opened = open_as(JUANA, "1701Q", 1);
    let month = ask(&opened, "birth_date").unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(opened["values"][month.as_str()], "");
    // The card's Save: the birth date goes to the taxpayer (update_taxpayer).
    call(
        "update_taxpayer",
        vec![ROOT.into(), json(json!({ "birth_date": "1980-02-03" }))],
    )
    .unwrap();
    let used = call(
        "screen_form_use_profile",
        vec![
            "1701Q".into(),
            2026i64.into(),
            1i64.into(),
            json(opened["draft"].clone()),
            json(json!(["birth_date"])),
        ],
    )
    .unwrap();
    assert_eq!(used["draft"]["date_of_birth"], "02/03/1980");
    assert_eq!(used["values"]["frm1701q:txtBirthYear"], "1980");
    assert!(
        ask(&used, "birth_date").is_none(),
        "the card goes: {}",
        used["asks"]
    );
    assert!(ask(&used, "eopt_tier").is_some(), "the others stay");

    // A box the page filled with a guess (0619E's Private) takes the answer.
    set_facts(
        JUANA,
        YearFactsPatch {
            is_government_withholding_entity: Some(None),
            ..YearFactsPatch::default()
        },
    );
    let e = open_as(JUANA, "0619E", 7);
    assert_eq!(e["draft"]["withholding_agent_category"], "Private");
    call(
        "set_year_facts",
        vec![
            JUANA.into(),
            2026i64.into(),
            json(json!({ "is_government_withholding_entity": true })),
        ],
    )
    .unwrap();
    let used = call(
        "screen_form_use_profile",
        vec![
            "0619E".into(),
            2026i64.into(),
            7i64.into(),
            json(e["draft"].clone()),
            json(json!(["is_government_withholding_entity"])),
        ],
    )
    .unwrap();
    assert_eq!(used["draft"]["withholding_agent_category"], "Government");
    assert!(ask(&used, "is_government_withholding_entity").is_none());
}

#[test]
fn queue_blocks_a_1701q_without_a_birth_date() {
    setup_juana();
    at((2026, 4, 1), || {
        let opened = open_as(JUANA, "1701Q", 1);
        let key = ask(&opened, "birth_date").unwrap()["key"].clone();
        let block = opened["fact_errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|error| error["field"] == "tax_profile.birth_date")
            .unwrap_or_else(|| panic!("a message on the row: {}", opened["fact_errors"]))
            .clone();
        assert_eq!(block["keys"], json!([key]), "on the card's row");
        assert_eq!(
            block["message"],
            "Date of birth is not set in the tax profile, and this return needs it. Answer it in the card at Item 11, then queue."
        );
        let saved = save("1701Q", &opened["draft"]);
        let check = queue_check("1701Q", &saved["draft"]);
        assert!(blocks_on(&check, "birth_date").is_some(), "{check}");
        let validated = call_async(
            "screen_form_validate",
            vec!["1701Q".into(), json(saved["draft"].clone())],
        )
        .unwrap();
        assert!(
            validated["fact_errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|error| error["field"] == "tax_profile.birth_date")
        );
        let refused = call(
            "screen_form_queue",
            vec!["1701Q".into(), json(saved["draft"].clone())],
        )
        .unwrap_err();
        assert!(
            refused.message().contains("Date of birth is not set"),
            "{}",
            refused.message()
        );
    });
}

#[test]
fn eopt_tier_warns_on_time_and_blocks_a_late_1601c() {
    setup_juana();
    at((2026, 3, 1), || {
        // February, due March 10: on time.
        let on_time = save("1601C", &open_as(JUANA, "1601C", 2)["draft"]);
        let check = queue_check("1601C", &on_time["draft"]);
        assert!(blocks_on(&check, "eopt_tier").is_none(), "{check}");
        assert!(warns(&check).contains(&EOPT_WARNING.to_string()), "{check}");
        assert!(
            warns(&check).contains(
                &"Line of business is not set in the tax profile. This return can go without it."
                    .to_string()
            ),
            "{check}"
        );
        // January, due February 10: late, the tier sets its penalty rates.
        let late = save("1601C", &open_as(JUANA, "1601C", 1)["draft"]);
        let check = queue_check("1601C", &late["draft"]);
        let block = blocks_on(&check, "eopt_tier").unwrap_or_else(|| panic!("{check}"));
        assert!(
            block["message"]
                .as_str()
                .unwrap()
                .starts_with("EOPT tier is not set in the tax profile, and it sets the penalty rates on this return."),
            "{block}"
        );
        assert!(!warns(&check).contains(&EOPT_WARNING.to_string()));
        let refused = call(
            "queue_form_draft",
            vec![JUANA.into(), "1601C".into(), 2026i64.into(), 1i64.into()],
        )
        .unwrap_err();
        assert!(
            refused.message().contains("EOPT tier is not set"),
            "{}",
            refused.message()
        );
        // Set, nothing about the tier is left.
        set_facts(
            JUANA,
            YearFactsPatch {
                eopt_tier: Some(Some(bir_core::profile::EoptTier::Small)),
                ..YearFactsPatch::default()
            },
        );
        let check = queue_check("1601C", &late["draft"]);
        assert!(blocks_on(&check, "eopt_tier").is_none());
        assert!(!warns(&check).iter().any(|warn| warn.starts_with("EOPT")));
    });
}

#[test]
fn penalty_amounts_entered_on_time_make_the_eopt_tier_block() {
    setup_juana();
    at((2026, 3, 1), || {
        let opened = open_as(JUANA, "1601C", 2);
        let group = penalties::computed_groups(kind_of("1601C").unwrap())
            .into_iter()
            .next()
            .unwrap();
        let filled = fill_unlocked(
            "1601C",
            &opened["draft"],
            json!({ group.keys[0].clone(): "500" }),
            &[group.keys[0].as_str()],
        );
        let saved = save("1601C", &filled["draft"]);
        assert!(blocks_on(&queue_check("1601C", &saved["draft"]), "eopt_tier").is_some());
    });
}

#[test]
fn the_0619e_government_answer_follows_the_forms_required_flag() {
    setup_juana();
    set_facts(
        JUANA,
        YearFactsPatch {
            is_government_withholding_entity: Some(None),
            eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
            ..YearFactsPatch::default()
        },
    );
    let kind = kind_of("0619E").unwrap();
    let placed = crate::screen_form_module::fact_asks::place(
        kind,
        bir_core::tax_profile::Fact::IsGovernmentWithholdingEntity,
    )
    .unwrap();
    assert!(placed.required, "0619E requires Item 12");
    at((2026, 8, 1), || {
        let saved = save("0619E", &open_as(JUANA, "0619E", 7)["draft"]);
        let check = queue_check("0619E", &saved["draft"]);
        let block = blocks_on(&check, "is_government_withholding_entity")
            .unwrap_or_else(|| panic!("{check}"));
        assert_eq!(block["keys"], json!([placed.key]));
        // Answered: Queue goes on.
        set_facts(
            JUANA,
            YearFactsPatch {
                is_government_withholding_entity: Some(Some(false)),
                line_of_business: Some(Some("Bookkeeping services".into())),
                ..YearFactsPatch::default()
            },
        );
        let check = queue_check("0619E", &saved["draft"]);
        assert_eq!(check["blocks"], json!([]), "{check}");
        call(
            "screen_form_queue",
            vec!["0619E".into(), json(saved["draft"].clone())],
        )
        .unwrap_or_else(|error| panic!("0619E queues: {}", error.message()));
    });
}

#[test]
fn a_draft_follows_a_later_fact() {
    setup_juana();
    let saved = save("1701Q", &open_as(JUANA, "1701Q", 1)["draft"]);
    assert_eq!(saved["draft"]["date_of_birth"], "");
    set_birth_date("1980-02-03");
    let reopened = open_as(JUANA, "1701Q", 1);
    assert_eq!(reopened["draft"]["date_of_birth"], "02/03/1980");
    let updates = reopened["profile_updates"].to_string();
    assert!(updates.contains("1980"), "the note names it: {updates}");
}

/// The 1601C's box bound to `path`.
fn key_for(path: &str) -> String {
    key_bound_to("1601C", path)
}

fn branch_value(tin: &str, field: BranchField) -> (String, bool) {
    with_database(|db| {
        let branch = db
            .get_branch(tin)
            .map_err(|error| HostError::new(error.to_string()))?
            .unwrap();
        let resolved = &branch.facts[&field];
        Ok((resolved.value.clone(), resolved.inherited))
    })
    .unwrap()
}

fn edited_save(tin: &str, edits: Value, keys: &[&str]) -> Value {
    let opened = open_as(tin, "1601C", 2);
    let filled = fill_unlocked("1601C", &opened["draft"], edits, keys);
    save("1601C", &filled["draft"])
}

#[test]
fn save_to_profile_too_writes_each_field_through_its_own_function() {
    setup_juana();
    set_facts(
        JUANA,
        YearFactsPatch {
            line_of_business: Some(Some("Bookkeeping services".into())),
            ..YearFactsPatch::default()
        },
    );
    let rdo = key_for("/rdo_code");
    let name = key_for("/taxpayer_name");
    let line = key_for("/line_of_business");
    let opened = open_as(JUANA, "1601C", 2);
    for key in [&rdo, &name, &line] {
        assert!(
            opened["profile_save"].get(key.as_str()).is_some(),
            "{key}: {}",
            opened["profile_save"]
        );
    }
    assert_eq!(opened["profile_save"][rdo.as_str()]["field"], "rdo_code");
    assert!(
        opened["profile_save"]
            .as_object()
            .unwrap()
            .keys()
            .all(|key| !key.contains("TIN") && !key.contains("Tin")),
        "no TIN box has a switch"
    );

    // Off: the edit stays on this return.
    let saved = edited_save(JUANA, json!({ rdo.clone(): "043" }), &[&rdo]);
    assert_eq!(saved["draft"]["rdo_code"], "043");
    assert_eq!(
        branch_value(JUANA, BranchField::RdoCode).0,
        "039",
        "the profile is untouched"
    );

    // On: written through, and the box is the profile's again.
    let to_profile = |draft: &Value, keys: &[&str]| {
        call(
            "screen_form_save_to_profile",
            vec![
                "1601C".into(),
                2026i64.into(),
                2i64.into(),
                json(draft.clone()),
                json(json!(keys)),
            ],
        )
        .unwrap_or_else(|error| panic!("save to profile: {}", error.message()))
    };
    let written = to_profile(&saved["draft"], &[&rdo]);
    assert_eq!(branch_value(JUANA, BranchField::RdoCode).0, "043");
    assert_eq!(written["saved"], json!(["RDO"]));
    assert!(
        !profile_edits(&written["draft"]).contains(&rdo),
        "no longer edited here"
    );
    assert_eq!(written["draft"]["rdo_code"], "043");
    let reopened = open_as(JUANA, "1601C", 2);
    assert_eq!(reopened["draft"]["rdo_code"], "043");
    assert!(
        !profile_edits(&reopened["draft"]).contains(&rdo),
        "stored so"
    );

    // The name goes to the taxpayer, the line of business to the year's facts.
    let saved = edited_save(
        JUANA,
        json!({ name.clone(): "Juana Santos Cruz", line.clone(): "Tax preparation" }),
        &[&name, &line],
    );
    to_profile(&saved["draft"], &[&name, &line]);
    with_database(|db| {
        let taxpayer = db
            .get_taxpayer(ROOT)
            .map_err(|error| HostError::new(error.to_string()))?
            .unwrap();
        assert_eq!(taxpayer.taxpayer.name, "Juana Santos Cruz");
        let facts = db
            .effective_year_facts(JUANA, 2026)
            .map_err(|error| HostError::new(error.to_string()))?;
        assert_eq!(
            facts.facts.line_of_business.as_deref(),
            Some("Tax preparation")
        );
        Ok(())
    })
    .unwrap();

    // A box with no safe write path is refused, and nothing is written.
    let tin_box = schema_keys("1601C")
        .iter()
        .find(|key| key.contains("TIN"))
        .expect("1601C shows its TIN")
        .clone();
    let refused = call(
        "screen_form_save_to_profile",
        vec![
            "1601C".into(),
            2026i64.into(),
            2i64.into(),
            json(saved["draft"].clone()),
            json(json!([tin_box])),
        ],
    );
    assert!(refused.is_err());
}

#[test]
fn save_to_profile_too_on_a_branch_that_follows_the_head_office() {
    setup_juana();
    with_database(|db| {
        db.create_branch(
            ROOT,
            NewBranch {
                branch_code: "00001".into(),
                name: "Branch 00001".into(),
                fields: BTreeMap::new(),
            },
        )
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
    assert_eq!(
        branch_value(BRANCH, BranchField::RdoCode),
        ("039".into(), true)
    );
    let rdo = key_for("/rdo_code");
    let opened = open_as(BRANCH, "1601C", 2);
    assert!(
        opened["profile_save"][rdo.as_str()]["note"]
            .as_str()
            .unwrap()
            .contains("head office"),
        "{}",
        opened["profile_save"]
    );
    let saved = edited_save(BRANCH, json!({ rdo.clone(): "044" }), &[&rdo]);
    let written = call(
        "screen_form_save_to_profile",
        vec![
            "1601C".into(),
            2026i64.into(),
            2i64.into(),
            json(saved["draft"].clone()),
            json(json!([rdo])),
        ],
    )
    .unwrap();
    assert_eq!(
        written["notes"],
        json!(["This branch used the head office's RDO; it now has its own."])
    );
    assert_eq!(
        branch_value(BRANCH, BranchField::RdoCode),
        ("044".into(), false)
    );
    assert_eq!(
        branch_value(JUANA, BranchField::RdoCode).0,
        "039",
        "the head office keeps its own"
    );
}

#[test]
fn a_filed_1601c_says_the_profile_changed_and_keeps_what_was_filed() {
    setup_juana();
    let opened = open_as(JUANA, "1601C", 2);
    assert_eq!(opened["profile_changed"], json!([]));
    let mut draft = opened["draft"].clone();
    draft.as_object_mut().unwrap().remove(SAMPLE_MARK);
    let id = save("1601C", &draft)["id"].as_i64().unwrap();
    let (_, raw) = raw_row(id);
    let filed = with_status(&serde_json::from_str(&raw).unwrap(), "Submitted");
    let filed_text = serde_json::to_string(&filed).unwrap();
    raw_exec(
        &format!("UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = {id}"),
        &[&filed_text],
    );
    assert_eq!(
        open_as(JUANA, "1601C", 2)["profile_changed"],
        json!([]),
        "nothing moved yet"
    );
    with_database(|db| {
        db.update_branch(
            JUANA,
            BranchUpdate {
                name: None,
                fields: BTreeMap::from([(
                    BranchField::RdoCode,
                    FieldSetting::Override("043".into()),
                )]),
            },
        )
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
    set_facts(
        JUANA,
        YearFactsPatch {
            eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
            ..YearFactsPatch::default()
        },
    );
    let reopened = open_as(JUANA, "1601C", 2);
    let changed: Vec<String> = serde_json::from_value(reopened["profile_changed"].clone()).unwrap();
    assert!(
        changed
            .iter()
            .any(|note| note.contains("039") && note.contains("profile now 043")),
        "{changed:?}"
    );
    assert!(
        changed.iter().any(|note| note == "EOPT tier"),
        "{changed:?}"
    );
    assert_eq!(
        reopened["draft"]["rdo_code"], "039",
        "it keeps what was filed"
    );
    assert_eq!(raw_row(id).1, filed_text, "and nothing is rewritten");
    assert_eq!(reopened["asks"], json!([]));
    // Another return is not affected: a Draft of another month follows.
    let other = open_as(JUANA, "1601C", 3);
    assert_eq!(other["draft"]["rdo_code"], "043");
    assert_eq!(other["profile_changed"], json!([]));
}
