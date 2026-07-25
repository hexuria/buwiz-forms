use crate::bir_xml::{BirXmlParseError, generate_bir_xml, parse_bir_xml_checked};
use crate::crypto::{
    BIR_IAF_PASSPHRASE, CryptoError, compress_and_encrypt, decrypt_and_decompress,
};
use crate::forms::{FORM_CAPABILITY_REGISTRY, FormCapabilityRecord};
use crate::transport::{TransportError, submit_iaf};
use chrono::Local;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OfficialImportError {
    #[error("invalid imported savefile name: {0}")]
    InvalidFileName(String),
    #[error("could not read imported savefile: {0}")]
    Read(#[from] std::io::Error),
    #[error("could not decrypt or encrypt imported savefile: {0}")]
    Crypto(#[from] CryptoError),
    #[error("invalid imported BIR payload: {0}")]
    InvalidPayload(#[from] BirXmlParseError),
    #[error("imported filename names unknown form identity {identity:?}")]
    UnknownFormIdentity { identity: String },
    #[error(
        "imported filename identity {filename_identity:?} does not match payload identities {payload_identities:?}"
    )]
    FormIdentityMismatch {
        filename_identity: String,
        payload_identities: Vec<String>,
    },
    #[error("imported payload has no exact form-specific field identity")]
    MissingPayloadFormIdentity,
    #[error("form {form_id} is not allowed to enter the submission queue")]
    QueueSubmissionUnsupported { form_id: String },
    #[error("direct imported submission is not certified for exact form {form_id}")]
    ImportedSubmissionUnsupported { form_id: String },
    #[error("validated imported payload is missing required metadata field {field_id:?}")]
    MissingMetadataField { field_id: String },
    #[error("validated imported payload has no email and no non-empty fallback email")]
    MissingEmail,
    #[error("an imported field was lost while preparing the validated payload: {field_id:?}")]
    ImportedFieldLost { field_id: String },
    #[error("imported payload submission failed: {0}")]
    Transport(#[from] TransportError),
}

#[derive(Debug)]
pub struct OfficialSavefile {
    pub tin: String,
    pub form_type: String,
    pub period_code: String,
    pub email: String,
    pub year: u16,
    pub quarter: Option<u8>,
    pub month: Option<u8>,
}

trait ImportedSubmissionClient {
    fn submit<'a>(
        &'a self,
        form_type: &'a str,
        filename: &'a str,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 'a>>;
}

struct NetworkSubmissionClient;

impl ImportedSubmissionClient for NetworkSubmissionClient {
    fn submit<'a>(
        &'a self,
        form_type: &'a str,
        filename: &'a str,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 'a>> {
        Box::pin(submit_iaf(form_type, filename, payload))
    }
}

struct PreparedImportedSubmission {
    tin: String,
    form_id: &'static str,
    period_code: String,
    email: String,
    encrypted: Vec<u8>,
}

/// Importing an opaque official savefile is a separate capability from
/// generating and queueing a typed draft. No exact-form imported-payload
/// validator is certified yet, so this path intentionally fails closed for
/// every current form before encryption or network I/O.
pub async fn import_and_submit_savefile(
    file_path: &Path,
    fallback_email: Option<&str>,
) -> Result<OfficialSavefile, OfficialImportError> {
    import_and_submit_savefile_with_client(file_path, fallback_email, &NetworkSubmissionClient)
        .await
}

async fn import_and_submit_savefile_with_client<C: ImportedSubmissionClient>(
    file_path: &Path,
    fallback_email: Option<&str>,
    client: &C,
) -> Result<OfficialSavefile, OfficialImportError> {
    let prepared = prepare_imported_submission(file_path, fallback_email)?;
    let submit_filename = format!(
        "{}-{}-{}#{}#.xml",
        prepared.tin, prepared.form_id, prepared.period_code, prepared.email
    );

    client
        .submit(prepared.form_id, &submit_filename, &prepared.encrypted)
        .await?;

    let (year, quarter, month) = parse_period_code(&prepared.period_code);
    Ok(OfficialSavefile {
        tin: prepared.tin,
        form_type: prepared.form_id.to_string(),
        period_code: prepared.period_code,
        email: prepared.email,
        year,
        quarter,
        month,
    })
}

fn prepare_imported_submission(
    file_path: &Path,
    fallback_email: Option<&str>,
) -> Result<PreparedImportedSubmission, OfficialImportError> {
    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| OfficialImportError::InvalidFileName("non-UTF-8 filename".to_string()))?;
    let (tin, filename_identity, period_code) = parse_imported_filename(file_name)?;
    let capability = find_capability_identity(&filename_identity).ok_or_else(|| {
        OfficialImportError::UnknownFormIdentity {
            identity: filename_identity.clone(),
        }
    })?;

    let ciphertext = std::fs::read(file_path)?;
    let plaintext = decrypt_and_decompress(&ciphertext, BIR_IAF_PASSPHRASE)?;
    let plaintext = std::str::from_utf8(&plaintext).map_err(|error| {
        OfficialImportError::InvalidPayload(BirXmlParseError::MalformedDiv {
            offset: error.valid_up_to(),
            reason: "decrypted payload is not UTF-8".to_string(),
        })
    })?;
    let fields = parse_bir_xml_checked(plaintext)?;

    validate_payload_identity(capability, &filename_identity, &fields)?;
    if !capability.can_queue() {
        return Err(OfficialImportError::QueueSubmissionUnsupported {
            form_id: capability.form_id.to_string(),
        });
    }

    let validator = imported_payload_validator(capability.form_id).ok_or_else(|| {
        OfficialImportError::ImportedSubmissionUnsupported {
            form_id: capability.form_id.to_string(),
        }
    })?;
    // The validator receives the untouched complete map. Metadata is mutated
    // only after exact-form semantics and required keys have passed.
    validator(&fields)?;

    prepare_validated_payload(tin, capability, period_code, fields, fallback_email)
}

type ImportedPayloadValidator = fn(&BTreeMap<String, String>) -> Result<(), OfficialImportError>;

fn imported_payload_validator(_form_id: &str) -> Option<ImportedPayloadValidator> {
    // Intentionally empty. Add a validator only with exact-form required-key,
    // computed-field, identity, and lifecycle evidence.
    None
}

fn prepare_validated_payload(
    tin: String,
    capability: &'static FormCapabilityRecord,
    period_code: String,
    mut fields: BTreeMap<String, String>,
    fallback_email: Option<&str>,
) -> Result<PreparedImportedSubmission, OfficialImportError> {
    let original_keys = fields.keys().cloned().collect::<BTreeSet<_>>();
    if !fields.contains_key("txtEmail") {
        return Err(OfficialImportError::MissingMetadataField {
            field_id: "txtEmail".to_string(),
        });
    }
    if !fields.contains_key("txtDateIssue") {
        return Err(OfficialImportError::MissingMetadataField {
            field_id: "txtDateIssue".to_string(),
        });
    }

    let email = fields
        .get("txtEmail")
        .filter(|email| !email.trim().is_empty())
        .cloned()
        .or_else(|| {
            fallback_email
                .map(str::trim)
                .filter(|email| !email.is_empty())
                .map(str::to_string)
        })
        .ok_or(OfficialImportError::MissingEmail)?;

    fields.insert("txtEmail".to_string(), email.clone());
    fields.insert(
        "txtDateIssue".to_string(),
        Local::now().format("%m/%d/%Y %H:%M:%S").to_string(),
    );

    for original_key in original_keys {
        if !fields.contains_key(&original_key) {
            return Err(OfficialImportError::ImportedFieldLost {
                field_id: original_key,
            });
        }
    }

    let outbound_xml = generate_bir_xml(&fields);
    let encrypted = compress_and_encrypt(outbound_xml.as_bytes(), BIR_IAF_PASSPHRASE)?;
    Ok(PreparedImportedSubmission {
        tin,
        form_id: capability.form_id,
        period_code,
        email,
        encrypted,
    })
}

fn parse_imported_filename(
    file_name: &str,
) -> Result<(String, String, String), OfficialImportError> {
    let stem = file_name
        .strip_suffix(".xml")
        .ok_or_else(|| OfficialImportError::InvalidFileName(file_name.to_string()))?;
    let base = stem.split('#').next().unwrap_or_default();
    let mut parts = base.splitn(3, '-');
    let tin = parts.next().unwrap_or_default();
    let identity = parts.next().unwrap_or_default();
    let period = parts.next().unwrap_or_default();
    if tin.is_empty() || identity.is_empty() || period.is_empty() {
        return Err(OfficialImportError::InvalidFileName(file_name.to_string()));
    }
    Ok((tin.to_string(), identity.to_string(), period.to_string()))
}

fn find_capability_identity(identity: &str) -> Option<&'static FormCapabilityRecord> {
    FORM_CAPABILITY_REGISTRY.iter().find(|capability| {
        capability.code.eq_ignore_ascii_case(identity)
            || capability.form_id.eq_ignore_ascii_case(identity)
    })
}

