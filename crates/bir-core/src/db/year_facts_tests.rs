//! Year facts (owner decisions 1, 4-7): head-office ownership, a new year
//! copying the last, the election lock, the forms list no save touches,
//! the 2551Q paths without a confirmed profile version, keyed-row moves,
//! the page's view and the profile archive.

use rusqlite::params;
use serde_json::json;

use super::{Claim2551QSubmissionResult, Database};
use crate::forms::FilingStatus;
use crate::forms::form_2551q::{Form2551QDraft, Item13Election};
use crate::profile::{
    EoptTier, IncomeTaxElection, TaxClassification, TaxProfileVersion, TaxProfileVersionSource,
    TaxProfileVersionStatus, TaxpayerType,
};
use crate::tax_profile::{
    BranchFacts, Fact, FactOwner, NewBranch, NewTaxpayer, TaxProfileError, TaxpayerUpdate,
    YearFactsPatch,
};

const ROOT: &str = "000123453";
const HEAD: &str = "00012345300000";
const BRANCH: &str = "00012345300001";

fn db() -> Database {
    let db = Database::open_in_memory_for_tests().unwrap();
    db.create_taxpayer(NewTaxpayer {
        tin_root: ROOT.into(),
        name: "Juana Santos".into(),
        taxpayer_type: TaxpayerType::Individual,
        birth_date: None,
        head_office: BranchFacts {
            rdo_code: "039".into(),
            registered_address: "Quezon City".into(),
            zip_code: "1100".into(),
            phone: "09170000000".into(),
            email: "juana@example.com".into(),
        },
    })
    .unwrap();
    db.create_branch(
        ROOT,
        NewBranch {
            branch_code: "00001".into(),
            name: "Branch".into(),
            fields: Default::default(),
        },
    )
    .unwrap();
    db
}

fn patch(value: serde_json::Value) -> YearFactsPatch {
    YearFactsPatch::from_json(&value).unwrap()
}

fn rows(db: &Database, tin: &str) -> i64 {
    db.conn
        .query_row(
            "SELECT count(*) FROM year_facts WHERE tin = ?1",
            [tin],
            |row| row.get(0),
        )
        .unwrap()
}

fn insert_return(db: &Database, tin: &str, code: &str, year: u16, status: &str) {
    db.conn
        .execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
             VALUES (?1, ?2, ?3, 1, 'Q1', ?4, '{}')",
            params![tin, code, i64::from(year), status],
        )
        .unwrap();
}

/// A Q1 PT010 return of `year` built from the branch's year profile, with
/// Item 13 answered and moved to Queued.
fn queued_q1(db: &Database, tin: &str, year: u16, election: Item13Election) -> Form2551QDraft {
    let profile = db.get_profile_for_year(tin, year).unwrap().unwrap();
    let mut draft = Form2551QDraft::new_from_effective_profile(&profile, year, 1);
    draft.item_13_election = election;
    draft
        .transition_to_queued()
        .unwrap_or_else(|errors| panic!("the Q1 return should queue: {errors:?}"));
    draft
}

