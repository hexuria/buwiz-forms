// Proof-only page geometry checker for issue #660.
// It intentionally mirrors bir-print's private schedule discovery so no
// production print code is exposed only for this ignored test.
#![allow(dead_code)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::chunks_exact_to_as_chunks)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::type_complexity)]

use serde_json::Value;

#[derive(Debug, Clone)]
struct ColumnSpec {
    stem: String,
    heading: String,
    is_amount: bool,
    start_idx: usize,
    end_idx: usize,
}

#[derive(Debug, Clone)]
struct ScheduleSpec {
    heading: String,
    columns: Vec<ColumnSpec>,
    has_total: bool,
}

fn collect_not_joined_keys(parsed: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(arr) = parsed.get("not_joined").and_then(Value::as_array) {
        for entry in arr {
            if let Some(k) = entry.get("writer_key").and_then(Value::as_str) {
                keys.insert(k.to_string());
            }
            if let Some(ks) = entry.get("writer_keys").and_then(Value::as_array) {
                for k in ks.iter().filter_map(Value::as_str) {
                    keys.insert(k.to_string());
                }
            }
        }
    }
    keys
}

struct RawJoin {
    writer_key: String,
    printed: String,
    is_money_section: bool,
}

fn discover_schedules(parsed: &Value) -> Vec<ScheduleSpec> {
    let mut raw_joins = Vec::new();
    for (section, is_money) in [
        ("joins", false),
        ("money_joins", true),
        ("peso_joins", true),
    ] {
        if let Some(arr) = parsed.get(section).and_then(Value::as_array) {
            for item in arr {
                let Some(wk) = item.get("writer_key").and_then(Value::as_str) else {
                    continue;
                };
                let printed = item
                    .get("printed")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                raw_joins.push(RawJoin {
                    writer_key: wk.to_string(),
                    printed,
                    is_money_section: is_money,
                });
            }
        }
    }

    // Group row-like joins by (schedule_heading, inferred_key_group) so:
    // - stems ending in a fixed column digit (`txtFMVCol1` + `0..4`,
    //   `txtFMVCol2` + `0..4`) never collide, AND
    // - a column whose row-1 caption is fuller than row-2..N (e.g.
    //   `Sub-total (between Columns 1 & 2)` vs `Sub-total`) stays one column
    //   with row 1's full official heading.
    struct ColGroup {
        first_order: usize,
        schedule_heading: String,
        col_heading: String,
        group_key: String,
        is_money_section: bool,
        /// (1-based printed row number from caption, writer_key)
        entries: Vec<(usize, String)>,
    }

    let mut col_groups: Vec<ColGroup> = Vec::new();
    for (order, join) in raw_joins.iter().enumerate() {
        if join.writer_key.starts_with("derived:") {
            continue;
        }
        if !looks_like_schedule_row_caption(&join.printed) {
            continue;
        }
        let Some((schedule_heading, row_num, col_heading)) =
            parse_schedule_row_and_column(&join.printed)
        else {
            continue;
        };
        if schedule_heading.is_empty() || col_heading.is_empty() {
            continue;
        }
        let group_key = infer_column_group_key(&join.writer_key, row_num, &col_heading);
        if let Some(existing) = col_groups.iter_mut().find(|g| {
            g.schedule_heading.eq_ignore_ascii_case(&schedule_heading)
                && g.group_key.eq_ignore_ascii_case(&group_key)
        }) {
            existing.entries.push((row_num, join.writer_key.clone()));
            existing.is_money_section |= join.is_money_section;
            if row_num == 1 && col_heading.len() > existing.col_heading.len() {
                existing.col_heading = col_heading;
            }
        } else {
            col_groups.push(ColGroup {
                first_order: order,
                schedule_heading,
                col_heading,
                group_key,
                is_money_section: join.is_money_section,
                entries: vec![(row_num, join.writer_key.clone())],
            });
        }
    }

    // Derive exact (stem, start_idx, end_idx) from the longest common prefix
    // of each column's writer keys.
    struct DiscoveredCol {
        first_order: usize,
        schedule_heading: String,
        col: ColumnSpec,
    }

    let mut discovered_cols = Vec::new();
    for mut group in col_groups {
        if group.entries.len() < 2 {
            continue;
        }
        group.entries.sort_by_key(|(r, _)| *r);
        let keys: Vec<&str> = group.entries.iter().map(|(_, k)| k.as_str()).collect();
        let Some((stem, start_idx, end_idx)) = derive_stem_and_range(&keys) else {
            continue;
        };
        let is_amount = group.is_money_section || infer_is_amount(&group.col_heading, &stem);
        discovered_cols.push(DiscoveredCol {
            first_order: group.first_order,
            schedule_heading: group.schedule_heading,
            col: ColumnSpec {
                stem,
                heading: group.col_heading,
                is_amount,
                start_idx,
                end_idx,
            },
        });
    }

    discovered_cols.sort_by_key(|c| c.first_order);

    // Group columns into schedules in order of first appearance.
    let mut schedules: Vec<ScheduleSpec> = Vec::new();
    for dc in discovered_cols {
        if let Some(existing) = schedules
            .iter_mut()
            .find(|s| s.heading.eq_ignore_ascii_case(&dc.schedule_heading))
        {
            existing.columns.push(dc.col);
        } else {
            let has_total = schedule_has_total(&raw_joins, &dc.schedule_heading, &dc.col.stem);
            schedules.push(ScheduleSpec {
                heading: dc.schedule_heading,
                columns: vec![dc.col],
                has_total,
            });
        }
    }

    schedules
}

fn schedule_has_total(raw_joins: &[RawJoin], schedule_heading: &str, sample_stem: &str) -> bool {
    // Check if any join in the same schedule namespace or with a matching
    // schedule caption mentions "total" (excluding per-row "sub-total").
    let ns_prefix = sample_stem
        .rfind(':')
        .map(|i| &sample_stem[..=i])
        .unwrap_or("");
    let sched_lower = schedule_heading.to_ascii_lowercase();

    for j in raw_joins {
        let key_lower = j.writer_key.to_ascii_lowercase();
        let pr_lower = j.printed.to_ascii_lowercase();
        let in_same_ns = !ns_prefix.is_empty() && j.writer_key.starts_with(ns_prefix);
        let mentions_sched = pr_lower.contains(&sched_lower);
        let is_total_key = (key_lower.contains("total") && !key_lower.contains("subtot"))
            || (pr_lower.contains("total") && !pr_lower.contains("sub-total"));
        if is_total_key && (in_same_ns || mentions_sched) {
            return true;
        }
    }
    // Also default to true if the schedule has at least one amount column and
    // a matching `not_joined` total exists (e.g. 1600WP Schedule 1.1 carries
    // its total in Part II); wait — for 1600WP the official Schedule 1.1 has no
    // total box, so returning false here accurately matches the official sheet!
    false
}

