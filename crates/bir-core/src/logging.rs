//! Process logging for painted `bir` and `bir-shell`.
//!
//! Both binaries write the same daily files under [`crate::log_files::log_dir`]
//! (`ebirforms.YYYY-MM-DD.log`). The Logs screen in each UI tails today's
//! file; there is no ring buffer and no log table. A stdout-only subscriber
//! leaves that file missing, which is why `bir-shell`'s Logs tab stayed empty
//! and a shipped build (no console) kept nothing.
//!
//! Lines are scrubbed before they hit the file or the terminal: OAuth tokens,
//! passwords, the database key and taxpayer TINs. A TIN keeps only its last
//! three root digits (and a written branch), enough to correlate one return
//! without exposing the identifier in the Logs tab or a copied log.

use std::io::{self, Write};
use std::path::Path;
use std::sync::{Once, OnceLock};

use regex::Regex;
use tracing_log::AsLog;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

use crate::log_files::{self, LOG_FILE_PREFIX, LOG_FILE_SUFFIX};

/// Level token the Logs tab filters on. Matches the painted classifier:
/// the fmt layer pads `INFO` and `WARN` so the marker is ` INFO ` / ` WARN `,
/// and `ERROR` / `FATAL` are five characters (` ERROR `).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLineLevel {
    Error,
    Warn,
    Info,
    Other,
}

pub fn log_line_level(line: &str) -> LogLineLevel {
    if line.contains(" ERROR ") || line.contains(" FATAL ") {
        LogLineLevel::Error
    } else if line.contains(" WARN ") {
        LogLineLevel::Warn
    } else if line.contains(" INFO ") {
        LogLineLevel::Info
    } else {
        LogLineLevel::Other
    }
}

/// `DEVELOPER_MODE=true`, or the Dev Mode "Verbose logs" switch (read here,
/// before the database is open, so it takes effect at startup).
pub fn developer_mode_enabled() -> bool {
    let _ = dotenvy::dotenv();
    std::env::var("DEVELOPER_MODE")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || crate::dev_mode::verbose_logs().value
}

/// Painted `bir` directives when `RUST_LOG` is unset.
pub fn desktop_filter(developer_mode: bool) -> &'static str {
    if developer_mode {
        "bir=debug,bir_shell=debug,bir_print=debug,bir_core=info"
    } else {
        "bir=info,bir_shell=info,bir_print=error,bir_core=info"
    }
}

/// `bir-shell` directives when `RUST_LOG` is unset.
///
/// `bir_core=info` keeps IMAP, confirmation-watcher, SFTP, and cron warnings.
/// `gpui_shell::script=info` is where QuickJS `console.log` / `warn` / `error`
/// are emitted. Unlisted targets stay at error, same as the painted filter.
pub fn shell_filter(developer_mode: bool) -> &'static str {
    // `panic=error` is the hook's target. Painted `bir` has no panic hook, so
    // its directives stay as they were. A directive list with no global level
    // leaves every other target off, same as painted `bir`.
    if developer_mode {
        "bir_shell=debug,bir_core=info,bir_print=debug,gpui_shell::script=debug,panic=error"
    } else {
        "bir_shell=info,bir_core=info,bir_print=error,gpui_shell::script=info,panic=error"
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InstallOptions {
    pub developer_mode: bool,
    pub default_filter: &'static str,
}

/// Install the process subscriber and the panic hook. A second call is a no-op
/// and reports whether the first call succeeded.
pub fn install(options: InstallOptions) -> bool {
    static INSTALLED: OnceLock<bool> = OnceLock::new();
    *INSTALLED.get_or_init(
        || match tracing::subscriber::set_global_default(subscriber(options)) {
            Ok(()) => {
                // NSH may already own the log facade; its logger must stay in place.
                let _ = tracing_log::LogTracer::builder()
                    .with_max_level(tracing::level_filters::LevelFilter::current().as_log())
                    .init();
                install_panic_hook();
                tracing::info!(
                    "🔍 Tracing initialized (developer_mode: {})",
                    options.developer_mode
                );
                true
            }
            Err(error) => {
                eprintln!("logging subscriber not installed: {error}");
                false
            }
        },
    )
}

/// Run `body` on a thread-local subscriber built the same way as [`install`].
///
/// Tests use this so one case does not capture every later test in the process.
pub fn with_thread_subscriber<T>(options: InstallOptions, body: impl FnOnce() -> T) -> T {
    tracing::subscriber::with_default(subscriber(options), || {
        // Callsite interest and the max level are cached process-wide over the
        // dispatchers alive when they were computed. Other tests creating and
        // dropping their own thread-local dispatchers in parallel (or running
        // under a global one) can leave a callsite cached as disabled for this
        // thread; recompute now that this thread's default is in place.
        tracing::callsite::rebuild_interest_cache();
        body()
    })
}

fn subscriber(options: InstallOptions) -> impl tracing::Subscriber + Send + Sync {
    let _ = dotenvy::dotenv();
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(options.default_filter))
        .unwrap_or_else(|_| EnvFilter::new("error"));
    let file_layer = file_appender(&log_files::log_dir()).map(|appender| {
        tracing_subscriber::fmt::layer()
            .with_writer(RedactingMake::new(appender))
            .with_ansi(false)
            .with_target(true)
            .with_file(options.developer_mode)
            .with_line_number(options.developer_mode)
    });
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(RedactingMake::new(std::io::stdout))
        .with_target(true)
        .with_file(options.developer_mode)
        .with_line_number(options.developer_mode);
    Registry::default()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
}

