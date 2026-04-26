use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxpayerClass {
    Micro,
    Small,
    Medium,
    Large,
    Regular,
}

impl Default for TaxpayerClass {
    fn default() -> Self {
        Self::Regular
    }
}