fn infer_column_group_key(writer_key: &str, row_num: usize, col_heading: &str) -> String {
    if row_num >= 1 {
        let zero_based = (row_num - 1).to_string();
        if let Some(stem) = writer_key.strip_suffix(&zero_based) {
            return format!("stem0:{stem}");
        }
        let one_based = row_num.to_string();
        if let Some(stem) = writer_key.strip_suffix(&one_based) {
            return format!("stem1:{stem}");
        }
    }
    format!("col:{col_heading}")
}

fn derive_stem_and_range(keys: &[&str]) -> Option<(String, usize, usize)> {
    if keys.len() < 2 {
        return None;
    }
    // Longest common prefix of all keys, then step back if needed so that
    // every remaining suffix is a non-empty ASCII integer (e.g. for
    // `txtDatePurchase10`, `txtDatePurchase11`, `txtDatePurchase12`, LCP is
    // `txtDatePurchase1`, which leaves `0, 1, 2` — still valid; if keys span
    // `9` and `10`, LCP stops before `9`/`1`, leaving `9, 10`).
    let first = keys[0].as_bytes();
    let mut prefix_len = first.len();
    for k in &keys[1..] {
        let kb = k.as_bytes();
        prefix_len = prefix_len.min(kb.len());
        let mut i = 0;
        while i < prefix_len && first[i] == kb[i] {
            i += 1;
        }
        prefix_len = i;
    }
    // If the LCP splits a multi-digit row index (e.g. `txtDatePurchase10..13`
    // has LCP `txtDatePurchase1`), check whether stepping back 0 or 1 digits
    // yields consecutive indices. Prefer the longest prefix where all suffixes
    // parse as consecutive non-negative integers (except step back when a
    // suffix would be empty).
    loop {
        let stem = &keys[0][..prefix_len];
        let mut indices: Vec<usize> = Vec::with_capacity(keys.len());
        let mut all_ok = true;
        for k in keys {
            let suffix = &k[prefix_len..];
            if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
                all_ok = false;
                break;
            }
            match suffix.parse::<usize>() {
                Ok(n) => indices.push(n),
                Err(_) => {
                    all_ok = false;
                    break;
                }
            }
        }
        if all_ok {
            indices.sort_unstable();
            indices.dedup();
            let start = indices[0];
            let end = *indices.last().unwrap();
            if indices.len() >= 2 && end - start + 1 == indices.len() {
                return Some((stem.to_string(), start, end));
            }
        }
        if prefix_len == 0 || !first[prefix_len - 1].is_ascii_digit() {
            return None;
        }
        prefix_len -= 1;
    }
}

fn match_stem_index(key: &str, stem: &str) -> Option<usize> {
    let rest = key.strip_prefix(stem)?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse::<usize>().ok()
}

fn looks_like_schedule_row_caption(printed: &str) -> bool {
    let lower = printed.to_ascii_lowercase();
    lower.contains(" row ") && (lower.contains("schedule ") || lower.contains("part "))
}

/// Extract `(schedule_heading, row_number, column_heading)` from a
/// `writer-cells.json` `printed` caption. Handles both common shapes:
/// - `"OCT/TCT/CCT No. (Schedule 1.A row 1)"` -> `("Schedule 1.A", 1, "OCT/TCT/CCT No.")`
/// - `"Schedule A row 1 Particulars"` -> `("Schedule A", 1, "Particulars")`
/// - `"Schedule 1.1 row 1 Individual/Corporation"` -> `("Schedule 1.1", 1, "Individual/Corporation")`
/// - `"Schedule 1 continuation row 1 FMV per TD"` -> `("Schedule 1", 1, "FMV per TD")`
pub(crate) fn parse_schedule_row_and_column_pub(printed: &str) -> Option<(String, usize, String)> {
    parse_schedule_row_and_column(printed)
}

fn parse_schedule_row_and_column(printed: &str) -> Option<(String, usize, String)> {
    let trimmed = printed.trim();

    // Shape 1: "<column> (Schedule <X> [continuation] row <N>)"
    if let Some(open) = trimmed.rfind('(') {
        if trimmed.ends_with(')') {
            let inside = trimmed[open + 1..trimmed.len() - 1].trim();
            if let Some((sched, row_num, _)) = extract_schedule_row_and_after(inside) {
                let col = trimmed[..open].trim().to_string();
                return Some((sched, row_num, col));
            }
        }
    }

    // Shape 2: "Schedule <X> [continuation] row <N> <column>"
    if let Some((sched, row_num, after_row)) = extract_schedule_row_and_after(trimmed) {
        let col = after_row
            .trim_start_matches(|c: char| c == ':' || c == '-' || c == ',' || c.is_whitespace())
            .to_string();
        return Some((sched, row_num, col));
    }

    None
}

fn extract_schedule_row_and_after(s: &str) -> Option<(String, usize, &str)> {
    let lower = s.to_ascii_lowercase();
    let row_pos = lower.find(" row ")?;
    let before = s[..row_pos].trim();
    let after = s[row_pos + 5..].trim_start();
    let digits_end = after.bytes().take_while(|b| b.is_ascii_digit()).count();
    let (row_num, rest) = if digits_end > 0 {
        let n = after[..digits_end].parse::<usize>().ok()?;
        (n, after[digits_end..].trim_start())
    } else if after
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_uppercase())
        && after
            .as_bytes()
            .get(1)
            .is_none_or(|b| !b.is_ascii_alphanumeric())
    {
        // Lettered rows such as "Schedule 2 row A Name of Corporate Stock" (1707).
        let n = (after.as_bytes()[0] - b'A') as usize + 1;
        (n, after[1..].trim_start())
    } else {
        return None;
    };
    // Normalize trailing "continuation" so Schedule 1 and Schedule 1 continuation merge
    let sched = strip_trailing_word_ci(before, "continuation")
        .trim()
        .to_string();
    if sched.is_empty() {
        return None;
    }
    Some((sched, row_num, rest))
}

struct PopupColDef {
    /// Key template with `{n}` replaced by the 1-based popup row index.
    key_template: &'static str,
    heading: &'static str,
    is_amount: bool,
}

struct PopupScheduleDef {
    slug: &'static str,
    heading: &'static str,
    printed_capacity: usize,
    /// Continuing row number for popup index `n = 1` is `row_num_base + n`.
    row_num_base: usize,
    has_total: bool,
    columns: &'static [PopupColDef],
}

