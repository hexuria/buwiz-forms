use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use bir_core::profile::{
    CorDocumentRef, RegisteredTaxType, TaxProfileVersion, TaxProfileVersionSource,
    TaxProfileVersionStatus, TaxpayerProfile, TaxpayerType, VatRegistrationTextClassification,
    classify_vat_registration_text,
};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;
use serde_json::json;

const GEMINI_KEYCHAIN_SERVICE: &str = "dev.goldcoders.bir.gemini_ocr";
const GEMINI_KEYCHAIN_USER: &str = "gemini_api_key";
pub(crate) const DEFAULT_GEMINI_MODEL: &str = "gemini-3.5-flash";
pub(crate) const COR_OCR_GEMINI_ENABLED_SETTING: &str = "cor_ocr_gemini_enabled";
pub(crate) const COR_OCR_GEMINI_MODEL_SETTING: &str = "cor_ocr_gemini_model";

pub(crate) const SUPPORTED_GEMINI_MODELS: &[&str] = &[
    "gemini-3.5-flash",
    "gemini-3.1-flash-lite",
    "gemini-3.1-flash-lite-preview",
    "gemini-3.1-pro-preview",
    "gemini-2.5-pro",
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CorOcrProviderKind {
    Manual,
    SidecarText,
    GeminiByok,
    NativeOcr,
}

#[derive(Debug, Clone)]
pub(crate) struct CorOcrOptions {
    pub provider: CorOcrProviderKind,
    pub gemini_model: String,
    pub allow_cloud_upload: bool,
}

impl Default for CorOcrOptions {
    fn default() -> Self {
        Self {
            provider: CorOcrProviderKind::SidecarText,
            gemini_model: DEFAULT_GEMINI_MODEL.to_string(),
            allow_cloud_upload: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CorOcrFields {
    pub document_type: Option<String>,
    pub tin: Option<String>,
    pub registration_date: Option<NaiveDate>,
    pub registered_name: Option<String>,
    pub trade_name: Option<String>,
    pub rdo_code: Option<String>,
    pub registered_address: Option<String>,
    pub line_of_business_code: Option<String>,
    pub line_of_business_description: Option<String>,
    pub taxpayer_type: Option<TaxpayerType>,
    pub registered_tax_types: Vec<RegisteredTaxType>,
    pub extracted_form_codes: Vec<String>,
    pub filing_reminders: Vec<String>,
    pub field_bboxes: std::collections::HashMap<String, [u16; 4]>,
}

#[derive(Debug, Clone)]
pub(crate) struct CorOcrResult {
    pub text: Option<String>,
    pub confidence: Option<f32>,
    pub fields: CorOcrFields,
    pub status_message: String,
}

trait CorOcrProvider {
    fn extract(&self, source_path: &Path) -> CorOcrResult;
}

pub(crate) fn extract_cor_document(source_path: &Path) -> CorOcrResult {
    extract_cor_document_with_options(source_path, &CorOcrOptions::default())
}

pub(crate) fn resolve_gemini_model_id(selected_model: &str, custom_model: &str) -> String {
    let custom_model = custom_model.trim();
    if !custom_model.is_empty() {
        return custom_model.to_string();
    }

    normalize_model_id(selected_model)
}

pub(crate) fn extract_cor_document_with_options(
    source_path: &Path,
    options: &CorOcrOptions,
) -> CorOcrResult {
    match options.provider {
        CorOcrProviderKind::Manual => ManualProvider.extract(source_path),
        CorOcrProviderKind::SidecarText => SidecarTextProvider.extract(source_path),
        CorOcrProviderKind::NativeOcr => NativeOcrProvider.extract(source_path),
        CorOcrProviderKind::GeminiByok => {
            if !options.allow_cloud_upload {
                tracing::info!(
                    "[COR OCR] Gemini BYOK selected but cloud_upload consent is OFF — skipping API call"
                );
                return CorOcrResult {
                    text: None,
                    confidence: None,
                    fields: CorOcrFields::default(),
                    status_message: "Gemini OCR was not run because cloud upload consent was not enabled for this upload. Review the draft manually.".to_string(),
                };
            }

            GeminiByokProvider {
                model: normalize_model_id(&options.gemini_model),
            }
            .extract(source_path)
        }
    }
}

pub(crate) fn create_draft_cor_version_from_ocr(
    profile: &TaxpayerProfile,
    mut evidence: CorDocumentRef,
    ocr: CorOcrResult,
    now: NaiveDateTime,
) -> TaxProfileVersion {
    evidence.ocr_text = ocr.text.clone();
    evidence.ocr_confidence = ocr.confidence;
    evidence.document_type = ocr.fields.document_type.clone();
    evidence.extracted_form_codes = ocr.fields.extracted_form_codes.clone();
    evidence.field_bboxes = ocr.fields.field_bboxes.clone();
    if evidence.uploaded_at.is_none() {
        evidence.uploaded_at = Some(now);
    }

    let mut version = TaxProfileVersion::from_profile_backfill(profile);
    version.id = format!("ocr-cor-{}", now.and_utc().timestamp_millis());
    version.label = format!("Uploaded COR {}", now.date().format("%Y-%m-%d"));
    version.status = TaxProfileVersionStatus::Draft;
    version.source = TaxProfileVersionSource::OcrCor;
    version.needs_effective_date_review = version.effective_from.is_none();

    if let Some(tin) = ocr.fields.tin {
        let trimmed = tin.trim();
        if !trimmed.is_empty() {
            version.cor.tin = Some(trimmed.to_string());
        }
    }
    if let Some(registration_date) = ocr.fields.registration_date {
        version.cor.registration_date = Some(registration_date);
        if version.effective_from.is_none() {
            version.effective_from = Some(registration_date);
            version.needs_effective_date_review = false;
        }
    }
    if let Some(registered_name) = ocr.fields.registered_name {
        let trimmed = registered_name.trim();
        if !trimmed.is_empty() {
            version.cor.registered_name = trimmed.to_string();
        }
    }
    if let Some(trade_name) = ocr.fields.trade_name {
        let trimmed = trade_name.trim();
        if !trimmed.is_empty() {
            version.cor.trade_name = Some(trimmed.to_string());
        }
    }
    if let Some(rdo_code) = ocr.fields.rdo_code {
        let trimmed = rdo_code.trim();
        if !trimmed.is_empty() {
            version.cor.rdo_code = trimmed.to_string();
        }
    }
    if let Some(registered_address) = ocr.fields.registered_address {
        let trimmed = registered_address.trim();
        if !trimmed.is_empty() {
            version.cor.registered_address = trimmed.to_string();
        }
    }
    if let Some(line_of_business_code) = ocr.fields.line_of_business_code {
        let trimmed = line_of_business_code.trim();
        if !trimmed.is_empty() {
            version.cor.line_of_business_code = Some(trimmed.to_string());
        }
    }
    if let Some(line_of_business_description) = ocr.fields.line_of_business_description {
        let trimmed = line_of_business_description.trim();
        if !trimmed.is_empty() {
            version.cor.line_of_business_description = trimmed.to_string();
        }
    }
    if let Some(taxpayer_type) = ocr.fields.taxpayer_type {
        version.taxpayer_type = taxpayer_type;
    }
    if !ocr.fields.registered_tax_types.is_empty() {
        version.registered_tax_types = ocr.fields.registered_tax_types;
        sync_version_flags_from_registered_tax_types(&mut version);
    }
    if !ocr.fields.filing_reminders.is_empty() {
        let reminders = ocr.fields.filing_reminders.join("\n");
        evidence.ocr_text = Some(match evidence.ocr_text.take() {
            Some(text) if !text.trim().is_empty() => {
                format!("{text}\n\nFiling reminders detected:\n{reminders}")
            }
            _ => reminders,
        });
    }
    version.evidence.push(evidence);
    version
}

fn sync_version_flags_from_registered_tax_types(version: &mut TaxProfileVersion) {
    version.is_vat_registered = version
        .registered_tax_types
        .contains(&RegisteredTaxType::ValueAddedTax);
    version.withholds_compensation = version
        .registered_tax_types
        .contains(&RegisteredTaxType::WithholdingCompensation);
    version.withholds_expanded = version
        .registered_tax_types
        .contains(&RegisteredTaxType::WithholdingExpanded);
    version.withholds_final = version
        .registered_tax_types
        .contains(&RegisteredTaxType::WithholdingFinal);
}

pub(crate) fn save_gemini_api_key(api_key: &str) -> Result<(), String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        tracing::info!("[Keychain] save_gemini_api_key called with empty key, skipping");
        return Ok(());
    }
    tracing::info!(
        "[Keychain] Saving API key ({} chars) to service={} user={}",
        api_key.len(),
        GEMINI_KEYCHAIN_SERVICE,
        GEMINI_KEYCHAIN_USER
    );

    let entry =
        keyring::Entry::new(GEMINI_KEYCHAIN_SERVICE, GEMINI_KEYCHAIN_USER).map_err(|error| {
            tracing::info!("[Keychain] Entry::new failed: {error}");
            format!("Could not open OS keychain for Gemini API key: {error}")
        })?;
    entry.set_password(api_key).map_err(|error| {
        tracing::info!("[Keychain] set_password failed: {error}");
        format!("Could not store Gemini API key in OS keychain: {error}")
    })
}

pub(crate) fn delete_gemini_api_key() -> Result<(), String> {
    tracing::info!(
        "[Keychain] Deleting API key from service={} user={}",
        GEMINI_KEYCHAIN_SERVICE,
        GEMINI_KEYCHAIN_USER
    );
    let entry =
        keyring::Entry::new(GEMINI_KEYCHAIN_SERVICE, GEMINI_KEYCHAIN_USER).map_err(|error| {
            tracing::info!("[Keychain] Entry::new failed: {error}");
            format!("Could not open OS keychain for Gemini API key: {error}")
        })?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            tracing::info!("[Keychain] API key deleted (or was not present)");
            Ok(())
        }
        Err(error) => {
            tracing::info!("[Keychain] delete_credential failed: {error}");
            Err(format!(
                "Could not remove Gemini API key from OS keychain: {error}"
            ))
        }
    }
}

