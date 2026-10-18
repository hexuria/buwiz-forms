//! The shared Surcharge / Interest / Compromise model for every form whose
//! return carries those three items.
//!
//! A form declares its items once, in a [`PenaltyItems`] value: which draft
//! fields hold the surcharge, interest and compromise, the amount the engine
//! is run on (the form's own tax still due before penalties), the compromise
//! schedule ([`PenaltyProfile`]), and how its period maps to the app's
//! deadline calendar. Everything else is shared, so every wired form behaves
//! identically:
//!
//! - **Computed default.** While the return is a Draft, [`PenaltyItems::apply`]
//!   (called from the form's own compute chain, just before it sums the
//!   penalties) asks the engine what the return owes if filed today and keeps
//!   that, with its inputs, in the draft's [`PenaltyState`].
//! - **Per-component override.** Each item files its override when one is
//!   set, else the computed value, so the XML, print and queue fingerprint
//!   carry what is filed. An optional note records the reason.
//! - **Clock.** A Queued return refreshes only at queue revalidation, where a
//!   change fails closed; Submitted, Confirmed and Paid returns never move.
//! - **No deadline, no computation.** A return whose period has no deadline in
//!   the app's calendar (event-based returns) computes nothing: its items are
//!   overrides only. A deadline is never invented.
//! - **Back-compat.** A draft saved while the items were manual-only (no
//!   `penalty_model_version`) keeps every non-zero hand-typed amount as an
//!   override, so nothing it carried changes value.
//! - **Typed trigger date (maintainer decision D8, 2026-10-18).** A form
//!   whose penalties run from a date the filer types, not from the calendar
//!   (the excise returns 2200A, 2200AN, 2200M, 2200P, 2200S and 2200T: the
//!   removal date; 2200C: the 10th of the month after its Item 1 month),
//!   declares [`PenaltyItems::typed_due_date`]. It is measured from that
//!   date once the draft is on [`TYPED_DUE_DATE_MODEL_VERSION`] (a new
//!   return, or one that joins); a draft stored before computes nothing
//!   until it joins, so it keeps what it holds.

use std::cell::Cell;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::FilingStatus;
use super::queueable::QueueableForm;
use crate::calendar_rules::{DeadlineOverride, DeadlinePeriod, DeadlineResolver};
use crate::penalties::{
    ComputedPenalties, PenaltyComponent, PenaltyOverrides, PenaltyProfile, PenaltyRequest,
    TaxpayerClass, compute_penalties,
};
use crate::profile::{EoptTier, TaxpayerProfile};

/// Current penalty model: computed default with a per-component override.
pub const PENALTY_MODEL_VERSION: u8 = 1;

/// The model of a form with a [`PenaltyItems::typed_due_date`] (decision
/// D8): computed from the date the filer typed. A draft of such a form
/// stored at [`PENALTY_MODEL_VERSION`] predates it and computes nothing.
pub const TYPED_DUE_DATE_MODEL_VERSION: u8 = 2;

/// The penalty engine's class for a profile's EOPT classification.
pub fn taxpayer_class_of(tier: &EoptTier) -> TaxpayerClass {
    match tier {
        EoptTier::Micro => TaxpayerClass::Micro,
        EoptTier::Small => TaxpayerClass::Small,
        EoptTier::Medium => TaxpayerClass::Medium,
        EoptTier::Large => TaxpayerClass::Large,
    }
}

/// What a wired draft stores about its penalties, next to the three items.
/// Embedded with `#[serde(flatten)]`, so the stored JSON keys are the same on
/// every form.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PenaltyState {
    /// `0` for a draft saved while the items were manual-only; its non-zero
    /// hand-typed values become overrides the first time it is computed.
    #[serde(default)]
    pub penalty_model_version: u8,
    /// The app's resolved deadline for this period (business-day and deadline
    /// overrides applied). `None` falls back to the standard calendar rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub penalty_due_date: Option<NaiveDate>,
    /// The profile's EOPT classification; `None` assumes the standard class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub penalty_taxpayer_class: Option<TaxpayerClass>,
    /// What the engine last computed, with its inputs. Frozen once the return
    /// leaves Draft, refreshed only by queue revalidation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computed_penalties: Option<ComputedPenalties>,
    /// Per-component overrides and the reason recorded with them.
    #[serde(default)]
    pub penalty_overrides: PenaltyOverrides,
}