/// Complete registry of forms in `crates/bir-core/src/forms/` whose overflow
/// ("Add more..." / popup / modal) rows are emitted under keys distinct from
/// the printed row stems (`extend_layout` and friends):
/// - `1700-2018` (`form_1700.rs`): Schedule 1 (`4_{n}` employer & comp keys)
/// - `1701a-2018` (`form_1701a.rs`): Part IV-A (`txtmodalPartIVA*_{n}C1..3`) &
///   Part IV-B (`txtmodalPartIVB*_{n}C1..3`)
/// - `1702ex-2018` (`form_1702ex.rs`): 5 popups (`txtPg3Pt6{S1I17,S2I4,S3I3,S3I6,S3I8}_{n}Col*`)
/// - `1702mx-2018c` (`form_1702mx_official.rs`): Schedule 6 (`txtPg6Sc6_{n}Col1..6`)
/// - `1702rt-2018c` (`form_1702rt_official.rs`): 6 modals (`<rowName>.{n}C1..3`)
/// - `1707-2021` (`form_1707.rs`): Schedule 2 (`txtPg2Pt4S2_{k}Col1..4`) &
///   Schedule 3 (`txtPg2Pt4S3_{k}Col1..2`)
/// - `1707a-2021` (`form_1707a.rs`): Schedule 1 (`txtS1C*_{k}`) & Schedule 2 (`txtS2C*_{k}`)
const POPUP_SCHEDULES: &[PopupScheduleDef] = &[
    // 1707
    PopupScheduleDef {
        slug: "1707-2021",
        heading: "Schedule 2",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col1",
                heading: "Name of Corporate Stock",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col2",
                heading: "No. of Shares",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col3",
                heading: "Stock Certificate Number",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S2_{n}Col4",
                heading: "Taxable Base/Selling Price",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1707-2021",
        heading: "Schedule 3",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S3_{n}Col1",
                heading: "Particulars",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707:txtPg2Pt4S3_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    // 1701A
    PopupScheduleDef {
        slug: "1701a-2018",
        heading: "Part IV-A Other Non-Operating Income",
        printed_capacity: 2,
        row_num_base: 41,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVADesc_{n}C1",
                heading: "Other Non-Operating Income (specify)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVA_{n}C2",
                heading: "A) Taxpayer/Filer",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVA_{n}C3",
                heading: "B) Spouse",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1701a-2018",
        heading: "Part IV-B Other Non-Operating Income",
        printed_capacity: 2,
        row_num_base: 50,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVBDesc_{n}C1",
                heading: "Other Non-Operating Income (specify)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVB_{n}C2",
                heading: "A) Taxpayer/Filer",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1701A:txtmodalPartIVB_{n}C3",
                heading: "B) Spouse",
                is_amount: true,
            },
        ],
    },
    // 1707A
    PopupScheduleDef {
        slug: "1707a-2021",
        heading: "Schedule 1",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C1DateOfTransaction_{n}",
                heading: "(A) Date of Transaction (MM/DD/YYYY)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C2NameOfCorporateStock_{n}",
                heading: "(B) Name of Corporate Stock",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C3SellingPrice_{n}",
                heading: "(C) Selling Price",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C4Cost_{n}",
                heading: "(D) Cost",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C5CapitalGains_{n}",
                heading: "(E = C-D) Capital Gains",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS1C6TaxPaid_{n}",
                heading: "(F) Tax Paid",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1707a-2021",
        heading: "Schedule 2",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C1DateOfTransaction_{n}",
                heading: "(A) Date of Transaction (MM/DD/YYYY)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C2NameOfCorporateStock_{n}",
                heading: "(B) Name of Corporate Stock",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C3SellingPrice_{n}",
                heading: "(C) Selling Price",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C4Cost_{n}",
                heading: "(D) Cost",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1707Av2021:txtS2C5CapitalLoss_{n}",
                heading: "(E = C-D) Capital Loss",
                is_amount: true,
            },
        ],
    },
    // 1700
    PopupScheduleDef {
        slug: "1700-2018",
        heading: "Schedule 1",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerName1",
                heading: "Name of Employer",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerName2",
                heading: "Name of Employer (line 2)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerTIN1",
                heading: "TIN (1)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerTIN2",
                heading: "TIN (2)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerTIN3",
                heading: "TIN (3)",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2I4_{n}PartVIEmployerBranchCode",
                heading: "Branch Code",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2ISched1c_4_{n}REG",
                heading: "Gross Compensation Income (Regular)",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2ISched1d_4_{n}CIFR",
                heading: "Compensation Income (Subject to Final/Other)",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1700:txtPg2ISched1e_4_{n}TW",
                heading: "Tax Withheld",
                is_amount: true,
            },
        ],
    },
    // 1702EX
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 1 Item 17",
        printed_capacity: 6,
        row_num_base: 5,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S1I17_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S1I17_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 2",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S2I4_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S2I4_{n}Col2",
                heading: "Legal Basis",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S2I4_{n}Col3",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 3 Item 3",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I3_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I3_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 3 Item 6",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I6_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I6_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702ex-2018",
        heading: "Schedule 3 Item 8",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I8_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702EX:txtPg3Pt6S3I8_{n}Col2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    // 1702MX
    PopupScheduleDef {
        slug: "1702mx-2018c",
        heading: "Schedule 6",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col2",
                heading: "Legal Basis",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col3",
                heading: "Column A",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col4",
                heading: "Column B",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col5",
                heading: "Column C",
                is_amount: true,
            },
            PopupColDef {
                key_template: "frm1702MX:txtPg6Sc6_{n}Col6",
                heading: "Column D",
                is_amount: true,
            },
        ],
    },
    // 1702RT
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Part IV Item 54",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg2Pt4I54.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg2Pt4I54.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule I Item 17i",
        printed_capacity: 6,
        row_num_base: 5,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc1I17i.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc1I17i.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule II",
        printed_capacity: 4,
        row_num_base: 3,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc2I4.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc2I4.{n}C2",
                heading: "Legal Basis",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg3Sc2I4.{n}C3",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule V Item 3",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I3.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I3.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule V Item 6",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I6.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I6.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
    PopupScheduleDef {
        slug: "1702rt-2018c",
        heading: "Schedule V Item 8",
        printed_capacity: 2,
        row_num_base: 1,
        has_total: true,
        columns: &[
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I8.{n}C1",
                heading: "Description",
                is_amount: false,
            },
            PopupColDef {
                key_template: "frm1702RT:txtPg4Sc5I8.{n}C2",
                heading: "Amount",
                is_amount: true,
            },
        ],
    },
];

fn strip_trailing_word_ci<'a>(s: &'a str, word: &str) -> &'a str {
    let lower = s.to_ascii_lowercase();
    if lower.ends_with(word) {
        let cut = s.len() - word.len();
        if cut == 0 || s.as_bytes()[cut - 1].is_ascii_whitespace() {
            return s[..cut].trim_end();
        }
    }
    s
}

