//! Ñ-preserving name comparison (PUA spec §6.3: `PEÑA ≠ PENA`).

use pua_text::{Fold, NormalizeConfig, PunctRuns, normalize};

/// Case-folds a name with Unicode lowercase (keeps Ñ as a letter).
pub fn fold_name(name: &str) -> String {
    let config = NormalizeConfig {
        fold: Fold::UnicodeLower,
        punct_runs: PunctRuns::Keep,
    };
    match normalize(name, config) {
        Ok(n) => n.canonical().to_owned(),
        Err(_) => name.to_lowercase(),
    }
}

/// Whether two names are equal after Ñ-preserving fold.
pub fn names_equal(a: &str, b: &str) -> bool {
    fold_name(a) == fold_name(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pena_with_tilde_is_not_pena() {
        assert_ne!(fold_name("PEÑA"), fold_name("PENA"));
        assert!(!names_equal("PEÑA", "PENA"));
        assert!(names_equal("PEÑA", "peña"));
        assert!(names_equal("Ñiño", "ñiño"));
    }
}