pub(crate) fn test_gemini_api_key(
    model: &str,
    candidate_key: Option<&str>,
) -> Result<String, String> {
    let api_key = resolve_gemini_api_key(candidate_key)?;

    let model = normalize_model_id(model);
    let response = call_gemini(
        &api_key,
        &model,
        vec![json!({
            "text": "Return JSON only: {\"ok\": true}. This is a connectivity test for OCR settings."
        })],
        json!({
            "type": "object",
            "properties": {
                "ok": { "type": "boolean" }
            },
            "required": ["ok"]
        }),
    )?;

    let _: serde_json::Value = serde_json::from_str(&response)
        .map_err(|error| format!("Gemini responded, but the test JSON was invalid: {error}"))?;
    Ok(format!("Gemini OCR key accepted for model {model}."))
}

fn resolve_gemini_api_key(candidate_key: Option<&str>) -> Result<String, String> {
    resolve_gemini_api_key_from_sources(candidate_key, read_gemini_api_key)
}

fn resolve_gemini_api_key_from_sources<F>(
    candidate_key: Option<&str>,
    stored_key: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<Option<String>, String>,
{
    candidate_key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .or_else(|| stored_key().ok().flatten())
        .ok_or_else(|| "Add a Gemini API key before testing OCR.".to_string())
}

fn read_gemini_api_key() -> Result<Option<String>, String> {
    if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
        let api_key = api_key.trim().to_string();
        if !api_key.is_empty() {
            tracing::info!(
                "[Keychain] API key found from GEMINI_API_KEY env var ({} chars)",
                api_key.len()
            );
            return Ok(Some(api_key));
        }
    }

    tracing::info!(
        "[Keychain] Reading API key from keychain service={} user={}",
        GEMINI_KEYCHAIN_SERVICE,
        GEMINI_KEYCHAIN_USER
    );
    let entry =
        keyring::Entry::new(GEMINI_KEYCHAIN_SERVICE, GEMINI_KEYCHAIN_USER).map_err(|error| {
            tracing::info!("[Keychain] Entry::new failed: {error}");
            format!("Could not open OS keychain for Gemini API key: {error}")
        })?;
    match entry.get_password() {
        Ok(api_key) if !api_key.trim().is_empty() => {
            tracing::info!(
                "[Keychain] API key read from keychain ({} chars)",
                api_key.len()
            );
            Ok(Some(api_key))
        }
        Ok(_) | Err(keyring::Error::NoEntry) => {
            tracing::info!("[Keychain] No API key found in keychain");
            Ok(None)
        }
        Err(error) => {
            tracing::info!("[Keychain] get_password failed: {error}");
            Err(format!(
                "Could not read Gemini API key from OS keychain: {error}"
            ))
        }
    }
}

