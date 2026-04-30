use crate::integration::models::UniversalTaxPayload;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub mod generic;

/// Configuration for a specific data source connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: Option<i64>,
    pub profile_tin: String,
    /// The ID of the provider (e.g., "generic", "taxman")
    pub provider_id: String,
    /// User-friendly name (e.g., "My QBO Account")
    pub name: String,
    /// Encrypted credentials and settings (URL, API Key, Token)
    pub credentials: BTreeMap<String, String>,
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Network request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("API returned error status: {0} - {1}")]
    ApiError(reqwest::StatusCode, String),
    #[error("Data mapping failed: {0}")]
    MappingError(String),
}

/// The core trait that all remote data source plugins must implement.
pub trait DataProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "taxman", "quickbooks", "generic").
    fn id(&self) -> &str;

    /// Human-readable name of the provider.
    fn name(&self) -> &str;

    /// Fetch the raw remote data and transform it into the UniversalTaxPayload.
    fn fetch_tax_data(
        &self,
        config: &ProviderConfig,
        tin: &str,
        period_start: &str,
        period_end: &str,
        target_form: Option<&str>,
    ) -> impl std::future::Future<Output = Result<UniversalTaxPayload, ProviderError>> + Send;
}

/// Static dispatch wrapper for available providers
pub enum ProviderInstance {
    Generic(generic::GenericProvider),
}

impl ProviderInstance {
    pub fn get(id: &str) -> Option<Self> {
        match id {
            "generic" => Some(ProviderInstance::Generic(generic::GenericProvider)),
            _ => None,
        }
    }

    pub async fn fetch_tax_data(
        &self,
        config: &ProviderConfig,
        tin: &str,
        period_start: &str,
        period_end: &str,
        target_form: Option<&str>,
    ) -> Result<UniversalTaxPayload, ProviderError> {
        match self {
            ProviderInstance::Generic(p) => {
                p.fetch_tax_data(config, tin, period_start, period_end, target_form)
                    .await
            }
        }
    }
}

pub async fn sync_from_provider(
    db: &crate::db::Database,
    config: &ProviderConfig,
    tin: &str,
    period_start: &str,
    period_end: &str,
    target_form: Option<&str>,
) -> Result<crate::integration::service::SyncResponse, ProviderError> {
    let provider = ProviderInstance::get(&config.provider_id).ok_or_else(|| {
        ProviderError::ConfigError(format!("Unknown provider: {}", config.provider_id))
    })?;

    let payload = provider
        .fetch_tax_data(config, tin, period_start, period_end, target_form)
        .await?;

    crate::integration::service::process_sync(db, &payload)
        .map_err(|e| ProviderError::MappingError(e.to_string()))
}
