//! Agent tokens for the BIR apps this adapter may reach.
//!
//! `BIR_MCP_TOKENS` names a JSON file `{"<connect id or pid>": "<token>"}`
//! (a pid key may be written `"12345"` or `"pid:12345"`). On unix the file is
//! read only when no group or other permission bit is set (`chmod 600`).
//! `GPUI_AGENT_TOKEN` is the fallback token for any app without an entry.
//!
//! Tokens are never logged, never returned to the model and never part of an
//! error message: nothing here derives `Debug`, and parse errors report only
//! a line and column.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const TOKENS_ENV: &str = "BIR_MCP_TOKENS";
pub const TOKEN_ENV: &str = "GPUI_AGENT_TOKEN";

/// Where tokens come from. The file is re-read on every lookup so the user
/// can add an app's token without restarting `bir-mcp`.
pub struct TokenSource {
    file: Option<PathBuf>,
    fallback: Option<String>,
}

/// Tokens loaded for one lookup.
pub struct Tokens {
    by_id: BTreeMap<String, String>,
    fallback: Option<String>,
}

impl TokenSource {
    pub fn new(file: Option<PathBuf>, fallback: Option<String>) -> Self {
        Self {
            file,
            fallback: fallback.filter(|token| !token.is_empty()),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var_os(TOKENS_ENV)
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
            std::env::var(TOKEN_ENV).ok(),
        )
    }

    pub fn load(&self) -> Result<Tokens, String> {
        let by_id = match &self.file {
            Some(path) => read_token_file(path)?,
            None => BTreeMap::new(),
        };
        Ok(Tokens {
            by_id,
            fallback: self.fallback.clone(),
        })
    }
}

impl Tokens {
    /// The token for an app: its connect id's entry, then its pid's, then the
    /// fallback.
    pub fn for_app(&self, connect_id: Option<&str>, pid: u32) -> Option<&str> {
        connect_id
            .and_then(|id| self.by_id.get(id))
            .or_else(|| self.by_id.get(&pid.to_string()))
            .or_else(|| self.by_id.get(&format!("pid:{pid}")))
            .or(self.fallback.as_ref())
            .map(String::as_str)
    }
}

fn read_token_file(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let shown = path.display();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path)
            .map_err(|error| format!("cannot read {TOKENS_ENV} file {shown}: {error}"))?;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(format!(
                "refusing to read {TOKENS_ENV} file {shown}: mode {mode:o} lets other users read it; run `chmod 600 {shown}`"
            ));
        }
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {TOKENS_ENV} file {shown}: {error}"))?;
    let parsed: BTreeMap<String, String> = serde_json::from_str(&text).map_err(|error| {
        // Never echo serde's message: it can quote a value, i.e. a token.
        format!(
            "{TOKENS_ENV} file {shown} must be a JSON object of id -> token string (problem at line {}, column {})",
            error.line(),
            error.column()
        )
    })?;
    Ok(parsed
        .into_iter()
        .map(|(id, token)| (id.trim().to_string(), token))
        .filter(|(id, token)| !id.is_empty() && !token.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, body: &str, mode: u32) -> PathBuf {
        let path = dir.join("tokens.json");
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = mode;
        path
    }

    #[test]
    fn lookup_prefers_connect_id_then_pid_then_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            r#"{"bir-demo": "tok-demo", "42": "tok-42", "pid:43": "tok-43", "empty": ""}"#,
            0o600,
        );
        let tokens = TokenSource::new(Some(path.clone()), Some("tok-any".into()))
            .load()
            .unwrap();
        assert_eq!(tokens.for_app(Some("bir-demo"), 42), Some("tok-demo"));
        assert_eq!(tokens.for_app(Some("other"), 42), Some("tok-42"));
        assert_eq!(tokens.for_app(None, 43), Some("tok-43"));
        assert_eq!(tokens.for_app(Some("empty"), 7), Some("tok-any"));
        let no_fallback = TokenSource::new(Some(path), None).load().unwrap();
        assert_eq!(no_fallback.for_app(None, 7), None);
        assert_eq!(no_fallback.for_app(Some("empty"), 7), None);
        let nothing = TokenSource::new(None, Some(String::new())).load().unwrap();
        assert_eq!(nothing.for_app(Some("bir-demo"), 42), None);
    }

    #[cfg(unix)]
    #[test]
    fn group_or_world_readable_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for mode in [0o644, 0o640, 0o604, 0o660] {
            let path = write(dir.path(), r#"{"bir-demo": "secret-value"}"#, mode);
            let error = TokenSource::new(Some(path), None)
                .load()
                .err()
                .expect("refused");
            assert!(error.contains("chmod 600"), "{mode:o}: {error}");
            assert!(!error.contains("secret-value"), "{error}");
        }
        let path = write(dir.path(), r#"{"bir-demo": "secret-value"}"#, 0o400);
        assert!(TokenSource::new(Some(path), None).load().is_ok());
    }

    #[test]
    fn bad_json_and_missing_file_never_echo_a_token() {
        let dir = tempfile::tempdir().unwrap();
        for body in [
            r#"{"bir-demo": 123456789}"#,
            r#"["secret-value"]"#,
            "{secret-value",
        ] {
            let path = write(dir.path(), body, 0o600);
            let error = TokenSource::new(Some(path), None)
                .load()
                .err()
                .expect("refused");
            assert!(error.contains("JSON object"), "{error}");
            assert!(!error.contains("123456789"), "{error}");
            assert!(!error.contains("secret-value"), "{error}");
        }
        let missing = TokenSource::new(Some(dir.path().join("nope.json")), None)
            .load()
            .err()
            .expect("missing");
        assert!(missing.contains(TOKENS_ENV), "{missing}");
    }
}