struct ManualProvider;

impl CorOcrProvider for ManualProvider {
    fn extract(&self, _source_path: &Path) -> CorOcrResult {
        CorOcrResult {
            text: None,
            confidence: None,
            fields: CorOcrFields::default(),
            status_message:
                "OCR was not run. Review the COR draft fields manually before confirming."
                    .to_string(),
        }
    }
}

struct SidecarTextProvider;

impl CorOcrProvider for SidecarTextProvider {
    fn extract(&self, source_path: &Path) -> CorOcrResult {
        let Some(sidecar_path) = find_sidecar_text(source_path) else {
            return CorOcrResult {
                text: None,
                confidence: None,
                fields: CorOcrFields::default(),
                status_message:
                    "OCR unavailable. Review the draft fields manually before confirming."
                        .to_string(),
            };
        };

        match std::fs::read_to_string(&sidecar_path) {
            Ok(text) => {
                let fields = parse_cor_text(&text);
                CorOcrResult {
                    text: Some(text),
                    confidence: Some(0.6),
                    fields,
                    status_message:
                        "COR uploaded and sidecar OCR text was applied to the draft. Review before confirming."
                            .to_string(),
                }
            }
            Err(error) => CorOcrResult {
                text: None,
                confidence: None,
                fields: CorOcrFields::default(),
                status_message: format!(
                    "COR uploaded, but OCR sidecar text could not be read: {error}. Review manually."
                ),
            },
        }
    }
}

struct NativeOcrProvider;

impl CorOcrProvider for NativeOcrProvider {
    fn extract(&self, _source_path: &Path) -> CorOcrResult {
        CorOcrResult {
            text: None,
            confidence: None,
            fields: CorOcrFields::default(),
            status_message:
                "Native OCR is not wired yet. Review the COR draft manually before confirming."
                    .to_string(),
        }
    }
}

struct GeminiByokProvider {
    model: String,
}

impl CorOcrProvider for GeminiByokProvider {
    fn extract(&self, source_path: &Path) -> CorOcrResult {
        match self.extract_result(source_path) {
            Ok(result) => result,
            Err(error) => CorOcrResult {
                text: None,
                confidence: None,
                fields: CorOcrFields::default(),
                status_message: format!(
                    "Gemini OCR could not extract this COR: {error}. Review manually."
                ),
            },
        }
    }
}

impl GeminiByokProvider {
    fn extract_result(&self, source_path: &Path) -> Result<CorOcrResult, String> {
        tracing::info!("[Gemini OCR] Starting extraction with model={}", self.model);
        let api_key = read_gemini_api_key()
            .map_err(|e| {
                tracing::info!("[Gemini OCR] Keychain read error: {e}");
                e
            })?
            .ok_or_else(|| {
                tracing::info!("[Gemini OCR] No API key found in keychain or env");
                "No Gemini API key is configured. Add your own key first.".to_string()
            })?;
        tracing::info!("[Gemini OCR] API key resolved ({}… chars)", api_key.len());
        let bytes = std::fs::read(source_path)
            .map_err(|error| format!("Could not read COR document: {error}"))?;
        let mime_type = mime_type_for_path(source_path)?;
        tracing::info!(
            "[Gemini OCR] File read: {} bytes, mime={mime_type}",
            bytes.len()
        );
        let encoded = BASE64_STANDARD.encode(bytes);

        tracing::info!("[Gemini OCR] Sending request to Gemini API…");
        let response_text = call_gemini(
            &api_key,
            &self.model,
            vec![
                json!({ "text": cor_extraction_prompt() }),
                json!({
                    "inlineData": {
                        "mimeType": mime_type,
                        "data": encoded
                    }
                }),
            ],
            cor_response_schema(),
        )
        .map_err(|e| {
            tracing::info!("[Gemini OCR] API call failed: {e}");
            e
        })?;
        tracing::info!(
            "[Gemini OCR] Response received: {} chars",
            response_text.len()
        );
        let extraction = parse_gemini_cor_json(&response_text).map_err(|e| {
            tracing::info!("[Gemini OCR] JSON parse failed: {e}");
            e
        })?;
        let fields = extraction.clone().into_fields();
        tracing::info!(
            "[Gemini OCR] Extraction complete — TIN: {:?}, Name: {:?}, Type: {:?}",
            fields.tin,
            fields.registered_name,
            fields.taxpayer_type
        );
        Ok(CorOcrResult {
            text: Some(response_text),
            confidence: extraction.confidence,
            fields,
            status_message: format!(
                "Gemini OCR extracted COR fields with your API key using {}. Review every field before confirming.",
                self.model
            ),
        })
    }
}

fn call_gemini(
    api_key: &str,
    model: &str,
    parts: Vec<serde_json::Value>,
    response_schema: serde_json::Value,
) -> Result<String, String> {
    let base_url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let url = reqwest::Url::parse_with_params(&base_url, &[("key", api_key)])
        .map_err(|error| format!("Could not build Gemini request URL: {error}"))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| format!("Could not initialize Gemini client: {error}"))?;
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": parts,
        }],
        "generationConfig": {
            "temperature": 0.0,
            "responseMimeType": "application/json",
            "responseSchema": response_schema,
        },
    });

    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .map_err(|error| format!("Gemini request failed: {error}"))?;
    let status = response.status();
    let response_body = response
        .text()
        .map_err(|error| format!("Could not read Gemini response: {error}"))?;
    if !status.is_success() {
        let message = serde_json::from_str::<GeminiErrorResponse>(&response_body)
            .ok()
            .and_then(|error| error.error.map(|error| error.message))
            .unwrap_or_else(|| response_body.chars().take(240).collect());
        return Err(format!("Gemini API returned {status}: {message}"));
    }

    let response = serde_json::from_str::<GeminiGenerateResponse>(&response_body)
        .map_err(|error| format!("Gemini response shape was unexpected: {error}"))?;
    response
        .candidates
        .into_iter()
        .flat_map(|candidate| candidate.content.parts)
        .find_map(|part| part.text)
        .map(|text| strip_json_fences(&text))
        .ok_or_else(|| "Gemini response did not include JSON text.".to_string())
}

