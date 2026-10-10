#![allow(clippy::unwrap_used)]

use super::*;

/// The three A §5.3 cases: leading spaces, double space + multibyte Ñ, clean prefix.
#[test]
fn three_section_53_cases_extract_correctly() {
    let leading = "  TIN: 123-456-789-000";
    let fields = extract(leading);
    let tin = fields.iter().find(|f| f.label() == "TIN").expect("tin");
    assert_eq!(tin.value_text(leading), "123-456-789-000");
    assert_eq!(tin.match_type(), MatchType::Exact);

    let multi = "PEÑA TRADING  TRADE NAME: PEÑA STORE";
    let fields = extract(multi);
    let trade = fields
        .iter()
        .find(|f| f.label() == "TRADE NAME")
        .expect("trade");
    assert_eq!(trade.value_text(multi), "PEÑA STORE");

    let clean = "TIN:   123-456-789-000";
    let fields = extract(clean);
    let tin = fields.iter().find(|f| f.label() == "TIN").expect("tin");
    assert_eq!(tin.value_text(clean), "123-456-789-000");
}

#[test]
fn label_diacritics_not_folded() {
    // A label written with Ñ must not match a vocabulary label without it.
    let page = "TRADE ÑAME: PEÑA STORE";
    let fields = extract(page);
    assert!(
        fields.iter().all(|f| f.label() != "TRADE NAME"),
        "folded diacritic must not match: {fields:?}"
    );
}

#[test]
fn confidence_by_match_type() {
    assert_eq!(MatchType::Exact.name(), "exact");
    assert_eq!(MatchType::CaseFolded.name(), "case_folded");
    assert_eq!(MatchType::NextLine.name(), "next_line");
    assert!(MatchType::Exact.confidence() > MatchType::CaseFolded.confidence());
    assert!(MatchType::CaseFolded.confidence() > MatchType::NextLine.confidence());
}

#[test]
fn layout_fingerprint_stable_under_renumbering() {
    let page = "TIN: 1\nTRADE NAME: X\nRDO: 39\n";
    let a = extract(page);
    let mut b = a.clone();
    b.reverse();
    assert_eq!(layout_fingerprint(&a), layout_fingerprint(&b));
}

#[test]
fn mutants_survivors_pinned() {
    // Next-line value path (line_has_label_only).
    let page = "TIN\n123-456-789-000\n";
    let fields = extract(page);
    let tin = fields.iter().find(|f| f.label() == "TIN").expect("tin");
    assert_eq!(tin.value_text(page), "123-456-789-000");
    assert_eq!(tin.match_type(), MatchType::NextLine);
    assert!(line_has_label_only("TIN", "TIN"));
    // A trailing colon on the line means "not label-only" (value expected same-line).
    assert!(!line_has_label_only("TIN:", "TIN"));
    assert!(!line_has_label_only("TIN: 1", "TIN"));
    assert!(!line_has_label_only("NAME", "TIN"));

    // Case-folded label match (eq_label / chars_eq); diacritics still matter.
    assert!(eq_label("tin", "TIN", true));
    assert!(!eq_label("tin", "TIN", false));
    assert!(!eq_label("PEÑA", "PENA", true));
    assert!(chars_eq('A', 'a', true));
    assert!(!chars_eq('Ñ', 'N', true));
    assert!(!chars_eq('A', 'b', true));

    let case = "tin: 999";
    let f = extract(case);
    let tin = f.iter().find(|x| x.label() == "TIN").expect("tin");
    assert_eq!(tin.match_type(), MatchType::CaseFolded);
    assert_eq!(tin.value_text(case), "999");

    // Fingerprint not a constant fill; differs by label set.
    let a = extract("TIN: 1\nTRADE NAME: X\n");
    let b = extract("TIN: 1\n");
    let fa = layout_fingerprint(&a);
    let fb = layout_fingerprint(&b);
    assert_ne!(fa, [0u8; 32]);
    assert_ne!(fa, [1u8; 32]);
    assert_ne!(fa, fb);
}
