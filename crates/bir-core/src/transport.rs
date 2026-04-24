//! FTP transport for submitting encrypted returns to BIR servers.

use suppaftp::types::FileType;
use suppaftp::AsyncFtpStream;
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

/// Uploads an encrypted IAF file to the BIR FTP server.
/// Returns Ok(()) if the upload is successful.
///
/// `form_type` must match exactly the subfolder name on the BIR server (e.g., "2551Qv2018").
/// `filename` must be the IAF filename (e.g., "010558054000-2551Qv2018-122026Q1#email@.xml").
/// `payload` is the raw encrypted bytes of the IAF file.
pub async fn submit_iaf(form_type: &str, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
    info!("Connecting to BIR FTP server: {}", BIR_FTP_HOST);
    
    // Connect to the FTP server
    let mut ftp_stream = AsyncFtpStream::connect(BIR_FTP_HOST).await?;
    
    // Authenticate
    ftp_stream.login(BIR_FTP_USER, BIR_FTP_PASS).await?;
    info!("Successfully authenticated to BIR FTP");

    // `suppaftp` defaults to using passive mode for transfers automatically
    
    // Switch to Binary mode
    ftp_stream.transfer_type(FileType::Binary).await?;
    
    // Navigate to the form type directory
    // If the directory doesn't exist, we assume it's a server configuration issue,
    // as the BIR server is structured to have subdirectories for each form type.
    info!("Changing working directory to: /{}", form_type);
    ftp_stream.cwd(&format!("/{}", form_type)).await?;

    // Upload the file
    info!("Uploading file: {}", filename);
    let mut reader = payload; // &[u8] implements AsyncRead
    ftp_stream.put_file(filename, &mut reader).await?;

    info!("Upload complete: {}", filename);

    // Gracefully disconnect
    let _ = ftp_stream.quit().await;

    Ok(())
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
        let mut ftp_stream = AsyncFtpStream::connect(BIR_FTP_HOST).await.expect("Failed to connect");
        ftp_stream.login(BIR_FTP_USER, BIR_FTP_PASS).await.expect("Failed to login");
        ftp_stream.quit().await.expect("Failed to quit");
    }
}
