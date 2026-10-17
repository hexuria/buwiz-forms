use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt};
use bir_core::forms::form_1701q::{
    Form1701QAtc, Form1701QCivilStatus, Form1701QDeductionMethod, Form1701QDraft,
    Form1701QFilerType, Form1701QParty, Form1701QSpouseType,
};
use bir_core::forms::queueable::QueueableForm as _;

use crate::sample_payloads::penalty_items;
use bir_core::official_xml;
use sha2::{Digest, Sha256};

/// A professional on graduated rates (itemized) with a spouse on the 8%
/// rate, an amended return, a loss on Item 42 and centavos to round.
fn sample_1701q() -> Form1701QDraft {
    let mut draft =
        Form1701QDraft::new_from_profile(&dummy_profile("1701Qv2018", "Individual"), 2025, 2);
    let t = Form1701QParty::Taxpayer;
    let s = Form1701QParty::Spouse;
    draft.is_amended = true;
    draft.filer_type = Some(Form1701QFilerType::Professional);
    draft.atc = Some(Form1701QAtc::Ii014);
    draft.deduction_method = Some(Form1701QDeductionMethod::Itemized);
    draft.date_of_birth = "05/17/1985".into();
    draft.citizenship = "Filipino".into();
    draft.claims_foreign_tax_credits = Some(false);
    draft.has_spouse = true;
    draft.spouse_tin = "987-654-321-00000".into();
    draft.spouse_rdo_code = "040".into();
    draft.spouse_type = Some(Form1701QSpouseType::SingleProprietor);
    draft.spouse_atc = Some(Form1701QAtc::Ii015);
    draft.spouse_name = "Dummy Spouse Sample".into();
    draft.spouse_citizenship = "Filipino".into();
    draft.spouse_claims_foreign_tax_credits = Some(true);
    draft.spouse_foreign_tax_number = "x12345".into();
    draft.item_43_non_operating_income_description = "Rental income".into();
    draft.item_48_non_operating_income_description = "Interest".into();
    draft.item_61_other_tax_credit_description = "Other credit".into();
    for (item, party, value) in [
        (36, t, 1_234_567.0),
        (37, t, 345_678.0),
        (39, t, 123_456.0),
        (42, t, -1_000.505),
        (43, t, 50_000.0),
        (44, t, 7_000.0),
        (55, t, 10_000.0),
        (59, t, 2_500.0),
        (61, t, 300.0),
        (64, t, 1_000.0),
        (65, t, 250.0),
        (47, s, 900_000.0),
        (48, s, 12_345.0),
        (50, s, 15_000.505),
        (52, s, 250_000.0),
        (56, s, 4_000.0),
        (66, s, 500.0),
    ] {
        draft.set_amount(item, party, Some(value));
    }
    // Items 64-66 are on the shared penalty model since PR 5; the sample's
    // amounts are entered ones, so they file as typed (the upload below is
    // the official page's).
    draft.keep_entered_penalties();
    draft.recompute();
    draft
}

/// The rates the other way round, for the probe: the taxpayer on the 8%
/// rate and the spouse on graduated rates with itemized deductions, so
/// every A and B input of Schedules I and II, and Item 25A, is written.
fn sample_1701q_rates_swapped() -> Form1701QDraft {
    let mut draft =
        Form1701QDraft::new_from_profile(&dummy_profile("1701Qv2018", "Individual"), 2025, 3);
    let t = Form1701QParty::Taxpayer;
    let s = Form1701QParty::Spouse;
    // The same filer and spouse types as the first sample, so only the
    // rates differ between the two (the probe binds a radio to the answer
    // that changes it across the samples).
    draft.filer_type = Some(Form1701QFilerType::Professional);
    draft.atc = Some(Form1701QAtc::Ii017);
    draft.date_of_birth = "05/17/1985".into();
    draft.citizenship = "Filipino".into();
    draft.claims_foreign_tax_credits = Some(false);
    draft.set_civil_status(Some(Form1701QCivilStatus::Married));
    draft.spouse_tin = "987-654-321-00000".into();
    draft.spouse_rdo_code = "040".into();
    draft.spouse_type = Some(Form1701QSpouseType::SingleProprietor);
    draft.spouse_atc = Some(Form1701QAtc::Ii012);
    draft.spouse_deduction_method = Some(Form1701QDeductionMethod::Itemized);
    draft.spouse_name = "Dummy Spouse Sample".into();
    draft.spouse_citizenship = "Filipino".into();
    draft.spouse_claims_foreign_tax_credits = Some(false);
    draft.item_43_non_operating_income_description = "Rental income".into();
    draft.item_48_non_operating_income_description = "Interest".into();
    for (item, party, value) in [
        (47, t, 900_000.0),
        (48, t, 12_345.0),
        (50, t, 15_000.0),
        (52, t, 250_000.0),
        (58, t, 3_000.0),
        (36, s, 1_234_567.0),
        (37, s, 345_678.0),
        (39, s, 123_456.0),
        (42, s, 1_000.0),
        (43, s, 50_000.0),
        (44, s, 7_000.0),
        (57, s, 2_000.0),
        (64, t, 300.0),
        (65, s, 200.0),
    ] {
        draft.set_amount(item, party, Some(value));
    }
    draft.keep_entered_penalties();
    draft.recompute();
    draft
}

