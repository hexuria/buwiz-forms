//! First-run seed: an empty database opens on a demo taxpayer with a forms setup applied, so a
//! restart never costs the user their data entry again. Runs once per process at boot; on an
//! existing database it only tops up the demo Forms Set (a persistent `BIR_DATABASE_PATH` keeps
//! whatever is entered after this; a scratch one simply reseeds each run).

use chrono::Datelike;
use serde_json::{Value, json};

use crate::state::AppState;
use crate::store;

/// Juana Santos — the demo taxpayer the tests and screenshots name (`bir/testing.ts`'s DEMO).
const DEMO_ROOT: &str = "000123453";

/// Creates the demo taxpayer (unless present), finishes solo setup so no PIN gate appears, adds
/// this year and next, applies the built-in "VAT business" setup to both, then selects it all.
pub fn run(app: &mut AppState) {
    if store::taxpayers().iter().any(|t| t.tin_root == DEMO_ROOT) {
        ensure_monthly_form();
        return;
    }
    let created = store::write(
        "create_taxpayer",
        vec![json!({
            "tin_root": "000-123-453",
            "name": "Juana Santos",
            "taxpayer_type": "Individual",
            "birth_date": Value::Null,
            "head_office": {
                "rdo_code": "039",
                "registered_address": "Quezon City",
                "zip_code": "1100",
                "phone": "09170000000",
                "email": "juana@example.com",
            },
        })],
    );
    if let Err(e) = created {
        eprintln!("seed: create_taxpayer failed: {e}");
        return;
    }
    // "Just me or my business" for the owner: no admin PIN, so nothing locks the app.
    let _ = store::write("setup_solo", vec![json!(DEMO_ROOT)]);

    let Some(tp) = store::taxpayers()
        .into_iter()
        .find(|t| t.tin_root == DEMO_ROOT)
    else {
        return;
    };
    let Some(branch) = tp
        .branches
        .iter()
        .find(|b| b.code == "00000")
        .or_else(|| tp.branches.first())
    else {
        return;
    };
    let tin = branch.tin.clone();

    let this_year = chrono::Local::now().year() as i64;
    for year in [this_year, this_year + 1] {
        // Already-there years answer an error; either way the year exists afterwards.
        let _ = store::write("add_tax_year", vec![json!(tin), json!(year), json!(true)]);
    }

    // The default forms: the built-in "VAT business" pack for an individual (2550Q, 1701Q, 1701),
    // or the first built-in pack the type offers.
    let setups = store::setups(Some("Individual"));
    let pick = setups
        .iter()
        .find(|s| s.id == "vat_business_individual" && s.built_in)
        .or_else(|| setups.iter().find(|s| s.built_in));
    if let Some(setup) = pick {
        for year in [this_year, this_year + 1] {
            let _ = store::write(
                "apply_setup",
                vec![json!(tin), json!(year), json!(setup.id), json!(false)],
            );
        }
    }

    ensure_monthly_form();
    store::bump(None);
    app.select_taxpayer(DEMO_ROOT, false, None);
    app.select_year(this_year);
}

/// 1601C (monthly) on the demo taxpayer's this-year and next-year Forms Sets, so the Calendar's
/// Monthly filter and month chips have rows to show. Idempotent; also runs on an existing seed.
fn ensure_monthly_form() {
    let Some(tp) = store::taxpayers()
        .into_iter()
        .find(|t| t.tin_root == DEMO_ROOT)
    else {
        return;
    };
    let Some(branch) = tp
        .branches
        .iter()
        .find(|b| b.code == "00000")
        .or_else(|| tp.branches.first())
    else {
        return;
    };
    let tin = branch.tin.clone();
    let this_year = chrono::Local::now().year() as i64;
    for year in [this_year, this_year + 1] {
        let mut codes = store::yearly_forms(&tin, year);
        if !codes.iter().any(|c| c == "1601C") {
            codes.push("1601C".to_owned());
            let _ = store::write(
                "set_yearly_forms",
                vec![json!(tin), json!(year), json!(codes)],
            );
        }
    }
}