fn file_appender(dir: &Path) -> Option<tracing_appender::rolling::RollingFileAppender> {
    let _ = std::fs::create_dir_all(dir);
    tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .build(dir)
        .map_err(|error| eprintln!("log file appender unavailable: {error}"))
        .ok()
}

thread_local! {
    static PANIC_HOOK_GUARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            report_panic(info);
            // Detached serves point stderr at the daemon log. The standard
            // hook would print the raw payload there, bypassing the log
            // writer, so this hook writes the same fact through the masker
            // instead of chaining it.
            redacted_stderr(&format!(
                "panic at {}: {}",
                panic_location(info),
                panic_payload(info)
            ));
        }));
    });
}

/// Record a panic on the `panic` target. Re-entry (a subscriber that panics
/// while logging) is ignored.
pub fn report_panic(info: &std::panic::PanicHookInfo<'_>) {
    let entered = PANIC_HOOK_GUARD.with(|guard| {
        if guard.get() {
            false
        } else {
            guard.set(true);
            true
        }
    });
    if !entered {
        return;
    }
    let payload = redact_secrets(&panic_payload(info));
    let location = panic_location(info);
    tracing::error!(target: "panic", "panic at {location}: {payload}");
    PANIC_HOOK_GUARD.with(|guard| guard.set(false));
}

fn panic_location(info: &std::panic::PanicHookInfo<'_>) -> String {
    match info.location() {
        Some(location) => {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        }
        None => "unknown location".to_string(),
    }
}

