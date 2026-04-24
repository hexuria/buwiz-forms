//! Taxpayer profile management.

use serde::{Deserialize, Serialize};
use crate::naming::Tin;

/// Taxpayer profile stored in encrypted SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxpayerProfile {
    pub id: Option<i64>,
    pub full_name: String,
    pub tin: Tin,
    pub rdo_code: String,
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub default_form_type: String,
}