fn find_sidecar_text(source_path: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(file_name) = source_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
    {
        candidates.push(source_path.with_file_name(format!("{file_name}.txt")));
    }
    candidates.push(source_path.with_extension("txt"));

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn parse_cor_text(text: &str) -> CorOcrFields {
    let registration_date =
        extract_label_value(text, &["REGISTRATION DATE"]).and_then(|value| parse_date(&value));
    let tin = extract_label_value(text, &["TIN", "TAXPAYER IDENTIFICATION NUMBER"]);
    let registered_name = extract_label_value(text, &["TAXPAYER'S NAME", "TAXPAYER NAME", "NAME"]);
    let trade_name = extract_label_value(text, &["TRADE NAME"]);
    let rdo_code = extract_label_value(text, &["REVENUE DISTRICT OFFICE", "RDO"]);
    let registered_address = extract_label_value(text, &["REGISTERED ADDRESS", "ADDRESS"]);
    let line_of_business = extract_label_value(
        text,
        &[
            "LINE OF BUSINESS",
            "REGISTERED ACTIVITIES",
            "REGISTERED ACTIVITY",
        ],
    );
    let (line_of_business_code, line_of_business_description) =
        split_line_of_business(line_of_business.as_deref());
    let extracted_form_codes = extract_form_codes_from_text(text);

    CorOcrFields {
        document_type: detect_document_type(text),
        tin,
        registration_date,
        registered_name,
        trade_name,
        rdo_code,
        registered_address,
        line_of_business_code,
        line_of_business_description,
        taxpayer_type: None,
        registered_tax_types: infer_tax_types_from_text(text),
        extracted_form_codes,
        filing_reminders: vec![],
        field_bboxes: std::collections::HashMap::new(),
    }
}

fn detect_document_type(text: &str) -> Option<String> {
    let normalized = normalize(text);
    if normalized.contains("CERTIFICATE OF REGISTRATION") || normalized.contains("FORM 2303") {
        Some("BIR Form 2303 / Certificate of Registration".to_string())
    } else {
        None
    }
}

fn extract_form_codes_from_text(text: &str) -> Vec<String> {
    const KNOWN_CODES: &[&str] = &[
        "2303", "0605", "0619E", "0619F", "1600", "1600PT", "1600VT", "1601C", "1601EQ", "1601FQ",
        "1602Q", "1603Q", "1604C", "1604CF", "1604E", "1621", "1701", "1701A", "1701MS", "1701Q",
        "1702EX", "1702MX", "1702Q", "1702RT", "1704", "2200M", "2200P", "2316", "2550DS", "2550M",
        "2550Q", "2551Q",
    ];
    let normalized = normalize(text);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let mut codes = Vec::new();
    let mut token_index = 0;

    while token_index < tokens.len() {
        // OCR commonly separates suffixes around punctuation (for example,
        // `1701-Q` or `1604-CF`). Prefer the longest exact known code formed
        // from at most three adjacent tokens. Never use substring/prefix
        // matching: `1701Q` is not evidence for `1701`, and `1604CF` is not
        // evidence for `1604C`.
        let mut candidate = String::new();
        let mut longest_match = None;
        for consumed in 1..=3 {
            let Some(token) = tokens.get(token_index + consumed - 1) else {
                break;
            };
            candidate.push_str(token);
            if KNOWN_CODES.contains(&candidate.as_str()) {
                longest_match = Some((candidate.clone(), consumed));
            }
        }

        if let Some((code, consumed)) = longest_match {
            codes.push(code);
            token_index += consumed;
        } else {
            token_index += 1;
        }
    }
    codes.sort();
    codes.dedup();
    codes
}

fn extract_label_value(text: &str, labels: &[&str]) -> Option<String> {
    let lines = text.lines().collect::<Vec<_>>();

    for (index, line) in lines.iter().enumerate() {
        let normalized_line = normalize(line);

        for label in labels {
            let normalized_label = normalize(label);
            if let Some(offset) = label_offset(&normalized_line, &normalized_label) {
                let value = line
                    .get(offset..)
                    .unwrap_or_default()
                    .trim_start_matches(|ch: char| {
                        ch.is_whitespace() || ch == ':' || ch == '-' || ch == '|'
                    })
                    .trim();

                if !value.is_empty() {
                    return Some(value.to_string());
                }

                if let Some(next_value) = lines
                    .iter()
                    .skip(index + 1)
                    .map(|next_line| next_line.trim())
                    .find(|next_line| !next_line.is_empty())
                {
                    return Some(next_value.to_string());
                }
            }
        }
    }

    None
}

fn label_offset(line: &str, label: &str) -> Option<usize> {
    if line.starts_with(label) {
        return Some(label.len());
    }

    if label == "NAME" {
        return None;
    }

    line.find(label).map(|offset| offset + label.len())
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    let cleaned = value.trim();
    for format in [
        "%Y-%m-%d",
        "%m/%d/%Y",
        "%m-%d-%Y",
        "%B %d, %Y",
        "%b %d, %Y",
        "%d %B %Y",
        "%d %b %Y",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(cleaned, format) {
            return Some(date);
        }
    }
    None
}

fn split_line_of_business(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return (None, None);
    };

    let mut parts = value.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    let rest = parts.next().map(str::trim).filter(|rest| !rest.is_empty());

    if first.chars().all(|ch| ch.is_ascii_digit()) && first.len() >= 3 {
        (Some(first.to_string()), rest.map(str::to_string))
    } else {
        (None, Some(value.to_string()))
    }
}

fn mime_type_for_path(source_path: &Path) -> Result<&'static str, String> {
    match source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => Ok("application/pdf"),
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        other => Err(format!(
            "Unsupported COR file type `{other}`. Use PDF, PNG, JPG, or JPEG."
        )),
    }
}

fn normalize_model_id(model: &str) -> String {
    let model = model.trim();
    if model.is_empty() {
        DEFAULT_GEMINI_MODEL.to_string()
    } else {
        model.to_string()
    }
}