#[test]
fn the_head_office_owns_the_income_type_election_and_tier_and_a_branch_its_line() {
    let db = db();
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({
            "tax_classification": "SelfEmployed",
            "income_tax_election": "EightPercent",
            "eopt_tier": "Small",
            "line_of_business": "Head office store",
        })),
    )
    .unwrap();
    // A head-office fact set through the branch lands on the head office.
    let branch = db
        .set_year_facts(
            BRANCH,
            2025,
            &patch(json!({
                "line_of_business": "Branch store",
                "eopt_tier": "Micro",
                "atc_codes": ["pt010"],
            })),
        )
        .unwrap();
    assert_eq!(branch.head_office_tin, HEAD);
    assert_eq!(
        branch.facts.tax_classification,
        Some(TaxClassification::SelfEmployed)
    );
    assert_eq!(
        branch.facts.income_tax_election,
        Some(IncomeTaxElection::EightPercent)
    );
    assert_eq!(branch.facts.eopt_tier, Some(EoptTier::Micro));
    assert_eq!(
        branch.facts.line_of_business.as_deref(),
        Some("Branch store")
    );
    assert_eq!(branch.facts.atc_codes, vec!["PT010".to_string()]);

    let head = db.effective_year_facts(HEAD, 2025).unwrap();
    assert_eq!(head.facts.eopt_tier, Some(EoptTier::Micro));
    assert_eq!(
        head.facts.line_of_business.as_deref(),
        Some("Head office store")
    );
    assert!(head.facts.atc_codes.is_empty());
    // The branch's own record holds only the facts it owns.
    let stored = db.year_facts(BRANCH, 2025).unwrap().unwrap();
    assert_eq!(stored.tax_classification, None);
    assert_eq!(stored.eopt_tier, None);
    assert_eq!(stored.income_tax_election, None);

    // The overlay: the year's profile of the branch reads all of it, the
    // flat profile is not rewritten.
    let profile = db.get_profile_for_year(BRANCH, 2025).unwrap().unwrap();
    assert!(profile.has_8_percent_election(2025));
    assert_eq!(profile.line_of_business, "Branch store");
    assert_eq!(profile.eopt_tier, Some(EoptTier::Micro));
    assert_eq!(
        profile.tax_classification,
        Some(TaxClassification::SelfEmployed)
    );
    assert_eq!(profile.atc_codes, vec!["PT010".to_string()]);
    let flat = db.get_profile(BRANCH).unwrap().unwrap();
    assert_eq!(flat.line_of_business, "");
    assert!(flat.tax_elections.is_empty());
    assert_eq!(flat.tax_classification, None);
    // Another year is untouched.
    let other = db.get_profile_for_year(BRANCH, 2024).unwrap().unwrap();
    assert!(!other.has_8_percent_election(2024));
    assert_eq!(other.line_of_business, "");
}

#[test]
fn a_year_without_a_record_reads_the_last_one_and_a_new_year_copies_it() {
    let db = db();
    db.add_tax_year(HEAD, 2024, false).unwrap();
    db.set_year_facts(
        HEAD,
        2024,
        &patch(json!({
            "tax_classification": "MixedIncome",
            "income_tax_election": "GraduatedOsd",
            "business_start_date": "2019-05-01",
        })),
    )
    .unwrap();
    let unrecorded = db.effective_year_facts(HEAD, 2025).unwrap();
    assert!(!unrecorded.election_recorded);
    assert_eq!(
        unrecorded.facts.tax_classification,
        Some(TaxClassification::MixedIncome)
    );
    assert_eq!(
        unrecorded.facts.business_start_date,
        chrono::NaiveDate::from_ymd_opt(2019, 5, 1)
    );
    assert_eq!(
        unrecorded.facts.income_tax_election, None,
        "the election is chosen each year"
    );
    assert!(
        db.year_facts(HEAD, 2025).unwrap().is_none(),
        "a read writes nothing"
    );

    db.add_tax_year(HEAD, 2025, false).unwrap();
    let copied = db.year_facts(HEAD, 2025).unwrap().unwrap();
    assert_eq!(
        copied.tax_classification,
        Some(TaxClassification::MixedIncome)
    );
    assert_eq!(copied.income_tax_election, None);
    assert_eq!(
        db.year_facts(HEAD, 2024)
            .unwrap()
            .unwrap()
            .income_tax_election,
        Some(IncomeTaxElection::GraduatedOsd)
    );
}

