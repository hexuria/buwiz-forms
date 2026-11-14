//! The tax facts of one branch for one tax year (owner decisions 4, 5, 6).
//!
//! A record lives in the `year_facts` table, keyed by full TIN and tax
//! year, outside the profile JSON: an older build that re-saves a profile
//! cannot drop it. The head office (branch `00000`) owns the income type,
//! the 8% or graduated choice and the EOPT tier for every branch; each
//! branch owns its start date, line of business, government withholding
//! entity flag and usual ATC codes. Birth date and taxpayer type stay on
//! the taxpayer row.
//!
//! A year with no stored record reads as the branch's nearest earlier record
//! (a new year copies the last one), else as the flat profile fields the
//! record would have been filled from; its election comes from the flat
//! `tax_elections` entries of that year. Once a record exists it is the
//! truth: a fact it does not set is not set.
//!
//! [`overlay_year_facts`] lays the effective facts over a flat
//! [`TaxpayerProfile`], so the form prefills and rules that read the flat
//! fields read the year's facts without being rewritten.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile::{
    EoptTier, IncomeTaxElection, TaxClassification, TaxElectionHistory, TaxProfileVersionStatus,
    TaxpayerProfile,
};

use super::TaxProfileError;

/// One fact a return may read from the tax profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fact {
    BirthDate,
    TaxClassification,
    IncomeTaxElection,
    EoptTier,
    BusinessStartDate,
    LineOfBusiness,
    IsGovernmentWithholdingEntity,
    AtcCodes,
}

impl Fact {
    pub const ALL: [Self; 8] = [
        Self::BirthDate,
        Self::TaxClassification,
        Self::IncomeTaxElection,
        Self::EoptTier,
        Self::BusinessStartDate,
        Self::LineOfBusiness,
        Self::IsGovernmentWithholdingEntity,
        Self::AtcCodes,
    ];

    /// The fact's key, as the host and the record spell it.
    pub fn key(self) -> &'static str {
        match self {
            Self::BirthDate => "birth_date",
            Self::TaxClassification => "tax_classification",
            Self::IncomeTaxElection => "income_tax_election",
            Self::EoptTier => "eopt_tier",
            Self::BusinessStartDate => "business_start_date",
            Self::LineOfBusiness => "line_of_business",
            Self::IsGovernmentWithholdingEntity => "is_government_withholding_entity",
            Self::AtcCodes => "atc_codes",
        }
    }

    /// Which row holds the fact for a year. `None` for the birth date, which
    /// is the taxpayer's and not a year's.
    pub fn owner(self) -> Option<FactOwner> {
        match self {
            Self::BirthDate => None,
            Self::TaxClassification | Self::IncomeTaxElection | Self::EoptTier => {
                Some(FactOwner::HeadOffice)
            }
            Self::BusinessStartDate
            | Self::LineOfBusiness
            | Self::IsGovernmentWithholdingEntity
            | Self::AtcCodes => Some(FactOwner::Branch),
        }
    }
}

/// The row that holds a year fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactOwner {
    HeadOffice,
    Branch,
}

/// `year_facts.facts_json`: one branch's facts for one tax year.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct YearFacts {
    #[serde(default)]
    pub tax_classification: Option<TaxClassification>,
    #[serde(default)]
    pub income_tax_election: Option<IncomeTaxElection>,
    /// Where the election was made, e.g. "2551Q Q1" or "Tax profile".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub election_source: Option<String>,
    #[serde(default)]
    pub eopt_tier: Option<EoptTier>,
    #[serde(default)]
    pub business_start_date: Option<NaiveDate>,
    #[serde(default)]
    pub line_of_business: Option<String>,
    #[serde(default)]
    pub is_government_withholding_entity: Option<bool>,
    #[serde(default)]
    pub atc_codes: Vec<String>,
}

impl YearFacts {
    /// The facts a record takes from the flat profile fields. The election
    /// is not one of them: it is per year ([`flat_election`]).
    pub fn from_flat_profile(profile: &TaxpayerProfile) -> Self {
        Self {
            tax_classification: profile.tax_classification.clone(),
            income_tax_election: None,
            election_source: None,
            eopt_tier: profile.eopt_tier.clone(),
            business_start_date: profile.business_start_date,
            line_of_business: non_blank(&profile.line_of_business),
            is_government_withholding_entity: Some(profile.is_government_withholding_entity),
            atc_codes: profile.atc_codes.clone(),
        }
    }

