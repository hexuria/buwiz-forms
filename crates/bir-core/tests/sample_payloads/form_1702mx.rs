use crate::{dummy_profile, samples_dir};
use bir_core::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use bir_core::forms::form_1702mx::{
    Form1702MXDeductionMethod, Form1702MXDraft, PercentInput, WholePeso, WholePesoInput,
};
use sha2::{Digest, Sha256};

fn peso(value: i64) -> WholePesoInput {
    WholePesoInput::from_amount(WholePeso(value))
}

/// A calendar 2025 return with exempt, special-rate and regular income,
/// itemized deductions, a NOLCO row, MCIT carried over and tax credits.
pub(crate) fn sample_1702mx() -> Form1702MXDraft {
    let mut d =
        Form1702MXDraft::new_from_profile(&dummy_profile("1702MXv2018C", "Corporation"), 2025);
    d.incorporation_date = "01/15/2015".into();
    d.deduction_method = Form1702MXDeductionMethod::Itemized;
    d.relief_details.investment_promotion_agency = ["Peza".into(), "Boi".into(), String::new()];
    d.relief_details.legal_basis = ["Ra 7916".into(), "Eo 226".into(), String::new()];
    d.relief_details.registered_activity = ["Export mfg".into(), "Tourism".into(), String::new()];
    d.relief_details.effectivity_from = ["01/01/2020".into(), "01/01/2021".into(), String::new()];
    d.relief_details.effectivity_until = ["12/31/2026".into(), "12/31/2027".into(), String::new()];
    d.relief_basis.special_tax_rate = PercentInput::from_hundredths(500);

    let s2 = &mut d.schedule_2.items;
    s2[0].exempt = peso(1_000_000);
    s2[0].special = peso(2_000_000);
    s2[0].regular = peso(5_000_000);
    s2[1].exempt = peso(100_000);
    s2[1].special = peso(500_000);
    s2[1].regular = peso(2_000_000);
    s2[3].regular = peso(25_000);
    s2[5].regular = peso(150_000);
    d.schedule_2.item_14_special_rate = PercentInput::from_hundredths(500);
    d.schedule_2.item_14_regular_rate = PercentInput::from_hundredths(2_500);
    s2[15].special = peso(1_000);
    s2[17].regular = peso(63_000);

    let s5 = &mut d.schedule_5.amounts;
    s5[0].exempt = peso(50_000);
    s5[0].special = peso(100_000);
    s5[0].regular = peso(800_000);
    s5[2].regular = peso(120_500);
    s5[19].regular = peso(10_000);
    d.schedule_5.other_descriptions_17d_to_17i[0] = "Training costs".into();

    d.schedule_6.rows[0].description = "Pension fund".into();
    d.schedule_6.rows[0].legal_basis = "Ra 4917".into();
    d.schedule_6.rows[0].amounts.exempt = peso(5_000);

    let row = &mut d.schedule_7_1.rows[0];
    row.year_incurred = "2022".into();
    row.amount = peso(300_000);
    row.applied_previous_years = peso(100_000);
    row.applied_current_year = peso(50_000);

    let mcit = &mut d.schedule_9.rows[0];
    mcit.year = "2023".into();
    mcit.normal_income_tax = peso(10_000);
    mcit.mcit = peso(30_000);
    mcit.applied_current_year = peso(5_000);

    let s3 = &mut d.schedule_3.items_20_to_33;
    s3[0].regular = peso(50_000);
    s3[2].special = peso(3_000);
    s3[4].regular = peso(20_000);

    d.schedule_4.items[0].exempt = peso(90_000);
    d.schedule_4.items[0].special = peso(30_000);
    d.schedule_4.items[1].exempt = peso(1_000);

    d.part_ii.item_17_surcharge = peso(1_000);
    d.recompute();

    // Schedule 10 reconciles to Schedule 2 Item 13 per column.
    for column in 0..3 {
        let value = match column {
            0 => d.schedule_2.items[12].exempt.clone(),
            1 => d.schedule_2.items[12].special.clone(),
            _ => d.schedule_2.items[12].regular.clone(),
        };
        let target = &mut d.schedule_10.items[0];
        match column {
            0 => target.exempt = value,
            1 => target.special = value,
            _ => target.regular = value,
        }
    }
    d.recompute();
    d
}

