//! PR 14 (decision P4-18): the shared untick rule (`untick_blanks`) on the
//! sweep of `radio_untick.rs`, its golden, and the review's findings on the
//! page's group size (F9) and 1600PT/1600VT's popup slots (F10).

use super::radio_untick::{NEVER_TICKED, groups_of_three_or_more, seed, sweep, try_fill};
use super::*;

const GOLDEN: &str = "src/screen_form_module/tests/fixtures/radio-untick-golden.json";

/// Where an untick keeps the answer (`UntickWrite::KeptText` or
/// `KeptNoBlank`), with the reason. Every other untick writes `null` or an
/// unanswered variant.
const KEPT: &[(&str, &str, &str)] = &[
    (
        "2552",
        "/transaction",
        "item 11: PR 9 keeps the answer; the group box refuses a blank",
    ),
    (
        "2550Q",
        "/quarter",
        "the quarter picks the return: the untick is refused as an identity box",
    ),
];

/// Members whose untick is not the shared rule's: the host's own writes
/// (1801 item 15D's frequency, PR 10; 2200S item 17's own tick box, PR 11;
/// 1701's filer types, which keep their compensation flag).
fn own_untick(kind: AgentFormKind, key: &str) -> bool {
    rows_donor_estate::is_frequency_tick(kind, key)
        || rows_excise::untick_writes_its_option(kind, key)
        || form_1701_type_paths(kind, key).is_some()
}