    /// v23's record for `year`: the confirmed profile version that covers
    /// the year when exactly one does, else the flat profile fields. A
    /// version's blank value falls back to the flat field. No election.
    pub fn backfill(profile: &TaxpayerProfile, year: u16) -> Self {
        let mut facts = Self::from_flat_profile(profile);
        let segments = profile
            .resolve_tax_profile_for_year(year)
            .effective_segments;
        if let [version] = segments.as_slice()
            && version.status == TaxProfileVersionStatus::Confirmed
        {
            if version.tax_classification.is_some() {
                facts.tax_classification = version.tax_classification.clone();
            }
            if version.eopt_tier.is_some() {
                facts.eopt_tier = version.eopt_tier.clone();
            }
            if version.cor.registration_date.is_some() {
                facts.business_start_date = version.cor.registration_date;
            }
            if let Some(line) = non_blank(&version.cor.line_of_business_description) {
                facts.line_of_business = Some(line);
            }
            facts.is_government_withholding_entity = Some(
                version.is_government_withholding_entity
                    || profile.is_government_withholding_entity,
            );
        }
        facts
    }

    /// The record a later year starts from: every fact but the election,
    /// which is chosen each year.
    pub fn carried_to_next_year(&self) -> Self {
        Self {
            income_tax_election: None,
            election_source: None,
            ..self.clone()
        }
    }

    /// Set the keys `patch` names, only those of `owner`'s row.
    pub fn apply(&mut self, patch: &YearFactsPatch, owner: FactOwner) {
        let head = owner == FactOwner::HeadOffice;
        if head {
            if let Some(value) = &patch.tax_classification {
                self.tax_classification = value.clone();
            }
            if let Some(value) = &patch.income_tax_election {
                if self.income_tax_election != *value {
                    self.election_source = value.as_ref().map(|_| "Tax profile".to_string());
                }
                self.income_tax_election = value.clone();
            }
            if let Some(value) = &patch.eopt_tier {
                self.eopt_tier = value.clone();
            }
        } else {
            if let Some(value) = &patch.business_start_date {
                self.business_start_date = *value;
            }
            if let Some(value) = &patch.line_of_business {
                self.line_of_business = value.as_deref().and_then(non_blank);
            }
            if let Some(value) = &patch.is_government_withholding_entity {
                self.is_government_withholding_entity = *value;
            }
            if let Some(value) = &patch.atc_codes {
                self.atc_codes = value
                    .iter()
                    .map(|code| code.trim().to_ascii_uppercase())
                    .filter(|code| !code.is_empty())
                    .collect();
            }
        }
    }

    /// Whether `fact` has a value here. The birth date is never a year's.
    pub fn is_set(&self, fact: Fact) -> bool {
        match fact {
            Fact::BirthDate => false,
            Fact::TaxClassification => self.tax_classification.is_some(),
            Fact::IncomeTaxElection => self.income_tax_election.is_some(),
            Fact::EoptTier => self.eopt_tier.is_some(),
            Fact::BusinessStartDate => self.business_start_date.is_some(),
            Fact::LineOfBusiness => self.line_of_business.is_some(),
            Fact::IsGovernmentWithholdingEntity => self.is_government_withholding_entity.is_some(),
            Fact::AtcCodes => !self.atc_codes.is_empty(),
        }
    }
}

fn non_blank(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// The keys a `set_year_facts` call writes. `Some(None)` clears a fact;
/// `None` leaves it as it is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct YearFactsPatch {
    pub tax_classification: Option<Option<TaxClassification>>,
    pub income_tax_election: Option<Option<IncomeTaxElection>>,
    pub eopt_tier: Option<Option<EoptTier>>,
    pub business_start_date: Option<Option<NaiveDate>>,
    pub line_of_business: Option<Option<String>>,
    pub is_government_withholding_entity: Option<Option<bool>>,
    pub atc_codes: Option<Vec<String>>,
}

