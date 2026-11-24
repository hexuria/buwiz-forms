//! Read-only Buwiz Books COA client (via the local Bot broker's `/v1/ext/read` door)
//! and Forms-owned fallback account-to-box mapping host functions (ADR 0009).
//!
//! Security & ownership invariants:
//! - Loopback-only: refuses any broker URL whose host is not `127.0.0.1`, `localhost`, or `[::1]`.
//! - Read-only: calls ONLY `app.buwiz.books` / `books_accounts_list` over `/v1/ext/read`.
//! - Never reads Books' SQLite file, never creates Books accounts, and never logs the bearer token.
//! - Mappings live in Forms' own `books_box_mappings` table and apply only to boxes without an
//!   authoritative Books hand-off rule.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use bir_core::db::BooksBoxMapping;
use gpui_shell::{HostArguments, HostError, HostModule, HostResult, HostValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::bir_module::{serialize, with_database, with_reader};

const DEFAULT_BROKER_URL: &str = "http://127.0.0.1:8787";
const BOOKS_PLUGIN_ID: &str = "app.buwiz.books";
const BOOKS_ACCOUNTS_TOOL: &str = "books_accounts_list";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const SETTING_BROKER_URL: &str = "books_broker_url";
const SETTING_BROKER_TOKEN: &str = "books_ext_read_token";

/// One account node returned by Buwiz Books' read-only `books_accounts_list` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BooksAccountNode {
    pub id: String,
    pub code: String,
    pub name: String,
    pub account_type: String,
    pub parent_account_id: Option<String>,
    pub balance: String,
    pub subtree_balance: String,
    pub is_active: bool,
    pub accrual_only: bool,
}

/// Parsed Books Chart of Accounts snapshot returned to the Forms UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BooksCoaResponse {
    pub book_id: String,
    pub book_name: String,
    pub basis: String,
    pub accounts: Vec<BooksAccountNode>,
}

/// Validate and parse a loopback-only HTTP broker URL into `(host_header, SocketAddr)`.
pub fn parse_loopback_broker_addr(raw_url: &str) -> Result<(String, SocketAddr), String> {
    let trimmed = raw_url.trim().trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("http://")
        .ok_or_else(|| "Books broker URL must use http:// on loopback".to_owned())?;
    if rest.contains('/') || rest.contains('?') || rest.contains('#') || rest.contains('@') {
        return Err("Books broker URL must be host:port only (no path or credentials)".to_owned());
    }
    let (host, port_str) = if let Some(after) = rest.strip_prefix("[::1]:") {
        ("[::1]", after)
    } else if rest == "[::1]" {
        ("[::1]", "8787")
    } else if let Some((h, p)) = rest.rsplit_once(':') {
        (h, p)
    } else {
        (rest, "8787")
    };
    let port: u16 = port_str
        .parse()
        .map_err(|_| "Books broker URL has an invalid port".to_owned())?;
    let addr: SocketAddr = match host {
        "127.0.0.1" | "localhost" => SocketAddr::from(([127, 0, 0, 1], port)),
        "[::1]" => SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], port)),
        _ => {
            return Err(
                "Books broker URL must point to loopback (127.0.0.1, localhost, or [::1])"
                    .to_owned(),
            );
        }
    };
    Ok((format!("{host}:{port}"), addr))
}

/// Perform a synchronous loopback HTTP/1.1 POST to `/v1/ext/read` with `Authorization: Bearer`.
pub fn post_ext_read(
    broker_url: &str,
    token: &str,
    plugin: &str,
    tool: &str,
    args: Value,
) -> Result<Value, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("Books read-only grant token is required".to_owned());
    }
    if token.contains('\r') || token.contains('\n') {
        return Err("Books read-only grant token contains invalid characters".to_owned());
    }
    let (host_header, addr) = parse_loopback_broker_addr(broker_url)?;
    let body = serde_json::to_vec(&json!({
        "plugin": plugin,
        "tool": tool,
        "args": args,
    }))
    .map_err(|e| format!("encode request: {e}"))?;

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|e| format!("cannot reach Buwiz Bot broker at {host_header}: {e}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut req = format!(
        "POST /v1/ext/read HTTP/1.1\r\nHost: {host_header}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(&body);
    stream
        .write_all(&req)
        .and_then(|_| stream.flush())
        .map_err(|e| format!("write to broker failed: {e}"))?;

    let mut raw = Vec::new();
    let mut limited = stream.take((MAX_RESPONSE_BYTES + 1) as u64);
    limited
        .read_to_end(&mut raw)
        .map_err(|e| format!("read from broker failed: {e}"))?;
    if raw.len() > MAX_RESPONSE_BYTES {
        return Err("broker response exceeded 2 MiB limit".to_owned());
    }

    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "malformed HTTP response from broker".to_owned())?;
    let head = std::str::from_utf8(&raw[..split])
        .map_err(|_| "non-UTF8 HTTP headers from broker".to_owned())?;
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| "missing HTTP status from broker".to_owned())?;

    let is_chunked = head.lines().skip(1).any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.starts_with("transfer-encoding:") && lower.contains("chunked")
    });
    let payload_bytes = if is_chunked {
        decode_chunked(&raw[split + 4..])?
    } else {
        raw[split + 4..].to_vec()
    };

    let parsed: Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("broker returned invalid JSON (HTTP {status}): {e}"))?;
    if status != 200 || parsed.get("ok").and_then(Value::as_bool) != Some(true) {
        let msg = parsed
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("broker refused /v1/ext/read request");
        return Err(format!("HTTP {status}: {msg}"));
    }
    Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
}