fn panic_payload(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(message) = info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Replace credential material and taxpayer identifiers in a log line.
///
/// A TIN is nine root digits plus an optional branch, written solid or in
/// the usual separated three-digit root groups. Dashes, spaces, dots,
/// slashes and underscores are accepted as exact group separators; no-break
/// and thin spaces are normalized for matching, as are full-width digits.
/// Solid runs are masked at 9,
/// 12 (9+3) and 14 (9+5) digits only: a solid 13-digit run can be a
/// millisecond timestamp, so a four-digit branch must be separated. Three
/// space-separated root groups are masked only with a branch or an adjacent
/// word "TIN"; dashed root groups are specific enough on their own. Numbers
/// with decimals are not TINs, nor are punctuated ISO dates/times.
pub fn redact_secrets(input: &str) -> String {
    let mut text = input.to_string();
    // Bearer and token shapes first. A labeled `Authorization: Bearer …`
    // would otherwise keep only the first word and leave the token behind.
    text = bearer()
        .replace_all(&text, "Bearer [redacted]")
        .into_owned();
    text = google_access_token()
        .replace_all(&text, "[redacted]")
        .into_owned();
    text = google_refresh_token()
        .replace_all(&text, "[redacted]")
        .into_owned();
    text = labeled_secret()
        .replace_all(&text, "$1[redacted]")
        .into_owned();
    // Dev start routes print `recovery key XXXX-XXXX-XXXX-XXXX` with a space,
    // which the labeled form (it wants `=` or `:`) does not catch.
    text = recovery_key_phrase()
        .replace_all(&text, "$1[redacted]")
        .into_owned();
    text = text.replace(TEST_MASTER_KEY, "[redacted]");
    text = labeled_hex_key()
        .replace_all(&text, "$1[redacted]")
        .into_owned();
    let text = mask_emails(&text);
    mask_tins(&text)
}

/// Mask a value the caller has already typed as a TIN. Unlike the generic
/// text scanner, this does not have to distinguish the value from a timestamp
/// or count: a compact nine- or twelve-to-fourteen-digit TIN, a compact
/// thirteen-digit TIN, and dashed or spaced groupings all mask the same way.
/// Malformed short values are hidden entirely rather than echoed.
pub fn tin_for_log(tin: &str) -> String {
    let digits: String = tin.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() < 9 {
        return "***".to_string();
    }
    let root_end = &digits[6..9];
    let branch = &digits[9..];
    if branch.is_empty() {
        format!("***-***-{root_end}")
    } else {
        format!("***-***-{root_end}-{branch}")
    }
}

/// Mask the leading TIN in a submission filename (`TIN-form-period[#email#].xml`).
/// Filenames are typed artifacts, so a compact thirteen-digit TIN is a TIN here
/// even though the generic log scanner leaves that shape alone as a timestamp.
pub fn submission_filename_for_log(filename: &str) -> String {
    static LEADING_TIN: OnceLock<Regex> = OnceLock::new();
    let pattern = LEADING_TIN.get_or_init(|| {
        Regex::new(r"^([0-9]{9}(?:[0-9]{3,5})?)-").expect("static filename TIN regex")
    });
    let Some(captures) = pattern.captures(filename) else {
        return filename.to_string();
    };
    let tin = captures
        .get(1)
        .expect("the leading TIN capture exists")
        .as_str();
    format!("{}{}", tin_for_log(tin), &filename[tin.len()..])
}

/// Mask the submission filename at the end of a local or remote path.
pub fn submission_path_for_log(path: &str) -> String {
    let filename_start = path.rfind(['/', '\\']).map(|index| index + 1).unwrap_or(0);
    format!(
        "{}{}",
        &path[..filename_start],
        submission_filename_for_log(&path[filename_start..])
    )
}

/// Write one stderr line through the same masker as the log writers. Detached
/// serves connect stderr directly to their log file, so a plain `eprintln!`
/// would bypass every redaction.
pub fn redacted_stderr(line: &str) {
    eprintln!("{}", redact_secrets(line));
}

/// Mask every TIN candidate in `text`. The regex names the official shapes;
/// the boundary check supplies what Rust's regex crate has no look-around for:
/// decimal adjacency and larger identifiers. Candidates are exact matches, so
/// a TIN at the start of a receipt filename is still masked before its `-form`
/// suffix.
fn mask_tins(text: &str) -> String {
    let normalized = TinNormalized::new(text);
    let haystack = normalized.text.as_str();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for candidate in tin_candidate().find_iter(haystack) {
        let start = candidate.start();
        let mut end = candidate.end();
        if !tin_boundaries(haystack.as_bytes(), start, end)
            || masked_tin(&haystack[start..end]).is_none()
        {
            // A greedy branch-shaped tail can make the real nine-digit TIN look
            // like part of a longer number (`123456789 2551Q`). Retry the root.
            let root_end = start + 9;
            if end <= root_end
                || !haystack[start..root_end]
                    .chars()
                    .all(|c| c.is_ascii_digit())
                || !tin_boundaries(haystack.as_bytes(), start, root_end)
                || masked_tin(&haystack[start..root_end]).is_none()
            {
                continue;
            }
            end = root_end;
        }
        if space_root_needs_context(&haystack[start..end]) && !tin_word_near(haystack, start, end) {
            continue;
        }
        let Some(masked) = masked_tin(&haystack[start..end]) else {
            continue;
        };
        let original_start = normalized.original_pos(start);
        let original_end = normalized.original_pos(end);
        out.push_str(&text[copied..original_start]);
        out.push_str(&masked);
        copied = original_end;
    }
    out.push_str(&text[copied..]);
    out
}

/// TIN matching sees no-break spaces as ordinary spaces and full-width digits
/// as ASCII digits. The output still uses the original spelling outside the
/// masked token; these positions map one normalized byte offset back to the
/// original string.
struct TinNormalized {
    text: String,
    positions: Vec<(usize, usize)>,
}

impl TinNormalized {
    fn new(input: &str) -> Self {
        let mut text = String::with_capacity(input.len());
        let mut positions = vec![(0, 0)];
        for (original_at, original) in input.char_indices() {
            let normalized = match original {
                '\u{00a0}' | '\u{2009}' | '\u{202f}' => ' ',
                '\u{ff10}'..='\u{ff19}' => {
                    char::from_u32(u32::from('0') + u32::from(original) - u32::from('\u{ff10}'))
                        .expect("full-width digit")
                }
                _ => original,
            };
            text.push(normalized);
            positions.push((text.len(), original_at + original.len_utf8()));
        }
        Self { text, positions }
    }

    fn original_pos(&self, normalized_at: usize) -> usize {
        let index = self
            .positions
            .partition_point(|(position, _)| *position <= normalized_at)
            .saturating_sub(1);
        let (normalized_base, original_base) = self.positions[index];
        original_base + normalized_at.saturating_sub(normalized_base)
    }
}

/// Mask the local part of an email address, keeping the domain for
/// operational diagnosis. The regex is compiled once and the regex crate's
/// search is linear in the input size.
fn mask_emails(text: &str) -> String {
    email_candidate()
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let local = &captures[1];
            let tld = captures[3].to_ascii_lowercase();
            if matches!(
                tld.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp"
            ) {
                return captures[0].to_owned();
            }
            let visible: String = local.chars().take(2).collect();
            format!("{visible}•••@{}", &captures[2])
        })
        .into_owned()
}

