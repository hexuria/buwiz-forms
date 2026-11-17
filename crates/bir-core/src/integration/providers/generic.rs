use super::{DataProvider, ProviderConfig, ProviderError};
use crate::integration::models::UniversalTaxPayload;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use std::time::Duration;

/// A generic provider that expects the remote URL to return a perfectly formatted
/// `UniversalTaxPayload` JSON response.
pub struct GenericProvider;

impl DataProvider for GenericProvider {
    fn id(&self) -> &str {
        "generic"
    }

    fn name(&self) -> &str {
        "Generic JSON API"
    }

    async fn fetch_tax_data(
        &self,
        config: &ProviderConfig,
        tin: &str,
        period_start: &str,
        period_end: &str,
        target_form: Option<&str>,
    ) -> Result<UniversalTaxPayload, ProviderError> {
        let remote_url = config
            .credentials
            .get("url")
            .ok_or_else(|| ProviderError::ConfigError("Missing 'url' in credentials".into()))?;

        let token = config.credentials.get("token").cloned().unwrap_or_default();

        let mut url = reqwest::Url::parse(remote_url)
            .map_err(|_| ProviderError::ConfigError("Invalid URL format".into()))?;

        url.query_pairs_mut()
            .append_pair("tin", tin)
            .append_pair("period_start", period_start)
            .append_pair("period_end", period_end);

        if let Some(form) = target_form {
            url.query_pairs_mut().append_pair("target_form", form);
        }

        let mut headers = HeaderMap::new();
        if !token.is_empty()
            && let Ok(auth_value) = HeaderValue::from_str(&format!("Bearer {}", token))
        {
            headers.insert(AUTHORIZATION, auth_value);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let response = client.get(url).headers(headers).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError(status, error_body));
        }

        let text = response.text().await?;
        let payload: UniversalTaxPayload = serde_json::from_str(&text)
            .map_err(|e| ProviderError::MappingError(format!("Invalid JSON payload: {}", e)))?;

        Ok(payload)
    }
}
