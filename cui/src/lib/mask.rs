//! Masks for typed numbers (port of `nsh/ui/mask.ts`): pure format and parse functions. The form
//! page's boxes call them on every edit; screens call format_money / format_tin / parse_typed_date
//! to show stored values.
//!
//! The caret rule (reconcile): an edit is found by comparing the text before it with the text after
//! it. CUI's input does not report selections, so `before` is always None here and the common
//! prefix/suffix guess applies.
//!
//! Money never goes through f64: sign, digits and decimals are handled as strings.

use crate::lib::dates::Ymd;

/// A masked text and where its caret goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Masked {
    pub text: String,
    pub caret: usize,
    /// The edit was refused: text and caret are as they were before it.
    pub rejected: bool,
}

/// What a mask does with the significant characters it is given.
pub struct MaskResult {
    /// keep[i]: the i-th significant character of the input is in the output.
    pub keep: Vec<bool>,
    /// The formatted text.
    pub text: String,
}

/// A mask: which characters count, and how the counted ones are formatted.
pub struct Mask {
    pub significant: fn(char) -> bool,
    pub format: Box<dyn Fn(&str) -> MaskResult>,
    /// Looks at the text an edit inserted before the mask does: the text to take in its place, or
    /// None to refuse the whole edit.
    pub insert: Option<Box<dyn Fn(&str, bool) -> Option<String>>>,
}

pub fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

/// Only the digits of a text: "000-123 453" -> "000123453".
pub fn digits_only(text: &str) -> String {
    text.chars().filter(|c| is_digit(*c)).collect()
}

fn sig_of(text: &str, mask: &Mask) -> String {
    text.chars().filter(|c| (mask.significant)(*c)).collect()
}

/// Position in `text` just after its `count`-th significant character (0: the start).
fn caret_after(text: &str, count: usize, mask: &Mask) -> usize {
    if count == 0 {
        return 0;
    }
    let mut seen = 0;
    for (i, ch) in text.char_indices() {
        if (mask.significant)(ch) {
            seen += 1;
            if seen == count {
                return i + ch.len_utf8();
            }
        }
    }
    text.len()
}

