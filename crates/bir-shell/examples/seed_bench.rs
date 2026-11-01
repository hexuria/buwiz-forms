//! Seeds a throwaway database for `scripts/bench-shell-scroll.sh`: the demo
//! taxpayer Juana Santos (TIN 000-123-453-00000) with 1701, 1601C, 2550Q and
//! 1702RT for 2026. Refuses to run without `EBIR_TEST_ENV` and a
//! `BIR_DATABASE_PATH`, so it never touches a real database.

use bir_core::profile::TaxpayerType;
use bir_core::tax_profile::{BranchFacts, NewTaxpayer};

fn main() {
    assert!(
        std::env::var_os("EBIR_TEST_ENV").is_some(),
        "test databases only"
    );
    assert!(
        std::env::var_os("BIR_DATABASE_PATH").is_some(),
        "set a scratch BIR_DATABASE_PATH"
    );
    let (db, path, _) = bir_core::db::open_live_database().expect("the scratch database opens");
    let root = "000123453";
    if db.get_taxpayer(root).ok().flatten().is_none() {
        db.create_taxpayer(NewTaxpayer {
            tin_root: root.into(),
            name: "Juana Santos".into(),
            taxpayer_type: TaxpayerType::Individual,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "040".into(),
                registered_address: "1 Example Street, Pasig".into(),
                zip_code: "1600".into(),
                phone: "0280000000".into(),
                email: "juana@example.com".into(),
            },
        })
        .expect("taxpayer");
    }
    db.set_yearly_forms(
        "00012345300000",
        2026,
        &["1701", "1601C", "2550Q", "1702RT"],
    )
    .expect("forms");
    println!("seeded {}", path.display());
}