impl YearFactsPatch {
    /// A patch from a JSON object: only the keys present, `null` clears.
    /// Enum values use the spellings the profile already serialises
    /// (`"SelfEmployed"`, `"EightPercent"`, `"Micro"`).
    pub fn from_json(value: &Value) -> Result<Self, TaxProfileError> {
        let object = value
            .as_object()
            .ok_or_else(|| TaxProfileError::Invalid("The facts must be an object".to_string()))?;
        fn field<T: serde::de::DeserializeOwned>(
            object: &serde_json::Map<String, Value>,
            key: &str,
        ) -> Result<Option<Option<T>>, TaxProfileError> {
            match object.get(key) {
                None => Ok(None),
                Some(Value::Null) => Ok(Some(None)),
                Some(raw) => serde_json::from_value(raw.clone())
                    .map(|value| Some(Some(value)))
                    .map_err(|_| TaxProfileError::Invalid(format!("`{raw}` is not a {key}"))),
            }
        }
        if let Some(unknown) = object.keys().find(|key| {
            !Fact::ALL
                .iter()
                .any(|fact| fact.owner().is_some() && fact.key() == key.as_str())
        }) {
            return Err(TaxProfileError::Invalid(format!(
                "`{unknown}` is not a fact of a tax year"
            )));
        }
        Ok(Self {
            tax_classification: field(object, "tax_classification")?,
            income_tax_election: field(object, "income_tax_election")?,
            eopt_tier: field(object, "eopt_tier")?,
            business_start_date: field(object, "business_start_date")?,
            line_of_business: field(object, "line_of_business")?,
            is_government_withholding_entity: field(object, "is_government_withholding_entity")?,
            atc_codes: field::<Vec<String>>(object, "atc_codes")?.map(Option::unwrap_or_default),
        })
    }

    pub fn touches(&self, owner: FactOwner) -> bool {
        match owner {
            FactOwner::HeadOffice => {
                self.tax_classification.is_some()
                    || self.income_tax_election.is_some()
                    || self.eopt_tier.is_some()
            }
            FactOwner::Branch => {
                self.business_start_date.is_some()
                    || self.line_of_business.is_some()
                    || self.is_government_withholding_entity.is_some()
                    || self.atc_codes.is_some()
            }
        }
    }
}

/// The election the flat `tax_elections` ledger records for `year`: its one
/// answer, the most specific graduated method when several graduated
/// entries agree, `None` for no entry or for an 8% and graduated pair.
pub fn flat_election(profile: &TaxpayerProfile, year: u16) -> Option<IncomeTaxElection> {
    election_of_ledger(&profile.tax_elections, year)
}

/// [`flat_election`] over a ledger's entries.
pub fn election_of_ledger(ledger: &[TaxElectionHistory], year: u16) -> Option<IncomeTaxElection> {
    let entries: Vec<&IncomeTaxElection> = ledger
        .iter()
        .filter(|entry| entry.taxable_year == year)
        .map(|entry| &entry.election)
        .collect();
    let eight = entries
        .iter()
        .any(|election| **election == IncomeTaxElection::EightPercent);
    let graduated: Vec<&IncomeTaxElection> = entries
        .iter()
        .copied()
        .filter(|election| **election != IncomeTaxElection::EightPercent)
        .collect();
    match (eight, graduated.is_empty()) {
        (true, true) => Some(IncomeTaxElection::EightPercent),
        (false, false) => Some(
            graduated
                .iter()
                .find(|election| **election != &IncomeTaxElection::GraduatedUnspecified)
                .copied()
                .unwrap_or(graduated[0])
                .clone(),
        ),
        _ => None,
    }
}

/// One branch's effective facts for one year, after head-office ownership.
#[derive(Debug, Clone)]
pub struct EffectiveYearFacts {
    pub tin: String,
    pub year: u16,
    pub facts: YearFacts,
    /// The row that holds the head-office facts: the head office, or the
    /// branch itself when it is the head office or its taxpayer has none.
    pub head_office_tin: String,
    /// The head-office row has a stored record for the year, so the
    /// election is the record's (set or not set).
    pub election_recorded: bool,
    /// When it has none: the flat `tax_elections` entries of the year the
    /// overlay keeps (the head office's, else the branch's own).
    pub fallback_elections: Vec<TaxElectionHistory>,
}

