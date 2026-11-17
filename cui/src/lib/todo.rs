//! The form page's to-do list (port of `nsh/lib/todo.ts`): the rows the pill's ‹ and › visit, in
//! form order, each once with the strongest reason it needs the user. Pure: the page hands in what
//! it knows and gets rows back.
//!
//! - message: the row holds a validation message.
//! - required: a required box (field flag r) is empty, not computed and not shut.
//! - stop: one of the row's boxes is a stop (effectiveStops), not all of its stop boxes hold a
//!   value, and the user has not stepped past it on this visit (passed).

use std::collections::{HashMap, HashSet};

use crate::form::schema::{SchemaField, SchemaItem, SchemaPart};

/// Why a row needs the user. Strongest first: message, then required, then stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TodoReason {
    Message,
    Required,
    Stop,
}

/// A row of the to-do list: its id ("part:item"), its strongest reason, and the stop boxes it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoRow {
    pub id: String,
    pub reason: TodoReason,
    pub stops: Vec<String>,
}

/// What todo_rows is given: the schema's parts and what the page knows about the draft.
pub struct TodoRowsOptions<'a> {
    pub parts: &'a [SchemaPart],
    /// Row id to its validation message.
    pub messages: &'a HashMap<String, String>,
    /// Box key to the value the draft holds.
    pub values: &'a HashMap<String, String>,
    /// The keys of the boxes the pill stops on.
    pub stops: &'a HashSet<String>,
    /// Row ids the user has stepped past on this visit.
    pub passed: &'a HashSet<String>,
    /// Whether a box is shut (locked, off, read-only).
    pub blocked: &'a dyn Fn(&str) -> bool,
}

/// The boxes the pill stops on: the automatic stops less the ones switched off, plus the ones
/// switched on (I-2). Absent fields count as empty.
pub fn effective_stops(
    auto_stops: &[String],
    stop_edits: &HashMap<String, bool>,
) -> HashSet<String> {
    let mut out = HashSet::new();
    for k in auto_stops {
        if stop_edits.get(k) != Some(&false) {
            out.insert(k.clone());
        }
    }
    for (k, on) in stop_edits {
        if *on {
            out.insert(k.clone());
        }
    }
    out
}

/// Where Stop here should send box k: back to automatic (None) when that already says so.
pub fn stop_call(k: &str, on: bool, auto_stops: &[String]) -> Option<bool> {
    let auto = auto_stops.iter().any(|a| a == k);
    if on == auto { None } else { Some(on) }
}

/// The keys an item's boxes write (a schedule's every cell, a pooled line's pool).
pub fn item_keys(it: &SchemaItem) -> Vec<String> {
    if it.rows.is_some() {
        return it.keys.iter().flat_map(|row| row.iter().cloned()).collect();
    }
    it.f.iter()
        .flat_map(|f| match &f.pool {
            Some(pool) => pool.keys.clone(),
            None => vec![f.k.clone()],
        })
        .collect()
}

fn ticked(v: &str) -> bool {
    matches!(v, "true" | "1" | "Y")
}

fn holds(v: Option<&String>) -> bool {
    v.is_some_and(|s| !s.trim().is_empty())
}

/// Whether box k of item it holds an answer: a tick group by any tick, any other box by any text.
fn answered(it: &SchemaItem, k: &str, values: &HashMap<String, String>) -> bool {
    let f = it.f.iter().find(|x| x.k == k);
    if let Some(f) = f {
        if f.t == "radio" || f.t == "chk" {
            let group: Vec<&SchemaField> = if f.t == "radio" {
                it.f.iter()
                    .filter(|x| {
                        x.t == "radio" && (f.grp.is_none() || x.grp == f.grp || x.grp.is_none())
                    })
                    .collect()
            } else {
                vec![f]
            };
            // Match the TS: radio group = same grp when the field has one.
            let group: Vec<&SchemaField> = if f.t == "radio" && f.grp.is_some() {
                it.f.iter()
                    .filter(|x| x.t == "radio" && x.grp == f.grp)
                    .collect()
            } else {
                group
            };
            return group
                .iter()
                .any(|x| ticked(values.get(&x.k).map(String::as_str).unwrap_or("")));
        }
    }
    holds(values.get(k))
}