fn decode_chunked(mut input: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let Some(pos) = input.windows(2).position(|w| w == b"\r\n") else {
            return Err("truncated chunked HTTP body".to_owned());
        };
        let size_str = std::str::from_utf8(&input[..pos])
            .map_err(|_| "invalid chunk size".to_owned())?
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        let size =
            usize::from_str_radix(size_str, 16).map_err(|_| "invalid hex chunk size".to_owned())?;
        input = &input[pos + 2..];
        if size == 0 {
            break;
        }
        if input.len() < size + 2 {
            return Err("truncated chunk payload".to_owned());
        }
        out.extend_from_slice(&input[..size]);
        input = &input[size + 2..];
    }
    Ok(out)
}

/// Fetch and normalize Books' chart of accounts via `/v1/ext/read`.
pub fn fetch_books_coa(
    broker_url: &str,
    token: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<BooksCoaResponse, String> {
    let mut args = serde_json::Map::new();
    if let Some(f) = from.filter(|s| !s.trim().is_empty()) {
        args.insert("from".to_owned(), Value::String(f.trim().to_owned()));
    }
    if let Some(t) = to.filter(|s| !s.trim().is_empty()) {
        args.insert("to".to_owned(), Value::String(t.trim().to_owned()));
    }
    let result = post_ext_read(
        broker_url,
        token,
        BOOKS_PLUGIN_ID,
        BOOKS_ACCOUNTS_TOOL,
        Value::Object(args),
    )?;
    parse_books_coa_result(&result)
}

/// Normalize Books' `books_accounts_list` payload (grouped or flat) into a flat list with `parent_account_id`.
pub fn parse_books_coa_result(result: &Value) -> Result<BooksCoaResponse, String> {
    let book_id = result
        .get("book")
        .and_then(|b| b.get("id"))
        .or_else(|| result.get("book_id"))
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_owned();
    let book_name = result
        .get("book")
        .and_then(|b| b.get("name"))
        .or_else(|| result.get("book_name"))
        .and_then(Value::as_str)
        .unwrap_or("Buwiz Books")
        .to_owned();
    let basis = result
        .get("book")
        .and_then(|b| b.get("basis"))
        .or_else(|| result.get("basis"))
        .and_then(Value::as_str)
        .unwrap_or("cash")
        .to_owned();

    let mut accounts = Vec::new();
    if let Some(groups) = result.get("groups").and_then(Value::as_array) {
        for g in groups {
            let group_type = g
                .get("type")
                .or_else(|| g.get("kind"))
                .or_else(|| g.get("group"))
                .and_then(Value::as_str)
                .unwrap_or("other");
            if let Some(rows) = g.get("accounts").and_then(Value::as_array) {
                for row in rows {
                    accounts.push(parse_account_row(row, group_type));
                }
            }
        }
    } else if let Some(rows) = result.get("accounts").and_then(Value::as_array) {
        for row in rows {
            accounts.push(parse_account_row(row, "other"));
        }
    }

    Ok(BooksCoaResponse {
        book_id,
        book_name,
        basis,
        accounts,
    })
}

fn parse_account_row(row: &Value, default_type: &str) -> BooksAccountNode {
    let str_of = |k: &str| {
        row.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let id = str_of("id");
    let code = row
        .get("code")
        .or_else(|| row.get("number"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let name = str_of("name");
    let account_type = row
        .get("type")
        .or_else(|| row.get("account_type"))
        .or_else(|| row.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or(default_type)
        .to_owned();
    let parent_account_id = row
        .get("parent_account_id")
        .or_else(|| row.get("parent_id"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    let balance = row
        .get("balance")
        .and_then(Value::as_str)
        .unwrap_or("0.00")
        .to_owned();
    let subtree_balance = row
        .get("subtree_balance")
        .and_then(Value::as_str)
        .unwrap_or(&balance)
        .to_owned();
    let is_active = row
        .get("is_active")
        .or_else(|| row.get("active"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let accrual_only = row
        .get("accrual_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    BooksAccountNode {
        id: if id.is_empty() { code.clone() } else { id },
        code,
        name,
        account_type,
        parent_account_id,
        balance,
        subtree_balance,
        is_active,
        accrual_only,
    }
}

fn str_arg(args: &HostArguments, idx: usize, name: &str) -> Result<String, HostError> {
    match args.get(idx) {
        Some(HostValue::Str(s)) if !s.trim().is_empty() => Ok(s.trim().to_owned()),
        _ => Err(HostError::new(format!(
            "`{name}` must be a non-empty string"
        ))),
    }
}

fn opt_str_arg(args: &HostArguments, idx: usize) -> Option<String> {
    match args.get(idx) {
        Some(HostValue::Str(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        _ => None,
    }
}

fn books_coa_list(args: &HostArguments) -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let from = opt_str_arg(args, 0);
    let to = opt_str_arg(args, 1);
    let explicit_url = opt_str_arg(args, 2);
    let explicit_token = opt_str_arg(args, 3);

    let (stored_url, stored_token) = with_reader(|db| {
        let u = db.get_setting(SETTING_BROKER_URL).ok().flatten();
        let t = db.get_setting(SETTING_BROKER_TOKEN).ok().flatten();
        Ok((u, t))
    })?;

    let broker_url = explicit_url
        .or(stored_url)
        .or_else(|| {
            std::env::var("BUWIZ_BROKER_URL")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_BROKER_URL.to_owned());
    let token = explicit_token
        .or(stored_token)
        .or_else(|| std::env::var("BUWIZ_BOOKS_EXT_TOKEN").ok().filter(|s| !s.trim().is_empty()))
        .ok_or_else(|| {
            HostError::new(
                "Books is not connected yet. Grant read-only access in Buwiz Bot and save the token.",
            )
        })?;

    let coa = fetch_books_coa(&broker_url, &token, from.as_deref(), to.as_deref())
        .map_err(HostError::new)?;
    serialize(&coa)
}

fn books_box_mappings_list(args: &HostArguments) -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let tin = str_arg(args, 0, "tin")?;
    let form_type = str_arg(args, 1, "form_type")?;
    let rows = with_reader(|db| {
        db.list_books_box_mappings(&tin, &form_type)
            .map_err(|e| HostError::new(e.to_string()))
    })?;
    serialize(&rows)
}

fn books_box_mapping_save(args: &HostArguments) -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let entries = match args.get(0) {
        Some(HostValue::Object(entries)) => entries,
        _ => return Err(HostError::new("mapping object is required")),
    };
    let field = |k: &str| entries.iter().find(|(name, _)| name == k).map(|(_, v)| v);
    let req_str = |k: &str| -> Result<String, HostError> {
        match field(k) {
            Some(HostValue::Str(s)) if !s.trim().is_empty() => Ok(s.trim().to_owned()),
            _ => Err(HostError::new(format!("mapping.`{k}` is required"))),
        }
    };
    let include_subtree = match field("include_subtree") {
        Some(HostValue::Bool(b)) => *b,
        _ => true,
    };
    let mapping = BooksBoxMapping {
        tin: req_str("tin")?,
        form_type: req_str("form_type")?,
        box_key: req_str("box_key")?,
        book_id: req_str("book_id")?,
        account_id: req_str("account_id")?,
        account_code: req_str("account_code")?,
        account_name: req_str("account_name")?,
        account_type: req_str("account_type")?,
        include_subtree,
        updated_at: String::new(),
    };
    with_database(|db| {
        db.upsert_books_box_mapping(&mapping)
            .map_err(|e| HostError::new(e.to_string()))
    })?;
    Ok(HostValue::Null)
}

fn books_box_mapping_delete(args: &HostArguments) -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let tin = str_arg(args, 0, "tin")?;
    let form_type = str_arg(args, 1, "form_type")?;
    let box_key = str_arg(args, 2, "box_key")?;
    let removed = with_database(|db| {
        db.delete_books_box_mapping(&tin, &form_type, &box_key)
            .map_err(|e| HostError::new(e.to_string()))
    })?;
    Ok(HostValue::Bool(removed))
}

pub fn register(module: HostModule) -> HostModule {
    module
        .function("books_coa_list", books_coa_list)
        .function("books_box_mappings_list", books_box_mappings_list)
        .function("books_box_mapping_save", books_box_mapping_save)
        .function("books_box_mapping_delete", books_box_mapping_delete)
}

pub const DECLARATIONS: &str = r#"
export interface BooksAccountNode {
    id: string;
    code: string;
    name: string;
    account_type: string;
    parent_account_id: string | null;
    balance: string;
    subtree_balance: string;
    is_active: boolean;
    accrual_only: boolean;
}
export interface BooksCoaResponse {
    book_id: string;
    book_name: string;
    basis: string;
    accounts: BooksAccountNode[];
}
export interface BooksBoxMapping {
    tin: string;
    form_type: string;
    box_key: string;
    book_id: string;
    account_id: string;
    account_code: string;
    account_name: string;
    account_type: string;
    include_subtree: boolean;
    updated_at?: string;
}
/** Read-only Books chart of accounts via the local Buwiz Bot broker (/v1/ext/read). */
export function books_coa_list(from?: string, to?: string, broker_url?: string, token?: string): BooksCoaResponse;
/** Saved Forms-side fallback Books account mappings for a taxpayer and form type (ADR 0009). */
export function books_box_mappings_list(tin: string, form_type: string): BooksBoxMapping[];
/** Save or update a fallback Books account mapping for one form box. */
export function books_box_mapping_save(mapping: BooksBoxMapping): null;
/** Remove a fallback Books account mapping for one form box. */
export function books_box_mapping_delete(tin: string, form_type: string, box_key: string): boolean;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn rejects_non_loopback_broker_urls() {
        assert!(parse_loopback_broker_addr("http://127.0.0.1:8821").is_ok());
        assert!(parse_loopback_broker_addr("http://localhost:8787").is_ok());
        assert!(parse_loopback_broker_addr("http://[::1]:8821").is_ok());

        assert!(parse_loopback_broker_addr("https://buwiz.com").is_err());
        assert!(parse_loopback_broker_addr("http://192.168.1.5:8787").is_err());
        assert!(parse_loopback_broker_addr("http://127.0.0.1:8787/extra").is_err());
        assert!(parse_loopback_broker_addr("http://user:pass@127.0.0.1:8787").is_err());
    }

    #[test]
    fn fetches_nested_coa_over_loopback_ext_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = sock.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.starts_with("POST /v1/ext/read HTTP/1.1\r\n"), "{req}");
            assert!(
                req.contains("Authorization: Bearer ext_test_secret_123\r\n"),
                "{req}"
            );
            assert!(req.contains("\"plugin\":\"app.buwiz.books\""), "{req}");
            assert!(req.contains("\"tool\":\"books_accounts_list\""), "{req}");

            let body = json!({
                "ok": true,
                "plugin": "app.buwiz.books",
                "tool": "books_accounts_list",
                "result": {
                    "book": { "id": "book_1", "name": "Demo Co", "basis": "cash" },
                    "groups": [{
                        "type": "income",
                        "accounts": [
                            {
                                "id": "acc_41000",
                                "code": "41000",
                                "name": "Sales Revenue",
                                "parent_account_id": null,
                                "balance": "100000.00",
                                "subtree_balance": "150000.00",
                                "is_active": true,
                                "accrual_only": false
                            },
                            {
                                "id": "acc_41010",
                                "code": "41010",
                                "name": "Sales to Government",
                                "parent_account_id": "acc_41000",
                                "balance": "50000.00",
                                "subtree_balance": "50000.00",
                                "is_active": true,
                                "accrual_only": false
                            }
                        ]
                    }]
                }
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).unwrap();
        });

        let coa = fetch_books_coa(
            &format!("http://127.0.0.1:{port}"),
            "ext_test_secret_123",
            Some("2026-01-01"),
            Some("2026-03-31"),
        )
        .unwrap();
        server.join().unwrap();

        assert_eq!(coa.book_id, "book_1");
        assert_eq!(coa.basis, "cash");
        assert_eq!(coa.accounts.len(), 2);
        assert_eq!(coa.accounts[0].code, "41000");
        assert_eq!(coa.accounts[0].subtree_balance, "150000.00");
        assert_eq!(
            coa.accounts[1].parent_account_id.as_deref(),
            Some("acc_41000")
        );
    }
}
