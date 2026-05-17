//! Static official BIR tax calendar rules.
//!
//! Base recurring rules are compiled into the app. Calendar-year views are
//! resolved separately from taxable-year views so cross-year deadlines stay in
//! the correct UI year.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeadlinePeriod {
    Monthly { taxable_year: i32, month: u8 },
    Quarterly { taxable_year: i32, quarter: u8 },
    Annual { taxable_year: i32 },
    EventBased,
}

impl DeadlinePeriod {
    pub fn taxable_year(&self) -> Option<i32> {
        match self {
            Self::Monthly { taxable_year, .. }
            | Self::Quarterly { taxable_year, .. }
            | Self::Annual { taxable_year } => Some(*taxable_year),
            Self::EventBased => None,
        }
    }

    pub fn quarter(&self) -> Option<u8> {
        match self {
            Self::Monthly { month, .. } => Some(((*month - 1) / 3) + 1),
            Self::Quarterly { quarter, .. } => Some(*quarter),
            Self::Annual { .. } | Self::EventBased => None,
        }
    }

    pub fn month(&self) -> Option<u8> {
        match self {
            Self::Monthly { month, .. } => Some(*month),
            Self::Quarterly { .. } | Self::Annual { .. } | Self::EventBased => None,
        }
    }

    fn matches_month_filter(&self, months: &[u8]) -> bool {
        match self {
            Self::Monthly { month, .. } => months.contains(month),
            Self::Quarterly { quarter, .. } => {
                let first_month = ((*quarter - 1) * 3) + 1;
                let last_month = first_month + 2;
                months
                    .iter()
                    .any(|month| (first_month..=last_month).contains(month))
            }
            Self::Annual { .. } | Self::EventBased => false,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Monthly {
                taxable_year,
                month,
            } => format!("Tax period: {taxable_year}-{month:02}"),
            Self::Quarterly {
                taxable_year,
                quarter,
            } => format!("Tax period: {taxable_year} Q{quarter}"),
            Self::Annual { taxable_year } => format!("Tax period: {taxable_year} annual"),
            Self::EventBased => "Event-based obligation".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeadlineKind {
    Dated {
        original_deadline: NaiveDate,
        final_deadline: NaiveDate,
    },
    EventBased {
        trigger: String,
        statutory_window: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeadlineStatus {
    Normal,
    WeekendAdjusted,
    HolidayAdjusted,
    NonWorkingDayAdjusted,
    Extended,
    EventBased,
}

impl DeadlineStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::WeekendAdjusted => "Weekend Adjusted",
            Self::HolidayAdjusted => "Holiday Adjusted",
            Self::NonWorkingDayAdjusted => "Non-working Day Adjusted",
            Self::Extended => "Extended",
            Self::EventBased => "Event Based",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonWorkingDayKind {
    RegularHoliday,
    LocalHoliday,
    SpecialNonWorkingDay,
    OtherClosure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonWorkingDay {
    pub date: NaiveDate,
    pub name: String,
    pub kind: NonWorkingDayKind,
    pub regions: Vec<String>,
    pub source_reference: Option<String>,
}

impl NonWorkingDay {
    pub fn regular_holiday(date: NaiveDate, name: impl Into<String>) -> Self {
        Self {
            date,
            name: name.into(),
            kind: NonWorkingDayKind::RegularHoliday,
            regions: Vec::new(),
            source_reference: None,
        }
    }

    pub fn special_non_working_day(date: NaiveDate, name: impl Into<String>) -> Self {
        Self {
            date,
            name: name.into(),
            kind: NonWorkingDayKind::SpecialNonWorkingDay,
            regions: Vec::new(),
            source_reference: None,
        }
    }

    fn adjustment_status(&self) -> DeadlineStatus {
        match self.kind {
            NonWorkingDayKind::RegularHoliday | NonWorkingDayKind::LocalHoliday => {
                DeadlineStatus::HolidayAdjusted
            }
            NonWorkingDayKind::SpecialNonWorkingDay | NonWorkingDayKind::OtherClosure => {
                DeadlineStatus::NonWorkingDayAdjusted
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BusinessDayCalendar {
    pub non_working_days: Vec<NonWorkingDay>,
}

impl BusinessDayCalendar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_non_working_days(non_working_days: Vec<NonWorkingDay>) -> Self {
        Self { non_working_days }
    }

    pub fn is_non_working_day(&self, date: NaiveDate) -> bool {
        self.non_working_reason(date).is_some()
    }

    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        self.non_working_reason(date).is_none()
    }

    pub fn adjust_to_next_business_day(&self, date: NaiveDate) -> (NaiveDate, DeadlineStatus) {
        let Some(status) = self.non_working_reason(date) else {
            return (date, DeadlineStatus::Normal);
        };

        let mut adjusted = date + Duration::days(1);
        while self.is_non_working_day(adjusted) {
            adjusted += Duration::days(1);
        }

        (adjusted, status)
    }

    fn non_working_reason(&self, date: NaiveDate) -> Option<DeadlineStatus> {
        match date.weekday() {
            chrono::Weekday::Sat | chrono::Weekday::Sun => Some(DeadlineStatus::WeekendAdjusted),
            _ => self
                .non_working_days
                .iter()
                .find(|day| day.date == date)
                .map(NonWorkingDay::adjustment_status),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTaxDeadline {
    pub form_code: String,
    pub display_form_no: String,
    pub form_name: String,
    pub period: DeadlinePeriod,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub deadline: DeadlineKind,
    pub status: DeadlineStatus,
    pub description: String,
    pub source_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlineOverride {
    pub id: String,
    pub title: String,
    pub source_reference: String,
    pub affected_form_codes: Vec<String>,
    pub original_deadline: NaiveDate,
    pub adjusted_deadline: NaiveDate,
    pub affected_regions: Vec<String>,
    pub affected_taxpayer_types: Vec<String>,
    pub effective_from: Option<NaiveDate>,
    pub effective_until: Option<NaiveDate>,
    pub expires_at: Option<NaiveDate>,
}

impl ResolvedTaxDeadline {
    fn dated(
        display_form_no: &str,
        form_name: &str,
        period: DeadlinePeriod,
        period_start: NaiveDate,
        period_end: NaiveDate,
        original_deadline: NaiveDate,
        description: &str,
    ) -> Self {
        let (final_deadline, status) =
            BusinessDayCalendar::default().adjust_to_next_business_day(original_deadline);

        Self {
            form_code: canonical_form_code(display_form_no).to_string(),
            display_form_no: display_form_no.to_string(),
            form_name: form_name.to_string(),
            period,
            period_start: Some(period_start),
            period_end: Some(period_end),
            deadline: DeadlineKind::Dated {
                original_deadline,
                final_deadline,
            },
            status,
            description: description.to_string(),
            source_reference: None,
        }
    }

    fn event_based(
        display_form_no: &str,
        form_name: &str,
        trigger: &str,
        statutory_window: &str,
        description: &str,
    ) -> Self {
        Self {
            form_code: canonical_form_code(display_form_no).to_string(),
            display_form_no: display_form_no.to_string(),
            form_name: form_name.to_string(),
            period: DeadlinePeriod::EventBased,
            period_start: None,
            period_end: None,
            deadline: DeadlineKind::EventBased {
                trigger: trigger.to_string(),
                statutory_window: statutory_window.to_string(),
            },
            status: DeadlineStatus::EventBased,
            description: description.to_string(),
            source_reference: None,
        }
    }

    pub fn final_deadline_date(&self) -> Option<NaiveDate> {
        match self.deadline {
            DeadlineKind::Dated { final_deadline, .. } => Some(final_deadline),
            DeadlineKind::EventBased { .. } => None,
        }
    }

    pub fn original_deadline_date(&self) -> Option<NaiveDate> {
        match self.deadline {
            DeadlineKind::Dated {
                original_deadline, ..
            } => Some(original_deadline),
            DeadlineKind::EventBased { .. } => None,
        }
    }

    pub fn final_deadline_string(&self) -> String {
        self.final_deadline_date()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "Event Based".to_string())
    }

    pub fn period_start_string(&self) -> String {
        self.period_start
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "-".to_string())
    }

    pub fn period_end_string(&self) -> String {
        self.period_end
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "-".to_string())
    }

    pub fn route_year_quarter(&self) -> Option<(u16, u8)> {
        match self.period {
            DeadlinePeriod::Monthly {
                taxable_year,
                month,
            } => Some((taxable_year as u16, month)),
            DeadlinePeriod::Quarterly {
                taxable_year,
                quarter,
            } => Some((taxable_year as u16, quarter)),
            DeadlinePeriod::Annual { taxable_year } => Some((taxable_year as u16, 1)),
            DeadlinePeriod::EventBased => None,
        }
    }

    pub fn matches_period_filter(&self, months: &[u8], quarters: &[u8]) -> bool {
        if months.is_empty() && quarters.is_empty() {
            return true;
        }

        let month_match = !months.is_empty() && self.period.matches_month_filter(months);
        let quarter_match = !quarters.is_empty()
            && self
                .period
                .quarter()
                .is_some_and(|quarter| quarters.contains(&quarter));

        month_match || quarter_match
    }

    /// Filter by the actual deadline DATE's month/quarter, not the filing period.
    /// This allows a quarterly form (e.g., 2551Q due Jul 25) to match a "July" month filter.
    pub fn matches_deadline_date_filter(&self, months: &[u8], quarters: &[u8]) -> bool {
        let Some(date) = self.final_deadline_date() else {
            // Event-based deadlines don't have a date; include them only when no filter is active
            return months.is_empty() && quarters.is_empty();
        };

        let date_month = date.month() as u8;
        let date_quarter = ((date_month - 1) / 3) + 1;

        let month_match = months.is_empty() || months.contains(&date_month);
        let quarter_match = quarters.is_empty() || quarters.contains(&date_quarter);

        month_match && quarter_match
    }

    fn apply_business_day_calendar(&mut self, calendar: &BusinessDayCalendar) {
        let DeadlineKind::Dated {
            original_deadline,
            final_deadline,
        } = &mut self.deadline
        else {
            return;
        };

        let (adjusted, status) = calendar.adjust_to_next_business_day(*original_deadline);
        *final_deadline = adjusted;
        self.status = status;
    }

    fn apply_override(
        &mut self,
        deadline_override: &DeadlineOverride,
        calendar: &BusinessDayCalendar,
    ) -> bool {
        if !deadline_override
            .affected_form_codes
            .iter()
            .any(|code| code == &self.form_code)
        {
            return false;
        }

        let DeadlineKind::Dated {
            original_deadline,
            final_deadline,
        } = &mut self.deadline
        else {
            return false;
        };

        if *original_deadline != deadline_override.original_deadline
            && *final_deadline != deadline_override.original_deadline
        {
            return false;
        }

        *final_deadline = calendar
            .adjust_to_next_business_day(deadline_override.adjusted_deadline)
            .0;
        self.status = DeadlineStatus::Extended;
        self.source_reference = Some(deadline_override.source_reference.clone());
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frequency {
    Monthly,
    Quarterly,
    Annual,
    AsNeeded,
}

pub struct OfficialRule {
    pub form_nos: Vec<&'static str>,
    pub form_name: &'static str,
    pub frequency: Frequency,
    pub description: &'static str,
    pub generator: fn(i32, &[&str], &str, &str) -> Vec<ResolvedTaxDeadline>,
}

pub struct DeadlineResolver;

impl DeadlineResolver {
    /// Compatibility alias for old call sites. New code should choose the
    /// taxable-year or calendar-year API explicitly.
    #[deprecated(note = "use resolve_taxable_year or resolve_deadline_calendar_year explicitly")]
    pub fn resolve_year(year: i32) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_taxable_year(year)
    }

    pub fn resolve_taxable_year(taxable_year: i32) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_taxable_year_with_overrides(taxable_year, &[])
    }

    pub fn resolve_taxable_year_with_calendar(
        taxable_year: i32,
        calendar: &BusinessDayCalendar,
    ) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_taxable_year_with_overrides_and_calendar(taxable_year, &[], calendar)
    }

    pub fn resolve_taxable_year_with_overrides(
        taxable_year: i32,
        overrides: &[DeadlineOverride],
    ) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_taxable_year_with_overrides_and_calendar(
            taxable_year,
            overrides,
            &BusinessDayCalendar::default(),
        )
    }

    pub fn resolve_taxable_year_with_overrides_and_calendar(
        taxable_year: i32,
        overrides: &[DeadlineOverride],
        calendar: &BusinessDayCalendar,
    ) -> Vec<ResolvedTaxDeadline> {
        let mut all_deadlines = Vec::new();

        for rule in Self::official_rules() {
            let deadlines = (rule.generator)(
                taxable_year,
                &rule.form_nos,
                rule.description,
                rule.form_name,
            );
            all_deadlines.extend(deadlines);
        }

        Self::apply_business_day_calendar(&mut all_deadlines, calendar);
        Self::apply_overrides(&mut all_deadlines, overrides, calendar);
        Self::sort_deadlines(all_deadlines)
    }

    pub fn resolve_deadline_calendar_year(calendar_year: i32) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_deadline_calendar_year_with_overrides(calendar_year, &[])
    }

    pub fn resolve_deadline_calendar_year_with_calendar(
        calendar_year: i32,
        calendar: &BusinessDayCalendar,
    ) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_deadline_calendar_year_with_overrides_and_calendar(
            calendar_year,
            &[],
            calendar,
        )
    }

    pub fn resolve_deadline_calendar_year_with_overrides(
        calendar_year: i32,
        overrides: &[DeadlineOverride],
    ) -> Vec<ResolvedTaxDeadline> {
        Self::resolve_deadline_calendar_year_with_overrides_and_calendar(
            calendar_year,
            overrides,
            &BusinessDayCalendar::default(),
        )
    }

    pub fn resolve_deadline_calendar_year_with_overrides_and_calendar(
        calendar_year: i32,
        overrides: &[DeadlineOverride],
        calendar: &BusinessDayCalendar,
    ) -> Vec<ResolvedTaxDeadline> {
        let mut all_deadlines = Vec::new();

        for taxable_year in (calendar_year - 1)..=(calendar_year + 1) {
            all_deadlines.extend(Self::resolve_taxable_year_with_overrides_and_calendar(
                taxable_year,
                overrides,
                calendar,
            ));
        }

        let deadlines = all_deadlines
            .into_iter()
            .filter(|deadline| {
                deadline
                    .final_deadline_date()
                    .is_some_and(|date| date.year() == calendar_year)
            })
            .collect();

        Self::sort_deadlines(deadlines)
    }

    pub fn event_based_obligations() -> Vec<ResolvedTaxDeadline> {
        Self::official_rules()
            .into_iter()
            .filter(|rule| rule.frequency == Frequency::AsNeeded)
            .flat_map(|rule| (rule.generator)(0, &rule.form_nos, rule.description, rule.form_name))
            .collect()
    }

    pub fn official_rules() -> Vec<OfficialRule> {
        vec![
            // NOTE: Form 2000 (DST) is NOT a recurring monthly obligation.
            // It is event-based — only filed when a taxable document transaction
            // occurs in a given month. Moved to the AsNeeded bucket below.
            OfficialRule {
                form_nos: vec!["0620", "1600-VT", "1600-PT", "1606", "2200-C"],
                form_name: "Monthly Remittance / Excise Tax",
                frequency: Frequency::Monthly,
                description: "10th day of the following month",
                generator: |year, forms, desc, name| {
                    Self::gen_monthly(year, forms, desc, name, |y, m| {
                        let (ty, tm) = next_month(y, m);
                        NaiveDate::from_ymd_opt(ty, tm, 10).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["0619-E", "0619-F", "1601-C"],
                form_name: "Withholding Tax Remittance",
                frequency: Frequency::Monthly,
                description: "10th day of the following month (Non-eFPS) / 15th for Dec",
                generator: |year, forms, desc, name| {
                    Self::gen_monthly(year, forms, desc, name, |y, m| {
                        let (ty, tm) = next_month(y, m);
                        let day = if m == 12 { 15 } else { 10 };
                        NaiveDate::from_ymd_opt(ty, tm, day).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["1601-EQ", "1601-FQ", "1602Q", "1603Q", "1621"],
                form_name: "Quarterly Withholding Tax",
                frequency: Frequency::Quarterly,
                description: "Last day of the month following the close of the quarter",
                generator: |year, forms, desc, name| {
                    Self::gen_quarterly(year, forms, desc, name, |y, q| {
                        let (ty, tm) = if q == 4 { (y + 1, 1) } else { (y, q * 3 + 1) };
                        Self::last_day_of_month(ty, tm)
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["1701Q"],
                form_name: "Quarterly Income Tax Return (Individual)",
                frequency: Frequency::Quarterly,
                description: "Q1: May 15, Q2: Aug 15, Q3: Oct 15",
                generator: |year, forms, desc, name| {
                    let mut res = Vec::new();
                    let due_dates = [(1, 5, 15), (2, 8, 15), (3, 10, 15)];
                    for &form_no in forms {
                        for (quarter, due_month, due_day) in due_dates {
                            let end_month = quarter * 3;
                            let start = NaiveDate::from_ymd_opt(year, end_month - 2, 1).unwrap();
                            let end = Self::last_day_of_month(year, end_month);
                            let deadline =
                                NaiveDate::from_ymd_opt(year, due_month, due_day).unwrap();
                            res.push(ResolvedTaxDeadline::dated(
                                form_no,
                                name,
                                DeadlinePeriod::Quarterly {
                                    taxable_year: year,
                                    quarter: quarter as u8,
                                },
                                start,
                                end,
                                deadline,
                                desc,
                            ));
                        }
                    }
                    res
                },
            },
            OfficialRule {
                form_nos: vec!["1702Q"],
                form_name: "Quarterly Income Tax Return (Corporate)",
                frequency: Frequency::Quarterly,
                description: "60 days after the end of each quarter",
                generator: |year, forms, desc, name| {
                    Self::gen_quarterly(year, forms, desc, name, |y, q| {
                        let end = Self::last_day_of_month(y, q * 3);
                        end + Duration::days(60)
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["2550-DS", "2550Q", "2551Q"],
                form_name: "Quarterly Percentage/Value-Added Tax",
                frequency: Frequency::Quarterly,
                description: "25th day following the close of taxable quarter",
                generator: |year, forms, desc, name| {
                    Self::gen_quarterly(year, forms, desc, name, |y, q| {
                        let (ty, tm) = if q == 4 { (y + 1, 1) } else { (y, q * 3 + 1) };
                        NaiveDate::from_ymd_opt(ty, tm, 25).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["2200-M"],
                form_name: "Quarterly Excise Tax Return (Minerals)",
                frequency: Frequency::Quarterly,
                description: "15th day following the close of calendar quarter",
                generator: |year, forms, desc, name| {
                    Self::gen_quarterly(year, forms, desc, name, |y, q| {
                        let (ty, tm) = if q == 4 { (y + 1, 1) } else { (y, q * 3 + 1) };
                        NaiveDate::from_ymd_opt(ty, tm, 15).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["1604-C", "1604-F"],
                form_name: "Annual Information Return",
                frequency: Frequency::Annual,
                description: "January 31 following the calendar year",
                generator: |year, forms, desc, name| {
                    Self::gen_annual(year, forms, desc, name, |y| {
                        NaiveDate::from_ymd_opt(y + 1, 1, 31).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["1604-E"],
                form_name: "Annual Information Return",
                frequency: Frequency::Annual,
                description: "March 1 following the calendar year",
                generator: |year, forms, desc, name| {
                    Self::gen_annual(year, forms, desc, name, |y| {
                        NaiveDate::from_ymd_opt(y + 1, 3, 1).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec![
                    "1700", "1701", "1701A", "1701-MS", "1702-EX", "1702-MX", "1702-RT", "1707-A",
                ],
                form_name: "Annual Income Tax Return",
                frequency: Frequency::Annual,
                description: "April 15 / 15th day of 4th month following taxable year",
                generator: |year, forms, desc, name| {
                    Self::gen_annual(year, forms, desc, name, |y| {
                        NaiveDate::from_ymd_opt(y + 1, 4, 15).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec!["2316"],
                form_name: "Certificate of Compensation Payment",
                frequency: Frequency::Annual,
                description: "February 28 (submission to BIR)",
                generator: |year, forms, desc, name| {
                    Self::gen_annual(year, forms, desc, name, |y| {
                        let d = if NaiveDate::from_ymd_opt(y + 1, 2, 29).is_some() {
                            29
                        } else {
                            28
                        };
                        NaiveDate::from_ymd_opt(y + 1, 2, d).unwrap()
                    })
                },
            },
            OfficialRule {
                form_nos: vec![
                    "2552", "1600-WP", "1706", "1707", "1800", "1801", "0605", "0611-A", "0613",
                    "2200-A", "2200-AN", "2200-M", "2200P", "2200-S", "2200-T", "2553", "2000",
                ],
                form_name: "As Needed / Special / Event-Based",
                frequency: Frequency::AsNeeded,
                description: "Based on Special Law / Upon transaction",
                generator: |_year, forms, desc, name| {
                    forms
                        .iter()
                        .map(|form_no| {
                            ResolvedTaxDeadline::event_based(
                                form_no,
                                name,
                                "Taxpayer transaction or statutory event",
                                "Depends on the specific BIR form and triggering event",
                                desc,
                            )
                        })
                        .collect()
                },
            },
        ]
    }

    fn sort_deadlines(mut deadlines: Vec<ResolvedTaxDeadline>) -> Vec<ResolvedTaxDeadline> {
        deadlines.sort_by(
            |a, b| match (a.final_deadline_date(), b.final_deadline_date()) {
                (Some(a_date), Some(b_date)) => a_date
                    .cmp(&b_date)
                    .then_with(|| a.form_code.cmp(&b.form_code)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.form_code.cmp(&b.form_code),
            },
        );
        deadlines
    }

    fn apply_business_day_calendar(
        deadlines: &mut [ResolvedTaxDeadline],
        calendar: &BusinessDayCalendar,
    ) {
        for deadline in deadlines.iter_mut() {
            deadline.apply_business_day_calendar(calendar);
        }
    }

    fn apply_overrides(
        deadlines: &mut [ResolvedTaxDeadline],
        overrides: &[DeadlineOverride],
        calendar: &BusinessDayCalendar,
    ) {
        for deadline_override in overrides {
            if deadline_override.source_reference.trim().is_empty() {
                continue;
            }

            for deadline in deadlines.iter_mut() {
                deadline.apply_override(deadline_override, calendar);
            }
        }
    }

    /// Returns default 2026 Philippine holiday overrides that shift BIR deadlines.
    /// These cover major holidays where BIR offices are closed and deadlines
    /// are automatically moved to the next business day.
    pub fn seed_2026_holiday_overrides() -> Vec<DeadlineOverride> {
        use uuid::Uuid;

        // Helper: build one override that shifts ALL form deadlines on a holiday
        let mut overrides = Vec::new();

        // All form codes that could have a deadline on a holiday
        let all_monthly_forms: Vec<String> =
            vec!["2000", "0619E", "0619F", "1601C", "0620", "1606"]
                .into_iter()
                .map(String::from)
                .collect();

        let all_quarterly_forms: Vec<String> = vec![
            "2551Q", "2550Q", "1701Q", "1601EQ", "1601FQ", "1602Q", "1603Q",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        // 2026 PH Regular Holidays that could affect BIR deadlines
        let holidays: Vec<(&str, NaiveDate, &str)> = vec![
            // New Year's Day — Jan 1
            (
                "New Year's Day 2026",
                NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                "RR 2026-001",
            ),
            // Araw ng Kagitingan — Apr 9
            (
                "Araw ng Kagitingan 2026",
                NaiveDate::from_ymd_opt(2026, 4, 9).unwrap(),
                "RR 2026-001",
            ),
            // Maundy Thursday — Apr 2, 2026
            (
                "Maundy Thursday 2026",
                NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
                "RR 2026-001",
            ),
            // Good Friday — Apr 3, 2026
            (
                "Good Friday 2026",
                NaiveDate::from_ymd_opt(2026, 4, 3).unwrap(),
                "RR 2026-001",
            ),
            // Labor Day — May 1
            (
                "Labor Day 2026",
                NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
                "RR 2026-001",
            ),
            // Independence Day — Jun 12
            (
                "Independence Day 2026",
                NaiveDate::from_ymd_opt(2026, 6, 12).unwrap(),
                "RR 2026-001",
            ),
            // National Heroes Day — Aug 31 (last Monday of August)
            (
                "National Heroes Day 2026",
                NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
                "RR 2026-001",
            ),
            // Bonifacio Day — Nov 30
            (
                "Bonifacio Day 2026",
                NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
                "RR 2026-001",
            ),
            // Christmas Day — Dec 25
            (
                "Christmas Day 2026",
                NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(),
                "RR 2026-001",
            ),
            // Rizal Day — Dec 30
            (
                "Rizal Day 2026",
                NaiveDate::from_ymd_opt(2026, 12, 30).unwrap(),
                "RR 2026-001",
            ),
        ];

        for (title, holiday_date, source) in holidays {
            // Next business day after the holiday
            let mut adjusted = holiday_date + chrono::Duration::days(1);
            // Skip weekends
            while adjusted.weekday() == chrono::Weekday::Sat
                || adjusted.weekday() == chrono::Weekday::Sun
            {
                adjusted += chrono::Duration::days(1);
            }

            let mut affected = all_monthly_forms.clone();
            affected.extend(all_quarterly_forms.clone());

            overrides.push(DeadlineOverride {
                id: Uuid::new_v4().to_string(),
                title: title.to_string(),
                source_reference: source.to_string(),
                affected_form_codes: affected,
                original_deadline: holiday_date,
                adjusted_deadline: adjusted,
                affected_regions: vec![],
                affected_taxpayer_types: vec![],
                effective_from: None,
                effective_until: None,
                expires_at: None,
            });
        }

        overrides
    }

    fn gen_monthly(
        year: i32,
        forms: &[&str],
        desc: &str,
        name: &str,
        f: impl Fn(i32, u32) -> NaiveDate,
    ) -> Vec<ResolvedTaxDeadline> {
        let mut res = Vec::new();
        for &form_no in forms {
            for month in 1..=12 {
                let start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                let end = Self::last_day_of_month(year, month);
                let deadline = f(year, month);
                res.push(ResolvedTaxDeadline::dated(
                    form_no,
                    name,
                    DeadlinePeriod::Monthly {
                        taxable_year: year,
                        month: month as u8,
                    },
                    start,
                    end,
                    deadline,
                    desc,
                ));
            }
        }
        res
    }

    fn gen_quarterly(
        year: i32,
        forms: &[&str],
        desc: &str,
        name: &str,
        f: impl Fn(i32, u32) -> NaiveDate,
    ) -> Vec<ResolvedTaxDeadline> {
        let mut res = Vec::new();
        for &form_no in forms {
            for quarter in 1..=4 {
                let start_month = (quarter - 1) * 3 + 1;
                let start = NaiveDate::from_ymd_opt(year, start_month, 1).unwrap();
                let end = Self::last_day_of_month(year, start_month + 2);
                let deadline = f(year, quarter);
                res.push(ResolvedTaxDeadline::dated(
                    form_no,
                    name,
                    DeadlinePeriod::Quarterly {
                        taxable_year: year,
                        quarter: quarter as u8,
                    },
                    start,
                    end,
                    deadline,
                    desc,
                ));
            }
        }
        res
    }

    fn gen_annual(
        year: i32,
        forms: &[&str],
        desc: &str,
        name: &str,
        f: impl Fn(i32) -> NaiveDate,
    ) -> Vec<ResolvedTaxDeadline> {
        let mut res = Vec::new();
        for &form_no in forms {
            let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
            let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
            let deadline = f(year);
            res.push(ResolvedTaxDeadline::dated(
                form_no,
                name,
                DeadlinePeriod::Annual { taxable_year: year },
                start,
                end,
                deadline,
                desc,
            ));
        }
        res
    }

    fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
        let (ny, nm) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        NaiveDate::from_ymd_opt(ny, nm, 1).unwrap() - Duration::days(1)
    }
}

pub fn canonical_form_code(display_form_no: &str) -> &'static str {
    match display_form_no {
        "0619-E" => "0619E",
        "0619-F" => "0619F",
        "1600-WP" => "1600WP",
        "1601-C" => "1601C",
        "1601-EQ" => "1601EQ",
        "1601-FQ" => "1601FQ",
        "1604-C" | "1604-F" => "1604CF",
        "1604-E" => "1604E",
        "1701-MS" => "1701MS",
        "1702-EX" => "1702EX",
        "1702-MX" => "1702MX",
        "1702-RT" => "1702RT",
        "1707-A" => "1707A",
        "2000-OT" => "2000",
        "2200-A" => "2200A",
        "2200-AN" => "2200AN",
        "2200-M" => "2200M",
        "2200-S" => "2200S",
        "2200-T" => "2200T",
        "2550-DS" => "2550DS",
        "1600-VT" | "1600-PT" => "1600",
        "2200-C" => "2200C",
        other => match other {
            "0605" => "0605",
            "0611-A" => "0611A",
            "0613" => "0613",
            "0620" => "0620",
            "1606" => "1606",
            "1602Q" => "1602",
            "1603Q" => "1603",
            "1621" => "1621",
            "1700" => "1700",
            "1701" => "1701",
            "1701A" => "1701A",
            "1701Q" => "1701Q",
            "1702Q" => "1702Q",
            "1706" => "1706",
            "1707" => "1707",
            "1800" => "1800",
            "1801" => "1801",
            "2000" => "2000",
            "2200P" => "2200P",
            "2316" => "2316",
            "2550Q" => "2550Q",
            "2551Q" => "2551Q",
            "2552" => "2552",
            "2553" => "2553",
            _ => "UNKNOWN",
        },
    }
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_year_includes_prior_taxable_year_annual_itr() {
        let deadlines = DeadlineResolver::resolve_deadline_calendar_year(2026);
        let annual_1701 = deadlines.iter().find(|d| {
            d.form_code == "1701" && d.period == (DeadlinePeriod::Annual { taxable_year: 2025 })
        });

        let annual_1701 = annual_1701.expect("1701 annual taxable year 2025 deadline missing");
        assert_eq!(
            annual_1701.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 4, 15).unwrap())
        );
    }

    #[test]
    fn calendar_year_includes_prior_december_monthly_deadlines() {
        let deadlines = DeadlineResolver::resolve_deadline_calendar_year(2026);
        let december_1601c = deadlines.iter().find(|d| {
            d.form_code == "1601C"
                && d.period
                    == (DeadlinePeriod::Monthly {
                        taxable_year: 2025,
                        month: 12,
                    })
        });

        let december_1601c = december_1601c.expect("December 2025 1601C deadline missing");
        assert_eq!(
            december_1601c.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())
        );
    }

    #[test]
    fn calendar_year_excludes_next_calendar_year_deadlines() {
        let deadlines = DeadlineResolver::resolve_deadline_calendar_year(2026);
        assert!(!deadlines.iter().any(|d| {
            d.form_code == "1701"
                && d.period == (DeadlinePeriod::Annual { taxable_year: 2026 })
                && d.final_deadline_date() == Some(NaiveDate::from_ymd_opt(2027, 4, 15).unwrap())
        }));
    }

    #[test]
    fn q1_1701q_keeps_taxable_quarter_metadata() {
        let deadlines = DeadlineResolver::resolve_taxable_year(2026);
        let q1 = deadlines
            .iter()
            .find(|d| {
                d.form_code == "1701Q"
                    && d.period
                        == (DeadlinePeriod::Quarterly {
                            taxable_year: 2026,
                            quarter: 1,
                        })
            })
            .expect("1701Q Q1 missing");

        assert_eq!(
            q1.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert!(q1.matches_period_filter(&[], &[1]));
        assert!(!q1.matches_period_filter(&[], &[2]));
        assert!(q1.matches_period_filter(&[1], &[]));
        assert!(q1.matches_period_filter(&[2], &[]));
        assert!(q1.matches_period_filter(&[3], &[]));
        assert!(!q1.matches_period_filter(&[4], &[]));
        assert!(!q1.matches_period_filter(&[5], &[]));
    }

    #[test]
    fn weekend_adjustment_preserves_original_deadline() {
        let deadlines = DeadlineResolver::resolve_taxable_year(2026);
        let q2_1701q = deadlines
            .iter()
            .find(|d| {
                d.form_code == "1701Q"
                    && d.period
                        == (DeadlinePeriod::Quarterly {
                            taxable_year: 2026,
                            quarter: 2,
                        })
            })
            .expect("1701Q Q2 missing");

        assert_eq!(
            q2_1701q.original_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 8, 15).unwrap())
        );
        assert_eq!(
            q2_1701q.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 8, 17).unwrap())
        );
        assert_eq!(q2_1701q.status, DeadlineStatus::WeekendAdjusted);
    }

    #[test]
    fn business_day_calendar_moves_saturday_and_sunday_to_monday() {
        let calendar = BusinessDayCalendar::default();

        let saturday = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let monday = NaiveDate::from_ymd_opt(2026, 8, 17).unwrap();

        assert_eq!(
            calendar.adjust_to_next_business_day(saturday),
            (monday, DeadlineStatus::WeekendAdjusted)
        );
        assert_eq!(
            calendar.adjust_to_next_business_day(sunday),
            (monday, DeadlineStatus::WeekendAdjusted)
        );
    }

    #[test]
    fn business_day_calendar_keeps_advancing_after_weekend_into_holiday() {
        let calendar =
            BusinessDayCalendar::with_non_working_days(vec![NonWorkingDay::regular_holiday(
                NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
                "Configured Monday holiday",
            )]);

        assert_eq!(
            calendar.adjust_to_next_business_day(NaiveDate::from_ymd_opt(2026, 5, 30).unwrap()),
            (
                NaiveDate::from_ymd_opt(2026, 6, 2).unwrap(),
                DeadlineStatus::WeekendAdjusted
            )
        );
    }

    #[test]
    fn business_day_calendar_moves_friday_holiday_past_following_weekend() {
        let calendar =
            BusinessDayCalendar::with_non_working_days(vec![NonWorkingDay::regular_holiday(
                NaiveDate::from_ymd_opt(2026, 6, 12).unwrap(),
                "Configured Friday holiday",
            )]);

        assert_eq!(
            calendar.adjust_to_next_business_day(NaiveDate::from_ymd_opt(2026, 6, 12).unwrap()),
            (
                NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
                DeadlineStatus::HolidayAdjusted
            )
        );
    }

    #[test]
    fn business_day_calendar_distinguishes_special_non_working_days() {
        let calendar = BusinessDayCalendar::with_non_working_days(vec![
            NonWorkingDay::special_non_working_day(
                NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(),
                "Configured special non-working day",
            ),
        ]);

        assert_eq!(
            calendar.adjust_to_next_business_day(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()),
            (
                NaiveDate::from_ymd_opt(2026, 11, 3).unwrap(),
                DeadlineStatus::NonWorkingDayAdjusted
            )
        );
    }

    #[test]
    fn q1_1702q_2026_keeps_weekend_adjusted_june_first_deadline() {
        let deadlines = DeadlineResolver::resolve_taxable_year(2026);
        let q1_1702q = deadlines
            .iter()
            .find(|d| {
                d.form_code == "1702Q"
                    && d.period
                        == (DeadlinePeriod::Quarterly {
                            taxable_year: 2026,
                            quarter: 1,
                        })
            })
            .expect("1702Q Q1 missing");

        assert_eq!(
            q1_1702q.original_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 5, 30).unwrap())
        );
        assert_eq!(
            q1_1702q.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
        );
        assert_eq!(q1_1702q.status, DeadlineStatus::WeekendAdjusted);
    }

    #[test]
    fn event_based_obligations_are_not_dated_deadlines() {
        let calendar = DeadlineResolver::resolve_deadline_calendar_year(2026);
        assert!(calendar.iter().all(|d| d.final_deadline_date().is_some()));

        let events = DeadlineResolver::event_based_obligations();
        assert!(events.iter().any(|d| d.form_code == "1800"));
        assert!(events.iter().all(|d| d.final_deadline_date().is_none()));
    }

    #[test]
    fn known_display_codes_canonicalize_for_app_routing() {
        assert_eq!(canonical_form_code("1601-C"), "1601C");
        assert_eq!(canonical_form_code("0619-E"), "0619E");
        assert_eq!(canonical_form_code("1701-MS"), "1701MS");
        assert_eq!(canonical_form_code("1702-RT"), "1702RT");
    }

    #[test]
    fn every_official_rule_has_canonical_form_code() {
        for rule in DeadlineResolver::official_rules() {
            for form_no in rule.form_nos {
                assert_ne!(
                    canonical_form_code(form_no),
                    "UNKNOWN",
                    "missing canonical mapping for {form_no}"
                );
            }
        }
    }

    #[test]
    fn override_extends_matching_deadline_with_source_reference() {
        let override_rule = DeadlineOverride {
            id: "test-extension".to_string(),
            title: "Test extension".to_string(),
            source_reference: "BIR test advisory".to_string(),
            affected_form_codes: vec!["1701Q".to_string()],
            original_deadline: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            adjusted_deadline: NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            affected_regions: Vec::new(),
            affected_taxpayer_types: Vec::new(),
            effective_from: None,
            effective_until: None,
            expires_at: None,
        };

        let deadlines =
            DeadlineResolver::resolve_taxable_year_with_overrides(2026, &[override_rule]);
        let q1 = deadlines
            .iter()
            .find(|d| {
                d.form_code == "1701Q"
                    && d.period
                        == (DeadlinePeriod::Quarterly {
                            taxable_year: 2026,
                            quarter: 1,
                        })
            })
            .expect("1701Q Q1 missing");

        assert_eq!(
            q1.original_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert_eq!(
            q1.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 15).unwrap())
        );
        assert!(q1.matches_period_filter(&[], &[1]));
        assert!(q1.matches_period_filter(&[1, 2, 3], &[]));
        assert!(!q1.matches_period_filter(&[6], &[]));
        assert_eq!(q1.status, DeadlineStatus::Extended);
        assert_eq!(q1.source_reference.as_deref(), Some("BIR test advisory"));
    }

    #[test]
    fn override_adjusted_date_is_moved_to_next_business_day() {
        let calendar =
            BusinessDayCalendar::with_non_working_days(vec![NonWorkingDay::regular_holiday(
                NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
                "Configured extension-day holiday",
            )]);
        let override_rule = DeadlineOverride {
            id: "test-extension-holiday".to_string(),
            title: "Test extension onto holiday".to_string(),
            source_reference: "BIR test advisory".to_string(),
            affected_form_codes: vec!["1701Q".to_string()],
            original_deadline: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            adjusted_deadline: NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            affected_regions: Vec::new(),
            affected_taxpayer_types: Vec::new(),
            effective_from: None,
            effective_until: None,
            expires_at: None,
        };

        let deadlines = DeadlineResolver::resolve_taxable_year_with_overrides_and_calendar(
            2026,
            &[override_rule],
            &calendar,
        );
        let q1 = deadlines
            .iter()
            .find(|d| {
                d.form_code == "1701Q"
                    && d.period
                        == (DeadlinePeriod::Quarterly {
                            taxable_year: 2026,
                            quarter: 1,
                        })
            })
            .expect("1701Q Q1 missing");

        assert_eq!(
            q1.original_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 5, 15).unwrap())
        );
        assert_eq!(
            q1.final_deadline_date(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 16).unwrap())
        );
        assert_eq!(q1.status, DeadlineStatus::Extended);
        assert_eq!(q1.source_reference.as_deref(), Some("BIR test advisory"));
    }
}