fn infer_is_amount(heading: &str, stem: &str) -> bool {
    let h = heading.to_ascii_lowercase();
    let s = stem.to_ascii_lowercase();

    // Non-amount columns even if they contain words like "tax" or "no."
    if h.contains("declaration")
        || h.contains("tax rate")
        || h.ends_with("rate")
        || h.contains("area")
        || h.contains("tin")
        || h.contains("atc")
        || h.contains("seq")
        || h.contains("oct/")
        || h.contains("tct/")
        || h.contains("cert")
        || h.contains("no. of shares")
        || h.contains("life")
        || s.contains("rate")
        || s.contains("area")
        || s.contains("taxdec")
        || s.contains("tin")
        || s.contains("atc")
    {
        return false;
    }

    h.contains("amount")
        || h.contains("fair market")
        || h.contains("fmv")
        || h.contains("value")
        || h.contains("sub-total")
        || h.contains("subtotal")
        || h.contains("total")
        || h.contains("withheld")
        || h.contains("tax due")
        || h.contains("tax paid")
        || h.contains("dst paid")
        || h.contains("tax base")
        || h.contains("input tax")
        || h.contains("exclusive")
        || h.contains("conjugal")
        || h.contains("deduction")
        || h.contains("price")
        || h.contains("cost")
        || h.contains("sales")
        || h.contains("purchases")
        || s.contains("fmv")
        || s.contains("amount")
        || s.contains("withheld")
        || s.contains("taxdue")
        || s.contains("dstpaid")
        || s.contains("parval")
}

use bir_app_api::print_sheet::prepare_html_for;
use bir_app_api::static_html::staticize;
use bir_print::frozen_html;
use lopdf::content::Content;
use lopdf::{Document, Object};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
struct TextRun {
    text: String,
    x: f64,
    y: f64,
    width: f64,
    size: f64,
}

#[derive(Debug, Clone, Copy)]
struct Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Matrix {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn compose(outer: Self, inner: Self) -> Self {
        Self {
            a: outer.a * inner.a + outer.c * inner.b,
            b: outer.b * inner.a + outer.d * inner.b,
            c: outer.a * inner.c + outer.c * inner.d,
            d: outer.b * inner.c + outer.d * inner.d,
            e: outer.a * inner.e + outer.c * inner.f + outer.e,
            f: outer.b * inner.e + outer.d * inner.f + outer.f,
        }
    }

    fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    fn point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

fn operand_float(obj: &Object) -> f64 {
    obj.as_float().unwrap_or(0.0) as f64
}

fn object_array_numbers(obj: &Object) -> Vec<f64> {
    obj.as_array()
        .map(|items| items.iter().map(operand_float).collect())
        .unwrap_or_default()
}

fn matrix_from_operands(ops: &[Object], start: usize) -> Matrix {
    Matrix {
        a: operand_float(&ops[start]),
        b: operand_float(&ops[start + 1]),
        c: operand_float(&ops[start + 2]),
        d: operand_float(&ops[start + 3]),
        e: operand_float(&ops[start + 4]),
        f: operand_float(&ops[start + 5]),
    }
}

fn page_media_box(document: &Document, page_id: lopdf::ObjectId) -> (f64, f64, f64, f64) {
    let mut cursor = Some(page_id);
    for _ in 0..8 {
        let Some(id) = cursor else {
            break;
        };
        let Ok(dict) = document.get_dictionary(id) else {
            break;
        };
        if let Ok(obj) = dict.get(b"MediaBox") {
            let nums = object_array_numbers(obj);
            if nums.len() >= 4 {
                return (nums[0], nums[1], nums[2], nums[3]);
            }
        }
        cursor = dict
            .get(b"Parent")
            .ok()
            .and_then(|obj| obj.as_reference().ok());
    }
    (0.0, 0.0, 612.0, 936.0)
}

fn hex_tokens(line: &str) -> Vec<Vec<u8>> {
    let mut tokens = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '<' || chars.peek() == Some(&'<') {
            continue;
        }
        let mut hex = String::new();
        for next in chars.by_ref() {
            if next == '>' {
                break;
            }
            if next.is_ascii_hexdigit() {
                hex.push(next);
            }
        }
        if hex.len() >= 2 {
            let mut bytes = Vec::new();
            let mut index = 0;
            while index + 1 < hex.len() {
                if let Ok(byte) = u8::from_str_radix(&hex[index..index + 2], 16) {
                    bytes.push(byte);
                }
                index += 2;
            }
            tokens.push(bytes);
        }
    }
    tokens
}

