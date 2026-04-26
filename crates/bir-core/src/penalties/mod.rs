pub mod engine;
pub mod taxpayer;
pub mod compromise;
pub mod config;

pub use config::PenaltyConfig;
pub use engine::{PenaltyContext, PenaltyEngine, PenaltyProfile, PenaltyResult};
pub use taxpayer::TaxpayerClass;
