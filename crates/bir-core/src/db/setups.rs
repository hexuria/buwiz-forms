//! Setups (owner decision 14): the built-in setups, the user's own in the
//! `setups` table (v24), applying one to a branch's year, saving one from a
//! branch's year, and copying another taxpayer's year.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Database;
use super::taxpayers::{MissingFormPolicy, TemplateConflictPolicy};
use crate::forms::registry::canonical_form_code;
use crate::profile::TaxpayerType;
use crate::tax_profile::setups::{SetupBody, setup_facts, user_setup_row};
use crate::tax_profile::{
    Fact, Setup, TaxProfileError, built_in_setups, check_tax_year, plan_facts,
};

type Result<T> = std::result::Result<T, TaxProfileError>;

/// What applying a setup, or a copy of another taxpayer's year, did.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SetupApplied {
    /// The year's forms list afterwards.
    pub forms: Vec<String>,
    /// Facts written.
    pub set: Vec<Fact>,
    /// Facts already set to something else and left as they are.
    pub kept: Vec<Fact>,
    /// The year's 8% or graduated choice cannot change.
    pub election_locked: bool,
    /// Templates copied (a copy only).
    pub templates_copied: Vec<String>,
    /// Templates the target already had and kept (a copy only).
    pub templates_kept: Vec<String>,
}

fn taxpayer_type_label(taxpayer_type: &TaxpayerType) -> &'static str {
    match taxpayer_type {
        TaxpayerType::Individual => "an individual",
        TaxpayerType::Corporation => "a corporation",
        TaxpayerType::Partnership => "a partnership",
        TaxpayerType::Cooperative => "a cooperative",
        TaxpayerType::Estate => "an estate",
        TaxpayerType::Trust => "a trust",
    }
}

impl Database {
    fn taxpayer_type_of(&self, tin: &str) -> Result<TaxpayerType> {
        let branch = self
            .get_branch(tin)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No branch with TIN {tin}")))?;
        let taxpayer = self
            .get_taxpayer(&branch.tin_root)?
            .ok_or_else(|| TaxProfileError::NotFound(format!("No taxpayer {}", branch.tin_root)))?;
        Ok(taxpayer.taxpayer.taxpayer_type)
    }

