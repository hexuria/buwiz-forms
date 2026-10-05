//! Property tests for bir-fields invariances.
#![allow(clippy::unwrap_used)]
use bir_suggest::{FormRecord, fold_name, layout_fingerprint, suggest_tin};
use proptest::prelude::*;
use unicode_normalization::UnicodeNormalization as _;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn tin_separator_and_case_of_ocr_letters(
        digits in "[0-9]{9,14}",
        seps in proptest::collection::vec(prop::sample::select(vec!['-', ' ', '.']), 0..4)
    ) {
        let mut field = String::new();
        for (i, ch) in digits.chars().enumerate() {
            field.push(ch);
            if let Some(s) = seps.get(i % (seps.len().max(1))).copied()
                && i + 1 < digits.chars().count()
                && !seps.is_empty()
                && i % 3 == 2
            {
                field.push(s);
            }
        }
        let a = suggest_tin(&digits);
        let b = suggest_tin(&field);
        prop_assert_eq!(a.ok().map(|s| s.suggested().to_owned()), b.ok().map(|s| s.suggested().to_owned()));
    }

    #[test]
    fn name_nfc_nfd_preserve_tilde_distinction(base in "[A-Z]{2,6}") {
        let with = format!("{base}Ñ");
        let without = format!("{base}N");
        prop_assert_ne!(fold_name(&with), fold_name(&without));
        let nfd: String = with.nfd().collect();
        prop_assert_eq!(fold_name(&with), fold_name(&nfd));
    }

    #[test]
    fn layout_ignores_field_insertion_order(
        keys in proptest::collection::btree_set("[a-z]{2,5}", 2..6)
    ) {
        let fields: Vec<_> = keys.iter().map(|k| (k.as_str(), "x")).collect();
        let mut rev = fields.clone();
        rev.reverse();
        prop_assert_eq!(
            layout_fingerprint(&FormRecord::new(fields)),
            layout_fingerprint(&FormRecord::new(rev))
        );
    }
}