fn golden() -> Map<String, Value> {
    let path = format!("{}/{GOLDEN}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).expect("golden")).expect("golden json")
}

/// The sweep's groups are the host's: `one_answer_groups`' groups of
/// three or more, member for member.
#[test]
fn pr14_the_sweep_covers_the_hosts_groups() {
    for kind in AgentFormKind::all() {
        let mut host: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (key, (group, members)) in one_answer_groups(kind) {
            if members >= 3 {
                host.entry(group).or_default().insert(key);
            }
        }
        let swept: BTreeMap<String, BTreeSet<String>> = groups_of_three_or_more(kind)
            .into_iter()
            .map(|(group, members)| (group, members.into_iter().collect()))
            .collect();
        assert_eq!(swept, host, "{}", kind.form_code());
    }
}

/// Decision P4-18 on every 3+ group's every member, ticked on its seeded
/// draft, then unticked with "false" and its group box sent "": no fill
/// fails (a refusal is a recorded answer, not a failure); each member reads
/// back ticked unless listed in `NEVER_TICKED`; the untick writes exactly
/// what `untick_blanks` says, keeping the answer only where `KEPT` says;
/// the draft parses and saves.
#[test]
fn pr14_every_untick_of_a_three_or_more_group_fills_parses_and_saves() {
    setup();
    let cases = sweep();
    assert!(
        cases.len() > 400,
        "the sweep covers every group: {}",
        cases.len()
    );
    let mut never: BTreeSet<(String, String)> = BTreeSet::new();
    let mut kept_seen: BTreeSet<(String, String)> = BTreeSet::new();
    for (name, (_, case)) in &cases {
        let mut words = name.splitn(3, ' ');
        let (code, member, edit) = (
            words.next().unwrap(),
            words.next().unwrap(),
            words.next().unwrap(),
        );
        let kind = kind_of(code).unwrap();
        let filled = case
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        if edit == "tick" {
            if filled["values"][member] != "true" {
                never.insert((code.to_string(), member.to_string()));
            }
            continue;
        }
        if edit == "false" && !own_untick(kind, member) {
            let refused = filled["rejected"].get(member).is_some();
            for (path, write) in untick_blanks(kind, &case.ticked, &case.member_paths) {
                let after = filled["draft"]
                    .pointer(&path)
                    .cloned()
                    .unwrap_or(Value::Null);
                let kept = case.ticked.pointer(&path).cloned().unwrap_or(Value::Null);
                match write {
                    UntickWrite::Blank(value) if !refused => {
                        assert!(
                            value.is_null()
                                || value
                                    .as_str()
                                    .is_some_and(|text| UNTICK_UNANSWERED.contains(&text)),
                            "{name}: {path} blank {value}"
                        );
                        assert_eq!(after, value, "{name}: {path}");
                    }
                    UntickWrite::Blank(_) => assert_eq!(after, kept, "{name}: {path} refused"),
                    UntickWrite::KeptText | UntickWrite::KeptNoBlank => {
                        assert!(
                            KEPT.iter().any(|(c, p, _)| *c == code && *p == path),
                            "{name}: {path} keeps its answer ({write:?}) but is not in KEPT"
                        );
                        assert_eq!(after, kept, "{name}: {path} kept");
                        kept_seen.insert((code.to_string(), path.clone()));
                    }
                }
            }
        }
        agent::identity(kind, &filled["draft"])
            .unwrap_or_else(|error| panic!("{name}: the draft parses: {error}"));
        // A fresh store: one return per form and period saves as new.
        setup();
        let saved = call(
            "screen_form_save",
            vec![code.into(), json(filled["draft"].clone())],
        )
        .unwrap_or_else(|error| panic!("{name}: saves: {error}"));
        assert!(saved["id"].is_number(), "{name}: {saved}");
    }
    let listed: BTreeSet<(String, String)> = NEVER_TICKED
        .iter()
        .map(|(code, member, _)| (code.to_string(), member.to_string()))
        .collect();
    assert_eq!(
        never, listed,
        "members that never read back ticked must be exactly NEVER_TICKED"
    );
    let kept: BTreeSet<(String, String)> = KEPT
        .iter()
        .map(|(code, path, _)| (code.to_string(), path.to_string()))
        .collect();
    assert_eq!(
        kept_seen, kept,
        "every KEPT case is reached, and only those"
    );
}

/// The golden: each case's outcome on the base of PR 14 (`before`) and
/// now (`after`), refusals and their messages included. Only cases that
/// failed before changed; the cases the base had and this PR no longer
/// sweeps are 1600PT's and 1600VT's popup slots, which F10 made the ATC
/// tick boxes they are (not a one-answer group); today's sweep matches
/// `after` exactly.
#[test]
fn pr14_only_the_unticks_that_failed_change() {
    setup();
    let golden = golden();
    let mut changed = Vec::new();
    for (name, entry) in &golden {
        if entry["after"].is_null() {
            assert!(
                name.starts_with("1600PT AtcCode") || name.starts_with("1600VT AtcCode"),
                "{name}: no longer swept"
            );
            continue;
        }
        assert!(!entry["before"].is_null(), "{name}: not swept on the base");
        assert!(entry["after"].get("error").is_none(), "{name}: still fails");
        if entry["before"] != entry["after"] {
            assert!(
                entry["before"]["error"].is_string(),
                "{name}: worked before and changed: {} -> {}",
                entry["before"],
                entry["after"]
            );
            changed.push(name.as_str());
        }
    }
    let mut forms: Vec<&str> = changed
        .iter()
        .map(|name| name.split(' ').next().unwrap())
        .collect();
    forms.dedup();
    assert_eq!(forms, CHANGED_FORMS, "{changed:#?}");
    assert_eq!(changed.len(), CHANGED_CASES, "{changed:#?}");
    // A group box sent "" is refused with its reason, recorded: the
    // group's own options, or 2550Q's identity lock. 1801 item 15D's
    // frequency is the one blank that applies: PR 10's host write clears it.
    for (name, entry) in golden
        .iter()
        .filter(|(name, entry)| name.ends_with(" blank (group)") && !entry["after"].is_null())
    {
        let after = &entry["after"];
        let refusals = after["rejected"].as_object().unwrap();
        if name.starts_with("1801 frm1801v2018:freq_") {
            assert!(refusals.is_empty(), "{name}");
            assert_eq!(
                after["fields"]["/installment_frequency"],
                Value::Null,
                "{name}"
            );
            continue;
        }
        assert_eq!(refusals.len(), 1, "{name}");
        let (key, why) = refusals.iter().next().unwrap();
        let why = why.as_str().unwrap();
        if name.starts_with("2550Q frm2550qv2024:OptQuarter") {
            assert_eq!(
                why,
                format!(
                    "`{key}` picks which return this is; reopen the form for another TIN or period"
                ),
                "{name}"
            );
        } else {
            assert!(
                why.starts_with(&format!("`{key}` must be one of ")) && why.ends_with("(got ``)"),
                "{name}: {why}"
            );
        }
    }
    // 2550Q's quarter picks the return: unticking it is refused, recorded.
    assert_eq!(
        golden["2550Q frm2550qv2024:OptQuarter1 false"]["after"]["rejected"],
        json!({
            "frm2550qv2024:OptQuarter1":
                "`frm2550qv2024:OptQuarter1` picks which return this is; reopen the form for another TIN or period"
        })
    );
    let now: Map<String, Value> = sweep()
        .into_iter()
        .map(|(name, (record, _))| (name, record))
        .collect();
    let swept: Vec<&String> = golden
        .iter()
        .filter(|(_, entry)| !entry["after"].is_null())
        .map(|(name, _)| name)
        .collect();
    assert_eq!(now.keys().collect::<Vec<_>>(), swept, "the same cases");
    for (name, record) in &now {
        assert_eq!(record, &golden[name]["after"], "{name}");
    }
}

/// The forms and number of cases whose untick failed the whole fill before
/// decision P4-18: PR 11's seven forms, plus the dispositions, civil
/// statuses, ATCs and spouse answers the seeded sweep now reaches.
const CHANGED_FORMS: &[&str] = &[
    "1601EQ", "1602Q", "1700", "1701", "1701A", "1701MS", "1702EX", "2000", "2000OT",
];
const CHANGED_CASES: usize = 61;

/// A batch that unticks one of these answers still applies its other
/// boxes: the formerly failing groups (now unanswered) and 2552 item 11
/// (kept, PR 9), each beside the first text box that takes "1" after the
/// untick alone, which fills in the batch as it does then.
#[test]
fn pr14_an_untick_never_fails_the_rest_of_the_batch() {
    let cases = [
        (
            "1700",
            "frm1700:rdoPg1I15CivilStatusM",
            "/civil_status",
            "unanswered",
        ),
        (
            "1701A",
            "frm1701A:optCivilStatus_2",
            "/civil_status",
            "unanswered",
        ),
        ("1701A", "frm1701A:optATC_1", "/atc", "unanswered"),
        (
            "1701MS",
            "frm1701MS:txtTaxpayerNo12c",
            "/taxpayer/tax_option",
            "unanswered",
        ),
        ("2000", "frm2000:optMode_1", "/affixture_mode", "unanswered"),
        ("2000", "frm2000:optParty_2", "/other_party", "unanswered"),
        ("2000OT", "frm2000OT:ATC_1", "/atc", "Unanswered"),
        (
            "2000OT",
            "frm2000OT:rbTransNature_3",
            "/nature",
            "unanswered",
        ),
        (
            "2552",
            "frm2552:rdoPg1I11TransactionLSE",
            "/transaction",
            "local_stock_exchange",
        ),
    ];
    for (code, key, path, ends) in cases {
        // A fresh store per case: each saves as a new return.
        setup();
        let kind = kind_of(code).unwrap();
        let (period, prerequisites) = seed(code, key);
        assert!(period.is_none(), "{code}");
        let group = kind
            .definition()
            .box_def(key)
            .and_then(|entry| entry.group.clone());
        let mut unlocked: Vec<String> = [Some(key.to_string()), group]
            .into_iter()
            .flatten()
            .collect();
        for edits in &prerequisites {
            unlocked.extend(edits.as_object().unwrap().keys().cloned());
        }
        let mut draft = open(code, page_period(code))["draft"].clone();
        for edits in &prerequisites {
            draft = try_fill(code, &draft, edits.clone(), &unlocked).unwrap()["draft"].clone();
        }
        let ticked = try_fill(code, &draft, json!({ key: "true" }), &unlocked).unwrap();
        assert_eq!(ticked["values"][key], "true", "{code}: {key}");
        let unticked =
            try_fill(code, &ticked["draft"], json!({ key: "false" }), &unlocked).unwrap();
        let (other, alone) = kind
            .definition()
            .fillable()
            .filter(|entry| entry.kind.contains("text") && entry.group.is_none())
            .find_map(|entry| {
                let alone =
                    try_fill(code, &unticked["draft"], json!({ &entry.key: "1" }), &[]).ok()?;
                let took = alone["rejected"].as_object().is_some_and(Map::is_empty)
                    && alone["values"][&entry.key] != unticked["values"][&entry.key];
                took.then(|| (entry.key.clone(), alone["values"][&entry.key].clone()))
            })
            .unwrap_or_else(|| panic!("{code}: a text box takes \"1\""));
        let batch = try_fill(
            code,
            &ticked["draft"],
            json!({ key: "false", &other: "1" }),
            &unlocked,
        )
        .unwrap_or_else(|error| panic!("{code}: {key} with {other}: {error}"));
        assert_eq!(batch["rejected"], json!({}), "{code}: {key} with {other}");
        assert_eq!(
            batch["values"][&other], alone,
            "{code}: {other} still applies"
        );
        assert_eq!(
            batch["draft"].pointer(path),
            Some(&json!(ends)),
            "{code}: {key}"
        );
        let saved = save(code, &batch["draft"]);
        assert!(saved["id"].is_number(), "{code}: {saved}");
    }
}

fn page_period(code: &str) -> i64 {
    match bir_core::forms::find_form(code).map(|form| &form.frequency) {
        Some(FilingFrequency::Annual) => 0,
        _ => 1,
    }
}

/// Review F3: on a 2552 return that is no longer a draft, unticking item
/// 11's answer is refused like every other edit there ("this return is no
/// longer a draft"); the base skipped it without a word. The answer stays.
#[test]
fn pr14_a_filed_2552_refuses_the_untick_like_any_edit() {
    setup();
    let key = "frm2552:rdoPg1I11TransactionLSE";
    let opened = open("2552", 1);
    let mut ticked = fill("2552", &opened["draft"], json!({ key: "true" }))["draft"].clone();
    assert_eq!(ticked["transaction"], "local_stock_exchange");
    ticked["status"] = json!("Submitted");
    let untick = fill("2552", &ticked, json!({ key: "false" }));
    assert_eq!(
        untick["rejected"],
        json!({ key: "this return is no longer a draft" })
    );
    assert_eq!(untick["draft"]["transaction"], "local_stock_exchange");
}

/// Review F9: every box of a one-answer group carries its group's size
/// (`rg`) in the served schema, so the page unticks the ticked box of a
/// group of three or more however the group is drawn. 2200S's manner of
/// payment is items 15 and 16 (radios) and item 17 (a tick box): all three
/// carry 3.
#[test]
fn pr14_the_schema_gives_each_radio_its_groups_size() {
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let groups = one_answer_groups(kind);
        let form = schema_entry(code).unwrap();
        for field in form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|part| part["items"].as_array().unwrap())
            .flat_map(|item| item["f"].as_array().cloned().unwrap_or_default())
        {
            let key = field["k"].as_str().unwrap();
            assert_eq!(
                field["rg"].as_u64(),
                groups.get(key).map(|(_, members)| *members as u64),
                "{code} {key}"
            );
        }
    }
    let form = schema_entry("2200S").unwrap();
    let rg = |key: &str| {
        form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|part| part["items"].as_array().unwrap())
            .flat_map(|item| item["f"].as_array().cloned().unwrap_or_default())
            .find(|field| field["k"] == key)
            .map(|field| (field["t"].clone(), field["rg"].clone()))
            .unwrap()
    };
    assert_eq!(rg("frm2200S:chkPymntManner_1"), (json!("radio"), json!(3)));
    assert_eq!(rg("frm2200S:chkPymntManner_2"), (json!("radio"), json!(3)));
    assert_eq!(rg("frm2200S:chkPymntManner_3"), (json!("chk"), json!(3)));
}