/// What an edit did to the text before it: `removed` characters from `at` became `ins`.
#[derive(Debug)]
struct Edit {
    at: usize,
    removed: usize,
    ins: String,
    dir: Dir,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Dir {
    Back,
    Forward,
}

/// The selection the edit started from, in `prev` (byte offsets, as the rest of this module's
/// indices; NSH's TextSelection is UTF-16 units — masked texts are ASCII so they coincide).
type Selection = (usize, usize);

fn find_edit(prev: &str, next: &str, caret: Option<usize>, before: Option<Selection>) -> Edit {
    let pb = prev.as_bytes();
    let nb = next.as_bytes();
    // The selection the edit started from says exactly what it did, when the texts agree with it.
    if let Some((a0, b0)) = before {
        let (a, b) = (a0.min(b0), a0.max(b0));
        let bounds_ok = a <= b
            && b <= prev.len()
            && prev.is_char_boundary(a)
            && prev.is_char_boundary(b)
            && next.is_char_boundary(a);
        if bounds_ok {
            let grew = next.len() as isize - (prev.len() - (b - a)) as isize;
            if grew >= 0 {
                let g = grew as usize;
                if next.len() >= a + g
                    && next.is_char_boundary(a + g)
                    && next[..a] == prev[..a]
                    && next[a + g..] == prev[b..]
                {
                    return Edit {
                        at: a,
                        removed: b - a,
                        ins: next[a..a + g].to_owned(),
                        dir: Dir::Back,
                    };
                }
            }
            if a == b && grew < 0 {
                // Characters went and none came, from a caret: Backspace took the ones before it,
                // Delete the ones after it.
                let n = (-grew) as usize;
                if a >= n
                    && prev.is_char_boundary(a - n)
                    && next == format!("{}{}", &prev[..a - n], &prev[a..])
                {
                    return Edit {
                        at: a - n,
                        removed: n,
                        ins: String::new(),
                        dir: Dir::Back,
                    };
                }
                if a + n <= prev.len()
                    && prev.is_char_boundary(a + n)
                    && next == format!("{}{}", &prev[..a], &prev[a + n..])
                {
                    return Edit {
                        at: a,
                        removed: n,
                        ins: String::new(),
                        dir: Dir::Forward,
                    };
                }
            }
        }
    }
    // The guess: what is left after the common prefix and the common suffix.
    let mut p = 0;
    while p < pb.len() && p < nb.len() && pb[p] == nb[p] {
        p += 1;
    }
    let mut s = 0;
    while s < pb.len() - p && s < nb.len() - p && pb[pb.len() - 1 - s] == nb[nb.len() - 1 - s] {
        s += 1;
    }
    // The caret after the edit settles where an inserted run of equal characters began.
    if let Some(caret) = caret {
        if caret <= nb.len() {
            let tail = nb.len() - caret;
            if tail <= pb.len() && pb[pb.len() - tail..] == nb[caret..] {
                let j = pb.len() - tail;
                let at = p.min(j).min(caret);
                return Edit {
                    at,
                    removed: j - at,
                    ins: next[at..caret].to_owned(),
                    dir: Dir::Back,
                };
            }
        }
    }
    Edit {
        at: p,
        removed: pb.len() - p - s,
        ins: next[p..next.len() - s].to_owned(),
        dir: Dir::Back,
    }
}

/// The masked result of an edit: `prev` is the text the field showed, `next` the text after the
/// user's edit. `caret`, when known, is the raw caret in `next`; `before`, when known, is the
/// selection the edit started from (in `prev`), which makes the edit exact; `typed` marks
/// key-press origin.
pub fn reconcile(
    prev: &str,
    next: &str,
    mask: &Mask,
    caret: Option<usize>,
    before: Option<Selection>,
    typed: bool,
) -> Masked {
    let e = find_edit(prev, next, caret, before);
    let mut ins = e.ins.clone();
    if !ins.is_empty() {
        if let Some(ref f) = mask.insert {
            match f(&ins, typed) {
                None => {
                    return Masked {
                        text: prev.to_owned(),
                        caret: e.at,
                        rejected: true,
                    };
                }
                Some(taken) => {
                    if taken != ins {
                        let mut nn = String::with_capacity(next.len());
                        nn.push_str(&next[..e.at]);
                        nn.push_str(&taken);
                        nn.push_str(&next[e.at + ins.len()..]);
                        ins = taken;
                        // `next` is rebound below via raw.
                        return reconcile_inner(prev, &nn, mask, e, ins);
                    }
                }
            }
        }
    }
    reconcile_inner(prev, next, mask, e, ins)
}

fn reconcile_inner(prev: &str, next: &str, mask: &Mask, e: Edit, ins: String) -> Masked {
    let mut raw = next.to_owned();
    let mut raw_caret = e.at + ins.len();
    // Separators deleted alone: delete the significant character beside them instead.
    if ins.is_empty() && e.removed > 0 && sig_of(&prev[e.at..e.at + e.removed], mask).is_empty() {
        if e.dir == Dir::Forward {
            let mut i = e.at;
            let rb = raw.as_bytes();
            while i < rb.len() && !(mask.significant)(rb[i] as char) {
                i += 1;
            }
            if i < rb.len() {
                raw = format!("{}{}", &raw[..i], &raw[i + 1..]);
                raw_caret = e.at;
            }
        } else {
            let nb = next.as_bytes();
            let mut i = e.at as isize - 1;
            while i >= 0 && !(mask.significant)(nb[i as usize] as char) {
                i -= 1;
            }
            if i >= 0 {
                let i = i as usize;
                raw = format!("{}{}", &raw[..i], &raw[i + 1..]);
                raw_caret = i;
            }
        }
    }
    let sig = sig_of(&raw, mask);
    let mut count = 0;
    for (i, ch) in raw.char_indices() {
        if i >= raw_caret {
            break;
        }
        if (mask.significant)(ch) {
            count += 1;
        }
    }
    let out = (mask.format)(&sig);
    let mut kept = 0;
    for (i, k) in out.keep.iter().enumerate() {
        if i >= count {
            break;
        }
        if *k {
            kept += 1;
        }
    }
    let caret = caret_after(&out.text, kept, mask);
    Masked {
        text: out.text,
        caret,
        rejected: false,
    }
}

/// A whole text through a mask, as a paste into an empty field: the caret at the end.
pub fn apply_mask(text: &str, mask: &Mask) -> String {
    (mask.format)(&sig_of(text, mask)).text
}

// ---------------------------------------------------------------- TIN

/// The TIN's printed groups: 3-3-3 for the taxpayer, 5 for the branch.
pub const TIN_GROUPS: [usize; 4] = [3, 3, 3, 5];
/// Digits in a full TIN with its branch code.
pub const TIN_LENGTH: usize = 14;

/// Digits as the printed TIN while typing: "0001234" -> "000-123-4"; no trailing dash, at most 14 digits.
pub fn format_tin_digits(digits: &str) -> String {
    let d = digits_only(digits);
    let d: String = d.chars().take(TIN_LENGTH).collect();
    let mut parts: Vec<String> = Vec::new();
    let mut at = 0;
    for &n in &TIN_GROUPS {
        if at >= d.len() {
            break;
        }
        parts.push(d[at..(at + n).min(d.len())].to_owned());
        at += n;
    }
    parts.join("-")
}

fn tin_format(sig: &str) -> MaskResult {
    let keep: Vec<bool> = sig
        .chars()
        .enumerate()
        .map(|(i, _)| i < TIN_LENGTH)
        .collect();
    MaskResult {
        keep,
        text: format_tin_digits(sig),
    }
}

/// The TIN mask: digits only, grouped 3-3-3-5 with dashes, leading zeros kept.
pub fn tin_mask() -> Mask {
    Mask {
        significant: is_digit,
        format: Box::new(tin_format),
        insert: None,
    }
}

/// A TIN as printed. With a `branch` code the root (its first 9 digits) is followed by the code as
/// written: "000-123-453-00000".
pub fn format_tin(root: &str, branch: Option<&str>) -> String {
    let r = digits_only(root);
    match branch {
        Some(b) => format!("{}-{}", format_tin_digits(&r[..9.min(r.len())]), b),
        None => format_tin_digits(&r),
    }
}

// ---------------------------------------------------------------- amounts (strings only)

/// Thousands separators on a run of digits: "1250000" -> "1,250,000".
pub fn group_thousands(int: &str) -> String {
    let digits: String = int.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A plain amount's parts: its sign, whole digits and decimals; None when the text is not an amount.
fn amount_parts(text: &str) -> Option<(bool, String, String)> {
    let cleaned: String = text
        .chars()
        .filter(|c| !matches!(c, '₱' | ',' | ' ' | '\t' | '\n' | '\r'))
        .collect();
    let cleaned = cleaned.trim();
    let (neg, rest) = match cleaned.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, cleaned),
    };
    let (int, frac) = match rest.split_once('.') {
        Some((i, f)) => (i.to_owned(), f.to_owned()),
        None => (rest.to_owned(), String::new()),
    };
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    Some((neg, int, frac))
}

/// An amount's one spelling: no separators, no leading or trailing zeros, no "-0"; None when it is
/// not an amount.
fn canonical_amount(text: &str) -> Option<String> {
    let (neg, int, frac) = amount_parts(text)?;
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    if int.is_empty() && frac.is_empty() {
        return Some("0".to_owned());
    }
    Some(format!(
        "{}{}{}",
        if neg { "-" } else { "" },
        if int.is_empty() { "0" } else { int },
        if frac.is_empty() {
            String::new()
        } else {
            format!(".{frac}")
        }
    ))
}