#[test]
fn the_election_locks_once_a_return_carrying_it_is_filed() {
    let db = db();
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({"tax_classification": "SelfEmployed"})),
    )
    .unwrap();
    db.record_income_tax_election(BRANCH, 2025, IncomeTaxElection::EightPercent)
        .unwrap();
    let head = db.year_facts(HEAD, 2025).unwrap().unwrap();
    assert_eq!(
        head.income_tax_election,
        Some(IncomeTaxElection::EightPercent)
    );
    assert_eq!(head.election_source.as_deref(), Some("Tax profile"));

    // A queued return does not lock the choice.
    insert_return(&db, BRANCH, "1701Q", 2025, "Queued");
    assert!(!db.income_tax_election_locked(HEAD, 2025).unwrap());
    db.record_income_tax_election(HEAD, 2025, IncomeTaxElection::GraduatedUnspecified)
        .unwrap();

    // Filed, on any branch: locked for every branch, that year only.
    db.conn
        .execute("UPDATE form_drafts SET status = 'Submitted'", [])
        .unwrap();
    assert!(db.income_tax_election_locked(HEAD, 2025).unwrap());
    assert!(db.income_tax_election_locked(BRANCH, 2025).unwrap());
    assert!(!db.income_tax_election_locked(HEAD, 2026).unwrap());
    let refused = db
        .record_income_tax_election(HEAD, 2025, IncomeTaxElection::EightPercent)
        .unwrap_err()
        .to_string();
    assert!(refused.contains("cannot change"), "{refused}");
    for change in [
        json!({"income_tax_election": "EightPercent"}),
        json!({"income_tax_election": null}),
    ] {
        let error = db.set_year_facts(BRANCH, 2025, &patch(change)).unwrap_err();
        assert!(matches!(error, TaxProfileError::Conflict(_)), "{error}");
    }
    assert_eq!(
        db.year_facts(HEAD, 2025)
            .unwrap()
            .unwrap()
            .income_tax_election,
        Some(IncomeTaxElection::GraduatedUnspecified),
        "unwritten"
    );
    // The same choice, or another fact, still saves.
    db.record_income_tax_election(HEAD, 2025, IncomeTaxElection::GraduatedUnspecified)
        .unwrap();
    db.set_year_facts(HEAD, 2025, &patch(json!({"eopt_tier": "Micro"})))
        .unwrap();
    // A non-individual records no election at all.
    db.update_taxpayer(
        ROOT,
        TaxpayerUpdate {
            taxpayer_type: Some(TaxpayerType::Corporation),
            ..TaxpayerUpdate::default()
        },
    )
    .unwrap();
    assert!(
        db.record_income_tax_election(HEAD, 2026, IncomeTaxElection::EightPercent)
            .is_err()
    );
}

#[test]
fn no_save_adds_or_removes_a_form() {
    use chrono::Datelike as _;
    let db = db();
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({"tax_classification": "SelfEmployed"})),
    )
    .unwrap();
    db.set_yearly_forms(HEAD, 2025, &["2551Q"]).unwrap();
    let list = |db: &Database| {
        (
            db.active_form_codes_for_year(HEAD, 2025).unwrap(),
            db.list_forms_set_years(HEAD).unwrap(),
            db.list_forms_set_years(BRANCH).unwrap(),
        )
    };
    let before = list(&db);
    assert_eq!(before.0, vec!["2551Q".to_string()]);

    // A profile save.
    let profile = db.get_profile(HEAD).unwrap().unwrap();
    db.save_profile(profile).unwrap();
    assert_eq!(list(&db), before);
    let current_year = chrono::Local::now().year() as u16;
    assert!(!db.has_per_year_forms(HEAD, current_year).unwrap());

    // An election.
    db.record_income_tax_election(HEAD, 2025, IncomeTaxElection::EightPercent)
        .unwrap();
    assert_eq!(list(&db), before);

    // A queued 2551Q that answers Item 13.
    let draft = queued_q1(&db, HEAD, 2025, Item13Election::EightPercent);
    db.save_queued_2551q_draft_and_election(&draft).unwrap();
    assert_eq!(list(&db), before);
    // Reading the profile does not tidy the list either.
    assert_eq!(
        db.get_profile(HEAD).unwrap().unwrap().per_year_forms[&2025].active_form_codes(),
        before.0
    );
}

