use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use bir_core::profile::{
    CorDocumentRef, RegisteredTaxType, TaxProfileVersion, TaxProfileVersionSource,
    TaxProfileVersionStatus, TaxpayerProfile, TaxpayerType,
};
use chrono::{NaiveDate, NaiveDateTime};
use serde::Deserialize;
use serde_json::json;

const GEMINI_KEYCHAIN_SERVICE: &str = "dev.goldcoders.bir.gemini_ocr";
const GEMINI_KEYCHAIN_USER: &str = "gemini_api_key";
pub(crate) const DEFAULT_GEMINI_MODEL: &str = "gemini-3.1-flash-lite";
pub(crate) const COR_OCR_GEMINI_ENABLED_SETTING: &str = "cor_ocr_gemini_enabled";
pub(crate) const COR_OCR_GEMINI_MODEL_SETTING: &str = "cor_ocr_gemini_model";

pub(crate) const SUPPORTED_GEMINI_MODELS: &[&str] = &[
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
    pub filing_reminders: Vec<String>,
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

    let mut version = TaxProfileVersion::from_profile_backfill(profile);
    version.id = format!("ocr-cor-{}", now.and_utc().timestamp_millis());
    version.label = format!("Uploaded COR {}", now.date().format("%Y-%m-%d"));
    version.status = TaxProfileVersionStatus::Draft;
    version.source = TaxProfileVersionSource::OcrCor;
    version.needs_effective_date_review = version.effective_from.is_none();

    if let Some(registration_date) = ocr.fields.registration_date {
        version.cor.registration_date = Some(registration_date);
        if version.effective_from.is_none() {
            version.effective_from = Some(registration_date);
            version.needs_effective_date_review = false;
        }
    }
    if let Some(registered_name) = ocr.fields.registered_name {
        version.cor.registered_name = registered_name;
    }
    if let Some(trade_name) = ocr.fields.trade_name {
        version.cor.trade_name = Some(trade_name);
    }
    if let Some(rdo_code) = ocr.fields.rdo_code {
        version.cor.rdo_code = rdo_code;
    }
    if let Some(registered_address) = ocr.fields.registered_address {
        version.cor.registered_address = registered_address;
    }
    if let Some(line_of_business_code) = ocr.fields.line_of_business_code {
        version.cor.line_of_business_code = Some(line_of_business_code);
    }
    if let Some(line_of_business_description) = ocr.fields.line_of_business_description {
        version.cor.line_of_business_description = line_of_business_description;
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

pub(crate) fn has_gemini_api_key() -> bool {
    read_gemini_api_key().ok().flatten().is_some()
}

pub(crate) fn save_gemini_api_key(api_key: &str) -> Result<(), String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Ok(());
    }

    let entry = keyring::Entry::new(GEMINI_KEYCHAIN_SERVICE, GEMINI_KEYCHAIN_USER)
        .map_err(|error| format!("Could not open OS keychain for Gemini API key: {error}"))?;
    entry
        .set_password(api_key)
        .map_err(|error| format!("Could not store Gemini API key in OS keychain: {error}"))
}

pub(crate) fn delete_gemini_api_key() -> Result<(), String> {
    let entry = keyring::Entry::new(GEMINI_KEYCHAIN_SERVICE, GEMINI_KEYCHAIN_USER)
        .map_err(|error| format!("Could not open OS keychain for Gemini API key: {error}"))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Could not remove Gemini API key from OS keychain: {error}"
        )),
    }
}

pub(crate) fn test_gemini_api_key(
    model: &str,
    candidate_key: Option<&str>,
) -> Result<String, String> {
    let api_key = candidate_key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .or_else(|| read_gemini_api_key().ok().flatten())
        .ok_or_else(|| "Add a Gemini API key before testing OCR.".to_string())?;

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

fn read_gemini_api_key() -> Result<Option<String>, String> {
    if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
        let api_key = api_key.trim().to_string();
        if !api_key.is_empty() {
            return Ok(Some(api_key));
        }
    }

    let entry = keyring::Entry::new(GEMINI_KEYCHAIN_SERVICE, GEMINI_KEYCHAIN_USER)
        .map_err(|error| format!("Could not open OS keychain for Gemini API key: {error}"))?;
    match entry.get_password() {
        Ok(api_key) if !api_key.trim().is_empty() => Ok(Some(api_key)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "Could not read Gemini API key from OS keychain: {error}"
        )),
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
        let api_key = read_gemini_api_key()?.ok_or_else(|| {
            "No Gemini API key is configured. Add your own key first.".to_string()
        })?;
        let bytes = std::fs::read(source_path)
            .map_err(|error| format!("Could not read COR document: {error}"))?;
        let mime_type = mime_type_for_path(source_path)?;
        let encoded = BASE64_STANDARD.encode(bytes);

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
        )?;
        let extraction = parse_gemini_cor_json(&response_text)?;
        let fields = extraction.clone().into_fields();
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

    CorOcrFields {
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
        filing_reminders: vec![],
    }
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
    "You are extracting structured data from a Philippine BIR Certificate of Registration, Form 2303. Return JSON only. If a value is unreadable, use an empty string. Do not invent facts. Filing reminders are evidence text only; do not convert them into legal rules. The user must review the result before it affects compliance."
}

fn cor_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "confidence": { "type": "number" },
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
            "filing_reminders": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": [
            "confidence",
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
            "filing_reminders"
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
    if normalized.contains("VALUE ADDED") || normalized == "VAT" || normalized.contains(" VAT ") {
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
    filing_reminders: Vec<String>,
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
            filing_reminders: self
                .filing_reminders
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
        }
    }
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

    fn temp_cor_dir() -> PathBuf {
        std::env::temp_dir().join(format!("bir-cor-ocr-test-{}", uuid::Uuid::new_v4()))
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
          "filing_reminders": ["1702Q - 60 days after quarter end"]
        }"#;

        let extraction = parse_gemini_cor_json(text).unwrap();
        assert_eq!(extraction.confidence, Some(0.78));
        let fields = extraction.into_fields();
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
    }

    #[test]
    fn malformed_gemini_json_is_review_needed_error() {
        let error = parse_gemini_cor_json("not-json").unwrap_err();
        assert!(error.contains("Gemini JSON could not be parsed"));
    }

    #[test]
    fn missing_key_disables_gemini_test_path() {
        let result = test_gemini_api_key(DEFAULT_GEMINI_MODEL, Some(""));
        if std::env::var("GEMINI_API_KEY").is_err() && !has_gemini_api_key() {
            assert!(result.unwrap_err().contains("Add a Gemini API key"));
        }
    }
}
