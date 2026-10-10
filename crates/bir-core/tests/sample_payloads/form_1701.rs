use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt};
use bir_core::forms::form_1701::{
    Form1701AmountSection as S, Form1701Atc, Form1701CivilStatus, Form1701DeductionMethod,
    Form1701Draft, Form1701EmployerRow, Form1701JointFilingStatus, Form1701Party,
    Form1701SpecialDeductionRow, Form1701SpouseType, Form1701TaxpayerType,
};
use sha2::{Digest, Sha256};

const TP: Form1701Party = Form1701Party::Taxpayer;
const SP: Form1701Party = Form1701Party::Spouse;

/// Mixed income (II013), graduated rates with OSD, rounding cases.
fn sample_1701() -> Form1701Draft {
    let mut d =
        Form1701Draft::new_from_profile(&dummy_profile("1701v2018", "Individual"), 2025, 12);
    d.taxpayer_type = Some(Form1701TaxpayerType::SingleProprietor);
    d.taxpayer_also_compensation_earner = true;
    d.atc = Some(Form1701Atc::Ii013);
    d.date_of_birth = "01/15/1980".into();
    d.citizenship = "Filipino".into();
    d.claims_foreign_tax_credits = Some(false);
    d.civil_status = Some(Form1701CivilStatus::Single);
    d.has_exempt_income = Some(false);
    d.has_special_rate_income = Some(false);
    d.deduction_method = Some(Form1701DeductionMethod::Osd);
    d.employers[0] = Form1701EmployerRow {
        owner: Some(TP),
        employer_name: "Sample Employer, Inc.".into(),
        employer_tin: "123-456-788-00000".into(),
        compensation_income: Some(480_000.005),
        tax_withheld: Some(25_000.0),
    };
    d.set_amount(S::Schedule2, 5, TP, Some(90_000.0));
    d.set_amount(S::Schedule3, 8, TP, Some(1_250_000.5));
    d.set_amount(S::Schedule3, 9, TP, Some(300_000.25));
    d.computations
        .schedule_3_descriptions
        .insert(19, "Rental income".into());
    d.set_amount(S::Schedule3, 19, TP, Some(12_345.67));
    d.set_amount(S::PartVii, 1, TP, Some(10_000.0));
    d.set_amount(S::PartVii, 2, TP, Some(20_000.5));
    d.set_amount(S::PartIi, 27, TP, Some(500.0));
    d.set_amount(S::PartIi, 28, TP, Some(250.25));
    d.recompute();
    d
}

/// Joint filing: a professional on the 8% rate and a spouse with mixed
/// income on graduated rates with itemized deductions and NOLCO.
fn sample_1701_joint() -> Form1701Draft {
    let mut d =
        Form1701Draft::new_from_profile(&dummy_profile("1701v2018", "Individual"), 2025, 12);
    d.taxpayer_type = Some(Form1701TaxpayerType::Professional);
    d.atc = Some(Form1701Atc::Ii017);
    d.date_of_birth = "03/01/1975".into();
    d.citizenship = "Filipino".into();
    d.claims_foreign_tax_credits = Some(false);
    d.civil_status = Some(Form1701CivilStatus::Married);
    d.spouse_has_income = Some(true);
    d.joint_filing_status = Some(Form1701JointFilingStatus::Joint);
    d.has_exempt_income = Some(false);
    d.has_special_rate_income = Some(false);
    d.spouse.tin = "123-456-788-00001".into();
    d.spouse_rdo_code = "039".into();
    d.spouse.filer_type = Some(Form1701SpouseType::SingleProprietor);
    d.spouse_also_compensation_earner = true;
    d.spouse.atc = Some(Form1701Atc::Ii013);
    d.spouse.name = "Sample Dummy Spouse".into();
    d.spouse.contact_number = "09170000001".into();
    d.spouse.citizenship = "Filipino".into();
    d.spouse.claims_foreign_tax_credits = Some(false);
    d.spouse.has_exempt_income = Some(false);
    d.spouse.has_special_rate_income = Some(false);
    d.spouse.deduction_method = Some(Form1701DeductionMethod::Itemized);
    d.employers[1] = Form1701EmployerRow {
        owner: Some(SP),
        employer_name: "Spouse Employer Corp".into(),
        employer_tin: "123-456-788-00002".into(),
        compensation_income: Some(300_000.0),
        tax_withheld: Some(5_000.0),
    };
    d.set_amount(S::Schedule3, 26, TP, Some(1_800_000.4));
    d.computations
        .schedule_3_descriptions
        .insert(27, "Interest income".into());
    d.set_amount(S::Schedule3, 27, TP, Some(100_000.0));
    d.set_amount(S::Schedule3, 29, TP, Some(250_000.0));
    d.set_amount(S::PartIx, 1, TP, Some(1_650_000.0));
    d.set_amount(S::PartVii, 3, TP, Some(1_000.0));
    d.set_amount(S::Schedule3, 8, SP, Some(800_000.0));
    d.set_amount(S::Schedule3, 9, SP, Some(200_000.5));
    d.set_amount(S::Schedule3, 11, SP, Some(10_000.0));
    d.set_amount(S::Schedule4, 1, SP, Some(50_000.0));
    d.set_amount(S::Schedule4, 2, SP, Some(20_000.75));
    d.computations.schedule_4_item_17[3].spouse = Some(1_000.0);
    d.computations.schedule_4_item_17d_description = "Miscellaneous".into();
    d.computations.schedule_5_spouse[0] = Form1701SpecialDeductionRow {
        description: "Special deduction".into(),
        legal_basis: "RA 1234".into(),
        amount: Some(5_000.0),
    };
    let row = &mut d.computations.schedule_6_spouse_nolco[0];
    row.year_incurred = "2023".into();
    row.amount = Some(30_000.0);
    row.applied_previous_years = Some(10_000.0);
    row.applied_current_year = Some(15_000.0);
    d.set_amount(S::PartIx, 1, SP, Some(498_999.0));
    d.recompute();
    d
}

#[test]
fn form_1701_joint_sample_matches_the_official_page() {
    let payload = sample_1701_joint()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy joint 1701 must validate: {errors:?}"));
    let dir = samples_dir();
    let stem = "1701-122025-joint";
    let encrypted = compress_and_encrypt(payload.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::write(dir.join(format!("{stem}.plain.xml")), &payload).unwrap();
        std::fs::write(dir.join(format!("{stem}.iaf.xml")), &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{stem}: differs from the official saveEncryptedProfile() output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, "3c5a13146fee709de65020aa6f25c6670aa8e419ed6d850a5fe404f6551d1eeb",
        "{stem}: Encrypt.exe hash"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{stem}.plain.xml"))).unwrap(),
        payload
    );
}

/// Our plaintext against the official page. The 1701 layout names the RDO
/// selects differently from the page (see `form_1701_official`), so this
/// sample is compared directly instead of through `check_sample`.
#[test]
fn form_1701_sample_payload_matches_the_official_page() {
    let payload = sample_1701()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1701 must validate: {errors:?}"));
    let dir = samples_dir();
    let stem = "1701-122025";
    let encrypted = compress_and_encrypt(payload.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::write(dir.join(format!("{stem}.plain.xml")), &payload).unwrap();
        std::fs::write(dir.join(format!("{stem}.iaf.xml")), &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{stem}: differs from the official saveEncryptedProfile() output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest, "09dc705424c7359b08bf32b77f3eb9fe68443920ac98db2bd24b48b63725cf9a",
        "{stem}: Encrypt.exe hash"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{stem}.plain.xml"))).unwrap(),
        payload
    );
}