fn utf16_hex(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn cid(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || bytes.len() > 4 {
        return None;
    }
    let mut value = 0u32;
    for byte in bytes {
        value = (value << 8) | u32::from(*byte);
    }
    Some(value)
}

fn parse_to_unicode_cmap(content: &[u8]) -> BTreeMap<u32, String> {
    let text = String::from_utf8_lossy(content);
    let mut map = BTreeMap::new();
    let mut in_char = false;
    let mut in_range = false;
    for line in text.lines() {
        if line.contains("beginbfchar") {
            in_char = true;
            continue;
        }
        if line.contains("endbfchar") {
            in_char = false;
            continue;
        }
        if line.contains("beginbfrange") {
            in_range = true;
            continue;
        }
        if line.contains("endbfrange") {
            in_range = false;
            continue;
        }
        let tokens = hex_tokens(line);
        if in_char && tokens.len() >= 2 {
            if let Some(source) = cid(&tokens[0]) {
                map.insert(source, utf16_hex(&tokens[1]));
            }
        } else if in_range && tokens.len() >= 3 {
            let Some(start) = cid(&tokens[0]) else {
                continue;
            };
            let Some(end) = cid(&tokens[1]) else {
                continue;
            };
            if line.contains('[') {
                for (offset, destination) in tokens.iter().skip(2).enumerate() {
                    map.insert(start + offset as u32, utf16_hex(destination));
                }
            } else if let Some(destination) = cid(&tokens[2]) {
                for source in start..=end {
                    let width = tokens[2].len().max(2);
                    let encoded = (destination + source - start).to_be_bytes();
                    map.insert(
                        source,
                        utf16_hex(&encoded[encoded.len().saturating_sub(width)..]),
                    );
                }
            }
        }
    }
    map
}

fn decode_cid_text(bytes: &[u8], map: Option<&BTreeMap<u32, String>>) -> String {
    let one_byte = map.is_some_and(|map| map.keys().all(|code| *code <= 0xff))
        && !(bytes.len() % 2 == 0
            && !bytes.is_empty()
            && bytes.iter().step_by(2).all(|byte| *byte == 0));
    if one_byte {
        return bytes
            .iter()
            .map(|byte| {
                let code = u32::from(*byte);
                map.and_then(|map| map.get(&code).cloned())
                    .unwrap_or_else(|| char::from_u32(code).unwrap_or('\u{fffd}').to_string())
            })
            .collect();
    }
    let mut out = String::new();
    for chunk in bytes.chunks_exact(2) {
        let code = u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
        if let Some(map) = map {
            if let Some(text) = map.get(&code) {
                out.push_str(text);
                continue;
            }
        }
        if let Some(ch) = char::from_u32(code) {
            out.push(ch);
        }
    }
    if out.chars().all(|ch| ch.is_control()) && bytes.iter().any(|byte| byte.is_ascii_graphic()) {
        return bytes
            .iter()
            .filter(|byte| byte.is_ascii_graphic() || **byte == b' ')
            .map(|byte| *byte as char)
            .collect();
    }
    out
}

fn owned_dictionary(document: &Document, object: &Object) -> Option<lopdf::Dictionary> {
    match object {
        Object::Reference(id) => document.get_dictionary(*id).ok().cloned(),
        Object::Dictionary(dictionary) => Some(dictionary.clone()),
        _ => None,
    }
}

fn owned_stream(document: &Document, object: &Object) -> Option<lopdf::Stream> {
    match object {
        Object::Reference(id) => document
            .get_object(*id)
            .ok()
            .and_then(|object| object.as_stream().ok())
            .cloned(),
        Object::Stream(stream) => Some(stream.clone()),
        _ => None,
    }
}

fn extract_text_runs(pdf_path: &Path) -> Vec<(usize, (f64, f64, f64, f64), Vec<TextRun>)> {
    use std::collections::VecDeque;

    let document = Document::load(pdf_path).expect("load proof PDF");
    if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
        println!(
            "PDF_DOC_DEBUG {} bytes pages={}",
            pdf_path.display(),
            document.get_pages().len()
        );
    }
    let mut output = Vec::new();
    for (page_number, page_id) in document.get_pages() {
        let page_box = page_media_box(&document, page_id);
        let page_resources = document
            .get_dictionary(page_id)
            .ok()
            .and_then(|page| page.get(b"Resources").ok())
            .and_then(|resources| owned_dictionary(&document, resources))
            .unwrap_or_default();
        let mut fonts: BTreeMap<Vec<u8>, lopdf::Dictionary> = BTreeMap::new();
        let mut resource_stack = vec![page_resources.clone()];
        let mut seen_resources = BTreeSet::new();
        while let Some(resources) = resource_stack.pop() {
            let marker = format!("{resources:?}");
            if !seen_resources.insert(marker) {
                continue;
            }
            if let Ok(font_object) = resources.get(b"Font") {
                if let Some(font_dict) = owned_dictionary(&document, font_object) {
                    for (name, value) in font_dict.iter() {
                        if !fonts.contains_key(name) {
                            if let Some(font) = owned_dictionary(&document, value) {
                                fonts.insert(name.clone(), font);
                            }
                        }
                    }
                }
            }
            if let Ok(xobject_object) = resources.get(b"XObject") {
                if let Some(xobjects) = owned_dictionary(&document, xobject_object) {
                    for (_, value) in xobjects.iter() {
                        if let Some(stream) = owned_stream(&document, value) {
                            if let Ok(child_resources) = stream.dict.get(b"Resources") {
                                if let Some(child_resources) =
                                    owned_dictionary(&document, child_resources)
                                {
                                    resource_stack.push(child_resources);
                                }
                            }
                        }
                    }
                }
            }
        }
        let encodings: BTreeMap<Vec<u8>, _> = fonts
            .iter()
            .filter_map(|(name, font)| {
                font.get_font_encoding(&document)
                    .ok()
                    .map(|encoding| (name.clone(), encoding))
            })
            .collect();
        let font_maps: BTreeMap<Vec<u8>, BTreeMap<u32, String>> = fonts
            .iter()
            .filter_map(|(name, font)| {
                let object = font.get(b"ToUnicode").ok()?;
                let stream = owned_stream(&document, object)?;
                let content = stream.get_plain_content().ok()?;
                Some((name.clone(), parse_to_unicode_cmap(&content)))
            })
            .collect();
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            println!(
                "PDF_FONT_DEBUG page={page_number} fonts={:?} encodings={:?} cmap_sizes={:?}",
                fonts.keys().collect::<Vec<_>>(),
                encodings.keys().collect::<Vec<_>>(),
                font_maps
                    .iter()
                    .map(|(name, map)| (name.clone(), map.len()))
                    .collect::<Vec<_>>()
            );
        }
        let content = document
            .get_and_decode_page_content(page_id)
            .expect("decode proof PDF page");
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for operation in &content.operations {
                *counts.entry(operation.operator.as_str()).or_default() += 1;
            }
            println!(
                "PDF_OPS_DEBUG page={page_number} ops={} counts={counts:?}",
                content.operations.len()
            );
        }
        struct Frame {
            operations: VecDeque<lopdf::content::Operation>,
            resources: lopdf::Dictionary,
            restore: Option<(Matrix, Matrix, Option<Vec<u8>>, f64)>,
        }
        let mut frames = vec![Frame {
            operations: content.operations.into_iter().collect(),
            resources: page_resources,
            restore: None,
        }];
        let mut ctm = Matrix::identity();
        let mut graphics_stack = Vec::new();
        let mut text_matrix = Matrix::identity();
        let mut current_font: Option<Vec<u8>> = None;
        let mut font_size = 0.0f64;
        let mut runs = Vec::new();
        let mut matrix_trace = 0usize;
        loop {
            let frame_depth = frames.len();
            let Some(frame) = frames.last_mut() else {
                break;
            };
            let Some(operation) = frame.operations.pop_front() else {
                let restore = frame.restore.take();
                frames.pop();
                if let Some((old_ctm, old_text, old_font, old_size)) = restore {
                    ctm = old_ctm;
                    text_matrix = old_text;
                    current_font = old_font;
                    font_size = old_size;
                }
                continue;
            };
            let ops = operation.operands;
            match operation.operator.as_str() {
                "q" => graphics_stack.push(ctm),
                "Q" => {
                    if let Some(previous) = graphics_stack.pop() {
                        ctm = previous;
                    }
                }
                "cm" if ops.len() >= 6 => {
                    let incoming = matrix_from_operands(&ops, 0);
                    if false && matrix_trace < 30 {
                        println!(
                            "MATRIX_DEBUG page={page_number} depth={frame_depth} cm={incoming:?} before={ctm:?}"
                        );
                        matrix_trace += 1;
                    }
                    ctm = Matrix::compose(ctm, incoming);
                }
                "Do" if !ops.is_empty() && frame_depth < 16 => {
                    let name = ops[0].as_name().unwrap_or_default().to_vec();
                    let xobject = frame
                        .resources
                        .get(b"XObject")
                        .ok()
                        .and_then(|objects| owned_dictionary(&document, objects))
                        .and_then(|objects| objects.get(&name).ok().cloned());
                    if let Some(xobject) = xobject {
                        if let Some(stream) = owned_stream(&document, &xobject) {
                            let is_form = stream
                                .dict
                                .get(b"Subtype")
                                .ok()
                                .and_then(|subtype| subtype.as_name().ok())
                                == Some(b"Form");
                            if is_form {
                                let bytes = stream.get_plain_content().unwrap_or_default();
                                if let Ok(child) = Content::decode(&bytes) {
                                    let restore =
                                        (ctm, text_matrix, current_font.clone(), font_size);
                                    if let Ok(matrix_object) = stream.dict.get(b"Matrix") {
                                        let nums = object_array_numbers(matrix_object);
                                        if nums.len() >= 6 {
                                            let form_matrix = Matrix {
                                                a: nums[0],
                                                b: nums[1],
                                                c: nums[2],
                                                d: nums[3],
                                                e: nums[4],
                                                f: nums[5],
                                            };
                                            if false && matrix_trace < 30 {
                                                println!(
                                                    "MATRIX_DEBUG page={page_number} depth={frame_depth} form={form_matrix:?} before={ctm:?}"
                                                );
                                                matrix_trace += 1;
                                            }
                                            ctm = Matrix::compose(ctm, form_matrix);
                                        }
                                    }
                                    text_matrix = Matrix::identity();
                                    let child_resources = stream
                                        .dict
                                        .get(b"Resources")
                                        .ok()
                                        .and_then(|resources| {
                                            owned_dictionary(&document, resources)
                                        })
                                        .unwrap_or_else(|| frame.resources.clone());
                                    frames.push(Frame {
                                        operations: child.operations.into_iter().collect(),
                                        resources: child_resources,
                                        restore: Some(restore),
                                    });
                                }
                            }
                        }
                    }
                }
                "BT" => text_matrix = Matrix::identity(),
                "Tf" if ops.len() >= 2 => {
                    if let Ok(name) = ops[0].as_name() {
                        current_font = Some(name.to_vec());
                    }
                    font_size = operand_float(&ops[1]);
                }
                "Tm" if ops.len() >= 6 => text_matrix = matrix_from_operands(&ops, 0),
                "Td" | "TD" if ops.len() >= 2 => {
                    let tx = operand_float(&ops[0]);
                    let ty = operand_float(&ops[1]);
                    text_matrix = Matrix::compose(text_matrix, Matrix::translate(tx, ty));
                }
                "Tj" if !ops.is_empty() => {
                    let bytes = match &ops[0] {
                        Object::String(bytes, _) => bytes.clone(),
                        _ => Vec::new(),
                    };
                    let text = current_font
                        .as_ref()
                        .and_then(|name| font_maps.get(name))
                        .map_or_else(
                            || decode_cid_text(&bytes, None),
                            |map| decode_cid_text(&bytes, Some(map)),
                        );
                    let text_xform = Matrix::compose(ctm, text_matrix);
                    let (x, y) = text_xform.point(0.0, 0.0);
                    let width_scale =
                        (text_xform.a * text_xform.a + text_xform.b * text_xform.b).sqrt();
                    let height_scale =
                        (text_xform.c * text_xform.c + text_xform.d * text_xform.d).sqrt();
                    let width = text.len().max(1) as f64 * font_size * 0.52 * width_scale.max(0.01);
                    if !text.trim().is_empty() {
                        runs.push(TextRun {
                            text,
                            x,
                            y,
                            width,
                            size: font_size * height_scale.max(0.01),
                        });
                    }
                }
                "TJ" if !ops.is_empty() => {
                    let mut text = String::new();
                    let mut adjustment = 0.0f64;
                    if let Ok(items) = ops[0].as_array() {
                        for item in items {
                            match item {
                                Object::String(bytes, _) => {
                                    text.push_str(
                                        &current_font
                                            .as_ref()
                                            .and_then(|name| font_maps.get(name))
                                            .map_or_else(
                                                || decode_cid_text(bytes, None),
                                                |map| decode_cid_text(bytes, Some(map)),
                                            ),
                                    );
                                }
                                Object::Integer(n) => adjustment -= *n as f64 / 1000.0,
                                Object::Real(n) => adjustment -= *n as f64 / 1000.0,
                                _ => {}
                            }
                        }
                    }
                    let text_xform = Matrix::compose(ctm, text_matrix);
                    let (x, y) = text_xform.point(0.0, 0.0);
                    let width_scale =
                        (text_xform.a * text_xform.a + text_xform.b * text_xform.b).sqrt();
                    let height_scale =
                        (text_xform.c * text_xform.c + text_xform.d * text_xform.d).sqrt();
                    let width = (text.len().max(1) as f64 * 0.52 + adjustment)
                        * font_size
                        * width_scale.max(0.01);
                    if !text.trim().is_empty() {
                        runs.push(TextRun {
                            text,
                            x,
                            y,
                            width,
                            size: font_size * height_scale.max(0.01),
                        });
                    }
                }
                _ => {}
            }
        }
        output.push((page_number as usize, page_box, runs));
    }
    output
}