fn validate_payload_identity(
    capability: &FormCapabilityRecord,
    filename_identity: &str,
    fields: &BTreeMap<String, String>,
) -> Result<(), OfficialImportError> {
    let payload_identities = fields
        .keys()
        .filter_map(|field_id| {
            let namespace = field_id.split_once(':')?.0;
            if !namespace
                .get(..3)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("frm"))
            {
                return None;
            }
            namespace
                .get(3..)
                .filter(|identity| !identity.is_empty())
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    if payload_identities.is_empty() {
        return Err(OfficialImportError::MissingPayloadFormIdentity);
    }

    let expected_code = normalize_identity(capability.code);
    let expected_form_id = normalize_identity(capability.form_id);
    let all_match = payload_identities.iter().all(|identity| {
        let normalized = normalize_identity(identity);
        normalized == expected_code || normalized == expected_form_id
    });
    if !all_match {
        return Err(OfficialImportError::FormIdentityMismatch {
            filename_identity: filename_identity.to_string(),
            payload_identities: payload_identities.into_iter().collect(),
        });
    }
    Ok(())
}

fn normalize_identity(identity: &str) -> String {
    identity
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

// Helper to extract year/quarter/month from period code
pub fn parse_period_code(period_code: &str) -> (u16, Option<u8>, Option<u8>) {
    // 122026Q1 -> year 2026, month 12, quarter 1
    // 122026 -> year 2026, month 12
    if period_code.len() >= 6 {
        let month_str = &period_code[0..2];
        let year_str = &period_code[2..6];

        let month: Option<u8> = month_str.parse().ok();
        let year: u16 = year_str.parse().unwrap_or(
            chrono::Local::now()
                .format("%Y")
                .to_string()
                .parse()
                .unwrap_or(2024),
        );

        let mut quarter = None;
        if period_code.len() > 6 && period_code.contains('Q') {
            let q_part = period_code.split('Q').next_back().unwrap_or("");
            quarter = q_part.parse().ok();
        }

        (year, quarter, month)
    } else {
        (
            chrono::Local::now()
                .format("%Y")
                .to_string()
                .parse()
                .unwrap_or(2024),
            None,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::compress_and_encrypt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingSubmissionClient {
        calls: AtomicUsize,
    }

    impl ImportedSubmissionClient for CountingSubmissionClient {
        fn submit<'a>(
            &'a self,
            _form_type: &'a str,
            _filename: &'a str,
            _payload: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 'a>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    fn encrypted_fixture(
        file_name: &str,
        plaintext: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(file_name);
        let encrypted = compress_and_encrypt(plaintext.as_bytes(), BIR_IAF_PASSPHRASE).unwrap();
        std::fs::write(&path, encrypted).unwrap();
        (directory, path)
    }

    fn one_line_payload(fields: &[(&str, &str)]) -> String {
        let fields = fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>();
        generate_bir_xml(&fields).replace(['\n', '\t'], "")
    }

    #[tokio::test]
    async fn unsupported_0619e_is_blocked_without_transport() {
        let xml = one_line_payload(&[
            ("frm0619E:txtMonth", "04"),
            ("txtDateIssue", ""),
            ("txtEmail", "test@example.com"),
        ]);
        let (_directory, path) =
            encrypted_fixture("00000000000000-0619E-042026#test@example.com#.xml", &xml);
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::QueueSubmissionUnsupported { ref form_id }
                if form_id == "0619Ev2018"
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unsupported_0619f_is_blocked_without_transport() {
        let xml = one_line_payload(&[
            ("frm0619F:txtMonth", "04"),
            ("txtDateIssue", ""),
            ("txtEmail", "test@example.com"),
        ]);
        let (_directory, path) =
            encrypted_fixture("00000000000000-0619F-042026WB#test@example.com#.xml", &xml);
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::QueueSubmissionUnsupported { ref form_id }
                if form_id == "0619Fv2018"
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fallback_email_cannot_rescue_a_nearly_empty_payload() {
        let xml = one_line_payload(&[("frm0619E:txtMonth", "04")]);
        let (_directory, path) = encrypted_fixture("00000000000000-0619E-042026.xml", &xml);
        let client = CountingSubmissionClient::default();

        let error =
            import_and_submit_savefile_with_client(&path, Some("fallback@example.com"), &client)
                .await
                .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::QueueSubmissionUnsupported { .. }
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn malformed_payload_is_rejected_without_transport() {
        let (_directory, path) = encrypted_fixture(
            "00000000000000-0619E-042026.xml",
            "<?xml version='1.0'?><div>frm0619E:txtMonth=04</div>",
        );
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(error, OfficialImportError::InvalidPayload(_)));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn duplicate_field_ids_are_rejected_without_transport() {
        let xml = "<?xml version='1.0'?><div>frm0619E:txtMonth=04frm0619E:txtMonth=</div><div id=\"frm0619E:txtMonth\">05</div>";
        let (_directory, path) = encrypted_fixture("00000000000000-0619E-042026.xml", xml);
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::InvalidPayload(BirXmlParseError::DuplicateFieldId { .. })
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn mismatched_filename_and_payload_identity_are_rejected() {
        let xml = one_line_payload(&[("frm2551Q:txtYear", "2026")]);
        let (_directory, path) = encrypted_fixture("00000000000000-0619E-042026.xml", &xml);
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::FormIdentityMismatch { .. }
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unknown_form_identity_is_rejected_without_transport() {
        let xml = one_line_payload(&[("frm9999:txtYear", "2026")]);
        let (_directory, path) = encrypted_fixture("00000000000000-9999-042026.xml", &xml);
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::UnknownFormIdentity { .. }
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn queue_capable_form_still_requires_exact_import_validator() {
        let xml = one_line_payload(&[
            ("frm2551Q:txtYear", "2026"),
            ("txtDateIssue", ""),
            ("txtEmail", "test@example.com"),
        ]);
        let (_directory, path) = encrypted_fixture("00000000000000-2551Qv2018-122026Q1.xml", &xml);
        let client = CountingSubmissionClient::default();

        let error = import_and_submit_savefile_with_client(&path, None, &client)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OfficialImportError::ImportedSubmissionUnsupported { ref form_id }
                if form_id == "2551Qv2018"
        ));
        assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn validated_metadata_updates_preserve_every_unknown_field() {
        let capability = find_capability_identity("2551Q").unwrap();
        let fields = [
            ("frm2551Q:txtYear", "2026"),
            ("txtDateIssue", ""),
            ("txtEmail", "test@example.com"),
            ("futureUnknownField", "must survive"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
        let original_keys = fields.keys().cloned().collect::<BTreeSet<_>>();

        let prepared = prepare_validated_payload(
            "00000000000000".to_string(),
            capability,
            "122026Q1".to_string(),
            fields,
            None,
        )
        .unwrap();
        let plaintext = decrypt_and_decompress(&prepared.encrypted, BIR_IAF_PASSPHRASE).unwrap();
        let parsed = crate::bir_xml::parse_bir_xml_with_codec_checked(
            std::str::from_utf8(&plaintext).unwrap(),
            bir_rules::serialization::BodyCodec::Utf8PercentRfc3986Unreserved,
        )
        .unwrap();

        assert_eq!(
            parsed.keys().cloned().collect::<BTreeSet<_>>(),
            original_keys
        );
        assert_eq!(parsed["futureUnknownField"], "must survive");
    }
}