fn cor_extraction_prompt() -> &'static str {
    "You are extracting structured data from a Philippine BIR Certificate of Registration, Form 2303. \
    Return JSON only. If a value is unreadable, use an empty string. Do not invent facts. \
    In the `registered_tax_types` array, make sure to extract all registered tax types listed in the \"Tax Type\" column/section on the document (such as INCOME TAX, VALUE ADDED TAX, PERCENTAGE TAX, REGISTRATION FEE, WITHHOLDING TAX - EXPANDED, WITHHOLDING TAX - COMPENSATION, WITHHOLDING TAX - FINAL, WITHHOLDING TAX - VAT AND OTHER PERCENTAGE TAXES, EXCISE TAX). Treat WITHHOLDING TAX - VAT AND OTHER PERCENTAGE TAXES as one distinct withholding tax type; do not split it into VAT or PERCENTAGE TAX registration. Do not omit any registered tax types that are listed on the document. \
    For every extracted field, include a tightly fitted bounding box in `field_bboxes`. \
    The bounding box must be an array of `[ymin, xmin, ymax, xmax]` normalized to 0-1000. \
    It MUST accurately hug the edges of the text. \
    For the `extracted_form_codes` array, create a separate bounding box object for each individual form code found on the document, using the exact form code string as the `field` name in `field_bboxes` (e.g. `{\"field\": \"0605\", \"box_2d\": [...]}`)."
}

fn cor_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "confidence": { "type": "number" },
            "document_type": { "type": "string" },
            "tin": { "type": "string" },
            "registration_date": { "type": "string" },
            "registered_name": { "type": "string" },
            "trade_name": { "type": "string" },
            "rdo_code": { "type": "string" },
            "registered_address": { "type": "string" },
            "line_of_business_code": { "type": "string" },
            "line_of_business_description": { "type": "string" },
            "taxpayer_type": { "type": "string" },
            "registered_tax_types": {
                "type": "array",
                "items": { "type": "string" }
            },
            "extracted_form_codes": {
                "type": "array",
                "items": { "type": "string" }
            },
            "filing_reminders": {
                "type": "array",
                "items": { "type": "string" }
            },
            "field_bboxes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "field": { "type": "string" },
                        "box_2d": {
                            "type": "array",
                            "items": { "type": "number" }
                        }
                    },
                    "required": ["field", "box_2d"]
                }
            }
        },
        "required": [
            "confidence",
            "document_type",
            "tin",
            "registration_date",
            "registered_name",
            "trade_name",
            "rdo_code",
            "registered_address",
            "line_of_business_code",
            "line_of_business_description",
            "taxpayer_type",
            "registered_tax_types",
            "extracted_form_codes",
            "filing_reminders",
            "field_bboxes"
        ]
    })
}

fn parse_gemini_cor_json(text: &str) -> Result<GeminiCorExtraction, String> {
    serde_json::from_str::<GeminiCorExtraction>(&strip_json_fences(text))
        .map_err(|error| format!("Gemini JSON could not be parsed: {error}"))
}

fn strip_json_fences(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.trim_end_matches("```").trim().to_string();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.trim_end_matches("```").trim().to_string();
    }
    trimmed.to_string()
}

fn infer_tax_types_from_text(text: &str) -> Vec<RegisteredTaxType> {
    let mut tax_types = vec![];
    for line in text.lines() {
        if let Some(tax_type) = parse_registered_tax_type(line) {
            push_unique_tax_type(&mut tax_types, tax_type);
        }
    }
    tax_types.sort();
    tax_types
}

fn parse_registered_tax_type(value: &str) -> Option<RegisteredTaxType> {
    let normalized = normalize(value);
    let explicitly_non_vat =
        classify_vat_registration_text(value) == VatRegistrationTextClassification::NonVat;
    if normalized.contains("WITHHOLDING")
        && normalized.contains("VAT")
        && normalized.contains("PERCENTAGE")
    {
        Some(RegisteredTaxType::WithholdingVatAndPercentage)
    } else if !explicitly_non_vat
        && (normalized.contains("VALUE ADDED")
            || normalized == "VAT"
            || normalized.contains(" VAT "))
    {
        Some(RegisteredTaxType::ValueAddedTax)
    } else if normalized.contains("PERCENTAGE") {
        Some(RegisteredTaxType::PercentageTax)
    } else if normalized.contains("REGISTRATION FEE") {
        Some(RegisteredTaxType::RegistrationFee)
    } else if normalized.contains("WITHHOLDING") && normalized.contains("EXPANDED") {
        Some(RegisteredTaxType::WithholdingExpanded)
    } else if normalized.contains("WITHHOLDING") && normalized.contains("COMPENSATION") {
        Some(RegisteredTaxType::WithholdingCompensation)
    } else if normalized.contains("WITHHOLDING") && normalized.contains("FINAL") {
        Some(RegisteredTaxType::WithholdingFinal)
    } else if normalized.contains("EXCISE") {
        Some(RegisteredTaxType::ExciseTax)
    } else if normalized.contains("INCOME TAX") || normalized == "INCOME" {
        Some(RegisteredTaxType::IncomeTax)
    } else {
        None
    }
}

fn parse_taxpayer_type(value: &str) -> Option<TaxpayerType> {
    let normalized = normalize(value);
    if normalized.contains("CORPORATION") {
        Some(TaxpayerType::Corporation)
    } else if normalized.contains("PARTNERSHIP") {
        Some(TaxpayerType::Partnership)
    } else if normalized.contains("COOPERATIVE") {
        Some(TaxpayerType::Cooperative)
    } else if normalized.contains("ESTATE") {
        Some(TaxpayerType::Estate)
    } else if normalized.contains("TRUST") {
        Some(TaxpayerType::Trust)
    } else if normalized.contains("INDIVIDUAL")
        || normalized.contains("SINGLE PROPRIETOR")
        || normalized.contains("PROPRIETOR")
    {
        Some(TaxpayerType::Individual)
    } else {
        None
    }
}

