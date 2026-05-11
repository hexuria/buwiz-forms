use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesRecord {
    pub transaction_id: String,
    pub transaction_date: NaiveDate,
    pub amount: f64,
    pub vat_amount: f64,
    pub customer_tin: String,
    pub customer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseRecord {
    pub transaction_id: String,
    pub transaction_date: NaiveDate,
    pub amount: f64,
    pub category: String,
    pub supplier_tin: String,
    pub supplier_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollRecord {
    pub transaction_id: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub employee_tin: String,
    pub compensation: f64,
    pub tax_withheld: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationAudit {
    pub source_system: String,
    pub import_date: DateTime<Utc>,
    pub records_imported: usize,
    pub errors_encountered: usize,
}

pub trait ErpImporter {
    fn import_sales(&self, records: Vec<SalesRecord>) -> IntegrationAudit;
    fn import_expenses(&self, records: Vec<ExpenseRecord>) -> IntegrationAudit;
    fn import_payroll(&self, records: Vec<PayrollRecord>) -> IntegrationAudit;
}
