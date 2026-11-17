//! The Form library's rules (`nsh/screens/library/rules.ts`): the search text, the frequency
//! choices, and which forms a scope, a frequency and a search show. Pure — `rules.test.ts` is
//! mirrored in the tests below.

use std::collections::HashSet;

use crate::types::ScreenFormIndexEntry;

/// The frequency choices, in the old order (FREQS).
pub const FREQS: [&str; 5] = ["All", "Monthly", "Quarterly", "Annual", "As needed"];

/// "owned": the forms the selected taxpayer files in the selected year; "all": every form
/// (FormScope).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormScope {
    Owned,
    All,
}

/// What a search reads of a form: its code with and without dashes, and its title; "VAT" also
/// finds the Value-Added forms (searchText).
pub fn search_text(f: &ScreenFormIndexEntry) -> String {
    let vat = if f.t.to_lowercase().contains("value-added") {
        " vat"
    } else {
        ""
    };
    format!("{} {} {}{}", f.c, f.c.replace('-', ""), f.t, vat).to_lowercase()
}

/// A frequency choice from a control's value; anything that is not one of FREQS is "All"
/// (asFrequency).
pub fn as_frequency(value: &str) -> &'static str {
    FREQS.iter().copied().find(|f| *f == value).unwrap_or("All")
}

/// The three things the library filters by (FormFilter).
#[derive(Debug, Clone)]
pub struct FormFilter<'a> {
    pub scope: FormScope,
    /// The codes of the forms the selected taxpayer files this year.
    pub owned: &'a HashSet<String>,
    pub freq: &'static str,
    /// What the search box holds; trimmed and lower-cased here.
    pub query: &'a str,
}

/// The forms a filter shows, in the index's order (matchingForms).
pub fn matching_forms(forms: &[ScreenFormIndexEntry], f: &FormFilter) -> Vec<ScreenFormIndexEntry> {
    let needle = f.query.trim().to_lowercase();
    forms
        .iter()
        .filter(|form| {
            (f.scope == FormScope::All || f.owned.contains(&form.c))
                && (f.freq == "All" || form.freq == f.freq)
                && (needle.is_empty() || search_text(form).contains(&needle))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(c: &str, t: &str, freq: &str) -> ScreenFormIndexEntry {
        ScreenFormIndexEntry {
            c: c.to_owned(),
            t: t.to_owned(),
            freq: freq.to_owned(),
            ..Default::default()
        }
    }

    fn forms() -> Vec<ScreenFormIndexEntry> {
        vec![
            form(
                "1601C",
                "Monthly Remittance Return of Income Taxes Withheld on Compensation",
                "Monthly",
            ),
            form("2550Q", "Quarterly Value-Added Tax Return", "Quarterly"),
            form("2551Q", "Quarterly Percentage Tax Return", "Quarterly"),
            form("1702-RT", "Annual Income Tax Return", "Annual"),
            form("0605", "Payment Form", "As needed"),
        ]
    }

    fn owned() -> HashSet<String> {
        ["1601C".to_owned(), "2551Q".to_owned()]
            .into_iter()
            .collect()
    }

    fn codes(forms: &[ScreenFormIndexEntry], f: &FormFilter) -> Vec<String> {
        matching_forms(forms, f)
            .iter()
            .map(|x| x.c.clone())
            .collect()
    }

    #[test]
    fn frequency_choices_and_unknown_is_all() {
        assert_eq!(
            FREQS,
            ["All", "Monthly", "Quarterly", "Annual", "As needed"]
        );
        assert_eq!(as_frequency("Annual"), "Annual");
        assert_eq!(as_frequency("Weekly"), "All");
    }

    #[test]
    fn search_text_reads_code_title_and_vat() {
        let f = forms();
        assert_eq!(
            search_text(&f[3]),
            "1702-rt 1702rt annual income tax return"
        );
        assert_eq!(
            search_text(&f[1]),
            "2550q 2550q quarterly value-added tax return vat"
        );
        assert!(!search_text(&f[2]).contains("vat"));
    }

    #[test]
    fn scope_owned_vs_all() {
        let f = forms();
        let o = owned();
        let base = FormFilter {
            scope: FormScope::All,
            owned: &o,
            freq: "All",
            query: "",
        };
        assert_eq!(
            codes(
                &f,
                &FormFilter {
                    scope: FormScope::Owned,
                    ..base.clone()
                }
            ),
            vec!["1601C", "2551Q"]
        );
        assert_eq!(matching_forms(&f, &base).len(), 5);
    }

    #[test]
    fn frequency_narrows_within_scope() {
        let f = forms();
        let o = owned();
        let base = FormFilter {
            scope: FormScope::All,
            owned: &o,
            freq: "All",
            query: "",
        };
        assert_eq!(
            codes(
                &f,
                &FormFilter {
                    freq: "Quarterly",
                    ..base.clone()
                }
            ),
            vec!["2550Q", "2551Q"]
        );
        assert_eq!(
            codes(
                &f,
                &FormFilter {
                    scope: FormScope::Owned,
                    freq: "Quarterly",
                    ..base.clone()
                }
            ),
            vec!["2551Q"]
        );
        assert!(
            codes(
                &f,
                &FormFilter {
                    scope: FormScope::Owned,
                    freq: "Annual",
                    ..base
                }
            )
            .is_empty()
        );
    }

    #[test]
    fn search_trims_ignores_case_and_vat() {
        let f = forms();
        let o = owned();
        let base = FormFilter {
            scope: FormScope::All,
            owned: &o,
            freq: "All",
            query: "",
        };
        let q = |query: &str| {
            codes(
                &f,
                &FormFilter {
                    query,
                    ..base.clone()
                },
            )
        };
        assert_eq!(q("  1702 "), vec!["1702-RT"]);
        assert_eq!(q("1702rt"), vec!["1702-RT"]);
        assert_eq!(q("ANNUAL income"), vec!["1702-RT"]);
        assert_eq!(q("vat"), vec!["2550Q"]);
        assert!(q("zzz").is_empty());
        assert_eq!(q("   ").len(), 5);
    }

    #[test]
    fn scope_freq_and_search_together() {
        let f = forms();
        let o = owned();
        let base = FormFilter {
            scope: FormScope::Owned,
            owned: &o,
            freq: "Monthly",
            query: "",
        };
        assert_eq!(
            codes(
                &f,
                &FormFilter {
                    query: "remittance",
                    ..base.clone()
                }
            ),
            vec!["1601C"]
        );
        assert!(
            codes(
                &f,
                &FormFilter {
                    query: "vat",
                    ..base
                }
            )
            .is_empty()
        );
    }
}