/// Lay `effective`'s facts over the flat profile fields of `profile`, for
/// its year: the profile every form prefill and rule reads.
pub fn overlay_year_facts(profile: &mut TaxpayerProfile, effective: &EffectiveYearFacts) {
    let facts = &effective.facts;
    profile.tax_classification = facts.tax_classification.clone();
    profile.eopt_tier = facts.eopt_tier.clone();
    profile.business_start_date = facts.business_start_date;
    profile.line_of_business = facts.line_of_business.clone().unwrap_or_default();
    profile.is_government_withholding_entity =
        facts.is_government_withholding_entity.unwrap_or(false);
    profile.atc_codes = facts.atc_codes.clone();
    let year = effective.year;
    profile
        .tax_elections
        .retain(|entry| entry.taxable_year != year);
    if effective.election_recorded {
        if let Some(election) = &facts.income_tax_election {
            profile.tax_elections.push(TaxElectionHistory {
                taxable_year: year,
                election: election.clone(),
                elected_at: chrono::NaiveDateTime::default(),
                source_form: facts
                    .election_source
                    .clone()
                    .unwrap_or_else(|| "Tax profile".to_string()),
            });
        }
    } else {
        profile
            .tax_elections
            .extend(effective.fallback_elections.iter().cloned());
    }
}

/// What the Tax profile page shows for one branch and year (interface
/// I-1's `YearFactsView`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearFactsView {
    pub tin: String,
    pub year: u16,
    pub facts: ViewFacts,
    /// Which row holds each fact (the birth date is the taxpayer's).
    pub owner: std::collections::BTreeMap<&'static str, FactOwner>,
    /// Form codes on the branch's list for the year that read each fact.
    pub used_by: std::collections::BTreeMap<&'static str, Vec<String>>,
    /// Facts in use that are not set, in [`Fact::ALL`] order.
    pub not_set: Vec<Fact>,
    pub election_locked: bool,
    pub election_source: Option<String>,
}

/// [`YearFacts`] as the page reads it: every key present, `null` when not set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewFacts {
    pub tax_classification: Option<TaxClassification>,
    pub income_tax_election: Option<IncomeTaxElection>,
    pub eopt_tier: Option<EoptTier>,
    pub business_start_date: Option<NaiveDate>,
    pub line_of_business: Option<String>,
    pub is_government_withholding_entity: Option<bool>,
    pub atc_codes: Vec<String>,
}

