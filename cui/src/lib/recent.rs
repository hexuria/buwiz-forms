//! Recent returns (port of lib/recent.ts): what an entry is made of. Pure.

use serde::{Deserialize, Serialize};

/// What identifies a return: the taxpayer, the form, the tax year and the period, and the stored
/// return's exact key when it has one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentEntry {
    pub tin: String,
    pub code: String,
    pub year: i64,
    pub period: i64,
    /// The stored return's own key (returnKey); absent for a return that has no stored key.
    #[serde(rename = "returnKey", skip_serializing_if = "Option::is_none")]
    pub return_key: Option<serde_json::Value>,
    /// The entry's own id (always text).
    pub key: String,
    #[serde(rename = "newKey", skip_serializing_if = "Option::is_none")]
    pub new_key: Option<serde_json::Value>,
    #[serde(rename = "newKeyFrom", skip_serializing_if = "Option::is_none")]
    pub new_key_from: Option<serde_json::Value>,
}

impl Default for RecentEntry {
    fn default() -> Self {
        Self {
            tin: String::new(),
            code: String::new(),
            year: 0,
            period: 0,
            return_key: None,
            key: String::new(),
            new_key: None,
            new_key_from: None,
        }
    }
}

/// How many returns Recent keeps.
pub const RECENT_MAX: usize = 4;

fn key_of(
    tin: &str,
    code: &str,
    year: i64,
    period: i64,
    return_key: &Option<serde_json::Value>,
) -> String {
    let exact = match return_key {
        Some(k) if !k.is_null() => format!("|{k}"),
        _ => String::new(),
    };
    format!("{tin}|{code}|{year}|{period}{exact}")
}

/// A Recent entry for a return the app opened. `new_key`/`new_key_from` are dropped (they started a
/// new return once; reopening from Recent opens what is stored for its period).
pub fn recent_entry(
    tin: &str,
    code: &str,
    year: i64,
    period: i64,
    return_key: Option<serde_json::Value>,
) -> RecentEntry {
    let mut e = RecentEntry {
        tin: tin.to_owned(),
        code: code.to_owned(),
        year,
        period,
        return_key: return_key.clone(),
        ..Default::default()
    };
    e.key = key_of(tin, code, year, period, &return_key);
    e
}

/// Two entries for one return: the same taxpayer, form and period, and exact keys that do not
/// disagree. An entry without an exact key opens what is stored for its period, which is the return an
/// exact key names.
pub fn same_return(a: &RecentEntry, b: &RecentEntry) -> bool {
    if a.tin != b.tin || a.code != b.code || a.year != b.year || a.period != b.period {
        return false;
    }
    match (&a.return_key, &b.return_key) {
        (Some(x), Some(y)) => x == y,
        _ => true,
    }
}

/// Recent as stored. An entry without a text id is rebuilt; a return listed twice keeps its first
/// place (and its exact key); anything that is not a list is no list.
pub fn load_recent(stored: &serde_json::Value) -> Vec<RecentEntry> {
    let Some(items) = stored.as_array() else {
        return Vec::new();
    };
    let mut out: Vec<RecentEntry> = Vec::new();
    for raw in items {
        let Ok(mut r) = serde_json::from_value::<RecentEntry>(raw.clone()) else {
            continue;
        };
        if r.key.is_empty() {
            r = recent_entry(&r.tin, &r.code, r.year, r.period, r.return_key.clone());
        }
        match out.iter().position(|x| same_return(x, &r)) {
            None => out.push(r),
            Some(at) => {
                if out[at].return_key.is_none() && r.return_key.is_some() {
                    out[at] = r;
                }
            }
        }
    }
    out
}

/// Opening a return: one Recent already lists keeps its place. A return not listed yet joins at the
/// top, and the oldest entry past four goes.
pub fn add_recent(list: &[RecentEntry], entry: RecentEntry) -> Vec<RecentEntry> {
    if let Some(at) = list.iter().position(|r| same_return(r, &entry)) {
        let mut next = list.to_vec();
        if next[at].return_key.is_none() && entry.return_key.is_some() {
            next[at] = entry;
        }
        return next;
    }
    let mut next = Vec::with_capacity(RECENT_MAX);
    next.push(entry);
    next.extend(list.iter().cloned());
    next.truncate(RECENT_MAX);
    next
}

/// A return was changed and saved: it moves to the top (or joins there).
pub fn bump_recent(list: &[RecentEntry], entry: RecentEntry) -> Vec<RecentEntry> {
    let listed = list.iter().find(|r| same_return(r, &entry)).cloned();
    // The more exact of the two stays.
    let entry = match &listed {
        Some(l) if l.return_key.is_some() || entry.return_key.is_none() => l.clone(),
        _ => entry,
    };
    let mut next = vec![entry.clone()];
    next.extend(
        list.iter()
            .filter(|r| !same_return(r, &entry) && r.key != entry.key)
            .cloned(),
    );
    next.truncate(RECENT_MAX);
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_and_identity() {
        let a = recent_entry("00012345300000", "1601C", 2026, 9, None);
        assert_eq!(a.key, "00012345300000|1601C|2026|9");
        let b = recent_entry("00012345300000", "1601C", 2026, 9, Some(json!(1201)));
        assert_eq!(b.key, "00012345300000|1601C|2026|9|1201");
        assert!(same_return(&a, &b));
        let c = recent_entry("00012345300000", "1601C", 2026, 9, Some(json!(1202)));
        assert!(!same_return(&b, &c));
    }

    #[test]
    fn add_and_bump() {
        let mut list = Vec::new();
        for i in 0..5 {
            list = add_recent(&list, recent_entry("T", "F", 2026, i, None));
        }
        assert_eq!(list.len(), RECENT_MAX);
        assert_eq!(list[0].period, 4);
        let bumped = bump_recent(&list, recent_entry("T", "F", 2026, 2, None));
        assert_eq!(bumped[0].period, 2);
        assert_eq!(bumped.len(), RECENT_MAX);
    }

    #[test]
    fn load_rebuilds_and_dedupes() {
        let stored = json!([
            {"tin":"T","code":"F","year":2026,"period":1},
            {"tin":"T","code":"F","year":2026,"period":1,"key":"T|F|2026|1|9","returnKey":9},
            "junk"
        ]);
        let list = load_recent(&stored);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].key, "T|F|2026|1|9");
        assert!(load_recent(&json!({})).is_empty());
    }
}