/// `check_sample` reads the plaintext back through the official layout; the
/// 1702-MX plaintext also carries the two selects the page builds at run
/// time (`drpPg1Pt1I7RDO`, `drpPg3Sc1I11CB`), which the generated layout does
/// not list, so the same checks are made here directly.
#[test]
fn form_1702mx_sample_payload_is_current() {
    let payload = sample_1702mx()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1702MX must validate: {errors:?}"));
    let stem = "1702MX-1225";
    let official_encrypt_sha256 =
        "7cb99121d8a7392445bbef24670b31e6e50287db9e2b50a206064a73c00fbb1c";
    let encrypted = compress_and_encrypt(payload.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
    assert_eq!(
        decrypt_and_decompress(&encrypted, BIR_IAF_PASSPHRASE).unwrap(),
        payload.as_bytes()
    );
    let dir = samples_dir();
    let updating = std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some();
    if updating {
        std::fs::write(dir.join(format!("{stem}.plain.xml")), &payload).unwrap();
        std::fs::write(dir.join(format!("{stem}.iaf.xml")), &encrypted).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{stem}: differs from the official saveXMLsubmit() output"
    );
    let digest: String = Sha256::digest(&encrypted)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(digest, official_encrypt_sha256, "{stem}: Encrypt.exe hash");
    if updating {
        return;
    }
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{stem}.plain.xml"))).unwrap(),
        payload
    );
    assert_eq!(
        std::fs::read(dir.join(format!("{stem}.iaf.xml"))).unwrap(),
        encrypted
    );
}

/// A fiscal, amended return under the optional standard deduction with a
/// special-rate net operating loss, another ATC and an overpayment carried
/// over.
pub(crate) fn sample_1702mx_fiscal() -> Form1702MXDraft {
    use bir_core::forms::form_1702mx::{Form1702MXFilingBasis, Form1702MXOverpaymentDisposition};
    let mut d =
        Form1702MXDraft::new_from_profile(&dummy_profile("1702MXv2018C", "Corporation"), 2025);
    d.filing_basis = Form1702MXFilingBasis::Fiscal;
    d.month = 6;
    d.is_amended = true;
    d.incorporation_date = "03/10/2023".into();
    d.atc.other_selected = true;
    d.atc.other_code = "IC030".into();
    d.deduction_method = Form1702MXDeductionMethod::OptionalStandard;
    d.relief_details.investment_promotion_agency[1] = "Boi".into();
    d.relief_details.legal_basis[1] = "Eo 226".into();
    d.relief_details.registered_activity[1] = "Tourism".into();
    d.relief_details.effectivity_from[1] = "01/01/2021".into();
    d.relief_details.effectivity_until[1] = "12/31/2027".into();
    d.relief_basis.special_tax_rate = PercentInput::from_hundredths(750);
    let s2 = &mut d.schedule_2.items;
    s2[0].special = peso(1_000_000);
    s2[1].special = peso(300_000);
    s2[0].regular = peso(2_000_000);
    s2[1].regular = peso(1_500_000);
    d.schedule_2.item_14_special_rate = PercentInput::from_hundredths(750);
    d.schedule_2.item_14_regular_rate = PercentInput::from_hundredths(2_500);
    d.schedule_5.amounts[0].special = peso(900_000);
    let s3 = &mut d.schedule_3.items_20_to_33;
    s3[0].special = peso(100_000);
    s3[0].regular = peso(100_000);
    s3[7].regular = peso(10_000);
    d.recompute();
    d.schedule_10.items[0].special = d.schedule_2.items[12].special.clone();
    d.schedule_10.items[0].regular = d.schedule_2.items[12].regular.clone();
    d.part_ii.overpayment_disposition = Some(Form1702MXOverpaymentDisposition::CarryOver);
    d.recompute();
    d
}

/// Compared with the official page only (see the sample above).
#[test]
fn form_1702mx_fiscal_sample_matches_the_official_page() {
    let payload = sample_1702mx_fiscal()
        .to_official_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy fiscal 1702MX must validate: {errors:?}"));
    let dir = samples_dir();
    let stem = "1702MX-0625-fiscal";
    if std::env::var_os("UPDATE_SAMPLE_PAYLOADS").is_some() {
        std::fs::write(dir.join(format!("{stem}.plain.xml")), &payload).unwrap();
    }
    let official = std::fs::read_to_string(dir.join(format!("{stem}.official.xml"))).unwrap();
    assert_eq!(
        payload, official,
        "{stem}: differs from the official saveXMLsubmit() output"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(format!("{stem}.plain.xml"))).unwrap(),
        payload
    );
}