    fn user_setups(&self) -> Result<Vec<Setup>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, body_json FROM setups ORDER BY name COLLATE NOCASE, id")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut setups = Vec::new();
        for row in rows {
            let (id, name, body) = row?;
            // A body this build cannot read is skipped, not fatal.
            if let Ok(body) = serde_json::from_str::<SetupBody>(&body) {
                setups.push(Setup::from_row(id, name, body));
            }
        }
        Ok(setups)
    }

    /// The setups for `taxpayer_type`: the built-in ones, then the user's.
    pub fn list_setups(&self, taxpayer_type: &TaxpayerType) -> Result<Vec<Setup>> {
        let mut setups: Vec<Setup> = built_in_setups()
            .into_iter()
            .filter(|setup| setup.fits(taxpayer_type))
            .collect();
        setups.extend(
            self.user_setups()?
                .into_iter()
                .filter(|setup| setup.fits(taxpayer_type)),
        );
        Ok(setups)
    }

    /// One setup by id, built-in or the user's.
    pub fn get_setup(&self, id: &str) -> Result<Setup> {
        let found = match user_setup_row(id) {
            Some(_) => self.user_setups()?.into_iter().find(|setup| setup.id == id),
            None => built_in_setups().into_iter().find(|setup| setup.id == id),
        };
        found.ok_or_else(|| TaxProfileError::NotFound("That setup no longer exists.".to_string()))
    }

    /// Apply `setup` to `tin`'s `year`: the forms list (replaced, or added
    /// to for an adding setup), then the facts the setup holds. A fact
    /// already set to something else is kept unless `replace`; a locked
    /// 8% or graduated choice is always kept. Name, TIN, address and
    /// contact are never touched.
    fn apply_setup_value(
        &self,
        tin: &str,
        year: u16,
        setup: &Setup,
        replace: bool,
    ) -> Result<SetupApplied> {
        check_tax_year(year)?;
        let taxpayer_type = self.taxpayer_type_of(tin)?;
        if !setup.fits(&taxpayer_type) {
            return Err(TaxProfileError::Invalid(format!(
                "{} is not a setup for {}.",
                setup.name,
                taxpayer_type_label(&taxpayer_type)
            )));
        }
        let mut forms: Vec<String> = if setup.adds_forms {
            self.active_form_codes_for_year(tin, year)?
        } else {
            Vec::new()
        };
        for code in &setup.forms {
            let code = canonical_form_code(code);
            if !forms.iter().any(|have| canonical_form_code(have) == code) {
                forms.push(code);
            }
        }
        let set = self.set_yearly_forms(tin, year, &forms)?;
        let current = self.effective_year_facts(tin, year)?;
        let locked = self.income_tax_election_locked(tin, year)?;
        let plan = plan_facts(&setup_facts(&setup.facts), &current.facts, locked, replace);
        if !plan.set.is_empty() {
            self.set_year_facts(tin, year, &plan.patch)?;
        }
        Ok(SetupApplied {
            forms: set.active_form_codes(),
            set: plan.set,
            kept: plan.kept,
            election_locked: locked,
            ..SetupApplied::default()
        })
    }

    /// [`Self::apply_setup_value`] for the setup `setup_id` names.
    pub fn apply_setup(
        &self,
        tin: &str,
        year: u16,
        setup_id: &str,
        replace: bool,
    ) -> Result<SetupApplied> {
        check_tax_year(year)?;
        let setup = self.get_setup(setup_id)?;
        self.apply_setup_value(tin, year, &setup, replace)
    }

    /// The setup `tin`'s `year` makes: its forms and facts, with no dates,
    /// no line of business and nothing personal, for its taxpayer's type.
    fn setup_of_year(&self, tin: &str, year: u16, name: &str) -> Result<Setup> {
        let taxpayer_type = self.taxpayer_type_of(tin)?;
        let mut forms: Vec<String> = Vec::new();
        for code in self.active_form_codes_for_year(tin, year)? {
            let code = canonical_form_code(&code);
            if !forms.contains(&code) {
                forms.push(code);
            }
        }
        let facts = self.effective_year_facts(tin, year)?.facts;
        Ok(Setup {
            id: String::new(),
            name: name.to_string(),
            taxpayer_types: vec![taxpayer_type],
            forms,
            facts: setup_facts(&facts),
            built_in: false,
            adds_forms: false,
        })
    }

    /// Save `tin`'s `year` as a setup called `name`.
    pub fn save_setup(&self, tin: &str, year: u16, name: &str) -> Result<Setup> {
        check_tax_year(year)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(TaxProfileError::Invalid(
                "Type a name for the setup.".to_string(),
            ));
        }
        let setup = self.setup_of_year(tin, year, name)?;
        if setup.forms.is_empty() {
            return Err(TaxProfileError::Invalid(format!(
                "Choose the forms for {year} first. A setup keeps them."
            )));
        }
        let taken: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM setups WHERE name = ?1 COLLATE NOCASE",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        if taken.is_some()
            || built_in_setups()
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(name))
        {
            return Err(TaxProfileError::Conflict(format!(
                "A setup called {name} already exists. Choose another name."
            )));
        }
        self.conn.execute(
            "INSERT INTO setups (name, body_json, created_at) VALUES (?1, ?2, datetime('now'))",
            params![
                name,
                serde_json::to_string(&setup.body()).map_err(super::DbError::from)?
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_setup(&format!(
            "{}{id}",
            crate::tax_profile::setups::USER_SETUP_PREFIX
        ))
    }

    /// Delete one of the user's setups. A built-in setup cannot go.
    pub fn delete_setup(&self, id: &str) -> Result<bool> {
        let Some(row) = user_setup_row(id) else {
            return Err(TaxProfileError::Invalid(
                "A built-in setup cannot be deleted.".to_string(),
            ));
        };
        Ok(self
            .conn
            .execute("DELETE FROM setups WHERE id = ?1", [row])?
            > 0)
    }

    /// Start `to_tin`'s `year` from `from_tin`'s: the templates of the
    /// source's forms for the year, then what a setup saved from that year
    /// would hold (its forms list and facts; no dates, no line of business,
    /// never name, TIN, address or contact). A template or fact the target
    /// already has is kept unless `replace`; a locked choice always is.
    pub fn copy_setup_from(
        &self,
        from_tin: &str,
        to_tin: &str,
        year: u16,
        replace: bool,
    ) -> Result<SetupApplied> {
        check_tax_year(year)?;
        if from_tin == to_tin {
            return Err(TaxProfileError::Invalid(
                "Choose another taxpayer or branch to copy from.".to_string(),
            ));
        }
        let source = self.setup_of_year(from_tin, year, "copy")?;
        if source.forms.is_empty() {
            return Err(TaxProfileError::NotFound(format!(
                "That branch has no forms in {year} to copy."
            )));
        }
        let target_type = self.taxpayer_type_of(to_tin)?;
        if !source.fits(&target_type) {
            return Err(TaxProfileError::Invalid(format!(
                "Copy from a taxpayer of the same type: this one is {}.",
                taxpayer_type_label(&target_type)
            )));
        }
        let templates: Vec<String> = self
            .list_form_templates(from_tin)?
            .into_iter()
            .map(|template| canonical_form_code(&template.form_code))
            .filter(|code| source.forms.contains(code))
            .collect();
        let report = if templates.is_empty() {
            None
        } else {
            Some(self.copy_form_templates(
                from_tin,
                &templates,
                to_tin,
                year,
                if replace {
                    TemplateConflictPolicy::Replace
                } else {
                    TemplateConflictPolicy::Skip
                },
                MissingFormPolicy::AddForm,
            )?)
        };
        let mut applied = self.apply_setup_value(to_tin, year, &source, replace)?;
        if let Some(report) = report {
            applied.templates_copied = report.copied;
            applied.templates_copied.extend(report.replaced);
            applied.templates_kept = report.skipped_existing;
        }
        Ok(applied)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::profile::{EoptTier, IncomeTaxElection, TaxClassification};
    use crate::tax_profile::{BranchFacts, NewTaxpayer, YearFactsPatch};

    const ROOT: &str = "000123453";
    const HEAD: &str = "00012345300000";
    const OTHER_ROOT: &str = "000123454";
    const OTHER: &str = "00012345400000";

    fn taxpayer(db: &Database, root: &str, name: &str, taxpayer_type: TaxpayerType) {
        db.create_taxpayer(NewTaxpayer {
            tin_root: root.into(),
            name: name.into(),
            taxpayer_type,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "039".into(),
                registered_address: "Quezon City".into(),
                zip_code: "1100".into(),
                phone: "09170000000".into(),
                email: "juana@example.com".into(),
            },
        })
        .unwrap();
    }

    fn db() -> Database {
        let db = Database::open_in_memory_for_tests().unwrap();
        taxpayer(&db, ROOT, "Juana Santos", TaxpayerType::Individual);
        db
    }

    fn patch(value: serde_json::Value) -> YearFactsPatch {
        YearFactsPatch::from_json(&value).unwrap()
    }

    fn ids(setups: &[Setup]) -> Vec<&str> {
        setups.iter().map(|setup| setup.id.as_str()).collect()
    }

    #[test]
    fn setups_list_by_taxpayer_type_built_in_first() {
        let db = db();
        assert_eq!(
            ids(&db.list_setups(&TaxpayerType::Individual).unwrap()),
            [
                "non_vat_professional",
                "vat_business_individual",
                "employer"
            ]
        );
        assert_eq!(
            ids(&db.list_setups(&TaxpayerType::Corporation).unwrap()),
            [
                "vat_business_corporation",
                "corporation_non_vat",
                "employer"
            ]
        );
        db.set_yearly_forms(HEAD, 2026, &["2551Q"]).unwrap();
        let mine = db.save_setup(HEAD, 2026, "My shop").unwrap();
        assert!(!mine.built_in);
        assert_eq!(
            ids(&db.list_setups(&TaxpayerType::Individual).unwrap()).last(),
            Some(&mine.id.as_str()),
            "the user's setups come after the built-in ones"
        );
        assert!(
            !ids(&db.list_setups(&TaxpayerType::Corporation).unwrap()).contains(&mine.id.as_str()),
            "a setup saved from an individual is not offered to a corporation"
        );
    }

    #[test]
    fn applying_a_setup_replaces_or_adds_the_forms_and_sets_the_facts() {
        let db = db();
        db.set_yearly_forms(HEAD, 2026, &["0605"]).unwrap();
        let applied = db
            .apply_setup(HEAD, 2026, "non_vat_professional", false)
            .unwrap();
        assert_eq!(applied.forms, ["1701", "1701Q", "2551Q"], "replaced");
        assert_eq!(
            db.effective_year_facts(HEAD, 2026)
                .unwrap()
                .facts
                .tax_classification,
            Some(TaxClassification::SelfEmployed)
        );
        assert!(applied.set.contains(&Fact::TaxClassification));
        let added = db.apply_setup(HEAD, 2026, "employer", false).unwrap();
        assert_eq!(
            added.forms,
            ["1601C", "1604C", "1701", "1701Q", "2551Q"],
            "Employer adds"
        );
        assert!(added.set.is_empty() && added.kept.is_empty());

        // Name, TIN, address and contact are never touched.
        let branch = db.get_branch(HEAD).unwrap().unwrap();
        assert_eq!(
            branch.value(crate::tax_profile::BranchField::RegisteredAddress),
            "Quezon City"
        );
        assert_eq!(
            db.get_taxpayer(ROOT).unwrap().unwrap().taxpayer.name,
            "Juana Santos"
        );

        let wrong = db.apply_setup(HEAD, 2026, "corporation_non_vat", false);
        assert!(
            matches!(wrong, Err(TaxProfileError::Invalid(_))),
            "{wrong:?}"
        );
        assert!(matches!(
            db.apply_setup(HEAD, 2026, "user-999", false),
            Err(TaxProfileError::NotFound(_))
        ));
    }

    #[test]
    fn a_set_fact_is_kept_unless_the_caller_replaces_it() {
        let db = db();
        db.set_year_facts(
            HEAD,
            2026,
            &patch(serde_json::json!({"tax_classification": "MixedIncome"})),
        )
        .unwrap();
        let applied = db
            .apply_setup(HEAD, 2026, "non_vat_professional", false)
            .unwrap();
        assert_eq!(applied.kept, [Fact::TaxClassification]);
        assert_eq!(
            db.effective_year_facts(HEAD, 2026)
                .unwrap()
                .facts
                .tax_classification,
            Some(TaxClassification::MixedIncome),
            "not overwritten"
        );
        let replaced = db
            .apply_setup(HEAD, 2026, "non_vat_professional", true)
            .unwrap();
        assert!(replaced.kept.is_empty());
        assert_eq!(
            db.effective_year_facts(HEAD, 2026)
                .unwrap()
                .facts
                .tax_classification,
            Some(TaxClassification::SelfEmployed)
        );
    }

    #[test]
    fn a_locked_election_is_kept_even_when_replacing() {
        let db = db();
        db.set_yearly_forms(HEAD, 2026, &["2551Q"]).unwrap();
        db.set_year_facts(
            HEAD,
            2026,
            &patch(serde_json::json!({
                "tax_classification": "SelfEmployed",
                "income_tax_election": "EightPercent",
                "eopt_tier": "Micro",
            })),
        )
        .unwrap();
        db.set_yearly_forms(HEAD, 2025, &["2551Q"]).unwrap();
        db.set_year_facts(
            HEAD,
            2025,
            &patch(serde_json::json!({"income_tax_election": "GraduatedOsd"})),
        )
        .unwrap();
        let saved = db.save_setup(HEAD, 2025, "Graduated").unwrap();
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES (?1, '2551Q', 2026, 1, 'Q1', 'Filed', '{}')",
                params![HEAD],
            )
            .unwrap();
        let applied = db.apply_setup(HEAD, 2026, &saved.id, true).unwrap();
        assert!(applied.election_locked);
        assert!(applied.kept.contains(&Fact::IncomeTaxElection));
        assert_eq!(
            db.effective_year_facts(HEAD, 2026)
                .unwrap()
                .facts
                .income_tax_election,
            Some(IncomeTaxElection::EightPercent)
        );
    }

    #[test]
    fn a_saved_setup_round_trips_with_no_personal_data() {
        let db = db();
        db.set_yearly_forms(HEAD, 2026, &["2551Q", "1701Q", "1701", "0619E"])
            .unwrap();
        db.set_year_facts(
            HEAD,
            2026,
            &patch(serde_json::json!({
                "tax_classification": "SelfEmployed",
                "eopt_tier": "Small",
                "business_start_date": "2019-04-02",
                "line_of_business": "Bookkeeping services",
                "is_government_withholding_entity": false,
                "atc_codes": ["PT010"],
            })),
        )
        .unwrap();
        let saved = db.save_setup(HEAD, 2026, "  Bookkeeper  ").unwrap();
        assert_eq!(saved.name, "Bookkeeper");
        assert_eq!(saved.taxpayer_types, [TaxpayerType::Individual]);
        assert!(matches!(
            db.save_setup(HEAD, 2026, "bookkeeper"),
            Err(TaxProfileError::Conflict(_))
        ));
        assert!(matches!(
            db.save_setup(HEAD, 2026, " "),
            Err(TaxProfileError::Invalid(_))
        ));
        let body: String = db
            .conn
            .query_row("SELECT body_json FROM setups", [], |row| row.get(0))
            .unwrap();
        for personal in [
            "Juana",
            "Santos",
            ROOT,
            "000-123-453",
            "Quezon",
            "1100",
            "0917",
            "example.com",
            "2019-04-02",
            "Bookkeeping services",
        ] {
            assert!(!body.contains(personal), "{personal} in {body}");
        }

        taxpayer(&db, OTHER_ROOT, "Pedro Reyes", TaxpayerType::Individual);
        let applied = db.apply_setup(OTHER, 2026, &saved.id, false).unwrap();
        assert_eq!(applied.forms, ["0619E", "1701", "1701Q", "2551Q"]);
        let facts = db.effective_year_facts(OTHER, 2026).unwrap().facts;
        assert_eq!(
            facts.tax_classification,
            Some(TaxClassification::SelfEmployed)
        );
        assert_eq!(facts.eopt_tier, Some(EoptTier::Small));
        assert_eq!(facts.atc_codes, ["PT010"]);
        assert_eq!(facts.business_start_date, None, "no dates");
        assert_eq!(facts.line_of_business, None, "no line of business");

        assert!(db.delete_setup(&saved.id).unwrap());
        assert!(!db.delete_setup(&saved.id).unwrap());
        assert!(matches!(
            db.delete_setup("employer"),
            Err(TaxProfileError::Invalid(_))
        ));
    }

    #[test]
    fn a_copy_from_another_taxpayer_carries_forms_facts_and_templates() {
        let db = db();
        db.set_yearly_forms(HEAD, 2026, &["2551Q", "1601C"])
            .unwrap();
        db.set_year_facts(
            HEAD,
            2026,
            &patch(serde_json::json!({"tax_classification": "SelfEmployed", "eopt_tier": "Micro"})),
        )
        .unwrap();
        // A template for the form: one "Stop here" switch makes it.
        db.set_template_stop(HEAD, "2551Q", "frm2551Qv2018:txtSheets", Some(true))
            .unwrap();
        taxpayer(&db, OTHER_ROOT, "Pedro Reyes", TaxpayerType::Individual);

        let copied = db.copy_setup_from(HEAD, OTHER, 2026, false).unwrap();
        assert_eq!(copied.forms, ["1601C", "2551Q"]);
        assert_eq!(copied.templates_copied, ["2551Q"]);
        let facts = db.effective_year_facts(OTHER, 2026).unwrap().facts;
        assert_eq!(
            facts.tax_classification,
            Some(TaxClassification::SelfEmployed)
        );
        assert_eq!(facts.eopt_tier, Some(EoptTier::Micro));
        assert!(db.get_form_template(OTHER, "2551Q").unwrap().is_some());
        let other = db.get_taxpayer(OTHER_ROOT).unwrap().unwrap();
        assert_eq!(other.taxpayer.name, "Pedro Reyes", "never the name");

        let again = db.copy_setup_from(HEAD, OTHER, 2026, false).unwrap();
        assert_eq!(again.templates_kept, ["2551Q"], "a template it has is kept");

        taxpayer(&db, "000123455", "Example Corp", TaxpayerType::Corporation);
        assert!(matches!(
            db.copy_setup_from(HEAD, "00012345500000", 2026, false),
            Err(TaxProfileError::Invalid(_))
        ));
        assert!(matches!(
            db.copy_setup_from(OTHER, HEAD, 2025, false),
            Err(TaxProfileError::NotFound(_))
        ));
    }
}