/// Whether a stored text is a plain decimal amount.
pub fn is_plain_amount(text: &str) -> bool {
    canonical_amount(text).is_some()
}

/// Two amounts are the same number; anything that is not an amount is the same only when equal.
pub fn same_amount(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match canonical_amount(a) {
        Some(x) => Some(x) == canonical_amount(b),
        None => false,
    }
}

/// The sign and separators of text inserted into a money box.
fn amount_insert(negative: bool) -> impl Fn(&str, bool) -> Option<String> {
    move |inserted: &str, typed: bool| {
        let t = inserted.replace('\u{2212}', "-");
        if typed {
            return Some(if negative { t } else { t.replace('-', "") });
        }
        // Accounting negatives: "(1,234.00)".
        let t = {
            let trimmed = t.trim();
            // "(1,234.00)" and "₱ (1,234.00)" both read as negative.
            let inner = trimmed.trim_start_matches('₱').trim_start();
            if inner.starts_with('(') && inner.ends_with(')') {
                let body = &inner[1..inner.len() - 1];
                if body.chars().any(|c| c.is_ascii_digit()) {
                    format!("-{body}")
                } else {
                    t.clone()
                }
            } else {
                t.clone()
            }
        };
        if t.trim_start().starts_with([' ', '₱', '-'].as_ref())
            && t.trim_start_matches([' ', '₱']).starts_with('-')
            && !negative
        {
            return None;
        }
        if t.trim_start_matches([' ', '₱']).starts_with('-') && !negative {
            return None;
        }
        amount_separators(&t)
    }
}

/// Separators in a pasted amount, when it has two or more of them.
fn amount_separators(t: &str) -> Option<String> {
    let seps: Vec<char> = t.chars().filter(|c| *c == '.' || *c == ',').collect();
    if seps.len() < 2 {
        return Some(t.to_owned());
    }
    let dots = seps.iter().filter(|c| **c == '.').count();
    let commas = seps.len() - dots;
    if dots > 0 && commas > 0 {
        let last_dot = t.rfind('.');
        let last_comma = t.rfind(',');
        let last = last_dot.max(last_comma).unwrap();
        let point = t[last..].chars().next().unwrap();
        let other = if point == '.' { ',' } else { '.' };
        let after: String = t[last + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let after_len = after.len();
        let point_count = if point == '.' { dots } else { commas };
        let other_last = if other == '.' { last_dot } else { last_comma };
        if point_count != 1 || !(1..=2).contains(&after_len) || other_last.is_some_and(|i| i > last)
        {
            return None;
        }
        let body: String = t.chars().filter(|c| *c != other).collect();
        return Some(if point == '.' {
            body
        } else {
            body.replacen(',', ".", 1)
        });
    }
    let sep = if dots > 0 { '.' } else { ',' };
    let parts: Vec<&str> = t.split(sep).collect();
    let head = digits_only(parts[0]).len();
    if !(1..=3).contains(&head) || !parts[1..].iter().all(|p| digits_only(p).len() == 3) {
        return None;
    }
    Some(parts.concat())
}

/// The money mask while typing: digits, one decimal point and a leading minus; thousands separators
/// on the whole part.
pub fn money_mask(negative: bool) -> Mask {
    Mask {
        significant: |ch| is_digit(ch) || ch == '.' || ch == '-',
        format: Box::new(move |sig: &str| money_format(sig)),
        insert: Some(Box::new(move |i, t| amount_insert(negative)(i, t))),
    }
}

fn money_format(sig: &str) -> MaskResult {
    let chars: Vec<char> = sig.chars().collect();
    let mut keep = vec![false; chars.len()];
    let mut neg = false;
    let mut int = String::new();
    let mut int_at: Vec<usize> = Vec::new();
    let mut dot = false;
    let mut frac = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '-' {
            if i == 0 {
                neg = true;
                keep[i] = true;
            }
        } else if ch == '.' {
            if !dot {
                dot = true;
                keep[i] = true;
            }
        } else if dot {
            frac.push(ch);
            keep[i] = true;
        } else {
            int.push(ch);
            int_at.push(i);
            keep[i] = true;
        }
    }
    // Leading zeros of the whole part: one stays while it is all zeros.
    let mut z = 0;
    while z < int.len().saturating_sub(1) && int.as_bytes()[z] == b'0' {
        z += 1;
    }
    for &k in int_at.iter().take(z) {
        keep[k] = false;
    }
    let int = &int[z.min(int.len())..];
    let text = format!(
        "{}{}{}",
        if neg { "-" } else { "" },
        group_thousands(int),
        if dot {
            format!(".{frac}")
        } else {
            String::new()
        }
    );
    MaskResult { keep, text }
}

/// The money mask with no minus to type.
pub fn money_mask_plain() -> Mask {
    money_mask(false)
}

/// The masks the form page picks per field kind (screens/form/controls).
pub fn mask_for(kind: &str, negative_ok: bool, max: Option<usize>) -> Option<Mask> {
    match kind {
        "money" => Some(money_mask(negative_ok)),
        "pct" => Some(percent_mask()),
        "int" => Some(int_mask(max)),
        _ => None,
    }
}

/// Whether a number box shows an amount of zero.
pub fn is_zero_amount(text: &str) -> bool {
    let t = text;
    t.chars().any(|c| c.is_ascii_digit())
        && t.chars()
            .all(|c| matches!(c, '₱' | ' ' | ',' | '.' | '-' | '0'))
}

/// The plain numeric string of a money text: "1,250,000.50" -> "1250000.50", "₱ -12" -> "-12".
pub fn money_plain(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '₱' | ',' | ' ' | '\t' | '\n' | '\r'))
        .collect()
}