fn email_candidate() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?-u:\b)([A-Za-z0-9._%+-]+)@([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.([A-Za-z]{2,}))(?-u:\b)")
            .expect("static email regex")
    })
}

fn tin_candidate() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"[0-9]{3}[- ./_]?[0-9]{3}[- ./_]?[0-9]{3}(?:[- ./_]?[0-9]{3,5})?")
            .expect("static TIN candidate regex")
    })
}

/// A TIN candidate does not touch a word character or a decimal point; what
/// remains adjacent decides whether it was part of a larger identifier or
/// number.
fn tin_boundaries(bytes: &[u8], start: usize, end: usize) -> bool {
    fn word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric()
    }
    fn hex(byte: u8) -> bool {
        byte.is_ascii_hexdigit()
    }
    // URL percent-escapes end in a hex letter (`%3D123…`), and escaped text in
    // Debug/JSON ends in the escape letter (`\\n123…`). Those letters are
    // encoding syntax, not part of a larger identifier.
    let escaped_before = (start >= 3
        && bytes[start - 3] == b'%'
        && hex(bytes[start - 2])
        && hex(bytes[start - 1]))
        || (start >= 2 && bytes[start - 2] == b'\\' && bytes[start - 1].is_ascii_alphanumeric());
    if start > 0 && word(bytes[start - 1]) && !escaped_before {
        return false;
    }
    if end < bytes.len() && word(bytes[end]) {
        return false;
    }
    if end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
        return false;
    }
    if start >= 2 && bytes[start - 1] == b'.' && bytes[start - 2].is_ascii_digit() {
        return false;
    }
    true
}

/// Whether this token is exactly the ambiguous space-separated root shape.
fn space_root_needs_context(token: &str) -> bool {
    token.matches(' ').count() == 2 && !token.contains('-') && token.len() == 11
}

/// Whether the word TIN is adjacent enough to disambiguate spaced root groups.
fn tin_word_near(text: &str, start: usize, end: usize) -> bool {
    fn ascii_word(bytes: &[u8], at: usize, word: &[u8]) -> bool {
        at + word.len() <= bytes.len()
            && bytes[at..at + word.len()].eq_ignore_ascii_case(word)
            && (at == 0 || !bytes[at - 1].is_ascii_alphanumeric())
            && (at + word.len() == bytes.len() || !bytes[at + word.len()].is_ascii_alphanumeric())
    }
    let bytes = text.as_bytes();
    let before = start.saturating_sub(8);
    let after = (end + 8).min(bytes.len());
    (before..start).any(|at| ascii_word(bytes, at, b"tin"))
        || (end..after).any(|at| ascii_word(bytes, at, b"tin"))
}

fn is_ipv4_token(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            (1..=3).contains(&part.len())
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
}

/// The masked spelling of one token, or `None` when it is not a TIN. Dashes
/// are either absent everywhere or in the official grouping (`3-3-3`, plus a
/// 3-, 4- or 5-digit branch); partial or differently placed separators are not TINs.
fn masked_tin(token: &str) -> Option<String> {
    if is_ipv4_token(token) {
        return None;
    }
    if token.starts_with('-') || token.ends_with('-') || token.contains("--") {
        return None;
    }
    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
    let separator_count = token.matches(['-', ' ', '.', '/', '_']).count();
    if digits.len() != token.len() - separator_count {
        return None;
    }
    if separator_count == 0 {
        // A solid 13-digit run is likelier a millisecond timestamp or count
        // than a TIN; a four-digit branch must be written with a separator.
        if !matches!(digits.len(), 9 | 12 | 14) {
            return None;
        }
    } else {
        let parts: Vec<&str> = token.split(['-', ' ', '.', '/', '_']).collect();
        if parts.len() < 3
            || parts.len() > 4
            || !parts[..3].iter().all(|part| part.len() == 3)
            || parts[3..].iter().any(|part| !(3..=5).contains(&part.len()))
        {
            return None;
        }
    }
    let root_end = &digits[6..9];
    let branch = &digits[9..];
    if branch.is_empty() {
        Some(format!("***-***-{root_end}"))
    } else {
        Some(format!("***-***-{root_end}-{branch}"))
    }
}

/// The `EBIR_TEST_ENV` SQLCipher key. It is not a taxpayer secret, but it is
/// the database key and must not land in a log a person might attach to a report.
const TEST_MASTER_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn labeled_secret() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)(\b(?:password|passwd|pwd|secret|access_token|refresh_token|id_token|client_secret|api_key|master_key|db_key|encryption_key|app_password|imap_app_password|oauth_access_token|oauth_refresh_token|authorization|recovery_key)\b["']?\s*[=:]\s*)(?:"[^"]*"|'[^']*'|\S+)"#,
        )
        .expect("secret redaction pattern")
    })
}