fn fill_proof_fields(slug: &str, amount: &str) -> (BTreeMap<String, String>, usize, usize) {
    let cells_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../html-frozen")
        .join(slug)
        .join("writer-cells.json");
    let parsed: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&cells_path)
            .unwrap_or_else(|error| panic!("read {cells_path:?}: {error}")),
    )
    .expect("parse writer cells");
    let specs = discover_schedules(&parsed);
    let mut fields = BTreeMap::new();
    let mut stem_rows = 0usize;
    for spec in &specs {
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            println!(
                "SPEC_DEBUG {slug} {} columns={:?}",
                spec.heading,
                spec.columns
                    .iter()
                    .map(|column| (
                        column.stem.clone(),
                        column.start_idx,
                        column.end_idx,
                        column.is_amount,
                        column.heading.clone()
                    ))
                    .collect::<Vec<_>>()
            );
        }
        let min_idx = spec
            .columns
            .iter()
            .map(|column| column.start_idx)
            .min()
            .unwrap_or(1);
        let max_idx = spec
            .columns
            .iter()
            .map(|column| column.end_idx)
            .max()
            .unwrap_or(0);
        for row_idx in min_idx..=(max_idx + 40) {
            for column in &spec.columns {
                if row_idx < column.start_idx || row_idx > column.end_idx + 40 {
                    continue;
                }
                let key = format!("{}{}", column.stem, row_idx);
                let value = if column.is_amount {
                    amount.to_string()
                } else {
                    format!(
                        "PB{}R{}",
                        row_idx,
                        column.heading.chars().next().unwrap_or('X')
                    )
                };
                fields.insert(key, value);
            }
            if row_idx > max_idx {
                stem_rows += 1;
            }
        }
    }
    let mut popup_rows = 0usize;
    for def in POPUP_SCHEDULES.iter().filter(|def| def.slug == slug) {
        let n_count = def.printed_capacity.saturating_sub(def.row_num_base) + 40;
        for n in 1..=n_count.max(1) {
            for column in def.columns {
                let key = column.key_template.replace("{n}", &n.to_string());
                let value = if column.is_amount {
                    amount.to_string()
                } else {
                    format!("PB{}P{}", n, column.heading.chars().next().unwrap_or('X'))
                };
                fields.insert(key, value);
            }
            let printed_row = def.row_num_base + n;
            if printed_row > def.printed_capacity {
                popup_rows += 1;
            }
        }
    }
    (fields, stem_rows, popup_rows)
}

