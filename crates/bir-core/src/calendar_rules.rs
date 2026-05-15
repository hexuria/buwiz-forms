//! E-BIRForms Tax Deadline Resolution Engine
//!
//! Evaluates deadline rules against the active tax calendar, applies special
//! overrides (extensions, holidays), and resolves the final deadline schedule.

use crate::db::{Database, DbError, ResolvedTaxDeadline, TaxCalendar};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuleConfig {
    #[serde(rename = "fixed_date")]
    FixedDate { month: u32, day: u32 },
    #[serde(rename = "following_month_day")]
    FollowingMonthDay { day: u32 },
    #[serde(rename = "quarter_relative")]
    QuarterRelative {
        mode: String, // e.g., "days_after_quarter_end"
        days: u32,
    },
    #[serde(rename = "efps_monthly_grouped")]
    EfpsMonthlyGrouped {
        non_efps_day: u32,
        groups: std::collections::HashMap<String, u32>,
    },
}

pub struct DeadlineResolver<'a> {
    db: &'a Database,
}

impl<'a> DeadlineResolver<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Evaluates all rules and overrides for a given calendar and persists them as resolved deadlines.
    pub fn generate_resolved_deadlines(&self, calendar: &TaxCalendar) -> Result<Vec<ResolvedTaxDeadline>, DbError> {
        let calendar_id = calendar.id.expect("Calendar must be saved before resolving deadlines");
        let year = calendar.year;
        
        let rules = self.db.list_tax_deadline_rules(calendar_id).unwrap_or_default();
        let overrides = self.db.list_tax_deadline_overrides(calendar_id).unwrap_or_default();
        let forms = self.db.list_tax_forms().unwrap_or_default();
        
        let mut form_map = std::collections::HashMap::new();
        for f in forms {
            if let Some(id) = f.id {
                form_map.insert(id, f);
            }
        }

        let mut resolved = Vec::new();

        for rule in rules {
            let form = match form_map.get(&rule.form_id) {
                Some(f) => f,
                None => continue,
            };

            let config: Result<RuleConfig, _> = serde_json::from_str(&rule.rule_config_json);
            let config = match config {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut base_deadlines = Vec::new();

            match rule.frequency.as_str() {
                "Monthly" => {
                    if let RuleConfig::FollowingMonthDay { day } = config {
                        for month in 1..=12 {
                            let (target_year, target_month) = if month == 12 {
                                (year + 1, 1)
                            } else {
                                (year, month + 1)
                            };
                            if let Some(date) = NaiveDate::from_ymd_opt(target_year, target_month, day) {
                                let period_start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                                let period_end = if month == 12 {
                                    NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
                                } else {
                                    NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - Duration::days(1)
                                };
                                base_deadlines.push((period_start, period_end, date));
                            }
                        }
                    }
                }
                "Quarterly" => {
                    if let RuleConfig::QuarterRelative { days, .. } = config {
                        let quarter_ends = vec![
                            (3, 31),
                            (6, 30),
                            (9, 30),
                            (12, 31),
                        ];
                        for (q, (m, d)) in quarter_ends.into_iter().enumerate() {
                            let start_m = (q as u32) * 3 + 1;
                            let period_start = NaiveDate::from_ymd_opt(year, start_m, 1).unwrap();
                            let period_end = NaiveDate::from_ymd_opt(year, m, d).unwrap();
                            let date = period_end + Duration::days(days as i64);
                            base_deadlines.push((period_start, period_end, date));
                        }
                    }
                }
                "Annual" => {
                    if let RuleConfig::FixedDate { month, day } = config {
                        if let Some(date) = NaiveDate::from_ymd_opt(year + 1, month, day) {
                            let period_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
                            let period_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
                            base_deadlines.push((period_start, period_end, date));
                        }
                    }
                }
                "EfpsMonthly" => {
                    if let RuleConfig::EfpsMonthlyGrouped { non_efps_day, ref groups } = config {
                        // Determine which day to use based on the rule's efps_group field
                        let due_day = rule.efps_group.as_deref()
                            .and_then(|g| groups.get(g).copied())
                            .unwrap_or(non_efps_day);

                        for month in 1..=12u32 {
                            let (target_year, target_month) = if month == 12 {
                                (year + 1, 1)
                            } else {
                                (year, month + 1)
                            };
                            if let Some(date) = NaiveDate::from_ymd_opt(target_year, target_month, due_day) {
                                let period_start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                                let period_end = if month == 12 {
                                    NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
                                } else {
                                    NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - Duration::days(1)
                                };
                                base_deadlines.push((period_start, period_end, date));
                            }
                        }
                    }
                }
                _ => {}
            }

            for (period_start, period_end, mut base_date) in base_deadlines {
                base_date = Self::adjust_for_weekend(base_date);
                let original_deadline_str = base_date.format("%Y-%m-%d").to_string();
                
                let mut final_date = base_date;
                let mut adjusted_str = None;
                let mut status = "Normal".to_string();
                let mut override_id = None;

                // Check overrides
                for ov in &overrides {
                    let affected_forms: Vec<i64> = serde_json::from_str(&ov.affected_form_ids_json).unwrap_or_default();
                    if affected_forms.contains(&rule.form_id) && ov.original_deadline == original_deadline_str {
                        if let Ok(adj_date) = NaiveDate::parse_from_str(&ov.adjusted_deadline, "%Y-%m-%d") {
                            final_date = Self::adjust_for_weekend(adj_date);
                            adjusted_str = Some(final_date.format("%Y-%m-%d").to_string());
                            status = ov.reason.clone();
                            override_id = ov.id;
                        }
                    }
                }

                resolved.push(ResolvedTaxDeadline {
                    id: None,
                    calendar_id,
                    form_id: rule.form_id,
                    form_no: form.form_no.clone(),
                    form_name: form.form_name.clone(),
                    period_start: Some(period_start.format("%Y-%m-%d").to_string()),
                    period_end: Some(period_end.format("%Y-%m-%d").to_string()),
                    original_deadline: original_deadline_str,
                    adjusted_deadline: adjusted_str,
                    final_deadline: final_date.format("%Y-%m-%d").to_string(),
                    status,
                    override_id,
                    created_at: None,
                    updated_at: None,
                });
            }
        }

        Ok(resolved)
    }

    /// Adjusts a date forward if it falls on a weekend.
    pub fn adjust_for_weekend(mut date: NaiveDate) -> NaiveDate {
        match date.weekday() {
            chrono::Weekday::Sat => date += Duration::days(2),
            chrono::Weekday::Sun => date += Duration::days(1),
            _ => {}
        }
        date
    }
}