/// Review F9: the groups of three or more the page drew short, counting
/// only the radios of one item: members split over items, or drawn as tick
/// boxes. Each now unticks by `rg`.
#[test]
fn pr14_groups_the_page_counted_short_are_these() {
    let mut short = Vec::new();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let groups = one_answer_groups(kind);
        let form = schema_entry(code).unwrap();
        for item in form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|part| part["items"].as_array().unwrap())
        {
            let fields = item["f"].as_array().cloned().unwrap_or_default();
            let radios = fields.iter().filter(|f| f["t"] == "radio").count();
            for field in &fields {
                let key = field["k"].as_str().unwrap();
                if let Some((group, members)) = groups.get(key)
                    && *members >= 3
                    && (field["t"] != "radio" || radios < 3)
                {
                    short.push(format!(
                        "{code} {group} {key} ({})",
                        field["t"].as_str().unwrap_or("")
                    ));
                }
            }
        }
    }
    assert_eq!(short, SHORT_GROUPS, "{short:#?}");
}

/// `<form> <group> <member> (<page type>)`. A tick box (`chk`) already
/// sent "false" when unticked; the radios sent their answer again (a
/// pair's re-pick) and now untick.
const SHORT_GROUPS: &[&str] = &[
    "1601EQ frm1601EQ:if frm1601EQ:ifCarriedOver (chk)",
    "1601EQ frm1601EQ:if frm1601EQ:ifRefund (chk)",
    "1601EQ frm1601EQ:if frm1601EQ:ifIssueCert (chk)",
    "2200S frm2200S:chkPymntManner frm2200S:chkPymntManner_1 (radio)",
    "2200S frm2200S:chkPymntManner frm2200S:chkPymntManner_2 (radio)",
    "2200S frm2200S:chkPymntManner frm2200S:chkPymntManner_3 (chk)",
    "2200AN path:/manner frm2200ANv2018:MannerOfPayment1 (radio)",
    "2200AN path:/manner frm2200ANv2018:MannerOfPayment2 (radio)",
    "2200AN path:/manner frm2200ANv2018:MannerOfPayment3 (radio)",
    "2200M frm2200M:rdoPg1Pt2MOP frm2200M:rdoPg1Pt2MOPPaymentActual (radio)",
    "2200M frm2200M:rdoPg1Pt2MOP frm2200M:rdoPg1Pt2MOPPrepayment (radio)",
    "2200M frm2200M:rdoPg1Pt2MOP frm2200M:rdoPg1Pt2MOPOther (radio)",
    "2200T path:/manner_of_payment frm2200Tv2020:optPayment_1 (radio)",
    "2200T path:/manner_of_payment frm2200Tv2020:optPayment_2 (radio)",
    "2200T path:/manner_of_payment frm2200Tv2020:optPayment_3 (radio)",
    "2200A path:/manner_of_payment frm2200Av2020:optPayment_1 (radio)",
    "2200A path:/manner_of_payment frm2200Av2020:optPayment_2 (radio)",
    "2200A path:/manner_of_payment frm2200Av2020:optPayment_3 (radio)",
    "2200P frm2200P:rdoPg1Pt2MOP frm2200P:rdoPg1Pt2MOPPaymentActual (radio)",
    "2200P frm2200P:rdoPg1Pt2MOP frm2200P:rdoPg1Pt2MOPPrepayment (radio)",
    "2200P frm2200P:rdoPg1Pt2MOP frm2200P:rdoPg1Pt2MOPOther (radio)",
    "1701A path:/atc frm1701A:optTaxRate_1 (radio)",
    "1701A path:/atc frm1701A:optTaxRate_2 (radio)",
    "1701A path:/spouse/atc frm1701A:optSpouseTaxRate_1 (radio)",
    "1701A path:/spouse/atc frm1701A:optSpouseTaxRate_2 (radio)",
    "1701 frm1701:rdoPg1I6TaxpayerType frm1701:rdoPg1I6TaxpayerTypeS (chk)",
    "1701 frm1701:rdoPg1I6TaxpayerType frm1701:rdoPg1I6TaxpayerTypeP (chk)",
    "1701 frm1701:rdoPg1I6TaxpayerType frm1701:rdoPg1I6TaxpayerTypeE (chk)",
    "1701 frm1701:rdoPg1I6TaxpayerType frm1701:rdoPg1I6TaxpayerTypeT (chk)",
    "1701 path:/atc frm1701:rdoPg1I21TaxRateG (radio)",
    "1701 path:/atc frm1701:rdoPg1I21TaxRateP (radio)",
    "1701 path:/spouse/atc frm1701:rdoPg2I12TaxRateG (radio)",
    "1701 path:/spouse/atc frm1701:rdoPg2I12TaxRateP (radio)",
];