/// The 2551Q queue and claim paths (`save_queued_2551q_draft_and_election`,
/// `claim_queued_2551q_submission`) for a profile with no confirmed version
/// and for one whose versions are a mess: the year facts decide, Item 13
/// is recorded on the head office, and nothing asks for a version.
#[test]
fn the_2551q_queue_and_claim_need_no_confirmed_profile_version() {
    for messy in [false, true] {
        let db = db();
        db.set_year_facts(
            HEAD,
            2025,
            &patch(json!({"tax_classification": "SelfEmployed"})),
        )
        .unwrap();
        if messy {
            let mut profile = db.get_profile(HEAD).unwrap().unwrap();
            let date = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d);
            let version = |id: &str, from, until| {
                let mut version = TaxProfileVersion::from_profile_backfill(&profile);
                version.id = id.into();
                version.source = TaxProfileVersionSource::ManualCor;
                version.status = TaxProfileVersionStatus::Confirmed;
                version.effective_from = from;
                version.effective_until = until;
                version.cor.registered_name = "Someone Else".into();
                version.cor.rdo_code = "001".into();
                version
            };
            let versions = vec![
                version("a", date(2024, 1, 1), None),
                version("b", date(2025, 2, 1), None),
                version("undated", None, None),
            ];
            profile.profile_versions = versions;
            db.conn
                .execute(
                    "UPDATE profiles SET data_json = ?1 WHERE tin = ?2",
                    params![serde_json::to_string(&profile).unwrap(), HEAD],
                )
                .unwrap();
        }
        let draft = queued_q1(&db, HEAD, 2025, Item13Election::EightPercent);
        assert_eq!(draft.taxpayer_name, "Juana Santos", "messy={messy}");
        assert_eq!(draft.rdo_code, "039", "the branch's own RDO, messy={messy}");
        assert!(draft.profile_resolution_error.is_none());
        db.save_queued_2551q_draft_and_election(&draft)
            .unwrap_or_else(|error| panic!("messy={messy}: {error}"));
        let head = db.year_facts(HEAD, 2025).unwrap().unwrap();
        assert_eq!(
            head.income_tax_election,
            Some(IncomeTaxElection::EightPercent)
        );
        assert_eq!(head.election_source.as_deref(), Some("2551Q Q1"));

        let queued = db.get_2551q_draft(HEAD, 2025, 1).unwrap().unwrap();
        assert_eq!(queued.status, FilingStatus::Queued);
        match db
            .claim_queued_2551q_submission(
                HEAD,
                2025,
                1,
                &queued.queued_submission_fingerprint,
                &queued.next_retry_at,
                queued.submission_attempts,
            )
            .unwrap()
        {
            Claim2551QSubmissionResult::Claimed { draft, .. } => {
                assert!(draft.profile_resolution_error.is_none());
                assert!(!draft.profile_snapshot_stale, "messy={messy}");
            }
            Claim2551QSubmissionResult::Rejected { errors, .. } => {
                panic!("messy={messy}: claim rejected: {errors:?}")
            }
            Claim2551QSubmissionResult::Superseded => panic!("messy={messy}: superseded"),
        }
    }
}

/// Queue changes the year's choice while nothing carrying it is filed, and
/// refuses, unwritten, once a return carrying it is.
#[test]
fn the_2551q_queue_path_enforces_the_election_lock() {
    let db = db();
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({"tax_classification": "SelfEmployed"})),
    )
    .unwrap();
    // Answered graduated on the return while the year had no choice yet;
    // the profile then records 8% elsewhere.
    let graduated = queued_q1(&db, HEAD, 2025, Item13Election::Graduated);
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({"income_tax_election": "EightPercent"})),
    )
    .unwrap();

    // Locked by a filed 1701Q of the branch: the changed Item 13 is refused.
    insert_return(&db, BRANCH, "1701Q", 2025, "Confirmed");
    let refused = db
        .save_queued_2551q_draft_and_election(&graduated)
        .unwrap_err()
        .to_string();
    assert!(refused.contains("cannot change"), "{refused}");
    assert!(db.get_2551q_draft(HEAD, 2025, 1).unwrap().is_none());
    assert_eq!(
        db.year_facts(HEAD, 2025)
            .unwrap()
            .unwrap()
            .income_tax_election,
        Some(IncomeTaxElection::EightPercent)
    );

    // Nothing filed: queueing the other answer changes the year's choice.
    db.conn
        .execute("DELETE FROM form_drafts WHERE form_code = '1701Q'", [])
        .unwrap();
    db.save_queued_2551q_draft_and_election(&graduated).unwrap();
    let head = db.year_facts(HEAD, 2025).unwrap().unwrap();
    assert_eq!(
        head.income_tax_election,
        Some(IncomeTaxElection::GraduatedUnspecified)
    );
    assert_eq!(head.election_source.as_deref(), Some("2551Q Q1"));
}