impl PenaltyState {
    /// The state of a new draft: current model, the profile's class.
    pub fn new(taxpayer_class: Option<TaxpayerClass>) -> Self {
        Self {
            penalty_model_version: PENALTY_MODEL_VERSION,
            penalty_taxpayer_class: taxpayer_class,
            ..Self::default()
        }
    }

    /// Clear every component's override (the note stays): what the form's
    /// own handlers do when they clear and disable the penalty items.
    pub fn clear_overrides(&mut self) {
        for component in PenaltyComponent::ALL {
            self.penalty_overrides.set(component, None);
        }
    }

    /// [`Self::new`] with the class read from `profile`.
    pub fn for_profile(profile: &TaxpayerProfile) -> Self {
        Self::new(profile.eopt_tier.as_ref().map(taxpayer_class_of))
    }

    /// [`Self::for_profile`] for a new return of a form whose penalties run
    /// from a date the filer types (decision D8): on
    /// [`TYPED_DUE_DATE_MODEL_VERSION`] from the start.
    pub fn for_profile_typed_due_date(profile: &TaxpayerProfile) -> Self {
        Self {
            penalty_model_version: TYPED_DUE_DATE_MODEL_VERSION,
            ..Self::for_profile(profile)
        }
    }

    /// The state of a return imported from official XML: current model, every
    /// item kept as an override by [`PenaltyItems::adopt_imported`].
    pub fn imported() -> Self {
        Self::new(None)
    }
}

/// A form field that holds one penalty item's amount.
pub trait PenaltyField {
    fn penalty_value(&self) -> f64;
    fn set_penalty_value(&mut self, value: f64);
}

impl PenaltyField for f64 {
    fn penalty_value(&self) -> f64 {
        *self
    }
    fn set_penalty_value(&mut self, value: f64) {
        *self = value;
    }
}

/// How the engine's values are held in the form's items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenaltyRounding {
    /// Pesos and centavos.
    Cents,
    /// Whole pesos (the form's items take no centavos).
    WholePesos,
}

impl PenaltyRounding {
    fn round(self, value: f64) -> f64 {
        match self {
            Self::Cents => (value * 100.0).round() / 100.0,
            Self::WholePesos => value.round(),
        }
    }
}