fn recovery_key_phrase() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(\brecovery[_ ]key\b\s*[=:]?\s*)([0-9A-Z]{4}(?:-[0-9A-Z]{4}){3})")
            .expect("recovery key pattern")
    })
}

fn bearer() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)\bBearer\s+\S+").expect("bearer pattern"))
}

fn google_access_token() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\bya29\.[0-9A-Za-z\-._~+/]+").expect("ya29 pattern"))
}

fn google_refresh_token() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b1//[A-Za-z0-9_\-./]+").expect("refresh token pattern"))
}

fn labeled_hex_key() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)(\b(?:master_key|db_key|encryption_key|bir_key)\b[^0-9A-Fa-f]{0,24})([0-9A-Fa-f]{64})\b",
        )
        .expect("hex key pattern")
    })
}

struct RedactingMake<M> {
    inner: M,
}

impl<M> RedactingMake<M> {
    fn new(inner: M) -> Self {
        Self { inner }
    }
}

struct RedactingWriter<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> RedactingWriter<W> {
    fn emit(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&self.pending);
        let mut redacted = String::new();
        for piece in text.split_inclusive('\n') {
            let (body, newline) = match piece.strip_suffix('\n') {
                Some(body) => (body, true),
                None => (piece, false),
            };
            let body = body.strip_suffix('\r').unwrap_or(body);
            redacted.push_str(&redact_secrets(body));
            if newline {
                redacted.push('\n');
            }
        }
        self.pending.clear();
        self.inner.write_all(redacted.as_bytes())
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.emit()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let _ = self.emit();
        let _ = self.inner.flush();
    }
}