fn chrome_binary() -> &'static str {
    for candidate in ["/usr/bin/google-chrome", "/opt/google/chrome/google-chrome"] {
        if Path::new(candidate).exists() {
            return candidate;
        }
    }
    "google-chrome"
}

fn print_pdf(html_path: &Path, pdf_path: &Path, profile_dir: &Path) {
    let status = Command::new(chrome_binary())
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--no-pdf-header-footer")
        .arg(format!("--user-data-dir={}", profile_dir.display()))
        .arg(format!("--print-to-pdf={}", pdf_path.display()))
        .arg(format!("file://{}", html_path.display()))
        .env("BUWIZ_NO_KEYCHAIN", "1")
        .env("EBIR_TEST_ENV", "1")
        .env("BIR_AUTH_BYPASS", "1")
        .env("BIR_SFTP_DRY_RUN", "1")
        .output()
        .expect("start Chrome print");
    assert!(
        status.status.success() && pdf_path.exists(),
        "Chrome print failed: stdout={} stderr={}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
}

fn row_marker(text: &str) -> String {
    let trimmed = text.trim();
    let leading: String = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if !leading.is_empty()
        && (leading.len() == trimmed.len()
            || trimmed[leading.len()..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace))
    {
        return leading;
    }
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if let Some(pos) = compact.find("PB") {
        let digits: String = compact[pos + 2..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        if !digits.is_empty() {
            return digits;
        }
    }
    "unknown".to_string()
}

fn wide_plain_schedules(
    document_html: &str,
    overflow: &bir_print::continuation::OverflowReport,
) -> Vec<(String, usize)> {
    let mut plain_sections = Vec::new();
    for section in document_html.split("<div class=\"page ").skip(1) {
        let open_tag = section
            .split_once('>')
            .map(|(tag, _)| tag)
            .unwrap_or(section);
        if open_tag.contains("page-continuation") && !open_tag.contains("page-continuation-band") {
            plain_sections.push(section);
        }
    }
    overflow
        .schedules
        .iter()
        .filter(|schedule| schedule.columns.len() >= 10)
        .filter(|schedule| {
            plain_sections
                .iter()
                .any(|section| section.contains(&format!("{} (continued)", schedule.heading)))
        })
        .map(|schedule| (schedule.heading.clone(), schedule.columns.len()))
        .collect()
}

fn group_text_runs(mut runs: Vec<TextRun>) -> Vec<TextRun> {
    runs.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut grouped: Vec<TextRun> = Vec::new();
    let mut current: Option<TextRun> = None;
    for run in runs {
        match current.as_mut() {
            Some(line)
                if (line.y - run.y).abs() <= 0.8
                    && line.size.max(run.size) - line.size.min(run.size) <= 4.0 =>
            {
                if run.x > line.x + line.width + 0.8 && !line.text.ends_with(' ') {
                    line.text.push(' ');
                }
                line.text.push_str(&run.text);
                let x2 = (line.x + line.width).max(run.x + run.width);
                line.width = x2 - line.x.min(run.x);
                line.x = line.x.min(run.x);
                line.y = line.y.min(run.y);
                line.size = line.size.max(run.size);
            }
            _ => {
                if let Some(line) = current.take() {
                    grouped.push(line);
                }
                current = Some(run);
            }
        }
    }
    if let Some(line) = current {
        grouped.push(line);
    }
    grouped
}

fn minimum_numeric_text_size(pdf_path: &Path) -> Option<f64> {
    let mut minimum: Option<f64> = None;
    for (page_number, _, runs) in extract_text_runs(pdf_path) {
        let grouped_for_page = group_text_runs(runs.clone());
        let additional_sheet = grouped_for_page.iter().any(|line| {
            line.text
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
                .to_uppercase()
                .contains("ADDITIONALSHEET")
        });
        if !additional_sheet {
            continue;
        }
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
            for line in group_text_runs(runs.clone()) {
                *sizes.entry(format!("{:.2}", line.size)).or_default() += 1;
            }
            println!("SIZE_DIST_DEBUG page={page_number} {sizes:?}");
            for line in group_text_runs(runs.clone()).into_iter().take(12) {
                println!(
                    "LINE_DEBUG page={page_number} size={:.2} text={}",
                    line.size,
                    line.text.trim()
                );
            }
        }
        for line in group_text_runs(runs) {
            let compact: String = line.text.chars().filter(|ch| !ch.is_whitespace()).collect();
            let has_proof_amount = compact.contains("9,999,999.99");
            if has_proof_amount {
                if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
                    println!(
                        "AMOUNT_LINE_DEBUG size={:.2} text={}",
                        line.size,
                        line.text.trim()
                    );
                }
                minimum = Some(minimum.map_or(line.size, |old: f64| old.min(line.size)));
            }
        }
    }
    minimum
}

fn is_schedule_row_line(text: &str) -> bool {
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.contains("PB")
        || compact.contains("9,999,999,999")
        || compact.contains("9999999999999")
    {
        return true;
    }
    let trimmed = text.trim();
    trimmed.len() <= 3 && trimmed.chars().all(|ch| ch.is_ascii_digit()) && !trimmed.is_empty()
}