/// Whether a required box of a plain item is still empty.
fn required_blank(
    it: &SchemaItem,
    values: &HashMap<String, String>,
    blocked: &dyn Fn(&str) -> bool,
) -> bool {
    let fs: Vec<&SchemaField> =
        it.f.iter()
            .filter(|f| f.r == Some(true) && f.c != Some(true) && !blocked(&f.k))
            .collect();
    if fs.is_empty() {
        return false;
    }
    let radios: Vec<&&SchemaField> = fs
        .iter()
        .filter(|f| f.t == "radio" || f.t == "chk")
        .collect();
    // A tick group is one item: answered by any tick in it, shut ones too.
    if !radios.is_empty() {
        let all_ticks: Vec<&SchemaField> =
            it.f.iter()
                .filter(|f| f.t == "radio" || f.t == "chk")
                .collect();
        if !all_ticks
            .iter()
            .any(|f| ticked(values.get(&f.k).map(String::as_str).unwrap_or("")))
        {
            return true;
        }
    }
    fs.iter()
        .any(|f| f.t != "radio" && f.t != "chk" && !holds(values.get(&f.k)))
}

/// The rows needing the user, in form order (part, then item), each with its strongest reason.
pub fn todo_rows(opts: &TodoRowsOptions) -> Vec<TodoRow> {
    let mut out = Vec::new();
    for (pi, p) in opts.parts.iter().enumerate() {
        for (ii, it) in p.items.iter().enumerate() {
            let id = format!("{pi}:{ii}");
            let mut reasons = Vec::new();
            if opts
                .messages
                .get(&id)
                .map(|m| !m.is_empty())
                .unwrap_or(false)
            {
                reasons.push(TodoReason::Message);
            }
            if it.rows.is_none() && required_blank(it, opts.values, opts.blocked) {
                reasons.push(TodoReason::Required);
            }
            let own: Vec<String> = if !opts.stops.is_empty() {
                item_keys(it)
                    .into_iter()
                    .filter(|k| opts.stops.contains(k))
                    .collect()
            } else {
                Vec::new()
            };
            if !own.is_empty()
                && !opts.passed.contains(&id)
                && !own.iter().all(|k| answered(it, k, opts.values))
            {
                reasons.push(TodoReason::Stop);
            }
            if reasons.is_empty() {
                continue;
            }
            reasons.sort();
            out.push(TodoRow {
                id,
                reason: reasons[0],
                stops: own,
            });
        }
    }
    out
}

/// "p:i" in form order: negative when a comes first.
pub fn compare_rows(a: &str, b: &str) -> i64 {
    let parse = |s: &str| -> (i64, i64) {
        let mut it = s.split(':');
        (
            it.next().and_then(|x| x.parse().ok()).unwrap_or(0),
            it.next().and_then(|x| x.parse().ok()).unwrap_or(0),
        )
    };
    let (ap, ai) = parse(a);
    let (bp, bi) = parse(b);
    {
        let c = ap - bp;
        if c != 0 { c } else { ai - bi }
    }
}