/// Review F10: 1600PT's and 1600VT's `AtcCode<n>` (the official ATC
/// popup's tick boxes) tick the category's `n`th ATC into Part II and show
/// it ticked; the untick removes the row; item 10 stays as answered. They
/// are not a one-answer group.
#[test]
fn pr14_popup_slots_tick_their_atc_and_show_it() {
    setup();
    for (code, prefix, first, second) in [
        ("1600PT", "frm1600PT:", "AtcCode1", "AtcCode2"),
        ("1600VT", "frm1600VT:", "AtcCode1", "AtcCode2"),
    ] {
        let kind = kind_of(code).unwrap();
        assert!(
            !one_answer_groups(kind).contains_key(first),
            "{code}: {first} is a tick box of its own"
        );
        let opened = open(code, 1);
        let answered = fill(
            code,
            &opened["draft"],
            json!({
                format!("{prefix}rdoPg1I3TaxWithheldYes"): "true",
                format!("{prefix}rdoPg1I10CategoryofWthholdAgentPriv"): "true",
            }),
        );
        assert_eq!(answered["draft"]["agent_category"], "private", "{code}");
        let ticked = fill(
            code,
            &answered["draft"],
            json!({ first: "true", second: "true" }),
        );
        assert_eq!(ticked["rejected"], json!({}), "{code}");
        assert_eq!(
            ticked["values"][first], "true",
            "{code}: {first} shows ticked"
        );
        assert_eq!(
            ticked["values"][second], "true",
            "{code}: {second} shows ticked"
        );
        assert_eq!(
            ticked["draft"]["agent_category"], "private",
            "{code}: item 10 stays"
        );
        let rows = ticked["draft"]["atc_rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{code}: one Part II row each");
        assert_eq!(
            ticked["values"][format!("{prefix}txtAtcCd1")],
            rows[0]["atc_code"],
            "{code}: row 1 shows the first slot's ATC"
        );
        let unticked = fill(code, &ticked["draft"], json!({ first: "false" }));
        assert_eq!(unticked["rejected"], json!({}), "{code}");
        assert_eq!(
            unticked["values"][first], "false",
            "{code}: {first} unticked"
        );
        assert_eq!(unticked["values"][second], "true", "{code}: {second} stays");
        assert_eq!(
            unticked["draft"]["atc_rows"].as_array().unwrap().len(),
            1,
            "{code}"
        );
        assert_eq!(unticked["draft"]["agent_category"], "private", "{code}");
        // Before item 10 there is no list: the tick says so.
        let refused = fill(code, &opened["draft"], json!({ first: "true" }));
        assert_eq!(
            refused["rejected"][first], "Please select an option for Item 10.",
            "{code}"
        );
    }
}