impl<'a, M> MakeWriter<'a> for RedactingMake<M>
where
    M: MakeWriter<'a>,
{
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter {
            inner: self.inner.make_writer(),
            pending: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct RawTinHit {
        line: usize,
        excerpt: String,
    }

    fn strip_string_literals(source: &str) -> String {
        let mut out = source.as_bytes().to_vec();
        let mut in_string = false;
        let mut escaped = false;
        for byte in &mut out {
            let ch = *byte;
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == 92 {
                    escaped = true;
                } else if ch == b'"' {
                    in_string = false;
                }
                *byte = b' ';
            } else if ch == b'"' {
                in_string = true;
                *byte = b' ';
            }
        }
        String::from_utf8(out).expect("masking whole string literals keeps UTF-8 boundaries")
    }

    fn macro_end(source: &str, mut index: usize) -> usize {
        let bytes = source.as_bytes();
        let mut depth = 1usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut in_line_comment = false;
        let mut block_comments = 0usize;
        while index < bytes.len() && depth > 0 {
            let ch = bytes[index];
            let next = bytes.get(index + 1).copied().unwrap_or_default();
            if in_line_comment {
                if ch == b'\n' {
                    in_line_comment = false;
                }
            } else if block_comments > 0 {
                if ch == b'/' && next == b'*' {
                    block_comments += 1;
                    index += 1;
                } else if ch == b'*' && next == b'/' {
                    block_comments -= 1;
                    index += 1;
                }
            } else if in_string {
                if escaped {
                    escaped = false;
                } else if ch == 92 {
                    escaped = true;
                } else if ch == b'"' {
                    in_string = false;
                }
            } else if ch == b'/' && next == b'/' {
                in_line_comment = true;
                index += 1;
            } else if ch == b'/' && next == b'*' {
                block_comments = 1;
                index += 1;
            } else if ch == b'"' {
                in_string = true;
            } else if ch == b'(' {
                depth += 1;
            } else if ch == b')' {
                depth -= 1;
            }
            index += 1;
        }
        index
    }

    fn remove_masked_tin_calls(mut text: String) -> String {
        while let Some(start) = text.find("tin_for_log") {
            let Some(open_relative) = text[start..].find('(') else {
                break;
            };
            let open = start + open_relative;
            let bytes = text.as_bytes();
            let mut depth = 1usize;
            let mut end = open + 1;
            while end < bytes.len() && depth > 0 {
                if bytes[end] == b'(' {
                    depth += 1;
                } else if bytes[end] == b')' {
                    depth -= 1;
                }
                end += 1;
            }
            text.replace_range(start..end, &" ".repeat(end - start));
        }
        text
    }

    fn raw_tin_interpolations(source: &str) -> Vec<RawTinHit> {
        let stripped_source = strip_string_literals(source);
        let macro_start = Regex::new(r"\b(?:info|warn|error|debug|trace)!\s*\(")
            .expect("static tracing macro regex");
        let dot_tin = Regex::new(r"\.tin\b|\btin\s*\(").expect("static dot-TIN regex");
        let tin_word = Regex::new(r"\b[A-Za-z0-9_]*tin\b").expect("static TIN word regex");
        let mut hits = Vec::new();
        for macro_match in macro_start.find_iter(&stripped_source) {
            let end = macro_end(&stripped_source, macro_match.end());
            let body = &stripped_source[macro_match.end()..end.saturating_sub(1)];
            let body = remove_masked_tin_calls(body.to_string());
            let mut found = dot_tin.find(&body).is_some();
            if !found {
                for word in tin_word.find_iter(&body) {
                    let mut after = word.end();
                    while after < body.len() && body.as_bytes()[after].is_ascii_whitespace() {
                        after += 1;
                    }
                    if word.as_str() == "tin"
                        && (after < body.len() && body.as_bytes()[after] == b'=')
                    {
                        continue;
                    }
                    if word.as_str() == "tin_for_log" {
                        continue;
                    }
                    found = true;
                    break;
                }
            }
            if found {
                let line = source[..macro_match.start()].matches('\n').count() + 1;
                hits.push(RawTinHit {
                    line,
                    excerpt: source[macro_match.start()..end.min(source.len())]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                });
            }
        }
        hits
    }

    #[test]
    fn tracing_macros_never_interpolate_a_raw_tin() {
        let planted = concat!(
            "fn demo(draft: Draft, stored_tin: String) {
",
            " in",
            "fo!(\"{}\", draft.tin);
",
            " war",
            "n!(tin = %stored_tin, \"bad\");
",
            " er",
            "ror!(\"{}\", draft.tin());
",
            "}
"
        );
        let positive = raw_tin_interpolations(planted);
        assert_eq!(
            positive.iter().map(|hit| hit.line).collect::<Vec<_>>(),
            vec![2, 3, 4],
            "the planted raw call sites must be found: {positive:?}"
        );

        let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/bir-core has the crates directory as its parent");
        let mut files = Vec::new();
        let mut stack = vec![crates_dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read crates directory") {
                let path = entry.expect("read crates entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(path);
                }
            }
        }
        files.sort();
        let mut failures = Vec::new();
        for path in files {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            for hit in raw_tin_interpolations(&source) {
                failures.push(format!("{}:{}: {}", path.display(), hit.line, hit.excerpt));
            }
        }
        assert!(
            failures.is_empty(),
            "raw TIN interpolation in tracing macros:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn typed_tins_mask_compact_and_grouped_spellings() {
        for (plain, masked) in [
            ("123456789", "***-***-789"),
            ("123456789000", "***-***-789-000"),
            ("1234567890000", "***-***-789-0000"),
            ("12345678900000", "***-***-789-00000"),
            ("123-456-789", "***-***-789"),
            ("123-456-789-0000", "***-***-789-0000"),
            ("123 456 789 00000", "***-***-789-00000"),
            ("1234", "***"),
        ] {
            assert_eq!(tin_for_log(plain), masked, "{plain}");
        }
    }

    #[test]
    fn submission_filenames_and_paths_mask_their_leading_tin() {
        assert_eq!(
            submission_filename_for_log("1234567890000-1601Cv2018-052024#a@example.com#.xml"),
            "***-***-789-0000-1601Cv2018-052024#a@example.com#.xml"
        );
        assert_eq!(
            submission_path_for_log("/IAF/12345678900000-2551Qv2018-122026Q1#a@example.com#.xml"),
            "/IAF/***-***-789-00000-2551Qv2018-122026Q1#a@example.com#.xml"
        );
        assert_eq!(
            submission_filename_for_log("not-a-return.xml"),
            "not-a-return.xml"
        );
    }

    #[test]
    fn desktop_filter_strings_are_the_painted_directives() {
        assert_eq!(
            desktop_filter(false),
            "bir=info,bir_shell=info,bir_print=error,bir_core=info"
        );
        assert_eq!(
            desktop_filter(true),
            "bir=debug,bir_shell=debug,bir_print=debug,bir_core=info"
        );
    }

    #[test]
    fn shell_filter_keeps_core_warnings_and_script_console() {
        let filter = shell_filter(false);
        assert!(filter.contains("bir_core=info"));
        assert!(filter.contains("gpui_shell::script=info"));
        assert!(filter.contains("bir_shell=info"));
        assert!(filter.contains("panic=error"));
        assert!(shell_filter(true).contains("gpui_shell::script=debug"));
        assert!(EnvFilter::try_new(filter).is_ok());
        assert!(EnvFilter::try_new(shell_filter(true)).is_ok());
    }

    #[test]
    fn line_levels_match_the_painted_logs_tab() {
        assert_eq!(
            log_line_level("2026-09-12T00:00:00Z  WARN x: y"),
            LogLineLevel::Warn
        );
        assert_eq!(
            log_line_level("2026-09-12T00:00:00Z ERROR x: y"),
            LogLineLevel::Error
        );
        assert_eq!(
            log_line_level("2026-09-12T00:00:00Z FATAL x: y"),
            LogLineLevel::Error
        );
        assert_eq!(
            log_line_level("2026-09-12T00:00:00Z  INFO x: y"),
            LogLineLevel::Info
        );
        assert_eq!(
            log_line_level("continuation of a wrapped message"),
            LogLineLevel::Other
        );
    }

    #[test]
    fn redaction_strips_credentials_masks_tins_and_keeps_ordinary_text() {
        let tin = "12345678800000";
        let line = format!(
            "draft tin={tin} password=s3cret-token access_token=\"ya29.SUPERSECRET\" Authorization: Bearer abc.def master_key={TEST_MASTER_KEY} refresh 1//0refreshTOKEN recovery key ABCD-EFGH-IJKL-MNOP Failed to access OS keyring"
        );
        let redacted = redact_secrets(&line);
        assert!(!redacted.contains(tin), "{redacted}");
        assert!(redacted.contains("***-***-788-00000"), "{redacted}");
        assert!(
            redacted.contains("Failed to access OS keyring"),
            "{redacted}"
        );
        assert!(!redacted.contains("s3cret-token"), "{redacted}");
        assert!(!redacted.contains("ya29.SUPERSECRET"), "{redacted}");
        assert!(!redacted.contains("abc.def"), "{redacted}");
        assert!(!redacted.contains(TEST_MASTER_KEY), "{redacted}");
        assert!(!redacted.contains("1//0refreshTOKEN"), "{redacted}");
        assert!(!redacted.contains("ABCD-EFGH-IJKL-MNOP"), "{redacted}");
        assert!(redacted.contains("[redacted]"), "{redacted}");
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(redact_secrets(digest), digest);
        assert_eq!(redact_secrets(&line), redact_secrets(&redacted));
    }

    #[test]
    fn redaction_masks_every_tin_shape_but_not_amounts_or_timestamps() {
        let cases = [
            ("123456789", "***-***-789"),
            ("123-456-789", "***-***-789"),
            ("123456789000", "***-***-789-000"),
            ("123-456-789-000", "***-***-789-000"),
            ("12345678900000", "***-***-789-00000"),
            ("123-456-789-00000", "***-***-789-00000"),
        ];
        for (tin, masked) in cases {
            let line = format!("tin={tin} and {tin} end");
            let redacted = redact_secrets(&line);
            assert!(!redacted.contains(tin), "{tin} in {redacted}");
            assert!(redacted.contains(masked), "{tin}: {redacted}");
        }

        for (plain, masked) in [
            (
                "receipt 12345678900000-2551Qv2018-102026.xml",
                "receipt ***-***-789-00000-2551Qv2018-102026.xml",
            ),
            (
                "receipt 123-456-789-00000-2551Qv2018-102026.xml",
                "receipt ***-***-789-00000-2551Qv2018-102026.xml",
            ),
        ] {
            assert_eq!(redact_secrets(plain), masked, "{plain}");
        }

        for ordinary in [
            "amount 123456789.00",
            "amount 1,234,567,890.00",
            "timestamp 2026-11-28T06:07:01Z",
            "timestamp 1764346821",
            "timestamp 1764346821000",
            "duration 1764346821000123456",
            "id 1234567890000",
            "123 456 789 without context",
        ] {
            assert_eq!(redact_secrets(ordinary), ordinary);
        }
    }

    #[test]
    fn redaction_masks_unicode_and_punctuation_tin_spellings_but_not_ipv4() {
        for (plain, masked) in [
            ("123.456.789", "***-***-789"),
            ("123/456/789/00000", "***-***-789-00000"),
            ("123_456_789", "***-***-789"),
            (
                "123\u{00a0}456\u{00a0}789\u{00a0}00000",
                "***-***-789-00000",
            ),
            (
                "123\u{2009}456\u{2009}789\u{2009}00000",
                "***-***-789-00000",
            ),
            (
                "\u{ff11}\u{ff12}\u{ff13}\u{ff14}\u{ff15}\u{ff16}\u{ff17}\u{ff18}\u{ff19}",
                "***-***-789",
            ),
        ] {
            assert_eq!(redact_secrets(plain), masked, "{plain:?}");
        }
        assert_eq!(
            redact_secrets("log é １２３４５６７８９ end"),
            "log é ***-***-789 end"
        );
        for ordinary in ["192.168.1.1", "192.168.100.200"] {
            assert_eq!(redact_secrets(ordinary), ordinary);
        }
    }

    #[test]
    fn redaction_masks_branch_lengths_and_escaped_or_spaced_tins() {
        for (plain, masked) in [
            ("123-456-789-0000", "***-***-789-0000"),
            ("123 456 789 0000", "***-***-789-0000"),
            ("123 456 789 000", "***-***-789-000"),
            ("tin%3D123456789", "tin%3D***-***-789"),
            ("%2F123456789000-2551Q", "%2F***-***-789-000-2551Q"),
            ("\\n123456789000", "\\n***-***-789-000"),
            ("receipt_123456789000", "receipt_***-***-789-000"),
            ("123456789 2551Q", "***-***-789 2551Q"),
            ("TIN 123 456 789", "TIN ***-***-789"),
            ("123-112-150-00000", "***-***-150-00000"),
        ] {
            assert_eq!(redact_secrets(plain), masked, "{plain}");
        }
    }

    #[test]
    fn redaction_masks_email_local_parts_and_keeps_domains() {
        for (plain, masked) in [
            (
                "poll failed for payroll.team@example.com",
                "poll failed for pa•••@example.com",
            ),
            (
                "file 123456789000-1601Cv2018-092026#payroll.team@example.com#.xml",
                "file ***-***-789-000-1601Cv2018-092026#pa•••@example.com#.xml",
            ),
            ("mail a@example.com", "mail a•••@example.com"),
        ] {
            assert_eq!(redact_secrets(plain), masked, "{plain}");
        }
        for ordinary in ["asset icon@2x.png", "asset icon@2x.jpg"] {
            assert_eq!(redact_secrets(ordinary), ordinary);
        }
    }

    #[test]
    fn shell_subscriber_writes_errors_to_the_daily_file_and_drops_debug_noise() {
        let dir = tempfile::tempdir().unwrap();
        let options = InstallOptions {
            developer_mode: false,
            default_filter: shell_filter(false),
        };
        temp_env::with_vars(
            [
                ("BIR_LOG_DIR", Some(dir.path().as_os_str())),
                ("RUST_LOG", None),
            ],
            || {
                with_thread_subscriber(options, || {
                    tracing::warn!(
                        target: "bir_core::email",
                        "IMAP login failed for inbox@example.com password=s3cret-token"
                    );
                    tracing::error!(
                        target: "bir_shell::script",
                        "screen: render failed token=ya29.SUPERSECRET"
                    );
                    tracing::info!(
                        target: "bir_core",
                        "Creating new draft tin=12345678800000"
                    );
                    tracing::debug!(
                        target: "bir_core",
                        "hidden-debug-marker-9f3a"
                    );
                    tracing::info!(
                        target: "unrelated_crate",
                        "hidden-other-info-9f3a"
                    );
                    tracing::error!(
                        target: "unrelated_crate",
                        "hidden-other-error-9f3a"
                    );
                    let previous = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|info| {
                        report_panic(info);
                    }));
                    let panicked = std::panic::catch_unwind(|| {
                        panic!("probe-panic-9f3a password=panic-secret tin=12345678800000");
                    });
                    std::panic::set_hook(previous);
                    assert!(panicked.is_err());
                });
                let path = log_files::today_log_path(dir.path());
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                assert!(text.contains("IMAP login failed"), "{text}");
                assert!(text.contains("screen: render failed"), "{text}");
                assert!(!text.contains("12345678800000"), "{text}");
                assert!(text.contains("***-***-788-00000"), "{text}");
                let panic_line = text
                    .lines()
                    .find(|line| line.contains("probe-panic-9f3a"))
                    .expect("panic line");
                assert!(panic_line.contains("***-***-788-00000"), "{panic_line}");
                assert!(!panic_line.contains("12345678800000"), "{panic_line}");
                assert!(!text.contains("s3cret-token"), "{text}");
                assert!(!text.contains("ya29.SUPERSECRET"), "{text}");
                assert!(!text.contains("panic-secret"), "{text}");
                assert!(!text.contains("hidden-debug-marker-9f3a"), "{text}");
                assert!(!text.contains("hidden-other-info-9f3a"), "{text}");
                // No global level: an error from a crate the filter does not
                // name stays out, matching painted `bir`.
                assert!(!text.contains("hidden-other-error-9f3a"), "{text}");
                assert_eq!(
                    log_line_level(
                        text.lines()
                            .find(|line| line.contains("IMAP login failed"))
                            .unwrap()
                    ),
                    LogLineLevel::Warn
                );
                assert_eq!(
                    log_line_level(
                        text.lines()
                            .find(|line| line.contains("probe-panic-9f3a"))
                            .unwrap()
                    ),
                    LogLineLevel::Error
                );
            },
        );
    }
}
