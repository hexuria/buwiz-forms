use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum TaxpayerClass {
    Micro,
    Small,
    Medium,
    Large,
    #[default]
    Regular,
}