/// F1 follow-up: 1700's Schedule 1 row 2 Spouse box is the host's
/// (rows_1700.rs) and joins no one-answer group, though the probe bound it
/// to item 17. Unticking it changes only its employer row; item 17 stays
/// Joint. Item 17's own pair keeps its flip: unticking Joint leaves it
/// unanswered.
#[test]
fn pr14_1700_schedule_1_spouse_box_never_blanks_item_17() {
    setup();
    let spouse_box = "frm1700:rdoPg2I2PartVIEmployeeS";
    assert!(!one_answer_groups(kind_of("1700").unwrap()).contains_key(spouse_box));
    let opened = open("1700", 0);
    let joint = fill(
        "1700",
        &opened["draft"],
        json!({
            "frm1700:rdoPg1I15CivilStatusM": "true",
            "frm1700:rdoPg1I16SpouseHasIncomeY": "true",
            "frm1700:rdoPg1I17FilingStatusJ": "true",
        }),
    );
    let rows = fill(
        "1700",
        &joint["draft"],
        json!({
            "frm1700:txtPg2I1PartVIEmployerName1": "First Employer",
            "frm1700:txtPg2I2PartVIEmployerName1": "Second Employer",
        }),
    );
    let ticked = fill("1700", &rows["draft"], json!({ spouse_box: "true" }));
    assert_eq!(ticked["rejected"], json!({}));
    assert_eq!(ticked["values"][spouse_box], "true");
    assert_eq!(ticked["draft"]["filing_status"], "joint");
    let unticked = fill("1700", &ticked["draft"], json!({ spouse_box: "false" }));
    assert_eq!(unticked["rejected"], json!({}));
    assert_eq!(unticked["values"][spouse_box], "false");
    assert_eq!(unticked["draft"]["filing_status"], "joint", "item 17 stays");
    let flip = fill(
        "1700",
        &unticked["draft"],
        json!({ "frm1700:rdoPg1I17FilingStatusJ": "false" }),
    );
    assert_eq!(flip["draft"]["filing_status"], "unanswered");
}
