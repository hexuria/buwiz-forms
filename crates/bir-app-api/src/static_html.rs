//! Turn the prepared print HTML into fully static,
//! pre-filled markup that any engine (Blitz for the view, WebKit for the PDF)
//! renders identically without JavaScript:
//! - every `<script>` is dropped (the frozen sheet's scripts only re-stamp
//!   overflow rows, wire typing, and draw debug overlays; rules are pre-rendered);
//! - each top-level `.page` is wrapped in a static `<div class="sheet">`
//!   (labelled "Page i of n" for the screen view);
//! - each `<input class="fi …" value="v">` becomes an absolutely positioned
//!   `<span class="fi … fs">v</span>` keeping its id/class/style, centred
//!   vertically like an input and horizontally when the field is `.fc`.

/// CSS for the span stand-ins. `.fi` already gives position:absolute;inset:0
/// plus the font; `fhN` gives size/line-height.
pub const STATIC_CSS: &str = ".fi.fs{display:flex;align-items:center;justify-content:flex-start;\
white-space:pre;overflow:hidden;box-sizing:border-box}.fi.fs.fc{justify-content:center}\
.fi.fs[data-overflow-of]{inset:0.75pt 1pt 0.75pt 1pt}";

pub fn staticize(html: &str) -> String {
    let html = strip_scripts(html);
    let html = inputs_to_spans(&html);
    let html = wrap_sheets(&html);
    let tag = format!("<style id=\"static-fields\">{STATIC_CSS}</style>");
    match html.rfind("</head>") {
        Some(at) => format!("{}{}{}", &html[..at], tag, &html[at..]),
        None => format!("{tag}{html}"),
    }
}

fn strip_scripts(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</script>") {
            Some(end) => rest = &rest[start + end + "</script>".len()..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    let end = tag[at..].find('"')?;
    Some(&tag[at..at + end])
}

fn inputs_to_spans(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<input") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start..start + end + 1];
        rest = &rest[start + end + 1..];
        let class = attr(tag, "class").unwrap_or("");
        if !class.split_whitespace().any(|c| c == "fi") {
            out.push_str(tag); // not a form field: leave it
            continue;
        }
        out.push_str("<span class=\"");
        out.push_str(class);
        out.push_str(" fs\"");
        for a in [
            "id",
            "style",
            "data-slot-index",
            "data-overflow-warning",
            "data-overflow-of",
        ] {
            if let Some(v) = attr(tag, a) {
                out.push_str(&format!(" {a}=\"{v}\""));
            }
        }
        out.push('>');
        // Attribute-escaped text is valid element text (&amp; &quot; &lt;).
        out.push_str(attr(tag, "value").unwrap_or(""));
        out.push_str("</span>");
    }
    out.push_str(rest);
    out
}

/// Top-level pages are siblings `<div class="page …">` directly under body.
fn page_starts(html: &str) -> Vec<(usize, bool)> {
    let mut v = Vec::new();
    let mut from = 0;
    while let Some(i) = html[from..].find("<div class=\"page") {
        let at = from + i;
        let rest = &html[at + "<div class=\"page".len()..];
        if rest.starts_with('"') || rest.starts_with(' ') {
            let cls_end = rest.find('"').unwrap_or(0);
            let cls = &rest[..cls_end];
            v.push((
                at,
                cls.contains("page-receipt") || cls.contains("page-continuation"),
            ));
        }
        from = at + 4;
    }
    v
}

fn wrap_sheets(html: &str) -> String {
    let starts = page_starts(html);
    let n = starts.len();
    if n == 0 {
        return html.to_string();
    }
    let body_end = html.rfind("</body>").unwrap_or(html.len());
    let mut out = String::with_capacity(html.len() + n * 96);
    let mut cur = 0;
    for (i, (at, receipt)) in starts.iter().enumerate() {
        let mut before = &html[cur..*at];
        // A test-send warning is written before its page by bir-print. Move it inside the sheet: the wrapper's
        // ten-point top strip is reserved paper margin, so the warning never covers the form itself.
        let warning = before.ends_with(bir_print::frozen_html::TEST_SEND_WARNING_HTML);
        if warning {
            before = &before[..before.len() - bir_print::frozen_html::TEST_SEND_WARNING_HTML.len()];
        }
        out.push_str(before);
        if i > 0 {
            // close the previous sheet right before this page
            let trimmed = out.trim_end().len();
            out.truncate(trimmed);
            out.push_str("</div>\n");
        }
        out.push_str(&format!(
            "<div class=\"sheet{}\" data-preview-label=\"Page {} of {}\">",
            if *receipt { " sheet-flow" } else { "" },
            i + 1,
            n
        ));
        if warning {
            out.push_str(bir_print::frozen_html::TEST_SEND_WARNING_HTML);
        }
        cur = *at;
    }
    out.push_str(&html[cur..body_end]);
    out.push_str("</div>\n");
    out.push_str(&html[body_end..]);
    out
}