impl YearFactsView {
    /// The view of `effective` for a year whose forms list is `forms`, for a
    /// taxpayer born on `birth_date`, with the year's election lock.
    pub fn new(
        effective: &EffectiveYearFacts,
        forms: &[String],
        birth_date: Option<NaiveDate>,
        election_locked: bool,
    ) -> Self {
        let facts = &effective.facts;
        let mut owner = std::collections::BTreeMap::new();
        let mut used_by = std::collections::BTreeMap::new();
        let mut not_set = Vec::new();
        for fact in Fact::ALL {
            if let Some(holder) = fact.owner() {
                owner.insert(fact.key(), holder);
            }
            let readers: Vec<String> = forms
                .iter()
                .filter(|code| super::tax_types::facts_used_by(code).contains(&fact))
                .cloned()
                .collect();
            let set = match fact {
                Fact::BirthDate => birth_date.is_some(),
                other => facts.is_set(other),
            };
            if !readers.is_empty() && !set {
                not_set.push(fact);
            }
            used_by.insert(fact.key(), readers);
        }
        Self {
            tin: effective.tin.clone(),
            year: effective.year,
            facts: ViewFacts {
                tax_classification: facts.tax_classification.clone(),
                income_tax_election: facts.income_tax_election.clone(),
                eopt_tier: facts.eopt_tier.clone(),
                business_start_date: facts.business_start_date,
                line_of_business: facts.line_of_business.clone(),
                is_government_withholding_entity: facts.is_government_withholding_entity,
                atc_codes: facts.atc_codes.clone(),
            },
            owner,
            used_by,
            not_set,
            election_locked,
            election_source: facts
                .income_tax_election
                .as_ref()
                .and(facts.election_source.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Juana Santos",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "050",
            "line_of_business": "Sari-sari store",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "",
            "email": "",
            "default_form_type": "",
            "tax_classification": "SelfEmployed",
            "eopt_tier": "Micro",
            "atc_codes": ["PT010"],
            "tax_elections": [
                {"taxable_year": 2025, "election": "GraduatedUnspecified", "elected_at": "2025-01-01T00:00:00", "source_form": "x"},
                {"taxable_year": 2025, "election": "GraduatedOsd", "elected_at": "2025-01-01T00:00:00", "source_form": "x"},
                {"taxable_year": 2026, "election": "EightPercent", "elected_at": "2026-01-01T00:00:00", "source_form": "x"},
                {"taxable_year": 2027, "election": "EightPercent", "elected_at": "2026-01-01T00:00:00", "source_form": "x"},
                {"taxable_year": 2027, "election": "GraduatedOsd", "elected_at": "2026-01-01T00:00:00", "source_form": "x"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn the_flat_election_of_a_year_is_its_one_answer() {
        let profile = profile();
        assert_eq!(
            flat_election(&profile, 2025),
            Some(IncomeTaxElection::GraduatedOsd)
        );
        assert_eq!(
            flat_election(&profile, 2026),
            Some(IncomeTaxElection::EightPercent)
        );
        assert_eq!(flat_election(&profile, 2027), None, "a conflicting pair");
        assert_eq!(flat_election(&profile, 2024), None);
    }

    #[test]
    fn a_patch_sets_only_its_keys_and_only_its_owners_facts() {
        let patch = YearFactsPatch::from_json(&serde_json::json!({
            "tax_classification": "MixedIncome",
            "line_of_business": null,
            "atc_codes": [" pt010 ", ""],
        }))
        .unwrap();
        let mut head = YearFacts::from_flat_profile(&profile());
        head.apply(&patch, FactOwner::HeadOffice);
        assert_eq!(
            head.tax_classification,
            Some(TaxClassification::MixedIncome)
        );
        assert_eq!(head.line_of_business.as_deref(), Some("Sari-sari store"));
        let mut branch = YearFacts::from_flat_profile(&profile());
        branch.apply(&patch, FactOwner::Branch);
        assert_eq!(
            branch.tax_classification,
            Some(TaxClassification::SelfEmployed)
        );
        assert_eq!(branch.line_of_business, None);
        assert_eq!(branch.atc_codes, vec!["PT010".to_string()]);
        assert!(YearFactsPatch::from_json(&serde_json::json!({"birth_date": null})).is_err());
        assert!(
            YearFactsPatch::from_json(&serde_json::json!({"eopt_tier": "Huge"})).is_err(),
            "an unknown tier"
        );
    }

    #[test]
    fn the_overlay_replaces_the_flat_facts_and_the_years_election() {
        let mut flat = profile();
        let effective = EffectiveYearFacts {
            tin: flat.tin.full(),
            year: 2026,
            facts: YearFacts {
                tax_classification: Some(TaxClassification::MixedIncome),
                income_tax_election: Some(IncomeTaxElection::GraduatedUnspecified),
                election_source: Some("2551Q Q1".into()),
                eopt_tier: None,
                business_start_date: NaiveDate::from_ymd_opt(2020, 2, 1),
                line_of_business: None,
                is_government_withholding_entity: Some(true),
                atc_codes: vec![],
            },
            head_office_tin: flat.tin.full(),
            election_recorded: true,
            fallback_elections: vec![],
        };
        overlay_year_facts(&mut flat, &effective);
        assert_eq!(
            flat.tax_classification,
            Some(TaxClassification::MixedIncome)
        );
        assert_eq!(flat.eopt_tier, None);
        assert_eq!(flat.line_of_business, "");
        assert!(flat.is_government_withholding_entity);
        assert!(flat.atc_codes.is_empty());
        assert_eq!(
            flat.income_tax_election_for_year(2026),
            Some(IncomeTaxElection::GraduatedUnspecified)
        );
        assert_eq!(
            flat.tax_elections
                .iter()
                .filter(|e| e.taxable_year == 2026)
                .count(),
            1
        );
        assert_eq!(
            flat.income_tax_election_for_year(2025),
            Some(IncomeTaxElection::GraduatedUnspecified),
            "other years are untouched"
        );
    }
}
