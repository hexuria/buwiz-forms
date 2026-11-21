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

#[test]
fn later_2551q_profile_prerequisites_are_host_derived_and_disappear_after_profile_updates() {
    setup_juana();
    for quarter in 2..=4 {
        let opened = open_as(JUANA, "2551Q", quarter);
        let missing = &opened["profile_prerequisites"];
        assert_eq!(missing[0]["field"], "business_start_date");
        assert_eq!(missing[1]["field"], "income_tax_election");
        assert!(missing[1]["message"].as_str().unwrap().contains("2026"));
    }
    assert_eq!(
        open_as(JUANA, "2551Q", 1)["profile_prerequisites"],
        json!([])
    );
    assert_eq!(
        open_as(JUANA, "1601C", 3)["profile_prerequisites"],
        json!([])
    );
    set_facts(
        JUANA,
        YearFactsPatch {
            tax_classification: Some(Some(bir_core::profile::TaxClassification::SelfEmployed)),
            business_start_date: Some(Some("2020-01-01".parse().unwrap())),
            income_tax_election: Some(Some(
                bir_core::profile::IncomeTaxElection::GraduatedUnspecified,
            )),
            ..YearFactsPatch::default()
        },
    );
    assert_eq!(
        open_as(JUANA, "2551Q", 3)["profile_prerequisites"],
        json!([])
    );
}

#[test]
fn a_stored_2551q_refreshes_profile_prerequisites_on_return_to_the_form() {
    setup_juana();
    let opened = open_as(JUANA, "2551Q", 3);
    let saved = save("2551Q", &opened["draft"]);
    assert_eq!(saved["draft"]["annual_income_tax_election"], "unrecorded");
    set_facts(
        JUANA,
        YearFactsPatch {
            tax_classification: Some(Some(bir_core::profile::TaxClassification::SelfEmployed)),
            business_start_date: Some(Some("2026-01-01".parse().unwrap())),
            income_tax_election: Some(Some(
                bir_core::profile::IncomeTaxElection::GraduatedUnspecified,
            )),
            ..YearFactsPatch::default()
        },
    );
    let reopened = open_as(JUANA, "2551Q", 3);
    assert_eq!(reopened["draft"]["id"], saved["draft"]["id"]);
    assert_eq!(reopened["draft"]["business_start_date"], "2026-01-01");
    assert_eq!(reopened["draft"]["annual_income_tax_election"], "graduated");
    assert_eq!(reopened["profile_prerequisites"], json!([]));
    let filled = fill_unlocked("2551Q", &reopened["draft"], json!({}), &[]);
    assert_eq!(filled["profile_prerequisites"], json!([]));
}

#[test]
fn a_2551q_keeps_its_return_only_email_when_reopened() {
    setup_juana();
    let key = "txtEmail#occurrence-1";
    let opened = open_as(JUANA, "2551Q", 3);
    let from_profile = opened["draft"]["email"].clone();
    assert_ne!(from_profile, json!("books@example.com"));
    let filled = fill_unlocked(
        "2551Q",
        &opened["draft"],
        json!({ key: "books@example.com" }),
        &[key],
    );
    let saved = save("2551Q", &filled["draft"]);
    assert_eq!(saved["draft"]["email"], "books@example.com");
    assert!(profile_edits(&saved["draft"]).contains(key));
    // Reopening reconciles the profile context but keeps the edited box,
    // and the next save does not write the profile's email over it.
    let reopened = open_as(JUANA, "2551Q", 3);
    assert_eq!(reopened["draft"]["email"], "books@example.com");
    let refilled = fill_unlocked("2551Q", &reopened["draft"], json!({}), &[]);
    let resaved = save("2551Q", &refilled["draft"]);
    assert_eq!(resaved["draft"]["email"], "books@example.com");
    assert_eq!(
        open_as(JUANA, "2551Q", 3)["draft"]["email"],
        "books@example.com"
    );
}

#[test]
fn a_calendar_2551q_saved_with_another_year_end_month_reopens_to_be_corrected() {
    setup_juana();
    let opened = open_as(JUANA, "2551Q", 3);
    let mut draft = opened["draft"].clone();
    draft["year_end_month"] = json!(6);
    let saved = save("2551Q", &draft);
    assert_eq!(saved["draft"]["year_end_month"], 6);
    let reopened = open_as(JUANA, "2551Q", 3);
    assert_eq!(reopened["draft"]["id"], saved["draft"]["id"]);
    assert_eq!(reopened["draft"]["year_end_month"], 6);
    let mut fixed = reopened["draft"].clone();
    fixed["year_end_month"] = json!(12);
    let resaved = save("2551Q", &fixed);
    assert_eq!(resaved["draft"]["year_end_month"], 12);
    assert_eq!(open_as(JUANA, "2551Q", 3)["draft"]["year_end_month"], 12);
}

