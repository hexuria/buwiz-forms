#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn field_order_invariant_layout() {
    let a = FormRecord::new([("tin", "1"), ("name", "PEÑA"), ("rdo", "RDO-39")]);
    let b = FormRecord::new([("rdo", "RDO-39"), ("tin", "1"), ("name", "PEÑA")]);
    assert_eq!(layout_fingerprint(&a), layout_fingerprint(&b));
    assert_eq!(a.fields().len(), 3);
}

#[test]
fn catalog_sample_is_non_authoritative_but_matches() {
    let c = Catalog::load().unwrap();
    let hit = c.longest_exact("2303").unwrap();
    assert_eq!(hit.code(), "2303");
    assert_eq!(hit.label(), "COR");
    assert!(c.longest_exact("9999").is_none());
    let _ = data_version(&c);
}

#[test]
fn suggestion_reason_never_contains_valid() {
    let s = suggest_tin("000-000-000-00000").unwrap();
    assert!(!format!("{s:?}").to_ascii_lowercase().contains("valid"));
}

#[test]
fn mutants_survivors_pinned() {
    // Layout fingerprint is a real blake3 WL digest, not a constant fill.
    let a = FormRecord::new([("a", "1"), ("b", "2")]);
    let b = FormRecord::new([("a", "1")]);
    let fa = layout_fingerprint(&a);
    let fb = layout_fingerprint(&b);
    assert_ne!(fa, [0u8; 32]);
    assert_ne!(fa, [1u8; 32]);
    assert_ne!(fa, fb);

    let c = Catalog::load().unwrap();
    assert_eq!(CatalogError::Invalid("x".into()).to_string(), "catalog: x");
    let hit = c.get("2303").expect("get");
    assert_eq!(hit.code(), "2303");
    assert_eq!(hit.label(), "COR");
    assert!(c.get("nope").is_none());
    // get compares exact code (==); a flipped != would match the first non-equal entry.
    assert_ne!(
        c.get("2303").map(|h| h.code().to_owned()),
        c.get("039").map(|h| h.code().to_owned())
    );

    let fp = c.fingerprint();
    assert_eq!(fp.len(), 32);
    assert_ne!(fp, vec![]);
    assert_ne!(fp, vec![0]);
    assert_ne!(fp, vec![1]);
    assert_ne!(fp, vec![0u8; 32]);

    assert_eq!(SuggestionKind::TinFormat.name(), "tin_format");
    assert_eq!(SuggestionKind::Catalog.name(), "catalog");

    let sep = suggest_tin("123-456").unwrap();
    assert_eq!(sep.suggested(), "123456");
    assert!(sep.reason().contains("stripped separators"));
    assert!(!sep.reason().contains("valid"));
    assert_eq!(sep.kind().name(), "tin_format");

    let plain = suggest_tin("123456").unwrap();
    assert!(!plain.reason().contains("stripped"));
    assert!(!plain.reason().contains("repaired"));

    let ocr = suggest_tin("000O").unwrap(); // O → 0
    assert!(ocr.reason().contains("repaired"));
    assert!(!ocr.reason().contains("stripped"));

    let both = suggest_tin("000-000-000-0000O").unwrap();
    assert!(both.reason().contains("stripped separators"));
    assert!(both.reason().contains("repaired"));
    assert!(both.reason().contains(" and"));

    assert_eq!(TinError::Empty.to_string(), "TIN field is empty");
    assert_eq!(
        TinError::NotNumeric { at: 3, ch: 'x' }.to_string(),
        "TIN has non-digit 'x' at byte 3"
    );
    assert!(matches!(suggest_tin("---"), Err(TinError::Empty)));
    assert!(matches!(
        suggest_tin("12x"),
        Err(TinError::NotNumeric { ch: 'x', .. })
    ));
}