fn push_unique_tax_type(tax_types: &mut Vec<RegisteredTaxType>, tax_type: RegisteredTaxType) {
    if !tax_types.contains(&tax_type) {
        tax_types.push(tax_type);
    }
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiCorExtraction {
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    document_type: String,
    #[serde(default)]
    tin: String,
    #[serde(default)]
    registration_date: String,
    #[serde(default)]
    registered_name: String,
    #[serde(default)]
    trade_name: String,
    #[serde(default)]
    rdo_code: String,
    #[serde(default)]
    registered_address: String,
    #[serde(default)]
    line_of_business_code: String,
    #[serde(default)]
    line_of_business_description: String,
    #[serde(default)]
    taxpayer_type: String,
    #[serde(default)]
    registered_tax_types: Vec<String>,
    #[serde(default)]
    extracted_form_codes: Vec<String>,
    #[serde(default)]
    filing_reminders: Vec<String>,
    #[serde(default)]
    field_bboxes: Vec<GeminiBoundingBox>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiBoundingBox {
    field: String,
    box_2d: Vec<u16>,
}

impl GeminiCorExtraction {
    fn into_fields(self) -> CorOcrFields {
        let mut registered_tax_types = vec![];
        for raw in &self.registered_tax_types {
            if let Some(tax_type) = parse_registered_tax_type(raw) {
                push_unique_tax_type(&mut registered_tax_types, tax_type);
            }
        }
        registered_tax_types.sort();

        CorOcrFields {
            document_type: non_empty(self.document_type),
            tin: non_empty(self.tin),
            registration_date: parse_date(&self.registration_date),
            registered_name: non_empty(self.registered_name),
            trade_name: non_empty(self.trade_name),
            rdo_code: non_empty(self.rdo_code),
            registered_address: non_empty(self.registered_address),
            line_of_business_code: non_empty(self.line_of_business_code),
            line_of_business_description: non_empty(self.line_of_business_description),
            taxpayer_type: parse_taxpayer_type(&self.taxpayer_type),
            registered_tax_types,
            extracted_form_codes: normalize_form_codes(self.extracted_form_codes),
            filing_reminders: self
                .filing_reminders
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            field_bboxes: self
                .field_bboxes
                .into_iter()
                .filter_map(|bbox| {
                    if bbox.box_2d.len() == 4 {
                        Some((
                            bbox.field,
                            [
                                bbox.box_2d[0],
                                bbox.box_2d[1],
                                bbox.box_2d[2],
                                bbox.box_2d[3],
                            ],
                        ))
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }
}

fn normalize_form_codes(codes: Vec<String>) -> Vec<String> {
    let mut codes = codes
        .into_iter()
        .flat_map(|value| {
            value
                .split([',', ';', '\n'])
                .map(str::trim)
                .filter(|code| !code.is_empty())
                .map(|code| {
                    let cleaned = code
                        .chars()
                        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
                        .collect::<String>()
                        .to_ascii_uppercase();
                    bir_core::forms::registry::canonical_form_code(&cleaned)
                })
                .collect::<Vec<_>>()
        })
        .filter(|code| !code.is_empty())
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    codes
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorResponse {
    error: Option<GeminiError>,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::{
        naming::Tin,
        profile::{
            ComplianceSourceMode, EmailAuthMethod, RegistrationActivityStatus, TaxClassification,
        },
    };

    fn temp_cor_dir() -> PathBuf {
        std::env::temp_dir().join(format!("bir-cor-ocr-test-{}", uuid::Uuid::new_v4()))
    }

    fn sample_profile() -> TaxpayerProfile {
        TaxpayerProfile {
            id: None,
            full_name: "Legacy Name".to_string(),
            tin: Tin {
                segment1: "123".into(),
                segment2: "456".into(),
                segment3: "789".into(),
                branch: "00000".into(),
            },
            rdo_code: "018".to_string(),
            line_of_business: "Software Development".to_string(),
            registered_address: "Olongapo".to_string(),
            zip_code: "2200".to_string(),
            phone: "09123456789".to_string(),
            email: "taxpayer@example.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            birth_date: None,
            tax_classification: Some(TaxClassification::SelfEmployed),
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            is_archived: false,
            profile_pin_hash: None,
            totp_secret: None,
            email_tracking_enabled: false,
            email_auth_method: EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: RegistrationActivityStatus::Active,
            profile_versions: vec![],
            compliance_source_mode: ComplianceSourceMode::TemporalSuggestion,
            per_year_forms: Default::default(),
        }
    }

    fn sample_evidence() -> CorDocumentRef {
        CorDocumentRef {
            id: "doc-id".to_string(),
            file_name: "cor.pdf".to_string(),
            stored_path: "/tmp/cor.pdf".to_string(),
            uploaded_at: None,
            provider: None,
            model: None,
            document_type: None,
            extracted_form_codes: vec![],
            field_bboxes: std::collections::HashMap::new(),
            ocr_text: None,
            ocr_confidence: None,
        }
    }

    #[test]
    fn returns_unavailable_when_no_sidecar_text_exists() {
        let dir = temp_cor_dir();
        let source = dir.join("cor.pdf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&source, b"fake pdf").unwrap();

        let result = extract_cor_document(&source);

        assert!(result.text.is_none());
        assert!(result.confidence.is_none());
        assert!(result.status_message.contains("OCR unavailable"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_cor_fields_from_sidecar_text() {
        let dir = temp_cor_dir();
        let source = dir.join("cor.pdf");
        let sidecar = dir.join("cor.pdf.txt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&source, b"fake pdf").unwrap();
        std::fs::write(
            &sidecar,
            "\
TIN: 123-456-789-00000
REGISTRATION DATE: 2026-01-15
TAXPAYER'S NAME: Juan Dela Cruz Corp.
TRADE NAME: Goldcoders
REVENUE DISTRICT OFFICE: 018 - Olongapo City
REGISTERED ADDRESS: Olongapo
LINE OF BUSINESS: 7221 Software Publishing
INCOME TAX
PERCENTAGE TAX
",
        )
        .unwrap();

        let result = extract_cor_document(&source);

        assert!(result.text.is_some());
        assert_eq!(result.confidence, Some(0.6));
        assert_eq!(result.fields.tin.as_deref(), Some("123-456-789-00000"));
        assert_eq!(
            result.fields.registration_date,
            NaiveDate::from_ymd_opt(2026, 1, 15)
        );
        assert_eq!(
            result.fields.registered_name.as_deref(),
            Some("Juan Dela Cruz Corp.")
        );
        assert_eq!(result.fields.trade_name.as_deref(), Some("Goldcoders"));
        assert_eq!(
            result.fields.rdo_code.as_deref(),
            Some("018 - Olongapo City")
        );
        assert_eq!(
            result.fields.registered_address.as_deref(),
            Some("Olongapo")
        );
        assert_eq!(result.fields.line_of_business_code.as_deref(), Some("7221"));
        assert_eq!(
            result.fields.line_of_business_description.as_deref(),
            Some("Software Publishing")
        );
        assert!(
            result
                .fields
                .registered_tax_types
                .contains(&RegisteredTaxType::IncomeTax)
        );
        assert!(
            result
                .fields
                .registered_tax_types
                .contains(&RegisteredTaxType::PercentageTax)
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn manual_provider_never_reads_or_uploads_document() {
        let dir = temp_cor_dir();
        let source = dir.join("missing.pdf");
        let result = extract_cor_document_with_options(
            &source,
            &CorOcrOptions {
                provider: CorOcrProviderKind::Manual,
                ..Default::default()
            },
        );

        assert!(result.text.is_none());
        assert!(result.status_message.contains("OCR was not run"));
    }

    #[test]
    fn native_provider_is_review_needed_until_wired() {
        let dir = temp_cor_dir();
        let source = dir.join("missing.pdf");
        let result = extract_cor_document_with_options(
            &source,
            &CorOcrOptions {
                provider: CorOcrProviderKind::NativeOcr,
                ..Default::default()
            },
        );

        assert!(result.text.is_none());
        assert!(result.status_message.contains("Native OCR is not wired"));
    }

    #[test]
    fn gemini_provider_requires_cloud_consent() {
        let dir = temp_cor_dir();
        let source = dir.join("cor.pdf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&source, b"fake pdf").unwrap();

        let result = extract_cor_document_with_options(
            &source,
            &CorOcrOptions {
                provider: CorOcrProviderKind::GeminiByok,
                allow_cloud_upload: false,
                ..Default::default()
            },
        );

        assert!(result.text.is_none());
        assert!(result.status_message.contains("consent"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_gemini_json_into_cor_fields() {
        let text = r#"
        {
          "confidence": 0.78,
          "document_type": "BIR Form 2303 / Certificate of Registration",
          "tin": "123-456-789-00000",
          "registration_date": "2020-02-17",
          "registered_name": "Gold Coders Corp.",
          "trade_name": "Buwiz",
          "rdo_code": "018",
          "registered_address": "Olongapo City",
          "line_of_business_code": "7221",
          "line_of_business_description": "Software Publishing",
          "taxpayer_type": "Corporation",
          "registered_tax_types": ["Income Tax", "Percentage Tax", "Withholding Tax - Expanded"],
          "extracted_form_codes": ["1702Q", "2551Q"],
          "filing_reminders": ["1702Q - 60 days after quarter end"]
        }"#;

        let extraction = parse_gemini_cor_json(text).unwrap();
        assert_eq!(extraction.confidence, Some(0.78));
        let fields = extraction.into_fields();
        assert_eq!(
            fields.document_type.as_deref(),
            Some("BIR Form 2303 / Certificate of Registration")
        );
        assert_eq!(fields.tin.as_deref(), Some("123-456-789-00000"));
        assert_eq!(
            fields.registration_date,
            NaiveDate::from_ymd_opt(2020, 2, 17)
        );
        assert_eq!(fields.taxpayer_type, Some(TaxpayerType::Corporation));
        assert_eq!(
            fields.registered_tax_types,
            vec![
                RegisteredTaxType::IncomeTax,
                RegisteredTaxType::PercentageTax,
                RegisteredTaxType::WithholdingExpanded
            ]
        );
        assert_eq!(fields.filing_reminders.len(), 1);
        assert_eq!(
            fields.extracted_form_codes,
            vec!["1702Q".to_string(), "2551Q".to_string()]
        );
    }

    #[test]
    fn non_vat_language_never_becomes_positive_vat_evidence() {
        assert_eq!(parse_registered_tax_type("NON-VAT"), None);
        assert_eq!(parse_registered_tax_type("NOT VAT REGISTERED"), None);
        assert_eq!(parse_registered_tax_type("NOT REGISTERED FOR VAT"), None);
        assert_eq!(
            parse_registered_tax_type("NON-VAT / PERCENTAGE TAX"),
            Some(RegisteredTaxType::PercentageTax)
        );
    }

    #[test]
    fn explicit_vat_tax_type_still_parses() {
        assert_eq!(
            parse_registered_tax_type("VALUE ADDED TAX"),
            Some(RegisteredTaxType::ValueAddedTax)
        );
        assert_eq!(
            parse_registered_tax_type("VAT"),
            Some(RegisteredTaxType::ValueAddedTax)
        );
    }

    #[test]
    fn vat_and_percentage_withholding_phrase_is_one_distinct_tax_type() {
        assert_eq!(
            parse_registered_tax_type("WITHHOLDING TAX - VAT AND OTHER PERCENTAGE TAXES"),
            Some(RegisteredTaxType::WithholdingVatAndPercentage)
        );
    }

    #[test]
    fn vat_and_percentage_withholding_text_does_not_infer_vat_or_percentage_registration() {
        assert_eq!(
            infer_tax_types_from_text("WITHHOLDING TAX - VAT AND OTHER PERCENTAGE TAXES"),
            vec![RegisteredTaxType::WithholdingVatAndPercentage]
        );
    }

    #[test]
    fn fallback_form_code_extraction_does_not_invent_prefix_codes() {
        assert_eq!(
            extract_form_codes_from_text("BIR Form 1701Q"),
            vec!["1701Q".to_string()]
        );
        assert_eq!(
            extract_form_codes_from_text("BIR Forms 1600PT and 1600VT"),
            vec!["1600PT".to_string(), "1600VT".to_string()]
        );
        assert_eq!(
            extract_form_codes_from_text("Annual Information Return 1604CF"),
            vec!["1604CF".to_string()]
        );
    }

    #[test]
    fn fallback_form_code_extraction_accepts_reviewable_code_boundaries() {
        assert_eq!(
            extract_form_codes_from_text("BIR Forms 1701-Q, 1600 PT, and 1604-C-F"),
            vec![
                "1600PT".to_string(),
                "1604CF".to_string(),
                "1701Q".to_string()
            ]
        );
        assert_eq!(
            extract_form_codes_from_text("BIR Form 1701"),
            vec!["1701".to_string()]
        );
        assert!(extract_form_codes_from_text("OCR artifact X1701QY").is_empty());
    }

    #[test]
    fn malformed_gemini_json_is_review_needed_error() {
        let error = parse_gemini_cor_json("not-json").unwrap_err();
        assert!(error.contains("Gemini JSON could not be parsed"));
    }

    #[test]
    fn gemini_provider_sends_pdf_directly_as_pdf_mime_type() {
        assert_eq!(
            mime_type_for_path(Path::new("certificate-of-registration.pdf")).unwrap(),
            "application/pdf"
        );
    }

    #[test]
    fn missing_key_resolution_disables_gemini_test_path_without_network() {
        let result = resolve_gemini_api_key_from_sources(Some("  "), || Ok(None));
        assert!(result.unwrap_err().contains("Add a Gemini API key"));
    }

    #[test]
    fn candidate_key_takes_precedence_over_stored_key() {
        let result = resolve_gemini_api_key_from_sources(Some(" candidate-key "), || {
            Ok(Some("stored-key".to_string()))
        })
        .unwrap();
        assert_eq!(result, "candidate-key");
    }

    #[test]
    fn custom_gemini_model_takes_precedence_over_selected_model() {
        let model = resolve_gemini_model_id("gemini-2.5-flash", " gemini-future-custom-preview ");

        assert_eq!(model, "gemini-future-custom-preview");
    }

    #[test]
    fn empty_gemini_model_selection_uses_default_model() {
        let model = resolve_gemini_model_id("  ", "  ");

        assert_eq!(model, DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn ocr_result_creates_review_only_draft_cor_version() {
        let mut ocr = CorOcrResult {
            text: Some("{\"source\":\"gemini\"}".to_string()),
            confidence: Some(0.77),
            fields: CorOcrFields {
                tin: Some("123-456-789-00000".to_string()),
                registration_date: NaiveDate::from_ymd_opt(2026, 2, 17),
                registered_name: Some("Gold Coders Corp.".to_string()),
                trade_name: Some("Buwiz".to_string()),
                rdo_code: Some("018".to_string()),
                registered_address: Some("Olongapo City".to_string()),
                line_of_business_code: Some("7221".to_string()),
                line_of_business_description: Some("Software Publishing".to_string()),
                taxpayer_type: Some(TaxpayerType::Corporation),
                document_type: Some("BIR Form 2303 / Certificate of Registration".to_string()),
                registered_tax_types: vec![
                    RegisteredTaxType::IncomeTax,
                    RegisteredTaxType::ValueAddedTax,
                    RegisteredTaxType::WithholdingCompensation,
                    RegisteredTaxType::WithholdingExpanded,
                    RegisteredTaxType::WithholdingFinal,
                ],
                extracted_form_codes: vec!["0605".to_string(), "1701Q".to_string()],
                filing_reminders: vec!["File on time".to_string()],
                field_bboxes: std::collections::HashMap::new(),
            },
            status_message: "Gemini OCR extracted COR fields.".to_string(),
        };
        ocr.fields.registered_tax_types.sort();

        let version = create_draft_cor_version_from_ocr(
            &sample_profile(),
            sample_evidence(),
            ocr,
            NaiveDate::from_ymd_opt(2026, 3, 1)
                .unwrap()
                .and_hms_opt(8, 30, 0)
                .unwrap(),
        );

        assert_eq!(version.status, TaxProfileVersionStatus::Draft);
        assert_eq!(version.source, TaxProfileVersionSource::OcrCor);
        assert_eq!(version.effective_from, NaiveDate::from_ymd_opt(2026, 2, 17));
        assert!(!version.needs_effective_date_review);
        assert_eq!(version.cor.tin.as_deref(), Some("123-456-789-00000"));
        assert_eq!(version.cor.registered_name, "Gold Coders Corp.");
        assert_eq!(version.cor.trade_name.as_deref(), Some("Buwiz"));
        assert_eq!(version.cor.rdo_code, "018");
        assert_eq!(version.taxpayer_type, TaxpayerType::Corporation);
        assert!(version.is_vat_registered);
        assert!(version.withholds_compensation);
        assert!(version.withholds_expanded);
        assert!(version.withholds_final);
        assert_eq!(version.evidence.len(), 1);
        assert_eq!(version.evidence[0].ocr_confidence, Some(0.77));
        assert_eq!(
            version.evidence[0].document_type.as_deref(),
            Some("BIR Form 2303 / Certificate of Registration")
        );
        assert_eq!(
            version.evidence[0].extracted_form_codes,
            vec!["0605".to_string(), "1701Q".to_string()]
        );
        assert!(
            version.evidence[0]
                .ocr_text
                .as_deref()
                .unwrap_or_default()
                .contains("Filing reminders detected")
        );
    }

    #[test]
    fn ocr_draft_affects_obligations_only_after_confirmation_and_forms_set_reconciliation() {
        let profile = sample_profile();
        let ocr = CorOcrResult {
            text: Some("{\"source\":\"gemini\"}".to_string()),
            confidence: Some(0.81),
            fields: CorOcrFields {
                tin: Some(profile.tin.formatted()),
                registration_date: NaiveDate::from_ymd_opt(2026, 1, 15),
                registered_name: Some("Legacy Name".to_string()),
                rdo_code: Some("018".to_string()),
                registered_address: Some("Olongapo".to_string()),
                line_of_business_description: Some("Software Development".to_string()),
                taxpayer_type: Some(TaxpayerType::Individual),
                registered_tax_types: vec![
                    RegisteredTaxType::IncomeTax,
                    RegisteredTaxType::PercentageTax,
                ],
                ..Default::default()
            },
            status_message: "Gemini OCR extracted COR fields.".to_string(),
        };
        let draft = create_draft_cor_version_from_ocr(
            &profile,
            sample_evidence(),
            ocr,
            NaiveDate::from_ymd_opt(2026, 2, 1)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap(),
        );

        let mut draft_profile = profile.clone();
        draft_profile.compliance_source_mode = ComplianceSourceMode::CorVersioned;
        draft_profile.profile_versions = vec![draft.clone()];
        let draft_preview = draft_profile.preview_obligations_for_year(2026);
        assert!(draft_preview.form_codes.is_empty());
        assert!(draft_preview.active_version_ids.is_empty());

        let mut confirmed_profile = draft_profile;
        confirmed_profile.profile_versions[0].status = TaxProfileVersionStatus::Confirmed;
        let suggestions =
            bir_core::integration::form_suggestions_for_profile_year(&confirmed_profile, 2026);
        let reconciliation =
            bir_core::forms::reconcile_forms_set_for_year(2026, None, &suggestions);
        confirmed_profile
            .per_year_forms
            .insert(2026, reconciliation.forms_set);
        let confirmed_preview = confirmed_profile.preview_obligations_for_year(2026);
        assert!(
            confirmed_preview
                .form_codes
                .iter()
                .any(|code| code == "1701Q")
        );
        assert!(
            confirmed_preview
                .form_codes
                .iter()
                .any(|code| code == "2551Q")
        );
        assert_eq!(confirmed_preview.active_version_ids, vec![draft.id]);
    }
}
