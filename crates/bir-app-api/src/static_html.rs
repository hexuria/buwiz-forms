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
white-space:pre;overflow:hidden;box-sizing:border-box}.fi.fs.fc{justify-content:center}";

pub fn staticize(html: &str) -> String {
    staticize_with_furniture(html, None)
}

/// What every printed page carries outside the official drawing: the footer
/// line (form, TIN, period, filing state) and, while the return is unfiled,
/// the diagonal DRAFT mark. Built once per print from the stored return's
/// own fields; `left` is plain text and is escaped here.
pub struct SheetFurniture {
    pub left: String,
    pub draft_mark: bool,
}

/// The mark's exact wording, decided by the owner.
pub const DRAFT_MARK_TEXT: &str = "DRAFT – not filed";

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn staticize_with_furniture(html: &str, furniture: Option<&SheetFurniture>) -> String {
    let html = strip_scripts(html);
    let html = inputs_to_spans(&html);
    let html = wrap_sheets(&html, furniture);
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
        for a in ["id", "style", "data-slot-index"] {
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
            v.push((at, rest[..cls_end].contains("page-receipt")));
        }
        from = at + 4;
    }
    v
}

fn wrap_sheets(html: &str, furniture: Option<&SheetFurniture>) -> String {
    let starts = page_starts(html);
    let n = starts.len();
    if n == 0 {
        return html.to_string();
    }
    let body_end = html.rfind("</body>").unwrap_or(html.len());
    let mut out = String::with_capacity(html.len() + n * 96);
    let mut cur = 0;
    for (i, (at, receipt)) in starts.iter().enumerate() {
        out.push_str(&html[cur..*at]);
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
        if let Some(furniture) = furniture {
            out.push_str(&format!(
                "<div class=\"print-foot\"><span class=\"pf-left\">{}</span>\
<span class=\"pf-page\">Page {} of {}</span></div>",
                escape_html(&furniture.left),
                i + 1,
                n
            ));
            if furniture.draft_mark {
                out.push_str("<div class=\"print-draft\">");
                out.push_str(DRAFT_MARK_TEXT);
                out.push_str("</div>");
            }
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
    fn furniture_lands_inside_every_sheet_with_its_page_of_pages() {
        let furniture = super::SheetFurniture {
            left: "BIR Form 2551Q · TIN 000-123-453-00000 · Q3 2025".to_string(),
            draft_mark: true,
        };
        let out = super::staticize_with_furniture(DOC, Some(&furniture));
        assert_eq!(out.matches(r#"<div class="print-foot">"#).count(), 2);
        assert_eq!(out.matches(r#"<div class="print-draft">"#).count(), 2);
        assert!(out.contains(">DRAFT – not filed</div>"));
        assert!(out.contains(r#"<span class="pf-page">Page 1 of 2</span>"#));
        assert!(out.contains(r#"<span class="pf-page">Page 2 of 2</span>"#));
        // The furniture sits inside its sheet, before the official page.
        let first = out.find(r#"data-preview-label="Page 1 of 2">"#).unwrap();
        let foot = out.find(r#"<div class="print-foot">"#).unwrap();
        let page = out.find(r#"<div class="page page-1">"#).unwrap();
        assert!(first < foot && foot < page);
        // Balanced: furniture adds two divs per sheet, each closed inline.
        let body = &out[out.find("<body>").unwrap()..out.find("</body>").unwrap()];
        assert_eq!(body.matches("<div").count(), body.matches("</div>").count());
    }

    #[test]
    fn furniture_escapes_text_and_a_filed_return_marks_nothing() {
        let furniture = super::SheetFurniture {
            left: "BIR Form 1701 · TIN 000-123-453-00000 · 2025 · Filed 11/20/2025 · Ref a&b<.xml"
                .to_string(),
            draft_mark: false,
        };
        let out = super::staticize_with_furniture(DOC, Some(&furniture));
        assert!(out.contains("Ref a&amp;b&lt;.xml"));
        assert!(!out.contains("print-draft"));
        assert_eq!(out.matches(r#"<div class="print-foot">"#).count(), 2);
    }

    #[test]
    fn no_furniture_keeps_the_sheet_bare() {
        let out = super::staticize(DOC);
        assert!(!out.contains("print-foot"));
        assert!(!out.contains("print-draft"));
    }
}