/// Files written by the old path (or `BIR_PREVIEW_HTML`) still carry
/// scripts/inputs; convert those, pass static ones through.
pub fn staticize_if_needed(html: String) -> String {
    if html.contains("<script") || html.contains("<input") {
        staticize(&html)
    } else {
        html
    }
}

/// `(span id, text)` for every field span, in document order.
pub fn field_map(html: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = html[from..].find(" fs\"") {
        let at = from + i;
        from = at + 4;
        let Some(gt) = html[at..].find('>') else {
            break;
        };
        let tag = &html[at..at + gt];
        let padded = format!(" {tag}");
        let Some(id) = attr(&padded, "id") else {
            continue;
        };
        let open = at + gt + 1;
        let Some(close) = html[open..].find("</span>") else {
            break;
        };
        out.push((id.to_string(), html[open..open + close].to_string()));
    }
    out
}

/// The text of `cell` in a static print document: its one span, or its
/// `<cell>-i` input span or `<cell>-s<slot>` comb spans in slot order.
/// Test-only proof helper.
#[cfg(test)]
pub(crate) fn cell_text(html: &str, cell: &str) -> String {
    let prefix = format!("{cell}-s");
    let mut slots: Vec<(usize, String)> = field_map(html)
        .into_iter()
        .filter_map(|(id, text)| {
            if id == cell || id == format!("{cell}-i") {
                return Some((0, text));
            }
            id.strip_prefix(&prefix)
                .and_then(|slot| slot.parse::<usize>().ok())
                .map(|slot| (slot, text))
        })
        .collect();
    slots.sort_by_key(|(slot, _)| *slot);
    slots.into_iter().map(|(_, text)| text).collect()
}

#[cfg(test)]
mod tests {
    const DOC: &str = r##"<html><head></head><body><a class="doc-link" href="guide.html">Guide</a><div class="page page-1"><input type="text" class="fi fh4 fc" id="p1c1-i" style="inset:1pt" value="X"></div><div class="page page-2 page-receipt"><input type="text" class="fi fh22" id="p2c3-s0" data-slot-index="0" value="A&amp;B"></div><script>alert(1)</script></body></html>"##;

    #[test]
    fn inputs_become_positioned_spans_and_scripts_go() {
        let out = super::staticize(DOC);
        assert!(!out.contains("<script"));
        assert!(!out.contains("<input"));
        assert!(
            out.contains(r#"<span class="fi fh4 fc fs" id="p1c1-i" style="inset:1pt">X</span>"#)
        );
        assert!(out.contains(
            r#"<span class="fi fh22 fs" id="p2c3-s0" data-slot-index="0">A&amp;B</span>"#
        ));
        assert!(out.contains(r#"id="static-fields""#));
    }

    #[test]
    fn each_page_gets_one_labelled_sheet() {
        let out = super::staticize(DOC);
        assert_eq!(out.matches(r#"<div class="sheet"#).count(), 2);
        assert!(out.contains(
            r#"<div class="sheet" data-preview-label="Page 1 of 2"><div class="page page-1">"#
        ));
        assert!(out.contains(r#"<div class="sheet sheet-flow" data-preview-label="Page 2 of 2">"#));
        // Balanced: every sheet closes before </body>.
        let body = &out[out.find("<body>").unwrap()..out.find("</body>").unwrap()];
        assert_eq!(body.matches("<div").count(), body.matches("</div>").count());
    }
    #[test]
    fn overflow_spans_keep_marker_without_white_knockout_rule() {
        let doc = r##"<html><head></head><body><div class="page page-1"><input type="text" class="fi" readonly value="LONG TEXT" style="font-size:7.50pt;text-align:left;padding-left:1pt;z-index:1" data-overflow-of="p1c10"><input type="text" class="fi" readonly value="CLONED LONG TEXT" style="font-size:7.50pt;text-align:left;padding-left:1pt;z-index:1" data-overflow-of="cont2-p1c10"></div></body></html>"##;
        let out = super::staticize(doc);
        assert!(out.contains(r#"data-overflow-of="p1c10""#));
        assert!(out.contains(r#"data-overflow-of="cont2-p1c10""#));
        assert!(out.contains(".fi.fs[data-overflow-of]{inset:0.75pt 1pt 0.75pt 1pt}"));
        assert!(!out.contains("background:#fff;"));
        assert!(!out.contains("white-space:pre-line"));
    }

    #[test]
    fn continuation_page_gets_sheet_flow() {
        let doc = r##"<html><head></head><body><div class="page page-1"><input type="text" class="fi" id="p1c1" value="X"></div><div class="page page-continuation" id="page-continuation"><table><tr><td>Row 6</td></tr></table></div></body></html>"##;
        let out = super::staticize(doc);
        assert_eq!(out.matches(r#"<div class="sheet"#).count(), 2);
        assert!(out.contains(r#"<div class="sheet sheet-flow" data-preview-label="Page 2 of 2">"#));
    }
}