/// Where the previous row (dir -1) or the next (dir 1) is, in ids (the to-do rows, in order),
/// from row at. With no row at, it is from the start of part, or from the top when from_top.
/// None at either end.
pub fn next_row(
    ids: &[String],
    dir: i32,
    at: Option<&str>,
    part: usize,
    from_top: bool,
) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    if let Some(here) = at {
        return if dir > 0 {
            ids.iter().find(|id| compare_rows(id, here) > 0).cloned()
        } else {
            ids.iter()
                .rev()
                .find(|id| compare_rows(id, here) < 0)
                .cloned()
        };
    }
    if from_top {
        return if dir > 0 { ids.first().cloned() } else { None };
    }
    let part_of = |id: &str| -> i64 {
        id.split(':')
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(0)
    };
    if dir > 0 {
        ids.iter().find(|id| part_of(id) >= part as i64).cloned()
    } else {
        ids.iter()
            .rev()
            .find(|id| part_of(id) < part as i64)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::schema::{SchemaColumn, SchemaField, SchemaItem, SchemaPart};

    fn field(k: &str, t: &str) -> SchemaField {
        SchemaField {
            k: k.to_owned(),
            l: String::new(),
            t: t.to_owned(),
            c: None,
            r: None,
            m: None,
            o: None,
            a: None,
            w: None,
            grp: None,
            rg: None,
            pool: None,
        }
    }
    fn radio(k: &str, grp: &str, req: bool) -> SchemaField {
        SchemaField {
            k: k.to_owned(),
            l: String::new(),
            t: "radio".to_owned(),
            c: None,
            r: if req { Some(true) } else { None },
            m: None,
            o: None,
            a: None,
            w: None,
            grp: Some(grp.to_owned()),
            rg: None,
            pool: None,
        }
    }
    fn item(f: Vec<SchemaField>) -> SchemaItem {
        SchemaItem {
            no: String::new(),
            l: String::new(),
            p: 0.0,
            f,
            rows: None,
            cols: vec![],
            keys: vec![],
            note: None,
            one_date: None,
            from: None,
            row_labels: None,
            numbered: None,
            typed_tin: None,
            calendar: None,
            start: None,
        }
    }
    fn table() -> SchemaItem {
        SchemaItem {
            no: String::new(),
            l: "Table".to_owned(),
            p: 0.0,
            f: vec![],
            rows: Some(2.0),
            cols: vec![SchemaColumn {
                l: "x".to_owned(),
                t: "text".to_owned(),
                c: None,
                w: None,
                o: None,
            }],
            keys: vec![vec!["r0".to_owned()], vec!["r1".to_owned()]],
            note: None,
            one_date: None,
            from: None,
            row_labels: None,
            numbered: None,
            typed_tin: None,
            calendar: None,
            start: None,
        }
    }
    fn parts() -> Vec<SchemaPart> {
        vec![
            SchemaPart {
                n: String::new(),
                items: vec![
                    item(vec![SchemaField {
                        r: Some(true),
                        ..field("y", "text")
                    }]),
                    item(vec![radio("w1", "w", true), radio("w2", "w", true)]),
                    item(vec![SchemaField {
                        r: Some(true),
                        ..field("c", "chk")
                    }]),
                    item(vec![SchemaField {
                        r: Some(true),
                        c: Some(true),
                        ..field("t", "money")
                    }]),
                    item(vec![SchemaField {
                        r: Some(true),
                        ..field("l", "text")
                    }]),
                ],
            },
            SchemaPart {
                n: String::new(),
                items: vec![
                    item(vec![field("a", "money")]),
                    item(vec![radio("k1", "k", false), radio("k2", "k", false)]),
                    table(),
                ],
            },
        ]
    }
    fn blocked(k: &str) -> bool {
        k == "l"
    }
    fn answered() -> HashMap<String, String> {
        HashMap::from([
            ("y".to_owned(), "2026".to_owned()),
            ("w2".to_owned(), "true".to_owned()),
            ("c".to_owned(), "true".to_owned()),
        ])
    }
    fn ids(rows: &[TodoRow]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                format!(
                    "{}:{}",
                    r.id,
                    match r.reason {
                        TodoReason::Message => "message",
                        TodoReason::Required => "required",
                        TodoReason::Stop => "stop",
                    }
                )
            })
            .collect()
    }
    fn stops() -> HashSet<String> {
        ["y", "a", "k1", "r1"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn effective_stops_math() {
        let edits: HashMap<String, bool> =
            HashMap::from([("b".to_owned(), false), ("d".to_owned(), true)]);
        let mut got: Vec<String> = effective_stops(&["a".into(), "b".into(), "c".into()], &edits)
            .into_iter()
            .collect();
        got.sort();
        assert_eq!(got, vec!["a", "c", "d"]);
        assert!(effective_stops(&[], &HashMap::new()).is_empty());
        let edits: HashMap<String, bool> = HashMap::from([("x".to_owned(), true)]);
        assert_eq!(
            effective_stops(&[], &edits),
            HashSet::from(["x".to_owned()])
        );
        assert_eq!(
            effective_stops(&["a".to_owned()], &HashMap::new()),
            HashSet::from(["a".to_owned()])
        );
    }

    #[test]
    fn stop_call_math() {
        let auto = vec!["a".to_owned()];
        assert_eq!(stop_call("a", false, &auto), Some(false));
        assert_eq!(stop_call("a", true, &auto), None);
        assert_eq!(stop_call("z", true, &auto), Some(true));
        assert_eq!(stop_call("z", false, &auto), None);
        assert_eq!(stop_call("z", true, &[]), Some(true));
    }

    #[test]
    fn item_keys_math() {
        let it = item(vec![
            field("a", "text"),
            SchemaField {
                pool: Some(crate::form::schema::PoolKeys {
                    keys: vec!["p1".into(), "p2".into()],
                }),
                ..field("p1", "text")
            },
        ]);
        assert_eq!(item_keys(&it), vec!["a", "p1", "p2"]);
        assert_eq!(item_keys(&table()), vec!["r0", "r1"]);
        assert!(item_keys(&item(vec![])).is_empty());
    }

    #[test]
    fn todo_rows_cases() {
        let p = parts();
        // blank form: required boxes only; a tick group once; never computed or shut
        assert_eq!(
            ids(&todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &HashMap::new(),
                values: &HashMap::new(),
                stops: &HashSet::new(),
                passed: &HashSet::new(),
                blocked: &blocked,
            })),
            vec!["0:0:required", "0:1:required", "0:2:required"]
        );
        // answered boxes leave the list
        assert!(
            todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &HashMap::new(),
                values: &answered(),
                stops: &HashSet::new(),
                passed: &HashSet::new(),
                blocked: &blocked,
            })
            .is_empty()
        );
        // message beats required beats stop; one row once
        let msgs = HashMap::from([
            ("0:0".to_owned(), "Enter the year".to_owned()),
            ("1:2".to_owned(), "Row 2 is short".to_owned()),
        ]);
        assert_eq!(
            ids(&todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &msgs,
                values: &HashMap::new(),
                stops: &stops(),
                passed: &HashSet::new(),
                blocked: &blocked,
            })),
            vec![
                "0:0:message",
                "0:1:required",
                "0:2:required",
                "1:0:stop",
                "1:1:stop",
                "1:2:message"
            ]
        );
        // stops leave when filled
        let mut v = answered();
        v.insert("a".into(), "0.00".into());
        v.insert("k2".into(), "true".into());
        v.insert("r1".into(), "x".into());
        assert!(
            todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &HashMap::new(),
                values: &v,
                stops: &stops(),
                passed: &HashSet::new(),
                blocked: &blocked,
            })
            .is_empty()
        );
        // a radio stop answered by its other box is filled
        let mut v = answered();
        v.insert("k1".into(), "false".into());
        v.insert("k2".into(), "true".into());
        let s1: HashSet<String> = HashSet::from(["k1".to_owned()]);
        assert!(
            todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &HashMap::new(),
                values: &v,
                stops: &s1,
                passed: &HashSet::new(),
                blocked: &blocked,
            })
            .is_empty()
        );
        // a stepped-past row is left alone
        let passed: HashSet<String> = ["1:0", "1:2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            ids(&todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &HashMap::new(),
                values: &answered(),
                stops: &stops(),
                passed: &passed,
                blocked: &blocked,
            })),
            vec!["1:1:stop"]
        );
        // with every box shut only messages remain
        assert_eq!(
            ids(&todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &HashMap::from([("1:0".to_owned(), "x".to_owned())]),
                values: &HashMap::new(),
                stops: &HashSet::new(),
                passed: &HashSet::new(),
                blocked: &|_| true,
            })),
            vec!["1:0:message"]
        );
        // the stop keys a row holds come back with it
        let row = todo_rows(&TodoRowsOptions {
            parts: &p,
            messages: &HashMap::new(),
            values: &answered(),
            stops: &stops(),
            passed: &HashSet::new(),
            blocked: &blocked,
        })
        .into_iter()
        .find(|r| r.id == "1:2")
        .unwrap();
        assert_eq!(row.stops, vec!["r1"]);
        // empty message is no message
        let msgs = HashMap::from([("1:0".to_owned(), String::new())]);
        assert!(
            todo_rows(&TodoRowsOptions {
                parts: &p,
                messages: &msgs,
                values: &answered(),
                stops: &HashSet::new(),
                passed: &HashSet::new(),
                blocked: &blocked,
            })
            .is_empty()
        );
        // no parts, no rows
        assert!(
            todo_rows(&TodoRowsOptions {
                parts: &[],
                messages: &HashMap::new(),
                values: &HashMap::new(),
                stops: &HashSet::new(),
                passed: &HashSet::new(),
                blocked: &blocked,
            })
            .is_empty()
        );
    }

    #[test]
    fn compare_rows_math() {
        assert!(compare_rows("1:10", "2:0") < 0);
        assert!(compare_rows("2:3", "2:10") < 0);
        assert!(compare_rows("3:0", "2:9") > 0);
        assert_eq!(compare_rows("1:2", "1:2"), 0);
    }

    #[test]
    fn next_row_cases() {
        let list: Vec<String> = ["0:2", "1:0", "2:5", "2:10"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            next_row(&list, 1, Some("1:0"), 0, false).as_deref(),
            Some("2:5")
        );
        assert_eq!(
            next_row(&list, -1, Some("2:5"), 0, false).as_deref(),
            Some("1:0")
        );
        assert_eq!(next_row(&list, 1, Some("2:10"), 0, false), None);
        assert_eq!(next_row(&list, -1, Some("0:2"), 0, false), None);
        assert_eq!(
            next_row(&list, 1, Some("2:6"), 0, false).as_deref(),
            Some("2:10")
        );
        assert_eq!(next_row(&list, 1, None, 2, false).as_deref(), Some("2:5"));
        assert_eq!(next_row(&list, -1, None, 2, false).as_deref(), Some("1:0"));
        assert_eq!(next_row(&list, 1, None, 1, false).as_deref(), Some("1:0"));
        assert_eq!(next_row(&list, 1, None, 3, true).as_deref(), Some("0:2"));
        assert_eq!(next_row(&list, -1, None, 3, true), None);
        assert_eq!(next_row(&[], 1, None, 0, false), None);
    }
}