#[test]
fn profile_prerequisites_respect_commencement_pt010_and_taxpayer_applicability() {
    setup_juana();
    let kind = kind_of("2551Q").unwrap();
    let draft = open_as(JUANA, "2551Q", 3)["draft"].clone();
    for (field, value) in [
        ("taxpayer_type", json!("Corporation")),
        ("schedule_1", json!([])),
        ("business_start_date", json!("2026-08-15")),
    ] {
        let mut changed = draft.clone();
        changed[field] = value;
        assert_eq!(
            profile_prerequisites::of(kind, &changed).unwrap(),
            json!([])
        );
    }
    assert!(profile_prerequisites::of(kind, &json!({})).is_err());
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

/// The printed item a fact's placed box sits on, boxed or card (Q1 P1b).
fn placed_at(code: &str, fact: &str) -> String {
    let kind = kind_of(code).unwrap();
    let fact: bir_core::tax_profile::Fact =
        serde_json::from_value(json!(fact)).expect("a fact key");
    let placed = crate::screen_form_module::fact_asks::place(kind, fact)
        .unwrap_or_else(|| panic!("{code} places {fact:?}"));
    item_of(code, &placed.key)
}

/// Whether `fact` has a writable box of its own on `code` (Q1 P1b).
fn is_boxed(code: &str, fact: &str) -> bool {
    let kind = kind_of(code).unwrap();
    let fact: bir_core::tax_profile::Fact =
        serde_json::from_value(json!(fact)).expect("a fact key");
    crate::screen_form_module::fact_asks::boxed(kind, fact).is_some()
}

#[test]
fn each_fact_not_set_is_boxed_or_asked_at_the_item_that_uses_it() {
    setup_juana();
    set_facts(
        JUANA,
        YearFactsPatch {
            is_government_withholding_entity: Some(None),
            ..YearFactsPatch::default()
        },
    );
    let q = open_as(JUANA, "1701Q", 1);
    // Boxed facts get no card: the box is the only input (Q1 P1b, B5).
    for fact in ["birth_date", "atc_codes", "line_of_business"] {
        assert!(is_boxed("1701Q", fact), "1701Q {fact} is boxed");
        assert!(ask(&q, fact).is_none(), "1701Q {fact} has no card");
    }
    assert_eq!(placed_at("1701Q", "birth_date"), "11");
    assert_eq!(placed_at("1701Q", "atc_codes"), "8");
    // The EOPT tier has no writable box here: it stays a card.
    assert!(!is_boxed("1701Q", "eopt_tier"));
    assert!(ask(&q, "eopt_tier").is_some(), "{}", q["asks"]);
    assert!(
        ask(&q, "income_tax_election").is_none(),
        "the rate is the form's own item"
    );
    // A boxed fact's box carries its profile_save entry at the same item.
    let save = &q["profile_save"];
    assert_eq!(
        save["frm1701q:txtBirthMonth"]["field"], "birth_date",
        "the birth date's box saves to the profile"
    );
    for code in ["1700", "1701", "1701A"] {
        assert!(is_boxed(code, "birth_date"), "{code} birth date boxed");
        let opened = open_as(JUANA, code, 1);
        assert!(
            ask(&opened, "birth_date").is_none(),
            "{code}: no birth-date card"
        );
    }
    assert_eq!(placed_at("1701", "atc_codes"), "7");
    assert!(is_boxed("0619E", "is_government_withholding_entity"));
    let e = open_as(JUANA, "0619E", 7);
    assert!(ask(&e, "is_government_withholding_entity").is_none());
    assert_eq!(placed_at("0619E", "is_government_withholding_entity"), "12");
    assert!(is_boxed("0619F", "is_government_withholding_entity"));
    assert_eq!(placed_at("0619F", "is_government_withholding_entity"), "11");
    assert!(is_boxed("0619E", "line_of_business"));
    assert!(
        ask(&e, "line_of_business").is_none(),
        "the line of business is boxed at its box"
    );
    assert_eq!(
        e["profile_save"]["frm0619E:txtLineBus"]["field"],
        "line_of_business"
    );
    // The EOPT tier on a penalty form has no writable box: a card.
    assert!(!is_boxed("0619E", "eopt_tier"));
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
    assert_eq!(placed_at("2550Q", "eopt_tier"), "13");
    assert!(is_boxed("2550Q", "eopt_tier"), "2550Q's classification box");
    let pt = open_as(JUANA, "2551Q", 1);
    // 2551Q's Item 13 facts have no writable box: F2 cards.
    for fact in ["tax_classification", "business_start_date", "eopt_tier"] {
        assert!(!is_boxed("2551Q", fact), "2551Q {fact} is a card");
        assert!(ask(&pt, fact).is_some(), "2551Q asks {fact}");
    }
    assert_eq!(placed_at("2551Q", "tax_classification"), "13");
    assert_eq!(placed_at("2551Q", "business_start_date"), "13");
    assert!(is_boxed("1702EX", "business_start_date"));
    // 1702RT's incorporation date is derived: it stays a card.
    assert!(!is_boxed("1702RT", "business_start_date"));
    assert!(is_boxed("1702EX", "business_start_date"));
    let c = open_as(JUANA, "1601C", 2);
    assert!(is_boxed("1601C", "line_of_business"));
    assert!(ask(&c, "line_of_business").is_none());
    assert!(ask(&c, "birth_date").is_none(), "1601C reads no birth date");
    // 1700's line of business is unbound (lobContainer): card-only.
    assert!(!is_boxed("1700", "line_of_business"));
    assert!(is_boxed("1700", "birth_date"));

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
    // The EOPT tier is a card on 1601C: its note names the branches.
    let note = ask(&branch, "eopt_tier").unwrap()["note"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        note.ends_with("for every later return and for all branches."),
        "{note}"
    );
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
fn an_answer_saved_from_a_box_brings_the_return_up_to_date() {
    setup_juana();
    // 1701Q's birth date is boxed (Item 11, Q1 P1b): typed in its boxes and
    // saved to the profile, a later return of the year opens with it.
    let opened = open_as(JUANA, "1701Q", 1);
    assert!(
        ask(&opened, "birth_date").is_none(),
        "no card: {}",
        opened["asks"]
    );
    assert_eq!(opened["values"]["frm1701q:txtBirthYear"], "");
    let written = fact_to_profile(
        "1701Q",
        1,
        json!({
            "frm1701q:txtBirthMonth": "05",
            "frm1701q:txtBirthDay": "06",
            "frm1701q:txtBirthYear": "1975",
        }),
        &[
            "frm1701q:txtBirthMonth",
            "frm1701q:txtBirthDay",
            "frm1701q:txtBirthYear",
        ],
    )
    .unwrap();
    assert_eq!(birth_date(), "1975-05-06".parse().ok());
    let annual = open_as(JUANA, "1701", 0);
    assert_eq!(annual["draft"]["date_of_birth"], "05/06/1975");
    assert!(ask(&annual, "birth_date").is_none());
    assert_eq!(written["draft"]["date_of_birth"], "05/06/1975");

    // A box the page filled with a guess (0619E's Private) takes the
    // profile's answer on a later open.
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
        // The birth date is boxed (Item 11): one sentence, "Type it", on
        // its boxes — no card, and the form's own sentence is not doubled.
        let block = opened["fact_errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|error| error["field"] == "tax_profile.birth_date")
            .unwrap_or_else(|| panic!("a message on the row: {}", opened["fact_errors"]))
            .clone();
        assert_eq!(
            block["keys"],
            json!([
                "frm1701q:txtBirthMonth",
                "frm1701q:txtBirthDay",
                "frm1701q:txtBirthYear"
            ]),
            "on the date's boxes"
        );
        assert_eq!(
            block["message"],
            "Date of birth is not in the tax profile, and this return needs it. Type it at Item 11."
        );
        // The form's own required sentence for the same boxes is dropped.
        let own = opened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|error| {
                error["keys"]
                    .as_array()
                    .is_some_and(|keys| keys.iter().any(|key| key == "frm1701q:txtBirthMonth"))
            })
            .count();
        assert_eq!(own, 0, "one sentence, not two: {}", opened["errors"]);
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
            refused
                .message()
                .contains("Date of birth is not in the tax profile"),
            "{}",
            refused.message()
        );
        // Typed in the box, the fact error lifts and the profile is untouched.
        let filled = fill(
            "1701Q",
            &saved["draft"],
            json!({
                "frm1701q:txtBirthMonth": "05",
                "frm1701q:txtBirthDay": "06",
                "frm1701q:txtBirthYear": "1975",
            }),
        );
        let validated = call_async(
            "screen_form_validate",
            vec!["1701Q".into(), json(filled["draft"].clone())],
        )
        .unwrap();
        assert!(
            validated["fact_errors"]
                .as_array()
                .unwrap()
                .iter()
                .all(|error| error["field"] != "tax_profile.birth_date"),
            "a typed date answers it: {}",
            validated["fact_errors"]
        );
        assert_eq!(birth_date(), None, "the profile is not written by typing");
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
        assert_eq!(block["keys"], json!(placed.keys), "on its radios");
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

/// The page's "Use profile values" call after a card's Save (`facts`).
fn use_profile(code: &str, period: i64, draft: &Value, facts: &[&str]) -> Value {
    call(
        "screen_form_use_profile",
        vec![
            code.into(),
            2026i64.into(),
            period.into(),
            json(draft.clone()),
            json(json!(facts)),
        ],
    )
    .unwrap_or_else(|error| panic!("{code} uses the profile: {}", error.message()))
}

/// B6: a date the filer typed on the return stays when the card of that
/// fact is saved with another one.
#[test]
fn a_card_save_keeps_what_the_filer_typed_in_the_facts_own_box() {
    setup_juana();
    let opened = open_as(JUANA, "1701Q", 1);
    let filled = fill(
        "1701Q",
        &opened["draft"],
        json!({
            "frm1701q:txtBirthMonth": "05",
            "frm1701q:txtBirthDay": "06",
            "frm1701q:txtBirthYear": "1975",
        }),
    );
    assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
    assert_eq!(filled["draft"]["date_of_birth"], "05/06/1975");
    set_birth_date("1980-02-03");
    let used = use_profile("1701Q", 1, &filled["draft"], &["birth_date"]);
    assert_eq!(
        used["draft"]["date_of_birth"], "05/06/1975",
        "the typed date stays"
    );
    assert!(ask(&used, "birth_date").is_none(), "the card goes");
    // A blank box takes the answer, as before.
    let used = use_profile("1701Q", 1, &opened["draft"], &["birth_date"]);
    assert_eq!(used["draft"]["date_of_birth"], "02/03/1980");
}

/// B6: a card save brings only that fact's fields onto the return. A draft
/// with no baseline keeps its other profile boxes, typed or not.
#[test]
fn a_card_save_touches_only_the_fields_of_its_own_fact() {
    setup_juana();
    let opened = open_as(JUANA, "1601C", 2);
    let line = key_for("/line_of_business");
    let filled = fill_unlocked(
        "1601C",
        &opened["draft"],
        json!({ line.clone(): "Retail" }),
        &[&line],
    );
    assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
    let mut draft = filled["draft"].clone();
    // A return made before baselines existed, whose RDO is not the profile's.
    draft.as_object_mut().unwrap().remove("profile_baseline");
    draft["rdo_code"] = json!("043");
    set_facts(
        JUANA,
        YearFactsPatch {
            eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
            ..YearFactsPatch::default()
        },
    );
    let used = use_profile("1601C", 2, &draft, &["eopt_tier"]);
    assert_eq!(
        used["draft"]["rdo_code"], "043",
        "another profile box is not re-synced"
    );
    assert_eq!(used["draft"]["line_of_business"], "Retail");
    assert!(ask(&used, "eopt_tier").is_none(), "the card goes");
    assert!(
        used["draft"].get("profile_baseline").is_none(),
        "no baseline is made by a card save"
    );
    // "Use profile values" (no fact named) still brings every unedited box.
    let used = use_profile("1601C", 2, &draft, &[]);
    assert_eq!(used["draft"]["rdo_code"], "039");
    assert_eq!(used["draft"]["line_of_business"], "Retail");
}

/// B8: on 0619E and 0619F the filer's own pick of Private or Government
/// answers the category for this return; the page's opening Private does not.
#[test]
fn a_category_pick_on_the_return_lifts_the_withholding_block() {
    setup_juana();
    set_facts(
        JUANA,
        YearFactsPatch {
            is_government_withholding_entity: Some(None),
            eopt_tier: Some(Some(bir_core::profile::EoptTier::Micro)),
            line_of_business: Some(Some("Bookkeeping services".into())),
            ..YearFactsPatch::default()
        },
    );
    at((2026, 8, 1), || {
        for (code, item) in [("0619E", "12"), ("0619F", "11")] {
            let opened = open_as(JUANA, code, 7);
            let check = queue_check(code, &opened["draft"]);
            let block = blocks_on(&check, "is_government_withholding_entity")
                .unwrap_or_else(|| panic!("{code}: no pick blocks: {check}"));
            assert_eq!(
                block["message"],
                format!(
                    "Government withholding agent is not in the tax profile, and this return needs it. Pick it at Item {item}."
                ),
                "{code}"
            );
            for letter in ["P", "G"] {
                let radio = format!("frm{code}:optCategory:{letter}");
                let filled = fill(code, &opened["draft"], json!({ radio.clone(): "true" }));
                assert_eq!(filled["rejected"], json!({}), "{code} {radio}");
                let want = if letter == "G" {
                    "Government"
                } else {
                    "Private"
                };
                assert_eq!(filled["draft"]["withholding_agent_category"], want);
                let check = queue_check(code, &filled["draft"]);
                assert!(
                    blocks_on(&check, "is_government_withholding_entity").is_none(),
                    "{code} {letter}: a pick answers it: {check}"
                );
                // The pick is the return's own: it is still an answer after a save and a reopen.
                if letter == "P" {
                    continue;
                }
                save(code, &filled["draft"]);
                let reopened = open_as(JUANA, code, 7);
                assert_eq!(reopened["draft"]["withholding_agent_category"], want);
                let check = queue_check(code, &reopened["draft"]);
                assert!(
                    blocks_on(&check, "is_government_withholding_entity").is_none(),
                    "{code} {letter}: reopened: {check}"
                );
            }
            // The profile is not written by a pick.
            let facts = with_database(|db| {
                db.effective_year_facts(JUANA, 2026)
                    .map_err(|error| HostError::new(error.to_string()))
            })
            .unwrap();
            assert!(
                !facts
                    .facts
                    .is_set(bir_core::tax_profile::Fact::IsGovernmentWithholdingEntity),
                "{code}: the profile stays unset"
            );
        }
    });
}

#[test]
fn a_guessed_private_category_is_listed_until_the_filer_picks() {
    setup_juana();
    at((2026, 8, 1), || {
        let opened = open_as(JUANA, "0619E", 7);
        assert_eq!(
            opened["guessed"],
            json!(["frm0619E:optCategory:P", "frm0619E:optCategory:G"])
        );

        let filled = fill(
            "0619E",
            &opened["draft"],
            json!({ "frm0619E:optCategory:P": "true" }),
        );
        assert_eq!(filled["rejected"], json!({}));
        assert!(
            blocks_on(
                &queue_check("0619E", &filled["draft"]),
                "is_government_withholding_entity"
            )
            .is_none(),
            "an explicit Private pick answers the category"
        );
        save("0619E", &filled["draft"]);

        let reopened = open_as(JUANA, "0619E", 7);
        assert_eq!(reopened["guessed"], json!([]));
    });
}

fn year_facts() -> bir_core::tax_profile::YearFacts {
    with_database(|db| {
        db.effective_year_facts(JUANA, 2026)
            .map(|effective| effective.facts)
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap()
}

fn birth_date() -> Option<chrono::NaiveDate> {
    with_database(|db| {
        db.get_profile_for_year(JUANA, 2026)
            .map(|profile| profile.and_then(|profile| profile.birth_date))
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap()
}

/// `edits` filled on a new `code` return, saved, then `keys` saved to the
/// tax profile ("Save to profile too").
fn fact_to_profile(code: &str, period: i64, edits: Value, keys: &[&str]) -> Result<Value, String> {
    let opened = open_as(JUANA, code, period);
    let filled = fill(code, &opened["draft"], edits);
    assert_eq!(
        filled["rejected"],
        json!({}),
        "{code}: {}",
        filled["rejected"]
    );
    let saved = save(code, &filled["draft"]);
    call(
        "screen_form_save_to_profile",
        vec![
            code.into(),
            2026i64.into(),
            period.into(),
            json(saved["draft"].clone()),
            json(json!(keys)),
        ],
    )
    .map_err(|error| error.message().to_string())
}

/// B9: every fact a form box fills goes to the tax profile from the return,
/// through the function that owns it.
#[test]
fn each_fact_box_saves_to_the_tax_profile_through_its_own_route() {
    setup_juana();
    set_facts(
        JUANA,
        YearFactsPatch {
            is_government_withholding_entity: Some(None),
            ..YearFactsPatch::default()
        },
    );
    use bir_core::profile::{EoptTier, TaxClassification};

    // Birth date and the usual ATC, from 1701Q's Items 11 and 8.
    let written = fact_to_profile(
        "1701Q",
        1,
        json!({
            "frm1701q:txtBirthMonth": "05",
            "frm1701q:txtBirthDay": "06",
            "frm1701q:txtBirthYear": "1975",
            "frm1701q:optATC_2": "true",
        }),
        &["frm1701q:txtBirthMonth", "frm1701q:optATC_2"],
    )
    .unwrap();
    // In the boxes' key order.
    assert_eq!(written["saved"], json!(["Usual ATC", "Date of birth"]));
    assert_eq!(birth_date(), "1975-05-06".parse().ok());
    assert_eq!(year_facts().atc_codes, vec!["II014".to_string()]);
    assert_eq!(written["draft"]["date_of_birth"], "05/06/1975");
    // Another return of the year opens with them.
    let annual = open_as(JUANA, "1701", 0);
    assert_eq!(annual["draft"]["date_of_birth"], "05/06/1975");
    assert_eq!(annual["values"]["frm1701:rdoPg1I7ATC_II014"], "true");
    assert!(ask(&annual, "birth_date").is_none());
    assert!(ask(&annual, "atc_codes").is_none());

    // The income type, from 1701's Item 6.
    assert_eq!(year_facts().tax_classification, None);
    let written = fact_to_profile(
        "1701",
        0,
        json!({ "frm1701:rdoPg1I6TaxpayerTypeS": "true" }),
        &["frm1701:rdoPg1I6TaxpayerTypeS"],
    )
    .unwrap();
    assert_eq!(written["saved"], json!(["Income type"]));
    assert_eq!(
        year_facts().tax_classification,
        Some(TaxClassification::SelfEmployed)
    );
    assert_eq!(written["draft"]["taxpayer_type"], "SingleProprietor");

    // The withholding category, from 0619E's Item 12.
    let written = fact_to_profile(
        "0619E",
        7,
        json!({ "frm0619E:optCategory:G": "true" }),
        &["frm0619E:optCategory:G"],
    )
    .unwrap();
    assert_eq!(written["saved"], json!(["Withholding category"]));
    assert_eq!(year_facts().is_government_withholding_entity, Some(true));
    assert_eq!(
        open_as(JUANA, "0619F", 7)["draft"]["withholding_agent_category"],
        "Government"
    );

    // The EOPT tier, from 2550Q's Item 13.
    let written = fact_to_profile(
        "2550Q",
        1,
        json!({ "frm2550qv2024:taxPayerClassification2": "true" }),
        &["frm2550qv2024:taxPayerClassification2"],
    )
    .unwrap();
    assert_eq!(written["saved"], json!(["EOPT tier"]));
    assert_eq!(year_facts().eopt_tier, Some(EoptTier::Small));

    // The start date, from 1702EX's Item 10.
    let written = fact_to_profile(
        "1702EX",
        0,
        json!({ "frm1702EX:txtPg1Pt1I10DateofIncorporation": "06/01/2015" }),
        &["frm1702EX:txtPg1Pt1I10DateofIncorporation"],
    )
    .unwrap();
    assert_eq!(written["saved"], json!(["Business start date"]));
    assert_eq!(year_facts().business_start_date, "2015-06-01".parse().ok());
}

#[test]
fn the_usual_atc_saves_through_save_to_profile_and_a_spouse_only_code_is_refused() {
    setup_juana();
    let none = || {
        set_facts(
            JUANA,
            YearFactsPatch {
                atc_codes: Some(vec![]),
                ..YearFactsPatch::default()
            },
        )
    };
    none();
    let key = "frm1701q:optATC_1";
    let written = fact_to_profile("1701Q", 1, json!({ key: "true" }), &[key]).unwrap();
    assert_eq!(written["saved"], json!(["Usual ATC"]));
    assert_eq!(year_facts().atc_codes, vec!["II012".to_string()]);
    assert_eq!(
        open_as(JUANA, "1701", 0)["values"]["frm1701:rdoPg1I7ATC_II012"],
        "true"
    );

    none();
    let mut draft = written["draft"].clone();
    draft["atc"] = json!("Ii011");
    let refused = call(
        "screen_form_save_to_profile",
        vec![
            "1701Q".into(),
            2026i64.into(),
            1i64.into(),
            json(draft),
            json(json!([key])),
        ],
    );
    assert_eq!(
        refused.unwrap_err().message(),
        "That answer cannot be saved to the tax profile.",
        "a spouse-only code outside Item 8's choices is refused"
    );
    assert!(year_facts().atc_codes.is_empty(), "nothing is written");
}

/// B9: an answer the profile has no meaning for is refused in plain words
/// and nothing is written.
#[test]
fn a_fact_box_with_no_profile_meaning_is_refused() {
    setup_juana();
    const REFUSED: &str = "That answer cannot be saved to the tax profile.";
    // 1701 Item 6 "Estate" is no income type of an individual.
    let refused = fact_to_profile(
        "1701",
        0,
        json!({ "frm1701:rdoPg1I6TaxpayerTypeE": "true" }),
        &["frm1701:rdoPg1I6TaxpayerTypeE"],
    )
    .unwrap_err();
    assert_eq!(refused, REFUSED);
    assert_eq!(year_facts().tax_classification, None);
    // A blank birth date, with a valid ATC beside it: neither is written.
    let refused = fact_to_profile(
        "1701Q",
        1,
        json!({ "frm1701q:optATC_1": "true" }),
        &["frm1701q:optATC_1", "frm1701q:txtBirthMonth"],
    )
    .unwrap_err();
    assert_eq!(refused, REFUSED);
    assert!(year_facts().atc_codes.is_empty(), "nothing is written");
    assert_eq!(birth_date(), None);
}

/// B9: the page is told which fact boxes have a route, and where it goes.
#[test]
fn the_page_knows_which_fact_boxes_save_to_the_profile() {
    setup_juana();
    let q = open_as(JUANA, "1701Q", 1);
    let save = &q["profile_save"];
    assert_eq!(save["frm1701q:txtBirthMonth"]["field"], "birth_date");
    assert_eq!(
        save["frm1701q:txtBirthMonth"]["note"],
        "Saved for this taxpayer."
    );
    assert_eq!(save["frm1701q:optATC_3"]["field"], "atc_codes");
    assert_eq!(
        save["frm1701q:optATC_3"]["note"],
        "Saved to this branch's tax facts for 2026."
    );
    let e = open_as(JUANA, "0619E", 7);
    assert_eq!(
        e["profile_save"]["frm0619E:optCategory:P"]["field"],
        "is_government_withholding_entity"
    );
}

/// Q1 P1b: 1701's income type is boxed — Item 6's radios are the only
/// input, each a "Save to profile" box. Picking one and saving it writes
/// the income type the profile takes.
#[test]
fn the_1701_income_type_is_boxed_and_saves_through_item_6() {
    use bir_core::profile::TaxClassification;
    setup_juana();
    let opened = open_as(JUANA, "1701", 0);
    assert!(
        ask(&opened, "tax_classification").is_none(),
        "boxed: no card: {}",
        opened["asks"]
    );
    // Item 6's radios carry profile_save entries under the fact's name.
    let save = &opened["profile_save"];
    let radio = "frm1701:rdoPg1I6TaxpayerTypeS";
    assert_eq!(save[radio]["field"], "tax_classification", "{save}");
    // Pick Single proprietor on the return and save it to the profile.
    let written = fact_to_profile("1701", 0, json!({ radio: "true" }), &[radio]).unwrap();
    assert_eq!(written["saved"], json!(["Income type"]));
    assert_eq!(
        year_facts().tax_classification,
        Some(TaxClassification::SelfEmployed)
    );
    assert_eq!(written["draft"]["taxpayer_type"], "SingleProprietor");
    // A later return of the year opens with Item 6 ticked.
    let reopened = open_as(JUANA, "1701", 0);
    assert_eq!(reopened["draft"]["taxpayer_type"], "SingleProprietor");
    assert!(ask(&reopened, "tax_classification").is_none());

    // 2551Q's Item 13 keeps its income-type card (F2): three answers, each
    // the value the profile takes, with a `short` chip label.
    setup_juana();
    let q = open_as(JUANA, "2551Q", 1);
    let card = ask(&q, "tax_classification").unwrap();
    assert_eq!(
        card["choices"],
        json!([
            { "value": "SelfEmployed", "label": "Self-employed or professional", "short": "Self-employed" },
            { "value": "MixedIncome", "label": "Mixed income (compensation and business)", "short": "Mixed income" },
            { "value": "PurelyCompensation", "label": "Compensation only", "short": "Compensation only" },
        ])
    );
}

/// Q1 P1b: the boxed/card table — which facts answer in their own writable
/// box and which keep a card, per form.
#[test]
fn the_boxed_card_table() {
    let table: &[(&str, &str, bool)] = &[
        ("0605", "line_of_business", true),
        ("0605", "eopt_tier", false),
        ("2200S", "line_of_business", true),
        ("2200S", "eopt_tier", false),
        ("1701Q", "birth_date", true),
        ("1701Q", "atc_codes", true),
        ("1701Q", "line_of_business", true),
        ("1701Q", "eopt_tier", false),
        ("1701Q", "income_tax_election", false),
        ("2551Q", "tax_classification", false),
        ("2551Q", "business_start_date", false),
        ("2551Q", "eopt_tier", false),
        ("2551Q", "income_tax_election", false),
        ("0619E", "is_government_withholding_entity", true),
        ("0619E", "line_of_business", true),
        ("0619E", "eopt_tier", false),
        ("0619F", "is_government_withholding_entity", true),
        ("0619F", "line_of_business", true),
        ("0619F", "eopt_tier", false),
        ("1700", "birth_date", true),
        ("1700", "line_of_business", false),
        ("1700", "eopt_tier", false),
        ("1701", "birth_date", true),
        ("1701", "tax_classification", true),
        ("1701", "atc_codes", true),
        ("1701", "line_of_business", true),
        ("1701", "eopt_tier", false),
        ("1701A", "birth_date", true),
        ("1702RT", "business_start_date", false),
        ("1702EX", "business_start_date", true),
        ("1702EX", "line_of_business", true),
        ("1601C", "line_of_business", true),
        ("1601C", "eopt_tier", false),
        ("2550Q", "eopt_tier", true),
    ];
    for (code, fact, want) in table {
        assert_eq!(is_boxed(code, fact), *want, "{code} {fact}");
    }
}

/// Q1 P1b, B5/B7: a required boxed fact left blank shows ONE sentence —
/// the fact's "Type it/Pick it at Item N" — and the form's own required
/// sentence for the same box is dropped.
#[test]
fn a_blank_required_boxed_fact_says_it_once() {
    setup_juana();
    for (code, period, fact, at) in [
        ("0605", 1i64, "line_of_business", "Item 12"),
        ("2200S", 1i64, "line_of_business", "Item 9"),
    ] {
        let opened = open_as(JUANA, code, period);
        let block = opened["fact_errors"]
            .as_array()
            .unwrap_or_else(|| panic!("{code} fact_errors: {}", opened["fact_errors"]))
            .iter()
            .find(|error| error["field"] == format!("tax_profile.{fact}"))
            .unwrap_or_else(|| panic!("{code} blocks {fact}: {}", opened["fact_errors"]))
            .clone();
        let message = block["message"].as_str().unwrap();
        assert!(
            message.contains("is not in the tax profile, and this return needs it.")
                && (message.contains("Type it") || message.contains("Pick it")),
            "{code}: {message}"
        );
        assert!(message.contains(at), "{code}: {message} names {at}");
        // The form's own sentence for the same keys is gone.
        let keys: Vec<String> = block["keys"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|key| key.as_str().map(str::to_string))
            .collect();
        let doubled = opened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|error| {
                error["keys"].as_array().is_some_and(|own| {
                    !own.is_empty()
                        && own
                            .iter()
                            .all(|key| keys.contains(&key.as_str().unwrap().to_string()))
                })
            })
            .count();
        assert_eq!(
            doubled, 0,
            "{code}: one sentence, not two: {}",
            opened["errors"]
        );
    }
}

/// Q1·a: the "Save to profile?" answer is remembered per taxpayer TIN, tax
/// year and field in a host setting; `profile_asked` lists it on open.
#[test]
fn the_profile_ask_answer_is_remembered_per_tin_year_and_field() {
    setup_juana();
    let opened = open_as(JUANA, "0605", 1);
    assert_eq!(opened["profile_asked"], json!([]), "nothing remembered");
    call(
        "screen_form_profile_asked",
        vec![
            JUANA.into(),
            2026i64.into(),
            "line_of_business".into(),
            "only".into(),
        ],
    )
    .unwrap();
    let reopened = open_as(JUANA, "0605", 1);
    assert_eq!(reopened["profile_asked"], json!(["line_of_business"]));
    // A second answer for another field joins it.
    call(
        "screen_form_profile_asked",
        vec![
            JUANA.into(),
            2026i64.into(),
            "rdo_code".into(),
            "save".into(),
        ],
    )
    .unwrap();
    let reopened = open_as(JUANA, "0605", 1);
    let mut asked: Vec<String> = reopened["profile_asked"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap().to_string())
        .collect();
    asked.sort();
    assert_eq!(
        asked,
        vec!["line_of_business", "rdo_code"],
        "both remembered"
    );
    // Another tax year asks again.
    let other_year = call_async(
        "screen_form_open",
        vec![JUANA.into(), "0605".into(), 2025i64.into(), 1i64.into()],
    )
    .unwrap();
    assert_eq!(other_year["profile_asked"], json!([]), "2025 asks again");
    // Another TIN asks again (the branch's own TIN).
    let branch = call_async(
        "screen_form_open",
        vec![BRANCH.into(), "0605".into(), 2026i64.into(), 1i64.into()],
    );
    // A branch with no profile of its own opens nothing to compare.
    if let Ok(opened) = branch {
        assert_eq!(opened["profile_asked"], json!([]), "another TIN");
    }
    // A bad answer is refused.
    assert!(
        call(
            "screen_form_profile_asked",
            vec![
                JUANA.into(),
                2026i64.into(),
                "line_of_business".into(),
                "maybe".into()
            ],
        )
        .is_err()
    );
}

/// Q1b/F2: a card fact's Queue sentence reads "Answer it {at}, then queue."
/// — no "in the card".
#[test]
fn a_card_facts_queue_sentence_names_the_item() {
    setup_juana();
    at((2026, 3, 1), || {
        // 1601C's EOPT tier is a card; late, it blocks with the new wording.
        let saved = save("1601C", &open_as(JUANA, "1601C", 1)["draft"]);
        let check = queue_check("1601C", &saved["draft"]);
        let block = blocks_on(&check, "eopt_tier").unwrap_or_else(|| panic!("{check}"));
        let message = block["message"].as_str().unwrap();
        assert!(
            message.contains("Answer it at Item") && message.ends_with("then queue."),
            "{message}"
        );
        assert!(!message.contains("in the card"), "{message}");
    });
}

/// Review F4: the remembered answer's TIN must be a TIN (12-14 digits, no
/// dashes or letters), so a bad one never forms a settings key.
#[test]
fn a_profile_ask_for_a_malformed_tin_is_refused() {
    setup_juana();
    for tin in [
        "12AB",
        "",
        "000-123-453-00000",
        "0001234530000:",
        "123456789",
    ] {
        let answered = call(
            "screen_form_profile_asked",
            vec![tin.into(), 2026i64.into(), "rdo_code".into(), "save".into()],
        );
        assert!(answered.is_err(), "{tin:?} is refused");
    }
    let opened = open_as(JUANA, "0605", 1);
    assert_eq!(opened["profile_asked"], json!([]), "nothing was remembered");
}

/// Review F2: a failed read of the remembered answers is an error, not
/// "nothing asked" (which would repeat every ask on every open); opening
/// still works and logs it.
#[test]
fn a_failed_profile_ask_read_is_an_error() {
    setup_juana();
    let draft = open_as(JUANA, "0605", 1)["draft"].clone();
    let Ok(kind) = kind_of("0605") else {
        panic!("0605 kind")
    };
    with_database(|db| {
        db.conn_for_tests()
            .execute_batch("ALTER TABLE settings RENAME TO settings_gone")
            .expect("rename settings");
        assert!(crate::screen_form_module::fact_asks::profile_asked(db, kind, &draft).is_err());
        db.conn_for_tests()
            .execute_batch("ALTER TABLE settings_gone RENAME TO settings")
            .expect("restore settings");
        Ok(())
    })
    .unwrap();
}

#[test]
fn a_failed_profile_ask_read_opens_with_profile_asks_unknown() {
    setup_juana();
    with_database(|db| {
        db.conn_for_tests()
            .execute_batch("ALTER TABLE settings RENAME TO settings_gone")
            .expect("rename settings");
        Ok(())
    })
    .unwrap();

    let result = call_async(
        "screen_form_open",
        vec![JUANA.into(), "1701".into(), 2026i64.into(), 0i64.into()],
    );
    with_database(|db| {
        db.conn_for_tests()
            .execute_batch("ALTER TABLE settings_gone RENAME TO settings")
            .expect("restore settings");
        Ok(())
    })
    .unwrap();

    let opened = result.expect("the form still opens when remembered answers cannot be read");
    assert_eq!(opened["profile_ask_unknown"], true);
}
