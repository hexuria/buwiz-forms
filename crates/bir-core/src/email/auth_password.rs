//! App Password authentication — standard IMAP LOGIN.
//!
//! Works with Gmail (App Passwords), Outlook, Yahoo, or any IMAP server.

use super::fetcher::ImapAuthenticator;


pub struct AppPasswordAuth {
    email: String,
    password: String,
}

impl AppPasswordAuth {
    /// Create a new App Password authenticator.
    pub fn new(email: &str, password: &str) -> Result<Self, anyhow::Error> {
        if password.is_empty() {
            return Err(anyhow::anyhow!("App Password is empty."));
        }
        Ok(Self {
            email: email.to_string(),
            password: password.to_string(),
        })
    }
}

impl ImapAuthenticator for AppPasswordAuth {
    fn authenticate(
        &self,
        client: imap::Client<native_tls::TlsStream<std::net::TcpStream>>,
    ) -> Result<(imap::Session<native_tls::TlsStream<std::net::TcpStream>>, Option<String>), anyhow::Error> {
        client
            .login(&self.email, &self.password)
            .map(|session| (session, None))
            .map_err(|e| anyhow::anyhow!("IMAP login failed: {}", e.0))
    }

    fn host(&self) -> &str {
        // This is unused since the host comes from the profile, but we
        // implement the trait fully.
        "imap.gmail.com"
    }
}
