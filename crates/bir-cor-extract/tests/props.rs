//! Invariances for ocr-labels (PUA spec §6.4).
#![allow(clippy::unwrap_used)]
use bir_cor_extract::extract;
use proptest::prelude::*;
use unicode_normalization::UnicodeNormalization as _;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn leading_spaces_do_not_shift_tin_value(
        spaces in 0usize..8
    ) {
        let value = "123-456-789-000";
        let page = format!("{:spaces$}TIN: {value}", "", spaces = spaces);
        let fields = extract(&page);
        let tin = fields.iter().find(|f| f.label() == "TIN").unwrap();
        prop_assert_eq!(tin.value_text(&page), value);
    }

    /// NFC↔NFD is a declared free move (§6.4): semantic values match after NFC.
    #[test]
    fn nfc_nfd_on_value_preserves_extraction(
        tilde in prop::bool::ANY
    ) {
        let name = if tilde { "PEÑA STORE" } else { "PENA STORE" };
        let page = format!("TRADE NAME: {name}");
        let nfd: String = page.nfd().collect();
        let a = extract(&page);
        let b = extract(&nfd);
        let nfc_vals = |fields: &[bir_cor_extract::Field], src: &str| {
            fields
                .iter()
                .map(|f| f.value_text(src).nfc().collect::<String>())
                .collect::<Vec<_>>()
        };
        prop_assert_eq!(nfc_vals(&a, &page), nfc_vals(&b, &nfd));
        prop_assert_eq!(
            a.iter().map(bir_cor_extract::Field::label).collect::<Vec<_>>(),
            b.iter().map(bir_cor_extract::Field::label).collect::<Vec<_>>()
        );
    }
}
