use crate::{check_sample, dummy_profile};
use bir_core::forms::form_1801::{
    Form1801Business, Form1801Draft, Form1801Frequency, Form1801Particular, Form1801RealProperty,
    Form1801Shares, Form1801Split,
};

fn split(exclusive: f64, conjugal: f64) -> Form1801Split {
    Form1801Split {
        exclusive,
        conjugal,
    }
}

fn sample_1801() -> Form1801Draft {
    let mut d = Form1801Draft::new_from_profile(&dummy_profile("1801", "Estate"), 2025, 1);
    d.death_month = 6;
    d.death_day = 5;
    d.death_year = 2025;
    d.number_of_attached_sheets = 1;
    d.non_resident_alien = Some(false);
    d.administrator_name = "Maria Sample-Executor".into();
    d.administrator_tin = "111-222-333".into();
    d.administrator_branch_code = "00000".into();
    d.extension_to_file = true;
    d.set_installment_granted(true);
    d.installment_frequency = Some(Form1801Frequency::Quarterly);
    d.real_properties = vec![Form1801RealProperty {
        title_number: "T-12345".into(),
        tax_declaration_number: "TD-001".into(),
        location: "Quezon City".into(),
        lot_or_improvement: "Lot".into(),
        area: "200".into(),
        classification: "RR".into(),
        fmv_per_tax_declaration: "1000000".into(),
        fmv_per_zonal_value: "1200000".into(),
        value: split(1_200_000.005, 0.0),
    }];
    d.family_homes = vec![Form1801RealProperty {
        title_number: "T-67890".into(),
        tax_declaration_number: "TD-002".into(),
        location: "Pasig City".into(),
        area: "150".into(),
        classification: "RC".into(),
        fmv_per_tax_declaration: "1800000".into(),
        fmv_per_zonal_value: "2000000".into(),
        value: split(0.0, 2_000_000.0),
        ..Default::default()
    }];
    d.shares = vec![Form1801Shares {
        corporation: "Sample Corp".into(),
        listing: "Not Listed".into(),
        certificate_number: "C-100".into(),
        number_of_shares: "1000".into(),
        value_per_share: "300.00".into(),
        value: split(300_000.55, 0.0),
    }];
    d.other_personal = vec![
        Form1801Particular {
            particulars: "Car".into(),
            value: split(50_000.0, 0.0),
        },
        Form1801Particular {
            particulars: "Jewelry".into(),
            value: split(0.0, 25_000.125),
        },
    ];
    d.taxable_transfers = vec![Form1801Particular {
        particulars: "Cash in bank, Sample Bank 0001".into(),
        value: split(10_000.0, 0.0),
    }];
    d.business_interests = vec![Form1801Business {
        name: "Sample Store".into(),
        address: "Makati City".into(),
        rdo_code: "040".into(),
        value: split(40_000.0, 0.0),
    }];
    d.ordinary_deductions.claims_against_estate = split(100_000.0, 0.0);
    d.ordinary_deductions.settlement_losses = split(0.0, 5_000.0);
    d.standard_deduction = 5_000_000.0;
    d.family_home_deduction = 1_000_000.0;
    d.installment_year = "2026".into();
    d.surcharge = 25.5;
    d.recompute();
    d
}

#[test]
fn form_1801_sample_payload_is_current() {
    let draft = sample_1801();
    let payload = draft
        .to_bir_xml_payload()
        .unwrap_or_else(|errors| panic!("dummy 1801 must validate: {errors:?}"));
    check_sample(
        "1801-06052025",
        "1801-v2018",
        &payload,
        "a4c75411bcd1c0ededce0507e3b07f163f820f4964a3b81e629ac4d3397fbd37",
    );
}