/// Checks the 1701Q sample against the official upload.
#[test]
fn form_1701q_sample_payload_is_current() {
    const STEM: &str = "1701Q-2025Q2";
    let payload = sample_1701q()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1701Q must validate: {errors:?}"));
    let layout = official_xml::layout("1701q-v2018").unwrap();
    official_xml::read(layout, &payload).expect("the sample follows the official layout");

    let encrypted = compress_and_encrypt(payload.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    let dir = samples_dir();
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::write(dir.join(format!("{STEM}.plain.xml")), &payload).unwrap();
        std::fs::write(dir.join(format!("{STEM}.iaf.xml")), &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{STEM}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{STEM}: differs from the official saveXMLsubmit() output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, "7a1baccf54e3c9e98b9e8ce172fc47f8e71deb7ed9a8d0e58fb48dcc8dee7a12",
        "{STEM}: official Encrypt.exe hash"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{STEM}.plain.xml"))).unwrap(),
        payload
    );
    assert_eq!(
        std::fs::read(dir.join(format!("{STEM}.iaf.xml"))).unwrap(),
        encrypted
    );
}

/// Items 64-66 of both columns on the shared penalty model (PR 5), checked
/// the way `penalty_items::check` checks every wired form. That check reads
/// each item by a draft field named after it (`surcharge`); 1701Q keeps its
/// amounts by item number (`amounts.item_64.taxpayer`), and a draft stored
/// while the items were typed boxes keeps all three amounts, a zero too, so
/// it runs here with those two differences. Q2 2025 is due 15 August 2025
/// (calendar_rules.rs: "Q1: May 15, Q2: Aug 15, Q3: Oct 15"); the samples
/// are filed late on 13 October 2026. Rates: surcharge 25%, interest 12% a
/// year, compromise from the RMO 7-2015 schedule; whole pesos, as every
/// 1701Q input.
#[test]
fn form_1701q_penalty_items_follow_the_shared_model() {
    use bir_core::forms::FilingStatus;
    use bir_core::forms::penalty_items::with_clock;
    use bir_core::penalties::compromise::lookup_compromise;
    use bir_core::penalties::{PenaltyComponent, PenaltyProfile};
    use penalty_items::day;

    let filed = day(2026, 10, 13);
    let compute = |draft: &mut Form1701QDraft| with_clock(filed, || draft.recompute());
    let declarations = Form1701QDraft::penalty_items();
    assert_eq!(declarations.len(), 2);
    let money = |text: &str| text.replace(',', "").parse::<f64>().unwrap();

    // The sample's taxpayer and spouse both owe tax: both columns compute.
    let mut late = sample_1701q();
    late.penalty.penalty_overrides = Default::default();
    late.spouse_penalty.penalty_overrides = Default::default();
    compute(&mut late);
    for (items, column) in declarations.iter().zip(["A", "B"]) {
        assert!((items.applies)(&late), "column {column} applies");
        let computed = (items.state)(&late).computed_penalties.clone().unwrap();
        assert_eq!(
            computed.inputs.due_date,
            day(2025, 8, 15),
            "column {column}"
        );
        assert_eq!(computed.inputs.filing_date, filed);
        let basic = money(&late.field_map()[&format!("frm1701q:txt63{column}")]);
        assert!(basic > 0.0, "column {column} has tax still due");
        assert_eq!(computed.inputs.basic_tax_due, basic);
        let days = computed.inputs.days_late as f64;
        assert_eq!(days, (filed - day(2025, 8, 15)).num_days() as f64);
        let expected = [
            (basic * 0.25).round(),
            (basic * 0.12 * days / 365.0).round(),
            lookup_compromise(basic, 0.0, PenaltyProfile::StandardFiling).round(),
        ];
        let fields = late.field_map();
        let shown: Vec<f64> = [64, 65, 66]
            .iter()
            .map(|item| money(&fields[&format!("frm1701q:txt{item}{column}")]))
            .collect();
        assert_eq!(
            shown, expected,
            "column {column} files the computed amounts"
        );
        assert_eq!(
            money(&fields[&format!("frm1701q:txt67{column}")]),
            expected.iter().sum::<f64>(),
            "column {column}: Item 67 adds them up"
        );
        assert_eq!(
            money(&fields[&format!("frm1701q:txt29{column}")]),
            expected.iter().sum::<f64>(),
            "column {column}: Part III Item 29 follows"
        );
    }

    // One override changes only its item, its totals and the upload.
    let taxpayer = &declarations[0];
    let base = late.clone();
    let mut overridden = base.clone();
    taxpayer.set_override(&mut overridden, PenaltyComponent::Compromise, Some(500.0));
    compute(&mut overridden);
    let (before, after) = (base.field_map(), overridden.field_map());
    let changed: Vec<&String> = before
        .keys()
        .filter(|key| before.get(*key) != after.get(*key))
        .collect();
    assert_eq!(
        changed,
        [
            "frm1701q:txt29A",
            "frm1701q:txt30A",
            "frm1701q:txt31",
            "frm1701q:txt66A",
            "frm1701q:txt67A",
            "frm1701q:txt68A",
        ]
    );
    assert_eq!(after["frm1701q:txt66A"], "500.00");
    assert_ne!(
        overridden.submission_fingerprint(),
        base.submission_fingerprint()
    );
    // Clearing it brings the computed amount back.
    let mut cleared = overridden.clone();
    taxpayer.set_override(&mut cleared, PenaltyComponent::Compromise, None);
    compute(&mut cleared);
    assert_eq!(cleared.field_map(), before);

    // A draft stored while Items 64-66 were typed boxes (no penalty state)
    // files every amount it held, the zero interest too.
    let mut stored = serde_json::to_value(&base).unwrap();
    for state in ["penalty", "spouse_penalty"] {
        stored.as_object_mut().unwrap().remove(state);
    }
    for (pointer, value) in [
        ("/amounts/item_64/taxpayer", 125.0),
        ("/amounts/item_65/taxpayer", 0.0),
        ("/amounts/item_66/taxpayer", 1_000.0),
    ] {
        *stored.pointer_mut(pointer).unwrap() = serde_json::json!(value);
    }
    let mut legacy: Form1701QDraft = serde_json::from_value(stored).unwrap();
    assert_eq!(legacy.penalty.penalty_model_version, 0);
    compute(&mut legacy);
    assert_eq!(legacy.penalty.penalty_overrides.surcharge, Some(125.0));
    assert_eq!(legacy.penalty.penalty_overrides.interest, Some(0.0));
    assert_eq!(legacy.penalty.penalty_overrides.compromise, Some(1_000.0));
    let fields = legacy.field_map();
    assert_eq!(
        [
            &fields["frm1701q:txt64A"],
            &fields["frm1701q:txt65A"],
            &fields["frm1701q:txt66A"]
        ],
        ["125.00", "0.00", "1,000.00"]
    );

    // The spouse's column applies only to a married filer whose spouse is
    // not a compensation earner only; otherwise it stays 0.00.
    let mut single = base.clone();
    single.set_civil_status(Some(Form1701QCivilStatus::Single));
    compute(&mut single);
    assert!(!(declarations[1].applies)(&single));
    assert_eq!(single.field_map()["frm1701q:txt66B"], "0.00");
    assert_eq!(single.spouse_penalty.computed_penalties, None);

    // Filed returns never recompute.
    for status in [
        FilingStatus::Submitted,
        FilingStatus::Confirmed,
        FilingStatus::Paid,
    ] {
        let mut frozen = base.clone();
        frozen.lifecycle_mut().status = status;
        with_clock(day(2027, 1, 31), || frozen.recompute());
        assert_eq!(frozen.field_map(), base.field_map());
    }
}

/// An imported savefile keeps the penalties it was prepared with, though
/// its period is late: each column's three items become its overrides (a
/// zero too), and the import files what the savefile carried. 1701Q imports
/// its editable save (`to_bir_xml_payload`), not the outbound copy
/// `penalty_items::check_import` reads.
#[test]
fn form_1701q_import_keeps_its_penalties() {
    // The editable importer reads the typed amounts of USER_ENTERED_AMOUNT_ITEMS,
    // which leaves out Item 52; the sample's 52B goes for this check.
    let mut source = sample_1701q();
    source.set_amount(52, Form1701QParty::Spouse, None);
    source.recompute();
    let payload = source.to_bir_xml_payload().unwrap();
    let imported = Form1701QDraft::from_bir_xml_payload(&payload).unwrap();
    for items in Form1701QDraft::penalty_items() {
        assert!((items.applies)(&imported));
        let [s, i, c] = items.filed(&source);
        let state = (items.state)(&imported);
        assert_eq!(
            [
                state.penalty_overrides.surcharge,
                state.penalty_overrides.interest,
                state.penalty_overrides.compromise
            ],
            [Some(s), Some(i), Some(c)],
            "{:?}",
            items.party
        );
    }
    assert_eq!(imported.field_map(), source.field_map());
}

/// Every sample draft above, for the agent-form probe (`agent_forms`).
pub(crate) fn agent_seeds() -> Vec<serde_json::Value> {
    vec![
        serde_json::to_value(sample_1701q()).expect("sample serializes"),
        serde_json::to_value(sample_1701q_rates_swapped()).expect("sample serializes"),
    ]
}
