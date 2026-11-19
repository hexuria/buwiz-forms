//! B17 (form-fields PR 1 handoff 2): a code outside a select's list is
//! refused the same way on every form — the page judges it before the
//! host is asked, and the host's fill path judges it before any binding
//! arm. On a blank seed a select whose list is still empty (a province
//! waiting on its region) has no list to judge by and is not exercised
//! here.

use super::*;

/// Every `sel` box of every form whose list is known on a blank seed:
/// "ZZ" is never a listed code, so the write is refused — a gate's
/// reason where another answer keeps the box shut, the list refusal
/// everywhere else; no form accepts it.
#[test]
fn every_sel_box_refuses_a_code_off_its_list() {
    setup();
    let mut seen = 0;
    let mut unjudged = 0;
    let mut accepted: Vec<String> = Vec::new();
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let opened = open(code, page_period_of(code));
        for (key, t, _) in bound_page_types(code) {
            if t != "sel" || key.contains('{') {
                continue;
            }
            if opened["locks"][&key].as_str().is_some() {
                continue;
            }
            // The list the host judges this box's writes by on the blank
            // seed (B17): a pick list on a free binding does not judge.
            let known = catalogs::sel_judged(kind, &key, &opened["draft"])
                .is_some_and(|list| !list.is_empty());
            if !known {
                unjudged += 1;
                continue;
            }
            seen += 1;
            let filled = fill(code, &opened["draft"], json!({ &key: "ZZ" }));
            let Some(reason) = filled["rejected"][&key].as_str() else {
                accepted.push(format!("{code} {key}"));
                continue;
            };
            *reasons.entry(reason.to_string()).or_default() += 1;
            // The list refusal everywhere a list is known; a box another
            // answer keeps shut answers with its own gate instead, and a
            // select with a dedicated catalog writer (an ATC pick) names
            // its own list or code set.
            assert!(
                reason.contains("listed choices")
                    || reason.contains("must be one of")
                    || reason.contains("list")
                    || reason.contains("ATC")
                    || reason.starts_with("Needs ")
                    || rows_annual_individual::is_gate_refusal(code, reason)
                    || rows_income_q::is_gate_refusal(reason)
                    || rows_withholding::is_gate_refusal(reason),
                "{code} {key}: {reason}"
            );
        }
    }
    eprintln!("sel boxes judged: {seen}, unjudged (empty list): {unjudged}");
    eprintln!("sel refusal reasons: {reasons:#?}");
    assert!(seen > 100, "{seen} sel boxes exercised");
    assert!(
        accepted.is_empty(),
        "sel boxes that took ZZ:\n{}",
        accepted.join("\n")
    );
}