#[test]
fn confirming_an_income_type_sets_its_year_and_later_years_without_one() {
    let db = db();
    db.set_year_facts(HEAD, 2024, &patch(json!({"eopt_tier": "Micro"})))
        .unwrap();
    db.set_year_facts(HEAD, 2025, &patch(json!({"eopt_tier": "Micro"})))
        .unwrap();
    db.set_year_facts(
        HEAD,
        2026,
        &patch(json!({"tax_classification": "PurelyCompensation"})),
    )
    .unwrap();
    let date = chrono::NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
    let profile = db
        .confirm_income_registration(BRANCH, TaxClassification::SelfEmployed, date)
        .unwrap();
    assert!(profile.eligible_for_income_tax_election_in_year(2024));
    let classification = |year| {
        db.year_facts(HEAD, year)
            .unwrap()
            .unwrap()
            .tax_classification
    };
    assert_eq!(classification(2024), Some(TaxClassification::SelfEmployed));
    assert_eq!(classification(2025), Some(TaxClassification::SelfEmployed));
    assert_eq!(
        classification(2026),
        Some(TaxClassification::PurelyCompensation),
        "a later year with its own income type keeps it"
    );
    assert!(
        db.income_tax_election_readiness(BRANCH, 2025)
            .unwrap()
            .is_ready()
    );
    assert!(
        db.confirm_income_registration(BRANCH, TaxClassification::Corporation, date)
            .is_err()
    );
}

#[test]
fn deleting_and_moving_carry_the_year_facts_like_the_forms_list() {
    let db = db();
    db.set_year_facts(HEAD, 2025, &patch(json!({"eopt_tier": "Micro"})))
        .unwrap();
    db.set_year_facts(BRANCH, 2025, &patch(json!({"line_of_business": "Stall"})))
        .unwrap();
    db.move_branch_code(BRANCH, "00002").unwrap();
    assert_eq!(rows(&db, BRANCH), 0);
    assert_eq!(
        db.year_facts("00012345300002", 2025)
            .unwrap()
            .unwrap()
            .line_of_business
            .as_deref(),
        Some("Stall")
    );
    db.move_taxpayer_tin(ROOT, "111222333").unwrap();
    assert_eq!(rows(&db, HEAD), 0);
    assert_eq!(rows(&db, "11122233300000"), 1);
    assert_eq!(rows(&db, "11122233300002"), 1);
    assert_eq!(
        db.effective_year_facts("11122233300002", 2025)
            .unwrap()
            .facts
            .eopt_tier,
        Some(EoptTier::Micro)
    );
    db.delete_profile("11122233300002").unwrap();
    assert_eq!(rows(&db, "11122233300002"), 0);
    db.delete_taxpayer("111222333").unwrap();
    assert_eq!(rows(&db, "11122233300000"), 0);
}

#[test]
fn the_view_says_which_forms_read_each_fact_and_what_is_not_set() {
    let db = db();
    db.set_yearly_forms(BRANCH, 2025, &["2551Q", "0619E", "1701"])
        .unwrap();
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({"tax_classification": "SelfEmployed"})),
    )
    .unwrap();
    let view = db.year_facts_view(BRANCH, 2025).unwrap();
    assert_eq!(view.used_by["birth_date"], vec!["1701".to_string()]);
    assert_eq!(
        view.used_by["tax_classification"],
        vec!["1701".to_string(), "2551Q".to_string()]
    );
    assert_eq!(
        view.used_by["is_government_withholding_entity"],
        vec!["0619E".to_string()]
    );
    assert_eq!(view.owner["tax_classification"], FactOwner::HeadOffice);
    assert_eq!(view.owner["line_of_business"], FactOwner::Branch);
    assert!(!view.owner.contains_key("birth_date"));
    assert_eq!(
        view.not_set,
        vec![
            Fact::BirthDate,
            Fact::IncomeTaxElection,
            Fact::EoptTier,
            Fact::BusinessStartDate,
            Fact::LineOfBusiness,
            Fact::AtcCodes,
        ]
    );
    assert!(!view.election_locked);
    assert_eq!(view.election_source, None);
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(value["owner"]["eopt_tier"], "head_office");
    assert_eq!(value["not_set"][0], "birth_date");
    assert_eq!(value["facts"]["tax_classification"], "SelfEmployed");
    assert!(value["facts"]["income_tax_election"].is_null());

    db.update_taxpayer(
        ROOT,
        TaxpayerUpdate {
            birth_date: Some(chrono::NaiveDate::from_ymd_opt(1990, 1, 2)),
            ..TaxpayerUpdate::default()
        },
    )
    .unwrap();
    db.record_income_tax_election(BRANCH, 2025, IncomeTaxElection::EightPercent)
        .unwrap();
    let view = db.year_facts_view(BRANCH, 2025).unwrap();
    assert!(!view.not_set.contains(&Fact::BirthDate));
    assert!(!view.not_set.contains(&Fact::IncomeTaxElection));
    assert_eq!(view.election_source.as_deref(), Some("Tax profile"));

    let chips = db.tax_types_for_year(BRANCH, 2025).unwrap();
    let types: Vec<&str> = chips.iter().map(|chip| chip.tax_type).collect();
    assert_eq!(
        types,
        vec!["percentage_tax", "withholding_expanded", "income_tax"]
    );
}