/// The digits of `int`.`frac` rounded to `places` decimals, half up, on the digits themselves.
fn round_decimals(int: &str, frac: &str, places: usize) -> (String, String) {
    if frac.len() <= places {
        let mut f = frac.to_owned();
        while f.len() < places {
            f.push('0');
        }
        return (int.to_owned(), f);
    }
    let mut digits: Vec<u8> = int.bytes().chain(frac[..places].bytes()).collect();
    if frac.as_bytes()[places] >= b'5' {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let cut = digits.len() - places;
    (
        String::from_utf8(digits[..cut].to_vec()).unwrap(),
        String::from_utf8(digits[cut..].to_vec()).unwrap(),
    )
}

/// "1,234.50" from an amount, written on the digits themselves. `whole`: whole pesos; a typed
/// fraction stays as typed. `round` false: decimals past the second stay as they are.
pub fn format_money_full(text: &str, whole: bool, round: bool) -> String {
    let raw = text;
    let Some((neg, int_raw, frac)) = amount_parts(raw) else {
        return raw.to_owned();
    };
    if raw.trim().is_empty() {
        return raw.to_owned();
    }
    if whole && frac.chars().any(|c| c != '0') {
        return raw.to_owned();
    }
    let int_raw = int_raw.trim_start_matches('0');
    let int_raw = if int_raw.is_empty() { "0" } else { int_raw };
    let (int, frac) = if !whole && (round || frac.len() <= 2) {
        round_decimals(int_raw, &frac, 2)
    } else if !whole {
        (int_raw.to_owned(), frac)
    } else {
        (int_raw.to_owned(), frac)
    };
    let neg = neg && (int.chars().chain(frac.chars()).any(|c| c != '0'));
    let grouped = format!("{}{}", if neg { "-" } else { "" }, group_thousands(&int));
    if whole {
        grouped
    } else {
        format!("{grouped}.{frac}")
    }
}

/// What a money box shows for a value, never another number than the value.
pub fn shown_money(value: &str, focused: bool, whole: bool) -> String {
    if !is_plain_amount(value) {
        return value.to_owned();
    }
    if focused {
        apply_mask(value, &money_mask(true))
    } else {
        format_money_full(value, whole, false)
    }
}

// ---------------------------------------------------------------- percent and integers

fn no_minus(inserted: &str, typed: bool) -> Option<String> {
    let t = inserted.trim_start();
    if !typed && (t.starts_with('-') || t.starts_with('\u{2212}')) {
        return None;
    }
    Some(inserted.to_owned())
}

fn percent_insert(inserted: &str, typed: bool) -> Option<String> {
    if !typed && inserted.matches('.').count() > 1 {
        return None;
    }
    no_minus(inserted, typed)
}

fn percent_format(sig: &str) -> MaskResult {
    let mut dot = false;
    let mut keep = Vec::new();
    let mut text = String::new();
    for ch in sig.chars() {
        if ch != '.' {
            keep.push(true);
            text.push(ch);
        } else if !dot {
            dot = true;
            keep.push(true);
            text.push(ch);
        } else {
            keep.push(false);
        }
    }
    MaskResult { keep, text }
}

/// Percent while typing: digits and one decimal point, no grouping.
pub fn percent_mask() -> Mask {
    Mask {
        significant: |ch| is_digit(ch) || ch == '.',
        format: Box::new(percent_format),
        insert: Some(Box::new(percent_insert)),
    }
}

fn int_insert(inserted: &str, typed: bool) -> Option<String> {
    no_minus(inserted, typed)
}

/// Whole numbers while typing: digits only, at most `max` of them.
pub fn int_mask(max: Option<usize>) -> Mask {
    Mask {
        significant: is_digit,
        format: Box::new(move |sig: &str| {
            let keep: Vec<bool> = sig
                .chars()
                .enumerate()
                .map(|(i, _)| max.is_none() || i < max.unwrap())
                .collect();
            let text: String = sig
                .chars()
                .enumerate()
                .filter(|(i, _)| keep[*i])
                .map(|(_, c)| c)
                .collect();
            MaskResult { keep, text }
        }),
        insert: Some(Box::new(int_insert)),
    }
}

/// A calendar day (re-export of dates::Ymd for mask users).
pub type MaskYmd = Ymd;

/// The form's MM/DD/YYYY text: "12/01/2020".
pub fn format_mdy(date: Ymd) -> String {
    crate::lib::dates::format_mdy(date)
}

/// An ISO day, "2020-12-01", as the calendar speaks.
pub fn format_iso(date: Ymd) -> String {
    format!("{:04}-{:02}-{:02}", date.y, date.m, date.d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::dates;

    fn money() -> Mask {
        money_mask(false)
    }
    fn neg() -> Mask {
        money_mask(true)
    }
    fn pct() -> Mask {
        percent_mask()
    }
    fn int() -> Mask {
        int_mask(None)
    }
    fn int4() -> Mask {
        int_mask(Some(4))
    }
    fn tin() -> Mask {
        tin_mask()
    }

    fn rec(prev: &str, next: &str, m: &Mask) -> Masked {
        reconcile(prev, next, m, None, None, false)
    }
    fn rec_typed(prev: &str, next: &str, m: &Mask) -> Masked {
        reconcile(prev, next, m, None, None, true)
    }
    fn fm(t: &str) -> String {
        format_money_full(t, false, true)
    }
    fn fm_w(t: &str, whole: bool) -> String {
        format_money_full(t, whole, true)
    }
    fn fm_nr(t: &str, whole: bool, round: bool) -> String {
        format_money_full(t, whole, round)
    }

    #[test]
    fn tin_groups() {
        assert_eq!(format_tin_digits(""), "");
        assert_eq!(format_tin_digits("0"), "0");
        assert_eq!(format_tin_digits("000"), "000");
        assert_eq!(format_tin_digits("0001"), "000-1");
        assert_eq!(format_tin_digits("000123453"), "000-123-453");
        assert_eq!(format_tin_digits("0001234530"), "000-123-453-0");
        assert_eq!(format_tin_digits("00012345300000"), "000-123-453-00000");
        assert_eq!(format_tin_digits("00000000000000"), "000-000-000-00000");
        assert_eq!(format_tin_digits("0001234530000099"), "000-123-453-00000");
        assert_eq!(apply_mask("000", &tin()), "000");
        assert_eq!(apply_mask("000-123-453-00000", &tin()), "000-123-453-00000");
        assert_eq!(apply_mask(" 000 123 453 ", &tin()), "000-123-453");
        let m = rec("", "000-123-453-00000", &tin());
        assert_eq!(
            (m.text.as_str(), m.caret, m.rejected),
            ("000-123-453-00000", 17, false)
        );
        assert_eq!(digits_only("TIN: 000-123-453"), "000123453");
    }

    #[test]
    fn tin_caret() {
        let m = rec("000", "0001", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("000-1", 5));
        let m = rec("000-12", "000-123", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("000-123", 7));
        let m = rec("000-123", "000-1234", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("000-123-4", 9));
        let m = rec("000-123", "000-1923", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("000-192-3", 6));
        let m = rec("000-123", "000-13", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("000-13", 5));
        let m = rec("000-123", "000123", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("001-23", 2));
        let m = rec("000-123", "000-456123", &tin());
        assert_eq!((m.text.as_str(), m.caret), ("000-456-123", 7));
    }

    #[test]
    fn tin_from_root_and_branch() {
        assert_eq!(format_tin("000123453", Some("00000")), "000-123-453-00000");
        assert_eq!(format_tin("000123453", None), "000-123-453");
        assert_eq!(format_tin("0001234530", None), "000-123-453-0");
        assert_eq!(format_tin("00012345300000", None), "000-123-453-00000");
        assert_eq!(format_tin("0001234", None), "000-123-4");
        assert_eq!(format_tin("1234", None), "123-4");
        assert_eq!(format_tin("000-123-453-0000", None), "000-123-453-0000");
        assert_eq!(format_tin("", None), "");
        assert_eq!(
            format_tin("00012345300000", Some("00001")),
            "000-123-453-00001"
        );
    }

    #[test]
    fn money_groups() {
        assert_eq!(apply_mask("1250000", &money()), "1,250,000");
        assert_eq!(apply_mask("1250000.5", &money()), "1,250,000.5");
        assert_eq!(apply_mask("1,250,000.50", &money()), "1,250,000.50");
        assert_eq!(
            apply_mask("123456789012345", &money()),
            "123,456,789,012,345"
        );
        assert_eq!(apply_mask("12.345", &money()), "12.345");
        assert_eq!(apply_mask("", &money()), "");
        assert_eq!(apply_mask("1.2.3", &money()), "1.23");
        assert_eq!(apply_mask("12a3", &money()), "123");
        assert_eq!(apply_mask("007", &money()), "7");
        assert_eq!(apply_mask("0", &money()), "0");
        assert_eq!(apply_mask("0.5", &money()), "0.5");
        assert_eq!(apply_mask("000.5", &money()), "0.5");
        assert_eq!(apply_mask(".5", &money()), ".5");
        assert_eq!(apply_mask("₱ 1,000", &money()), "1,000");
        assert_eq!(apply_mask("-1500", &money()), "-1,500");
        assert_eq!(apply_mask("-250.00", &money()), "-250.00");
        assert_eq!(apply_mask("15-00", &money()), "1,500");
        let n = neg();
        assert_eq!(apply_mask("-1500", &n), "-1,500");
        assert_eq!(apply_mask("15-00", &n), "1,500");
        assert_eq!(apply_mask("--15", &n), "-15");
    }

    #[test]
    fn money_caret() {
        let m = rec("123", "1234", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234", 5));
        let m = rec("1,234", "1,2345", &money());
        assert_eq!((m.text.as_str(), m.caret), ("12,345", 6));
        let m = rec("1,234", "1,2934", &money());
        assert_eq!((m.text.as_str(), m.caret), ("12,934", 4));
        let m = rec("1,234", "1,23", &money());
        assert_eq!((m.text.as_str(), m.caret), ("123", 3));
        let m = rec("1,234", "1234", &money());
        assert_eq!((m.text.as_str(), m.caret), ("234", 0));
        let m = rec("0", "05", &money());
        assert_eq!((m.text.as_str(), m.caret), ("5", 1));
        let m = rec("", "1,250,000.00", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,250,000.00", 12));
    }

    #[test]
    fn money_plain_and_format() {
        assert_eq!(money_plain("1,250,000.50"), "1250000.50");
        assert_eq!(money_plain("₱ -12"), "-12");
        assert_eq!(money_plain(""), "");
        assert_eq!(fm("1250000"), "1,250,000.00");
        assert_eq!(fm("1234.5"), "1,234.50");
        assert_eq!(fm("-1234.5"), "-1,234.50");
        assert_eq!(fm("1,234"), "1,234.00");
        assert_eq!(fm_w("1234", true), "1,234");
        assert_eq!(fm_w("1234.5", true), "1234.5");
        assert_eq!(fm(""), "");
        assert_eq!(fm("abc"), "abc");
        assert_eq!(fm("0"), "0.00");
    }

    #[test]
    fn percent_and_int() {
        assert_eq!(apply_mask("12.5", &pct()), "12.5");
        assert_eq!(apply_mask("1.2.5%", &pct()), "1.25");
        assert_eq!(apply_mask("0012", &int()), "0012");
        assert_eq!(apply_mask("12345", &int4()), "1234");
        assert_eq!(apply_mask("1a2b", &int()), "12");
        let m = rec("12", "1x2", &int());
        assert_eq!((m.text.as_str(), m.caret), ("12", 1));
    }

    #[test]
    fn dates_parse() {
        let dec1 = Ymd {
            y: 2020,
            m: 12,
            d: 1,
        };
        assert_eq!(dates::parse_typed_date("12012020"), Some(dec1));
        assert_eq!(dates::parse_typed_date("12/01/2020"), Some(dec1));
        assert_eq!(dates::parse_typed_date("12-01-2020"), Some(dec1));
        assert_eq!(dates::parse_typed_date("2020-12-01"), Some(dec1));
        assert_eq!(dates::parse_typed_date("Dec 01 2020"), Some(dec1));
        assert_eq!(dates::parse_typed_date("December 1, 2020"), Some(dec1));
        assert_eq!(dates::parse_typed_date("02/30/2020"), None);
        assert_eq!(
            dates::parse_typed_date("02/29/2020"),
            Some(Ymd {
                y: 2020,
                m: 2,
                d: 29
            })
        );
        assert_eq!(dates::parse_typed_date("13/01/2020"), None);
        assert_eq!(dates::parse_typed_date(""), None);
        assert_eq!(dates::parse_typed_date("12/01"), None);
        assert_eq!(dates::format_mdy(dec1), "12/01/2020");
        assert_eq!(dates::format_long_date(dec1), "Dec 1, 2020");
        assert_eq!(format_iso(dec1), "2020-12-01");
        assert_eq!(dates::shown_date("12/01/2020"), "Dec 1, 2020");
        assert_eq!(dates::shown_date("12/0"), "12/0");
        assert!(dates::DATE_ERROR.contains("MM/DD/YYYY"));
    }

    #[test]
    fn money_rounding_on_digits() {
        assert_eq!(fm("1.255"), "1.26");
        assert_eq!(fm("2.675"), "2.68");
        assert_eq!(fm("1.005"), "1.01");
        assert_eq!(fm("1.004"), "1.00");
        assert_eq!(fm("0.995"), "1.00");
        assert_eq!(fm("999.995"), "1,000.00");
        assert_eq!(fm("1234.5678"), "1,234.57");
        assert_eq!(fm("-1.255"), "-1.26");
        assert_eq!(fm("-2.675"), "-2.68");
        assert_eq!(fm(".5"), "0.50");
        assert_eq!(fm("5."), "5.00");
        assert_eq!(fm("007.5"), "7.50");
    }

    #[test]
    fn money_no_infinity_and_big_digits() {
        assert_eq!(fm("-0"), "0.00");
        assert_eq!(fm("-0.00"), "0.00");
        assert_eq!(fm("-0.001"), "0.00");
        assert_eq!(fm("Infinity"), "Infinity");
        assert_eq!(fm("-Infinity"), "-Infinity");
        assert_eq!(fm("NaN"), "NaN");
        assert_eq!(fm("1e21"), "1e21");
        assert_eq!(fm("1.2.3"), "1.2.3");
        assert_eq!(fm("-"), "-");
        assert_eq!(fm("."), ".");
        assert_eq!(fm("123456789012345678"), "123,456,789,012,345,678.00");
        assert_eq!(fm("9007199254740993.01"), "9,007,199,254,740,993.01");
        assert_eq!(
            fm("12345678901234567890.125"),
            "12,345,678,901,234,567,890.13"
        );
        assert_eq!(
            apply_mask("123456789012345678.99", &money()),
            "123,456,789,012,345,678.99"
        );
        assert_eq!(fm_w("1234.00", true), "1,234");
        assert_eq!(fm_w("1234.", true), "1,234");
        assert_eq!(fm_w("-0", true), "0");
        assert_eq!(fm_w("1234.5", true), "1234.5");
    }

    #[test]
    fn shown_money_cases() {
        assert_eq!(shown_money("-250", false, false), "-250.00");
        assert_eq!(shown_money("-250", true, false), "-250");
        assert_eq!(shown_money("1250000.5", true, false), "1,250,000.5");
        assert_eq!(shown_money("1250000.5", false, false), "1,250,000.50");
        assert_eq!(shown_money("1234.5", false, true), "1234.5");
        assert_eq!(shown_money("-0", false, false), "0.00");
        assert_eq!(shown_money("", false, false), "");
        assert_eq!(shown_money("1e21", false, false), "1e21");
        assert_eq!(shown_money("1e21", true, false), "1e21");
        assert_eq!(shown_money("Infinity", false, false), "Infinity");
        assert_eq!(shown_money("abc", true, false), "abc");
        assert!(is_plain_amount("1,250.50"));
        assert!(!is_plain_amount("1e21"));
        assert!(!is_plain_amount(""));
    }

    #[test]
    fn same_amount_cases() {
        assert!(same_amount("1250.5", "1,250.50"));
        assert!(same_amount("0", "-0.00"));
        assert!(same_amount(".5", "0.50"));
        assert!(same_amount("", ""));
        assert!(!same_amount("1.255", "1.26"));
        assert!(!same_amount("12345678901234567891", "12345678901234567890"));
        assert!(same_amount("abc", "abc"));
        assert!(!same_amount("abc", "abd"));
    }

    #[test]
    fn minus_refused_unless_allowed() {
        for (prev, next) in [("12", "-12"), ("12", "1-2")] {
            let m = rec(prev, next, &money());
            assert!(m.rejected);
            assert_eq!(m.text, prev);
        }
        for next in ["-1,234.00", "₱ -1,234.00", "(1,234.00)", "\u{2212}1,234.00"] {
            let m = rec("", next, &money());
            assert!(m.rejected, "{next}");
            assert_eq!(m.text, "");
        }
        let m = rec("-250", "-2500", &money());
        assert_eq!((m.text.as_str(), m.caret), ("-2,500", 6));
        let n = neg();
        let m = rec("", "-1,234.00", &n);
        assert_eq!((m.text.as_str(), m.caret), ("-1,234.00", 9));
        let m = rec("", "(1,234.00)", &n);
        assert_eq!((m.text.as_str(), m.caret), ("-1,234.00", 9));
        let m = rec("", "\u{2212}1,234.00", &n);
        assert_eq!((m.text.as_str(), m.caret), ("-1,234.00", 9));
        let m = rec("", "₱ (1,234.00)", &n);
        assert_eq!((m.text.as_str(), m.caret), ("-1,234.00", 9));
        let m = rec("12", "-12", &n);
        assert_eq!((m.text.as_str(), m.caret), ("-12", 1));
    }

    #[test]
    fn percent_int_refuse_minus() {
        let m = rec("12", "-12", &pct());
        assert!(m.rejected && m.text == "12");
        let m = rec("", "-5", &int());
        assert!(m.rejected && m.text == "");
        let m = rec("", "1.234.567", &pct());
        assert!(m.rejected && m.text == "");
        let m = rec("12", "123", &pct());
        assert_eq!((m.text.as_str(), m.caret), ("123", 3));
        let m = rec("12", "12.5", &pct());
        assert_eq!((m.text.as_str(), m.caret), ("12.5", 4));
    }

    #[test]
    fn separator_paste_rules() {
        let m = rec("", "1.234.567", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234,567", 9));
        let m = rec("", "1,234,567", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234,567", 9));
        let m = rec("", "1.234,56", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234.56", 8));
        let m = rec("", "1,234.56", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234.56", 8));
        let m = rec("", "1.234.567,8", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234,567.8", 11));
        let m = rec("", "1,234,567.89", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1,234,567.89", 12));
        for ambiguous in [
            "1.2.3",
            "1,234.567",
            "1.234,567",
            "12,34,567",
            "1,2,3",
            "1.234.56",
            "1,234,56",
        ] {
            let m = rec("", ambiguous, &money());
            assert!(m.rejected, "{ambiguous}");
            assert_eq!(m.text, "");
        }
        let m = rec("1.5", "1.5.", &money());
        assert_eq!((m.text.as_str(), m.caret), ("1.5", 3));
    }

    #[test]
    fn typed_vs_pasted() {
        let n = neg();
        let m = rec_typed("", "-5", &money());
        assert_eq!((m.text.as_str(), m.caret), ("5", 1));
        let m = rec_typed("12", "-12", &money());
        assert_eq!((m.text.as_str(), m.caret), ("12", 0));
        let m = rec_typed("12", "-12", &n);
        assert_eq!((m.text.as_str(), m.caret), ("-12", 1));
        let m = rec_typed("12", "-12", &pct());
        assert_eq!((m.text.as_str(), m.caret), ("12", 0));
        let m = rec_typed("12,345", "1,234,56", &money());
        assert_eq!((m.text.as_str(), m.caret), ("123,456", 7));
        let m = rec("12,345", "1,234,56", &money());
        assert!(m.rejected && m.text == "12,345" && m.caret == 1);
    }

    #[test]
    fn stored_value_layout() {
        assert_eq!(fm_nr("1.255", false, false), "1.255");
        assert_eq!(fm_nr("1234.5678", false, false), "1,234.5678");
        assert_eq!(fm_nr("1.5", false, false), "1.50");
        assert_eq!(fm_nr("-0.001", false, false), "-0.001");
        assert_eq!(shown_money("1.255", false, false), "1.255");
        assert_eq!(shown_money("1234.5678", false, false), "1,234.5678");
        assert_eq!(shown_money("1234.5", false, false), "1,234.50");
    }

    #[test]
    fn zero_amount() {
        for z in ["0", "00", "0.00", "0.0", ".0", "-0.00", "₱ 0", "0,000.00"] {
            assert!(is_zero_amount(z), "{z}");
        }
        for o in [
            "", " ", "abc", "1e21", "0.01", "10", "0.00.1", ".", "-", "₱", "1,250.00", "Infinity",
        ] {
            assert!(!is_zero_amount(o), "{o}");
        }
    }

    // ------------------------------------------------------------ selections (mask.test.ts)
    //
    // NSH reports the selection the edit started from; reconcile's `before` makes the edit exact:
    // Delete against Backspace on a separator, and a digit typed beside an equal one, cannot be
    // told apart from the two texts alone.

    fn rec_sel(prev: &str, next: &str, m: &Mask, before: (usize, usize), typed: bool) -> Masked {
        reconcile(prev, next, m, None, Some(before), typed)
    }
    fn tc(m: &Masked) -> (&str, usize) {
        (m.text.as_str(), m.caret)
    }

    #[test]
    fn selection_delete_vs_backspace_on_a_separator() {
        // Without the selection the two keys leave the same texts: the guess stays Backspace.
        assert_eq!(tc(&rec("1,234", "1234", &money())), ("234", 0));
        // Backspace on "1,|234".
        assert_eq!(
            tc(&rec_sel("1,234", "1234", &money(), (2, 2), false)),
            ("234", 0)
        );
        // Delete on "1|,234": was "234".
        assert_eq!(
            tc(&rec_sel("1,234", "1234", &money(), (1, 1), false)),
            ("134", 1)
        );
        // TIN: Delete on "000|-123" takes the 1, Backspace on "000-|123" the last 0.
        assert_eq!(
            tc(&rec_sel("000-123", "000123", &tin(), (3, 3), false)),
            ("000-23", 3)
        );
        assert_eq!(
            tc(&rec_sel("000-123", "000123", &tin(), (4, 4), false)),
            ("001-23", 2)
        );
        // A digit deleted either way still goes, whichever side.
        assert_eq!(
            tc(&rec_sel("1,234", "1,24", &money(), (3, 3), false)),
            ("124", 2)
        );
        assert_eq!(
            tc(&rec_sel("1,234", "1,34", &money(), (3, 3), false)),
            ("134", 1)
        );
        // A selection replaced by a typed digit, and the whole text selected.
        assert_eq!(tc(&rec_sel("1,234", "9", &money(), (0, 5), true)), ("9", 1));
        assert_eq!(
            tc(&rec_sel("1,234", "1,9", &money(), (2, 5), true)),
            ("19", 2)
        );
    }

    #[test]
    fn selection_typed_digit_beside_an_equal_one() {
        // "000-1|23" + "2": the texts alone say the 2 went in before the 3, or after the other 2.
        assert_eq!(
            tc(&rec_sel("000-123", "000-1223", &tin(), (5, 5), true)),
            ("000-122-3", 6)
        );
        assert_eq!(tc(&rec("000-123", "000-1223", &tin())), ("000-122-3", 7));
        // The other reading, "000-12|3" + "2", is the guess without a hint and exact with one.
        assert_eq!(
            tc(&rec_sel("000-123", "000-1223", &tin(), (6, 6), true)),
            ("000-122-3", 7)
        );
        // "000-0|00" + "0": the caret stays after the typed 0 (was the end of the text).
        assert_eq!(
            tc(&rec_sel("000-000", "000-0000", &tin(), (5, 5), true)),
            ("000-000-0", 6)
        );
        // "00|0-000" + "0".
        assert_eq!(
            tc(&rec_sel("000-000", "0000-000", &tin(), (2, 2), true)),
            ("000-000-0", 3)
        );
        // Money: "1,2|34" + "3" and "1,23|4" + "3".
        assert_eq!(
            tc(&rec_sel("1,234", "1,2334", &money(), (3, 3), true)),
            ("12,334", 4)
        );
        assert_eq!(
            tc(&rec_sel("1,234", "1,2334", &money(), (4, 4), true)),
            ("12,334", 5)
        );
    }

    #[test]
    fn selection_that_does_not_fit_is_ignored() {
        assert_eq!(
            tc(&rec_sel("123", "1234", &money(), (1, 1), false)),
            ("1,234", 5)
        );
        assert_eq!(
            tc(&rec_sel("123", "1234", &money(), (9, 9), false)),
            ("1,234", 5)
        );
        assert_eq!(
            tc(&rec_sel(
                "123",
                "1234",
                &money(),
                (usize::MAX, usize::MAX),
                false
            )),
            ("1,234", 5)
        );
        assert_eq!(tc(&rec("1,234", "1,2345", &money())), ("12,345", 6));
    }

    #[test]
    fn selection_whole_text_and_ranges() {
        // The whole text selected and deleted leaves nothing, whichever end it was made from.
        for (m, text) in [
            (money(), "1,250,000.00"),
            (neg(), "-1,250.50"),
            (pct(), "12.5"),
            (int4(), "0123"),
            (tin(), "000-123-453-00000"),
        ] {
            assert_eq!(
                tc(&rec_sel(text, "", &m, (0, text.len()), false)),
                ("", 0),
                "{text}"
            );
            assert_eq!(
                tc(&rec_sel(text, "", &m, (text.len(), 0), false)),
                ("", 0),
                "{text}"
            );
            assert_eq!(tc(&rec(text, "", &m)), ("", 0), "{text}");
        }
        // "1,2[50,0]00.00": the range is 3..7.
        assert_eq!(
            tc(&rec_sel(
                "1,250,000.00",
                "1,200.00",
                &money(),
                (3, 7),
                false
            )),
            ("1,200.00", 3)
        );
        // "[1,2]50,000": the first three characters.
        assert_eq!(
            tc(&rec_sel("1,250,000", "50,000", &money(), (0, 3), false)),
            ("50,000", 0)
        );
        // TIN: "000[-123-]453" leaves the digits to regroup: "000-453", caret after the third digit.
        assert_eq!(
            tc(&rec_sel("000-123-453", "000453", &tin(), (3, 8), false)),
            ("000-453", 3)
        );
        // Percent and integer boxes have no separators to cross.
        assert_eq!(tc(&rec_sel("12.5", "15", &pct(), (1, 3), false)), ("15", 1));
        assert_eq!(
            tc(&rec_sel("0123", "01", &int4(), (2, 4), false)),
            ("01", 2)
        );
    }

    #[test]
    fn selection_digit_over_range_and_delete_over_caret() {
        // Money "[1,2]50,000" + "9": the typed key is the digit.
        assert_eq!(
            tc(&rec_sel("1,250,000", "950,000", &money(), (0, 3), true)),
            ("950,000", 1)
        );
        // The whole amount selected and a digit typed: that digit alone.
        assert_eq!(
            tc(&rec_sel("1,250.00", "7", &money(), (0, 8), true)),
            ("7", 1)
        );
        assert_eq!(tc(&rec_sel("0.00", "5", &money(), (0, 4), true)), ("5", 1));
        assert_eq!(tc(&rec_sel("0", "5", &pct(), (0, 1), true)), ("5", 1));
        assert_eq!(tc(&rec_sel("0", "42", &int4(), (0, 1), true)), ("42", 2));
        // "0.[0]0" + "0" is the same text, the caret after the typed digit.
        assert_eq!(
            tc(&rec_sel("0.00", "0.00", &money(), (2, 3), true)),
            ("0.00", 3)
        );
        // A letter over a selection is taken back: the range is gone and nothing replaces it.
        assert_eq!(tc(&rec_sel("12.5", "x", &pct(), (0, 4), true)), ("", 0));
        // A paste over a selection is judged as a paste: a refused one leaves the box as it was.
        let m = rec_sel("1,250.00", "-5", &money(), (0, 8), false);
        assert!(m.rejected && m.text == "1,250.00" && m.caret == 0);
        assert_eq!(
            tc(&rec_sel("1,250.00", "2,500.75", &money(), (0, 8), false)),
            ("2,500.75", 8)
        );
        // Over a range, forward and back are one edit.
        assert_eq!(
            tc(&rec_sel("1,250,000", "250,000", &money(), (0, 2), false)),
            ("250,000", 0)
        );
        // Over a caret, the digit after it: "1|,250" Delete.
        assert_eq!(
            tc(&rec_sel("1,250", "1250", &money(), (1, 1), false)),
            ("150", 1)
        );
        // The last character, from a caret at the end, is Backspace.
        assert_eq!(
            tc(&rec_sel("1,250", "1,25", &money(), (5, 5), false)),
            ("125", 3)
        );
        // A Cut of everything.
        assert_eq!(tc(&rec_sel("12.5", "", &pct(), (0, 4), false)), ("", 0));
    }
}