/// A form's declaration of its Surcharge, Interest and Compromise items.
pub struct PenaltyItems<F> {
    /// The form code the app's deadline calendar uses (`"1601C"`).
    pub form_code: &'static str,
    /// Whose items these are on a return that carries a set per party
    /// (`"Spouse"`); `None` on a single-party return.
    pub party: Option<&'static str>,
    /// Whether this party's items are part of the return as it stands (the
    /// spouse's only on a joint return). Items that do not apply are left to
    /// the form's own rules and never computed.
    pub applies: fn(&F) -> bool,
    /// The compromise schedule (`compromise.rs`): withholding agents' returns
    /// use the withholding schedule, every other return the standard one.
    pub schedule: PenaltyProfile,
    /// Draft field names (stored JSON keys, agent `form.fill` keys) of the
    /// surcharge, interest and compromise items, in that order.
    pub fields: [&'static str; 3],
    /// Official control ids of the three items, in the same order.
    pub controls: [&'static str; 3],
    /// Item labels as the form prints them (`"32 Surcharge"`).
    pub labels: [&'static str; 3],
    /// The item the engine's basic tax due is read from (`"Item 31"`).
    pub basic_tax_due_label: &'static str,
    pub rounding: PenaltyRounding,
    pub get: fn(&F, PenaltyComponent) -> f64,
    pub set: fn(&mut F, PenaltyComponent, f64),
    /// The form's tax still due before penalties (the amount its total adds
    /// the penalties to).
    pub basic_tax_due: fn(&F) -> f64,
    /// Gross sales/receipts for the engine's no-tax-due compromise tier;
    /// `None` for a return that carries no such basis.
    pub gross_sales: Option<fn(&F) -> f64>,
    pub state: fn(&F) -> &PenaltyState,
    pub state_mut: fn(&mut F) -> &mut PenaltyState,
    pub status: fn(&F) -> FilingStatus,
    /// The calendar period this return is filed for; `None` when the app's
    /// calendar has no period for it (event-based returns).
    pub deadline_period: fn(&F) -> Option<DeadlinePeriod>,
    /// A due date the return carries itself, read from what the filer typed
    /// (decision D8: the excise returns' removal date). When set it is the
    /// only due date, used once the draft is on
    /// [`TYPED_DUE_DATE_MODEL_VERSION`]; `None` for every other form.
    pub typed_due_date: Option<fn(&F) -> Option<NaiveDate>>,
    pub period_code: fn(&F) -> String,
    pub is_amended: fn(&F) -> bool,
    /// The form's full compute chain (which calls [`Self::apply`]).
    pub recompute: fn(&mut F),
}

impl<F> Clone for PenaltyItems<F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<F> Copy for PenaltyItems<F> {}

/// The calendar period of a queueable form's filing period. Open-ended
/// (event) returns have none.
pub fn calendar_period<F: QueueableForm>(form: &F) -> Option<DeadlinePeriod> {
    let taxable_year = i32::from(form.taxable_year());
    match form.filing_period() {
        super::FilingPeriod::Monthly(month) => Some(DeadlinePeriod::Monthly {
            taxable_year,
            month,
        }),
        super::FilingPeriod::Quarterly(quarter) => Some(DeadlinePeriod::Quarterly {
            taxable_year,
            quarter,
        }),
        super::FilingPeriod::Annual => Some(DeadlinePeriod::Annual { taxable_year }),
        super::FilingPeriod::OpenEnded(_) => None,
    }
}

/// The clock the compute chain reads: today, and whether a non-Draft return
/// refreshes its computed values.
#[derive(Clone, Copy)]
struct Clock {
    today: NaiveDate,
    /// `None`: refresh only while the return is a Draft.
    refresh: Option<bool>,
}

thread_local! {
    static CLOCK: Cell<Option<Clock>> = const { Cell::new(None) };
}

fn with_clock_value<R>(clock: Clock, run: impl FnOnce() -> R) -> R {
    let previous = CLOCK.with(|cell| cell.replace(Some(clock)));
    let result = run();
    CLOCK.with(|cell| cell.set(previous));
    result
}

/// Today as the penalty model sees it (pinned by [`with_clock`] in tests).
pub fn today() -> NaiveDate {
    CLOCK
        .with(Cell::get)
        .map(|clock| clock.today)
        .unwrap_or_else(|| chrono::Local::now().date_naive())
}

/// Run `run` as if today were `today` (a Draft still refreshes, a Queued
/// return still waits for revalidation).
pub fn with_clock<R>(today: NaiveDate, run: impl FnOnce() -> R) -> R {
    with_clock_value(
        Clock {
            today,
            refresh: None,
        },
        run,
    )
}

/// The deadline `form_code`'s calendar resolves for `period`, with `overrides`.
pub fn resolve_due_date(
    form_code: &str,
    period: &DeadlinePeriod,
    overrides: &[DeadlineOverride],
) -> Option<NaiveDate> {
    let taxable_year = period.taxable_year()?;
    DeadlineResolver::resolve_taxable_year_with_overrides(taxable_year, overrides)
        .into_iter()
        .find(|deadline| deadline.form_code == form_code && &deadline.period == period)
        .and_then(|deadline| deadline.final_deadline_date())
}

impl<F> PenaltyItems<F> {
    fn state_of<'a>(&self, form: &'a F) -> &'a PenaltyState {
        (self.state)(form)
    }

    /// The deadline penalties are measured from: the one the app resolved
    /// when the form was opened, else the standard calendar rule. `None` when
    /// the calendar has no deadline for this return. A form with a
    /// [`Self::typed_due_date`] uses that alone, and only on a draft on its
    /// model ([`Self::on_typed_due_date`]).
    pub fn due_date(&self, form: &F) -> Option<NaiveDate> {
        if let Some(typed) = self.typed_due_date {
            return self.on_typed_due_date(form).then(|| typed(form)).flatten();
        }
        self.state_of(form).penalty_due_date.or_else(|| {
            (self.deadline_period)(form)
                .and_then(|period| resolve_due_date(self.form_code, &period, &[]))
        })
    }

    /// Whether a form with a [`Self::typed_due_date`] has this draft on that
    /// model: a new return, or one that joined ([`Self::join_typed_due_date`]).
    pub fn on_typed_due_date(&self, form: &F) -> bool {
        self.typed_due_date.is_some()
            && self.state_of(form).penalty_model_version >= TYPED_DUE_DATE_MODEL_VERSION
    }

    /// Put a Draft of a form with a [`Self::typed_due_date`] on that model,
    /// so its penalties are computed from the typed date from now on. A
    /// return that has left Draft, or a form without one, is left alone.
    pub fn join_typed_due_date(&self, form: &mut F) {
        if self.typed_due_date.is_none() || !matches!((self.status)(form), FilingStatus::Draft) {
            return;
        }
        let state = (self.state_mut)(form);
        state.penalty_model_version = state
            .penalty_model_version
            .max(TYPED_DUE_DATE_MODEL_VERSION);
    }

    /// What the engine computes if the return is filed (and paid) on
    /// `filing_date`. `None` when no deadline is known.
    pub fn if_filed_on(&self, form: &F, filing_date: NaiveDate) -> Option<ComputedPenalties> {
        let due_date = self.due_date(form)?;
        let mut computed = compute_penalties(&PenaltyRequest {
            form_code: self.form_code,
            taxable_period: (self.period_code)(form),
            schedule: self.schedule,
            taxpayer_class: self.state_of(form).penalty_taxpayer_class,
            due_date,
            filing_date,
            payment_date: None,
            basic_tax_due: (self.basic_tax_due)(form),
            gross_sales_or_receipts: self.gross_sales.map(|gross| gross(form)),
            is_amended_return: (self.is_amended)(form),
        });
        computed.surcharge = self.rounding.round(computed.surcharge);
        computed.interest = self.rounding.round(computed.interest);
        computed.compromise = self.rounding.round(computed.compromise);
        Some(computed)
    }

    /// The three items as they stand on the form.
    pub fn filed(&self, form: &F) -> [f64; 3] {
        PenaltyComponent::ALL.map(|component| (self.get)(form, component))
    }

    /// The value each item files: its override, else the computed value.
    /// An override is passed on as entered; the form's compute chain holds
    /// it the way it holds every typed amount.
    pub fn used(&self, form: &F) -> [f64; 3] {
        let state = self.state_of(form);
        let computed = state.computed_penalties.as_ref();
        PenaltyComponent::ALL.map(|component| state.penalty_overrides.used(component, computed))
    }

    /// Set (`Some`) or clear (`None`) one component's override.
    pub fn set_override(&self, form: &mut F, component: PenaltyComponent, value: Option<f64>) {
        (self.state_mut)(form)
            .penalty_overrides
            .set(component, value);
    }

    /// The component whose draft field is `field`, if any.
    pub fn component_of_field(&self, field: &str) -> Option<PenaltyComponent> {
        PenaltyComponent::ALL
            .into_iter()
            .zip(self.fields)
            .find_map(|(component, name)| (name == field).then_some(component))
    }

    /// Record the profile-derived inputs. A return that has left Draft keeps
    /// the inputs it was reviewed with.
    pub fn set_context(
        &self,
        form: &mut F,
        due_date: Option<NaiveDate>,
        taxpayer_class: Option<TaxpayerClass>,
    ) {
        if !matches!((self.status)(form), FilingStatus::Draft) {
            return;
        }
        let state = (self.state_mut)(form);
        state.penalty_due_date = due_date;
        state.penalty_taxpayer_class = taxpayer_class;
    }

    /// Resolve the inputs from the profile: the period's deadline as the
    /// Calendar resolves it (global and profile deadline overrides applied)
    /// and the profile's EOPT class.
    pub fn apply_profile(
        &self,
        form: &mut F,
        profile: &TaxpayerProfile,
        global_overrides: &[DeadlineOverride],
    ) {
        let due_date = (self.deadline_period)(form).and_then(|period| {
            let mut overrides = global_overrides.to_vec();
            if let Some(year) = period.taxable_year().and_then(|y| u16::try_from(y).ok()) {
                overrides.extend(crate::integration::profile_deadline_overrides_for_year(
                    profile, year,
                ));
            }
            resolve_due_date(self.form_code, &period, &overrides)
        });
        self.set_context(
            form,
            due_date,
            profile.eopt_tier.as_ref().map(taxpayer_class_of),
        );
    }

    /// An imported return carries the penalties it was prepared with: keep
    /// every item as an override so the source amounts round-trip.
    pub fn adopt_imported(&self, form: &mut F) {
        let filed = self.filed(form);
        let state = (self.state_mut)(form);
        for (component, value) in PenaltyComponent::ALL.into_iter().zip(filed) {
            state.penalty_overrides.set(component, Some(value));
        }
        state.penalty_model_version = PENALTY_MODEL_VERSION;
    }

    /// A draft saved while the items were manual-only: keep every non-zero
    /// hand-typed amount as an override so it files the same value.
    fn adopt_legacy_manual_penalties(&self, form: &mut F) {
        if self.state_of(form).penalty_model_version >= PENALTY_MODEL_VERSION {
            return;
        }
        let filed = self.filed(form);
        let state = (self.state_mut)(form);
        for (component, value) in PenaltyComponent::ALL.into_iter().zip(filed) {
            if value != 0.0 && state.penalty_overrides.get(component).is_none() {
                state.penalty_overrides.set(component, Some(value));
            }
        }
        state.penalty_model_version = PENALTY_MODEL_VERSION;
    }

    /// The compute-chain hook: call it where the form has its tax still due
    /// and is about to sum its penalties. Refreshes the computed values while
    /// the return is a Draft (or when the clock asks for it) and writes the
    /// used value into each item. Submitted, Confirmed and Paid returns keep
    /// the items they were filed with.
    pub fn apply(&self, form: &mut F) {
        // Items that are not part of the return (a spouse column on a
        // separate return) are left entirely to the form's own rules.
        if !(self.applies)(form) {
            return;
        }
        self.adopt_legacy_manual_penalties(form);
        let status = (self.status)(form);
        if !matches!(status, FilingStatus::Draft | FilingStatus::Queued) {
            return;
        }
        let clock = CLOCK.with(Cell::get);
        let refresh = clock
            .and_then(|clock| clock.refresh)
            .unwrap_or(matches!(status, FilingStatus::Draft));
        if refresh {
            let computed = self.if_filed_on(form, today());
            (self.state_mut)(form).computed_penalties = computed;
        }
        for (component, value) in PenaltyComponent::ALL.into_iter().zip(self.used(form)) {
            (self.set)(form, component, value);
        }
    }

    /// Run the form's compute chain as of `today`; `refresh` recomputes the
    /// engine's values even for a Queued return.
    pub fn compute_on(&self, form: &mut F, today: NaiveDate, refresh: bool) {
        with_clock_value(
            Clock {
                today,
                refresh: Some(refresh),
            },
            || (self.recompute)(form),
        );
    }

    /// The total penalties this unfiled return would carry if filed on
    /// `filing_date` (overrides honoured). `None` for a filed return or when
    /// nothing can be computed.
    pub fn payable_if_filed_on(&self, form: &F, filing_date: NaiveDate) -> Option<f64>
    where
        F: Clone,
    {
        if !matches!(
            (self.status)(form),
            FilingStatus::Draft | FilingStatus::Queued
        ) || !(self.applies)(form)
        {
            return None;
        }
        let mut estimate = form.clone();
        self.compute_on(&mut estimate, filing_date, true);
        let state = self.state_of(&estimate);
        let computed = state.computed_penalties.as_ref()?;
        Some(state.penalty_overrides.used_total(Some(computed)))
    }

    /// The error queue revalidation reports when the filed items moved.
    pub fn changed_error(&self, form: &F, today: NaiveDate) -> (String, String) {
        let [surcharge, interest, compromise] = self.filed(form);
        let party = self
            .party
            .map(|party| format!(" ({party})"))
            .unwrap_or_default();
        (
            "computed_penalties".to_string(),
            format!(
                "Computed penalties{party} changed since the return was queued (filing on {today}: surcharge {surcharge:.2}, interest {interest:.2}, compromise {compromise:.2}); review the return and queue it again"
            ),
        )
    }
}

/// Build the [`PenaltyItems`] of a [`QueueableForm`] whose items are
/// [`PenaltyField`]s. The status, period code, amended flag, compute chain and
/// calendar period come from the trait; trailing `name: value` pairs replace
/// any default (e.g. `deadline_period`, `gross_sales`, `rounding`).
macro_rules! queueable_penalty_items {
    (
        $ty:ty,
        schedule: $schedule:ident,
        items: [
            $($s:ident).+ : $sl:expr,
            $($i:ident).+ : $il:expr,
            $($c:ident).+ : $cl:expr $(,)?
        ],
        controls: [$sc:expr, $ic:expr, $cc:expr $(,)?],
        state: $($state:ident).+,
        basic_tax_due: $basic_label:expr, |$d:ident| $basic:expr
        $(, $field:ident: $value:expr)* $(,)?
    ) => {{
        use $crate::forms::penalty_items::PenaltyField as _;
        use $crate::forms::queueable::QueueableForm as _;
        use $crate::penalties::PenaltyComponent as C;
        #[allow(clippy::needless_update)]
        let items = $crate::forms::penalty_items::PenaltyItems::<$ty> {
            $($field: $value,)*
            ..$crate::forms::penalty_items::PenaltyItems::<$ty> {
                form_code: <$ty as $crate::forms::queueable::QueueableForm>::FORM_CODE,
                party: None,
                applies: |_: &$ty| true,
                schedule: $crate::penalties::PenaltyProfile::$schedule,
                fields: [
                    stringify!($($s).+),
                    stringify!($($i).+),
                    stringify!($($c).+),
                ],
                controls: [$sc, $ic, $cc],
                labels: [$sl, $il, $cl],
                basic_tax_due_label: $basic_label,
                rounding: $crate::forms::penalty_items::PenaltyRounding::Cents,
                get: |d: &$ty, component| match component {
                    C::Surcharge => d.$($s).+.penalty_value(),
                    C::Interest => d.$($i).+.penalty_value(),
                    C::Compromise => d.$($c).+.penalty_value(),
                },
                set: |d: &mut $ty, component, value| match component {
                    C::Surcharge => d.$($s).+.set_penalty_value(value),
                    C::Interest => d.$($i).+.set_penalty_value(value),
                    C::Compromise => d.$($c).+.set_penalty_value(value),
                },
                basic_tax_due: |$d: &$ty| $basic,
                gross_sales: None,
                state: |d: &$ty| &d.$($state).+,
                state_mut: |d: &mut $ty| &mut d.$($state).+,
                status: |d: &$ty| d.lifecycle().status.clone(),
                deadline_period: $crate::forms::penalty_items::calendar_period::<$ty>,
                typed_due_date: None,
                period_code: |d: &$ty| d.period_code(),
                is_amended: |d: &$ty| d.amended_return_answer().unwrap_or(false),
                recompute: |d: &mut $ty| d.compute(),
            }
        };
        items
    }};
}
pub(crate) use queueable_penalty_items;
