//! BIR submission transport.
//!
//! eBIRForms 7.9.6.x replaced the historical FTP gateway (`103.56.5.254:21`)
//! with per-TIN/form SFTP: the endpoint is resolved at submit time from
//! `tinDispatcherSFTP.php` and no credentials are embedded. This module keeps the
//! crate-internal `submit_iaf` boundary and error surface stable while delegating
//! the actual upload to [`crate::submission_transport`]. See
//! `evidence/sftp-submission/` for the protocol reconstruction.

use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum TransportError {
    /// Retained for API/back-compat of the error surface (legacy FTP path).
    #[error("FTP error: {0}")]
    Ftp(#[from] suppaftp::FtpError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Submission rejected by server")]
    Rejected,
}

/// Legacy hardcoded FTP endpoint — no longer used for submission (the gateway is
/// gone). Kept only for the ignored manual connectivity test below.
#[allow(dead_code)]
const BIR_FTP_HOST: &str = "103.56.5.254:21";
#[allow(dead_code)]
const BIR_FTP_USER: &str = "uploadOnly";
#[allow(dead_code)]
const BIR_FTP_PASS: &str = "12birBIR";

/// Uploads an encrypted IAF file to BIR over SFTP.
/// Returns Ok(()) if the upload is successful.
///
/// `form_type` must match exactly the subfolder name on the BIR server (e.g., "2551Qv2018").
/// `filename` must be the IAF filename (e.g., "01055805400000-2551Qv2018-122026Q1#email#.xml").
/// `payload` is the raw encrypted bytes of the IAF file.
///
/// The dispatcher is keyed on the 9-digit TIN, which prefixes the IAF filename.
///
/// This raw irreversible boundary is crate-internal. External callers must not
/// bypass the reviewed Final Copy, queue-admission, and claim workflows.
pub(crate) async fn submit_iaf(
    form_type: &str,
    filename: &str,
    payload: &[u8],
) -> Result<(), TransportError> {
    let tin9: String = filename
        .split('-')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(9)
        .collect();
    if tin9.len() < 9 {
        info!("Refusing submission: filename lacks a 9-digit TIN prefix: {filename}");
        return Err(TransportError::Rejected);
    }

    info!("Resolving BIR SFTP endpoint for form {form_type} (dispatcher)");
    let transport = crate::submission_transport::DispatcherSftpTransport { tin: tin9 };
    use crate::submission_transport::SubmissionTransport as _;
    transport
        .submit(form_type, filename, payload)
        .await
        .map_err(map_submission_error)
}

/// Preserve the crate-internal error surface (`Ftp`/`Io`/`Rejected`) so the queue
/// workers' error categorization keeps compiling unchanged.
fn map_submission_error(error: crate::submission_transport::TransportError) -> TransportError {
    use crate::submission_transport::TransportError as S;
    match error {
        S::Io(io) => TransportError::Io(io),
        // Dispatcher/crypto/ssh/sftp/rejected all mean the submission did not
        // demonstrably succeed; treat as a server-side rejection for retry policy.
        _ => TransportError::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suppaftp::AsyncFtpStream;

    // Legacy manual FTP connectivity probe. The gateway is retired, so this is
    // ignored by default; kept for historical reference.
    #[tokio::test]
    #[ignore]
    async fn test_ftp_connection() {
        let mut ftp_stream = AsyncFtpStream::connect(BIR_FTP_HOST)
            .await
            .expect("Failed to connect");
        ftp_stream
            .login(BIR_FTP_USER, BIR_FTP_PASS)
            .await
            .expect("Failed to login");
        ftp_stream.quit().await.expect("Failed to quit");
    }
}