#[test]
fn the_profile_archive_carries_the_years_forms_and_facts() {
    let source = db();
    source
        .set_yearly_forms(BRANCH, 2025, &["2551Q", "1701Q"])
        .unwrap();
    source.add_tax_year(BRANCH, 2026, false).unwrap();
    source
        .set_year_facts(
            BRANCH,
            2025,
            &patch(json!({"line_of_business": "Stall", "atc_codes": ["PT010"]})),
        )
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("branch.zip");
    crate::export::export_profile_data(&source, BRANCH, &archive, None).unwrap();

    let target = Database::open_in_memory_for_tests().unwrap();
    crate::import::import_profile_data(&target, &archive).unwrap();
    assert_eq!(
        target.active_form_codes_for_year(BRANCH, 2025).unwrap(),
        vec!["1701Q".to_string(), "2551Q".to_string()]
    );
    let set = target.get_per_year_forms(BRANCH, 2025).unwrap();
    assert!(
        set.entries
            .iter()
            .all(|entry| entry.source == crate::forms::FormSetSource::Manual)
    );
    assert_eq!(target.list_tax_years(BRANCH).unwrap(), vec![2025, 2026]);
    let facts = target.year_facts(BRANCH, 2025).unwrap().unwrap();
    assert_eq!(facts.line_of_business.as_deref(), Some("Stall"));
    assert_eq!(facts.atc_codes, vec!["PT010".to_string()]);

    // An archive written before year facts (profile.json only) still imports.
    let old = dir.path().join("old.zip");
    {
        use std::io::Write as _;
        let file = std::fs::File::create(&old).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let mut profile = source.get_profile(HEAD).unwrap().unwrap();
        let record = profile.record_without_credentials().unwrap();
        zip.start_file("profile.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(record.to_string().as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    let fresh = Database::open_in_memory_for_tests().unwrap();
    crate::import::import_profile_data(&fresh, &old).unwrap();
    assert!(fresh.get_profile(HEAD).unwrap().is_some());
    assert!(fresh.list_forms_set_years(HEAD).unwrap().is_empty());
}

/// A graduated answer over a year already recorded as graduated keeps the
/// recorded deduction method, so a method a later return refined (OSD or
/// itemized) is not lost; an 8% answer replaces it.
#[test]
fn a_graduated_answer_keeps_a_refined_graduated_method() {
    let db = db();
    db.set_year_facts(
        HEAD,
        2025,
        &patch(json!({"tax_classification": "MixedIncome", "income_tax_election": "GraduatedOsd"})),
    )
    .unwrap();
    db.record_income_tax_election(HEAD, 2025, IncomeTaxElection::GraduatedUnspecified)
        .unwrap();
    assert_eq!(
        db.year_facts(HEAD, 2025)
            .unwrap()
            .unwrap()
            .income_tax_election,
        Some(IncomeTaxElection::GraduatedOsd)
    );
    db.record_income_tax_election(HEAD, 2025, IncomeTaxElection::EightPercent)
        .unwrap();
    assert_eq!(
        db.year_facts(HEAD, 2025)
            .unwrap()
            .unwrap()
            .income_tax_election,
        Some(IncomeTaxElection::EightPercent)
    );
}
