use chrono::NaiveDate;
use super::taxpayer::TaxpayerClass;
use super::compromise::lookup_compromise;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenaltyProfile {
    StandardFiling,
    Withholding,
}

#[derive(Debug, Clone)]
pub struct PenaltyContext {
    pub form_code: String,
    pub tax_type: PenaltyProfile,
    pub taxpayer_class: TaxpayerClass,
    pub taxable_period: String,
    
    // Dates
    pub due_date: NaiveDate,
    pub filing_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    
    // Financials
    pub basic_tax_due: f64,
    pub amount_paid_before_deadline: f64,
    pub gross_sales_or_receipts: f64,
    
    // Flags
    pub is_amended_return: bool,
    pub original_was_on_time: bool,
    pub is_fraud_or_willful_neglect: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PenaltyResult {
    pub surcharge: f64,
    pub interest: f64,
    pub compromise: f64,
    pub total_penalties: f64,
}

use super::config::PenaltyConfig;

pub struct PenaltyEngine;

impl PenaltyEngine {
    pub fn calculate(ctx: &PenaltyContext, config: &PenaltyConfig) -> PenaltyResult {
        let compare_date = ctx.payment_date.unwrap_or(ctx.filing_date);
        
        // 1. On-time check
        if compare_date <= ctx.due_date {
            return PenaltyResult::default();
        }
        
        // 2. Amended return check
        // "no penalty should be imposed on an amended return if the initial return and tax due were filed/paid on time"
        if ctx.is_amended_return && ctx.original_was_on_time {
            return PenaltyResult::default();
        }
        
        let days_late = (compare_date - ctx.due_date).num_days().max(0);
        if days_late == 0 {
            return PenaltyResult::default();
        }

        let is_micro_or_small = matches!(ctx.taxpayer_class, TaxpayerClass::Micro | TaxpayerClass::Small);

        // Multipliers and Rates from Config
        let surcharge_rate = config.get_surcharge_rate(ctx.taxpayer_class, ctx.is_fraud_or_willful_neglect);
        let interest_rate = config.get_interest_rate(ctx.taxpayer_class);
        let compromise_multiplier = if is_micro_or_small { 0.50 } else { 1.00 };

        let unpaid_tax = (ctx.basic_tax_due - ctx.amount_paid_before_deadline).max(0.0);

        let surcharge = if unpaid_tax > 0.0 { unpaid_tax * surcharge_rate } else { 0.0 };
        let interest = if unpaid_tax > 0.0 { unpaid_tax * interest_rate * (days_late as f64 / 365.0) } else { 0.0 };
        
        let base_compromise = lookup_compromise(unpaid_tax, ctx.gross_sales_or_receipts, ctx.tax_type);
        let compromise = base_compromise * compromise_multiplier;

        PenaltyResult {
            surcharge: (surcharge * 100.0).round() / 100.0,
            interest: (interest * 100.0).round() / 100.0,
            compromise: (compromise * 100.0).round() / 100.0,
            total_penalties: ((surcharge + interest + compromise) * 100.0).round() / 100.0,
        }
    }
}