fn bounds_report(slug: &str, pdf_path: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    let extracted = extract_text_runs(pdf_path);
    if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
        let run_count: usize = extracted.iter().map(|(_, _, runs)| runs.len()).sum();
        let first = extracted
            .iter()
            .flat_map(|(_, _, runs)| runs.iter())
            .take(8)
            .map(|run| format!("{:?}@{:.1},{:.1}s{:.1}", run.text, run.x, run.y, run.size))
            .collect::<Vec<_>>()
            .join(" | ");
        println!("PDF_TEXT_DEBUG {slug} runs={run_count} first={first}");
    }
    for (page, (left, bottom, right, top), runs) in extracted {
        let grouped = group_text_runs(runs);
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            let mut widest: Vec<&TextRun> = grouped.iter().collect();
            widest.sort_by(|a, b| {
                (b.x + b.width)
                    .partial_cmp(&(a.x + a.width))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for line in widest.into_iter().take(4) {
                println!(
                    "WIDEST_DEBUG page={page} x2={:.1} size={:.2} text={}",
                    line.x + line.width,
                    line.size,
                    line.text.trim()
                );
            }
            let mut highest: Vec<&TextRun> = grouped.iter().collect();
            highest.sort_by(|a, b| {
                (b.y + b.size)
                    .partial_cmp(&(a.y + a.size))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for line in highest.into_iter().take(8) {
                println!(
                    "HIGHEST_DEBUG page={page} y2={:.1} x={:.1} size={:.2} text={}",
                    line.y + line.size,
                    line.x,
                    line.size,
                    line.text.trim()
                );
            }
        }
        for line in grouped {
            let text = line.text.trim();
            if !is_schedule_row_line(text) {
                continue;
            }
            let x1 = line.x;
            let x2 = line.x + line.width;
            let y1 = line.y;
            let y2 = line.y + line.size;
            // A continuation row's ink sits inside a taller fixed table/grid
            // row. Comparing only the ink box misses the case the proof is
            // written for: the row starts in the preceding sheet fragment and
            // only its lower text lands just inside the next paper page.
            let row_top = y2 + line.size;
            let mut edges = Vec::new();
            if x1 < left - 0.05 {
                edges.push("left");
            }
            if x2 > right + 0.05 {
                edges.push("right");
            }
            if y1 < bottom - 0.05 {
                edges.push("bottom");
            }
            if row_top > top + 0.05 {
                edges.push("top");
            }
            if !edges.is_empty() {
                lines.push(format!(
                    "{slug} / row {} / page {page} / row text `{text}` / overflow {} (x {x1:.1}..{x2:.1}, y {y1:.1}..{y2:.1}; box x {left:.1}..{right:.1}, y {bottom:.1}..{top:.1})",
                    row_marker(text),
                    edges.join("+")
                ));
            }
        }
    }
    lines
}

#[test]
#[ignore = "proof: renders every schedule-heavy form through Chrome and inspects PDF geometry"]
fn schedule_rows_stay_inside_the_pdf_page_bounds() {
    let frozen_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../html-frozen");
    let mut slugs: Vec<String> = fs::read_dir(&frozen_root)
        .expect("read html-frozen")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            {
                let path = entry.path();
                (path.is_dir() && path.join("writer-cells.json").exists())
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            }
        })
        .collect();
    slugs.sort();
    if let Ok(filter) = std::env::var("PAGE_BOUNDS_SLUGS") {
        let wanted: BTreeSet<String> = filter
            .split(',')
            .map(|slug| slug.trim().to_string())
            .collect();
        slugs.retain(|slug| wanted.contains(slug));
    }
    let work = tempfile::tempdir().expect("proof tempdir");
    let mut all_violations = Vec::new();
    let mut checked = 0usize;
    let mut proof_rows = 0usize;
    let mut reported_rows = 0usize;
    for slug in &slugs {
        let (fields, stem_rows, popup_rows) = fill_proof_fields(slug, "9999999999999.00");
        if stem_rows + popup_rows == 0 {
            continue;
        }
        proof_rows += stem_rows + popup_rows;
        let document = frozen_html::filled_document_with_report(slug, &fields, None)
            .unwrap_or_else(|error| panic!("fill {slug}: {error}"));
        reported_rows += document.overflow.total_overflow_rows();
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            for schedule in &document.overflow.schedules {
                println!(
                    "OVERFLOW_DEBUG {slug} {} rows={} first={:?} last={:?}",
                    schedule.heading,
                    schedule.rows.len(),
                    schedule
                        .rows
                        .first()
                        .map(|row| (row.row_number, row.values.clone())),
                    schedule
                        .rows
                        .last()
                        .map(|row| (row.row_number, row.values.clone()))
                );
            }
        }
        let printed = staticize(&prepare_html_for(slug, &document.html, true));
        let base = work.path().join(slug);
        fs::create_dir_all(&base).expect("proof form dir");
        let html_path = base.join("print.html");
        let pdf_path = base.join("print.pdf");
        let profile_dir = base.join("chrome-profile");
        fs::write(&html_path, printed).expect("write proof html");
        print_pdf(&html_path, &pdf_path, &profile_dir);
        all_violations.extend(bounds_report(slug, &pdf_path));
        let wide = wide_plain_schedules(&document.html, &document.overflow);
        if std::env::var("PAGE_BOUNDS_DEBUG").is_ok() {
            for schedule in &document.overflow.schedules {
                let plain = wide_plain_schedules(&document.html, &document.overflow)
                    .iter()
                    .any(|(heading, _)| heading == &schedule.heading);
                println!(
                    "PLAIN_DEBUG {slug} {} columns={} plain={plain}",
                    schedule.heading,
                    schedule.columns.len()
                );
            }
        }
        if !wide.is_empty() {
            let (measure_fields, _, _) = fill_proof_fields(slug, "9999999.99");
            let measure_document =
                frozen_html::filled_document_with_report(slug, &measure_fields, None)
                    .unwrap_or_else(|error| panic!("fill measurement {slug}: {error}"));
            let measure_printed = staticize(&prepare_html_for(slug, &measure_document.html, true));
            let measure_html_path = base.join("measure.html");
            let measure_pdf_path = base.join("measure.pdf");
            let measure_profile_dir = base.join("measure-chrome-profile");
            fs::write(&measure_html_path, measure_printed).expect("write measurement html");
            print_pdf(&measure_html_path, &measure_pdf_path, &measure_profile_dir);
            let minimum = minimum_numeric_text_size(&measure_pdf_path);
            for (heading, columns) in wide {
                println!(
                    "WIDE_SCHEDULE {slug} / {heading} / columns={columns} / smallest_amount_pt={}",
                    minimum.map_or("unmeasured".to_string(), |size| format!("{size:.2}"))
                );
            }
        }
        checked += 1;
    }
    println!(
        "page-bounds proof: forms={checked}, proof_rows={proof_rows}, continuation_rows={reported_rows}, violations={}",
        all_violations.len()
    );
    for line in &all_violations {
        println!("OVERFLOW {line}");
    }
    assert!(
        all_violations.is_empty(),
        "schedule text escaped the PDF page bounds"
    );
}
