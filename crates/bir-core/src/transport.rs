//! FTP transport for submitting encrypted returns to BIR servers.

use suppaftp::AsyncFtpStream;
use suppaftp::types::FileType;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("FTP error: {0}")]
    Ftp(#[from] suppaftp::FtpError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Submission rejected by server")]
    Rejected,
}

/// BIR FTP credentials (hardcoded as in the original binaries)
const BIR_FTP_HOST: &str = "103.56.5.254:21";
const BIR_FTP_USER: &str = "uploadOnly";
const BIR_FTP_PASS: &str = "12birBIR";

/// Open FTP session through CWD. Connect / login / CWD does not STOR.
///
/// Queue workers must claim only after this succeeds, immediately before
/// [`IafFtpSession::store`]. A connect timeout therefore stays unclaimed.
pub(crate) async fn open_iaf_session(form_type: &str) -> Result<IafFtpSession, TransportError> {
    info!("Connecting to BIR Remote Gateway: {}", BIR_FTP_HOST);

    let mut ftp_stream = AsyncFtpStream::connect(BIR_FTP_HOST).await?;
    ftp_stream.login(BIR_FTP_USER, BIR_FTP_PASS).await?;
    info!("Securely authenticated to BIR Gateway");

    ftp_stream.transfer_type(FileType::Binary).await?;

    info!("Targeting route: /{}", form_type);
    ftp_stream.cwd(&format!("/{form_type}")).await?;

    Ok(IafFtpSession {
        stream: Some(ftp_stream),
    })
}

/// Prepared BIR FTP session that has not yet STOR'd the IAF file.
pub(crate) struct IafFtpSession {
    stream: Option<AsyncFtpStream>,
}

impl IafFtpSession {
    /// Irreversible STOR of the encrypted IAF payload.
    pub async fn store(mut self, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
        let mut ftp_stream = self
            .stream
            .take()
            .expect("IafFtpSession carries one FTP stream");
        info!("Transmitting payload: {}", filename);
        let mut reader = payload;
        let result = ftp_stream.put_file(filename, &mut reader).await;
        let _ = ftp_stream.quit().await;
        result?;
        info!("Transmission complete: {}", filename);
        Ok(())
    }
}

/// Uploads an encrypted IAF file to the BIR FTP server.
/// Returns Ok(()) if the upload is successful.
///
/// `form_type` must match exactly the subfolder name on the BIR server (e.g., "2551Qv2018").
/// `filename` must be the IAF filename (e.g., "010558054000-2551Qv2018-122026Q1#email@.xml").
/// `payload` is the raw encrypted bytes of the IAF file.
///
/// This raw irreversible boundary is crate-internal. External callers must not
/// bypass the reviewed Final Copy, queue-admission, and claim workflows.
/// Queue workers should call [`open_iaf_session`] then claim then
/// [`IafFtpSession::store`] so a connect timeout does not freeze a claim.
pub(crate) async fn submit_iaf(
    form_type: &str,
    filename: &str,
    payload: &[u8],
) -> Result<(), TransportError> {
    let session = open_iaf_session(form_type).await?;
    session.store(filename, payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We normally don't want to hit the live BIR production server during automated testing.
    // This test is ignored by default and can be run manually using:
    // `cargo test test_ftp_connection -- --ignored`
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
