//! Opaque taxpayer tickets (`tkt_...`) and TIN scrubbing at the MCP boundary.
//!
//! Raw TINs never cross the MCP wire:
//! - Outbound: every TIN field (`tin`, `tin_root`, `tin_display`, `branch_tin`, `root`)
//!   and any TIN-shaped digit run in free text or errors is replaced by an opaque
//!   Crockford base32 ticket (`tkt_<26 chars>`, 130 random bits) for known taxpayers,
//!   or `[TIN]` for unknown digit runs.
//! - Inbound: `bir_form_open` accepts only a ticket previously issued in this session
//!   and expands it back to the full TIN for the local BIR host call.

use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const TIN_KEYS: [&str; 5] = ["tin", "tin_root", "tin_display", "branch_tin", "root"];

/// Whether `text` matches the ticket grammar `^tkt_[0-9A-HJKMNP-TV-Z]{26}$`.
pub fn is_ticket(text: &str) -> bool {
    let Some(body) = text.strip_prefix("tkt_") else {
        return false;
    };
    body.len() == 26 && body.bytes().all(|b| CROCKFORD.contains(&b))
}

fn generate_ticket() -> String {
    let mut bytes = [0u8; 26];
    rand::fill(&mut bytes);
    let mut out = String::with_capacity(30);
    out.push_str("tkt_");
    for b in bytes {
        out.push(CROCKFORD[(b & 0x1f) as usize] as char);
    }
    out
}

fn digits_only(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

fn grouped_tin_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[0-9]{3}[-. ][0-9]{3}[-. ][0-9]{3}(?:[-. ][0-9]{3,5})?\b").expect("regex")
    })
}

fn bare_tin_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[0-9]{9,14}\b").expect("regex"))
}

/// Skip monetary amounts (peso sign before, decimals before/after) so
/// 9+ digit amounts like `1200000.00` or `₱1,000,000` are not scrubbed.
fn amount_at(text: &str, start: usize, end: usize) -> bool {
    let mut before = text[..start].chars().rev();
    let (one, two) = (before.next(), before.next());
    let decimals_before = one == Some('.') && two.is_some_and(|c| c.is_ascii_digit());
    let mut after = text[end..].chars();
    let separated = matches!(after.next(), Some('.' | ','));
    let digits = after.clone().take_while(char::is_ascii_digit).count();
    let decimals_after = separated && (1..=2).contains(&digits);
    one == Some('₱') || decimals_before || decimals_after
}

fn scrub_runs(text: &str, re: &Regex, word: &mut impl FnMut(&str) -> String) -> String {
    let (mut out, mut kept) = (String::new(), 0);
    for run in re.find_iter(text) {
        if amount_at(text, run.start(), run.end()) {
            continue;
        }
        out.push_str(&text[kept..run.start()]);
        out.push_str(&word(run.as_str()));
        kept = run.end();
    }
    out.push_str(&text[kept..]);
    out
}

/// Session-scoped bidirectional map between BIR taxpayer TINs and opaque tickets.
#[derive(Debug, Default)]
pub struct TicketBook {
    /// Normalized TIN digits (full and 9-digit root) -> ticket.
    by_digits: HashMap<String, String>,
    /// Ticket -> full TIN string as returned by BIR.
    by_ticket: HashMap<String, String>,
}

impl TicketBook {
    pub fn clear(&mut self) {
        self.by_digits.clear();
        self.by_ticket.clear();
    }

    /// Issue or reuse a ticket for `raw_tin`.
    pub fn issue(&mut self, raw_tin: &str) -> String {
        let digits = digits_only(raw_tin);
        if let Some(existing) = self.by_digits.get(&digits) {
            return existing.clone();
        }
        let ticket = generate_ticket();
        self.by_digits.insert(digits.clone(), ticket.clone());
        if digits.len() > 9 {
            self.by_digits
                .entry(digits[..9].to_string())
                .or_insert_with(|| ticket.clone());
        }
        self.by_ticket.insert(ticket.clone(), raw_tin.to_string());
        ticket
    }

    /// Resolve a ticket back to the full TIN for the local BIR host call.
    pub fn resolve(&self, ticket: &str) -> Option<&str> {
        self.by_ticket.get(ticket).map(String::as_str)
    }

    /// Replace TINs in free text: known taxpayers become their ticket, others `[TIN]`.
    pub fn scrub_text(&self, text: &str) -> String {
        let map_tin = |matched: &str| {
            let digits = digits_only(matched);
            if let Some(ticket) = self.by_digits.get(&digits) {
                return ticket.clone();
            }
            if digits.len() >= 9
                && let Some(ticket) = self.by_digits.get(&digits[..9])
            {
                return ticket.clone();
            }
            "[TIN]".to_string()
        };
        let mut word = map_tin;
        let grouped = scrub_runs(text, grouped_tin_re(), &mut word);
        scrub_runs(&grouped, bare_tin_re(), &mut word)
    }

    fn collect_tin_keys(&mut self, value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    if TIN_KEYS.contains(&key.as_str())
                        && let Value::String(text) = val
                    {
                        let digits = digits_only(text);
                        if digits.len() >= 9 && !is_ticket(text) {
                            self.issue(text);
                            continue;
                        }
                    }
                    self.collect_tin_keys(val);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.collect_tin_keys(item);
                }
            }
            _ => {}
        }
    }

    /// Recursively scrub a JSON value before returning it over MCP.
    pub fn scrub_value(&mut self, value: &mut Value) {
        self.collect_tin_keys(value);
        self.scrub_value_inner(value);
    }

    fn scrub_value_inner(&mut self, value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    if TIN_KEYS.contains(&key.as_str())
                        && let Value::String(text) = val
                    {
                        let digits = digits_only(text);
                        if digits.len() >= 9 && !is_ticket(text) {
                            *text = self.issue(text);
                            continue;
                        }
                    }
                    self.scrub_value_inner(val);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.scrub_value_inner(item);
                }
            }
            Value::String(text) if !is_ticket(text) => {
                let scrubbed = self.scrub_text(text);
                if scrubbed != *text {
                    *text = scrubbed;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ticket_grammar_and_roundtrip() {
        let mut book = TicketBook::default();
        let t1 = book.issue("11111111400000");
        assert!(is_ticket(&t1), "bad ticket {t1}");
        assert_eq!(book.issue("111-111-114-00000"), t1);
        assert_eq!(book.resolve(&t1), Some("11111111400000"));
        assert!(!is_ticket("11111111400000"));
        assert!(!is_ticket("tkt_short"));
        assert!(!is_ticket(&t1.to_lowercase()));
    }

    #[test]
    fn scrubs_known_unknown_and_preserves_amounts() {
        let mut book = TicketBook::default();
        let mut payload = json!({
            "profiles": [{"tin": "11111111400000", "name": "Test Taxpayer"}],
            "note": "TIN 111-111-114-00000 and stray 999-888-777-000",
            "amount": "1200000.00",
            "peso": "₱100000000"
        });
        book.scrub_value(&mut payload);
        let tkt = payload["profiles"][0]["tin"].as_str().unwrap().to_string();
        assert!(is_ticket(&tkt));
        assert_eq!(
            payload["note"].as_str().unwrap(),
            format!("TIN {tkt} and stray [TIN]")
        );
        assert_eq!(payload["amount"], "1200000.00");
        assert_eq!(payload["peso"], "₱100000000");
    }
}
