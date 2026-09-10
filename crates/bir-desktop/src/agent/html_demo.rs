//! Host-written demo HTML for `profile.html` / `dues.html`.
//!
//! The invoke result is an **absolute path** to `index.html` in a temp
//! bundle (`theme.css` + fonts). File bytes never go on the wire. Callers
//! must pass already-owned profile/dues fields; this module escapes every
//! text node and never interpolates client-supplied markup.

use std::path::PathBuf;

const THEME_CSS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../html-demo/theme.css"
));
const FONT_ARIMO_NORMAL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../html-frozen/fonts/arimo-latin-wght-normal.woff2"
));
const FONT_ARIMO_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../html-frozen/fonts/arimo-latin-wght-italic.woff2"
));

const HTML_NOTE: &str =
    "Host-written demo HTML. Not a BIR print form. PIN, TOTP, and secrets are omitted.";

#[derive(Debug, Clone)]
pub struct ProfileCard {
    pub full_name: String,
    pub tin: String,
    pub last4: String,
    pub rdo_code: String,
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub taxpayer_type: String,
    pub archived: bool,
    pub year: u16,
    pub form_codes: Vec<String>,
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn write_profile_card(card: &ProfileCard) -> Result<PathBuf, String> {
    write_demo_bundle("Taxpayer identity", &profile_card_body(card))
}

fn write_demo_bundle(title: &str, body: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("bir-agent-demo-{}", uuid::Uuid::new_v4()));
    let fonts = dir.join("fonts");
    std::fs::create_dir_all(&fonts)
        .map_err(|err| format!("could not create agent demo dir: {err}"))?;
    std::fs::write(dir.join("theme.css"), THEME_CSS)
        .map_err(|err| format!("could not write demo theme.css: {err}"))?;
    std::fs::write(
        fonts.join("arimo-latin-wght-normal.woff2"),
        FONT_ARIMO_NORMAL,
    )
    .map_err(|err| format!("could not write demo font: {err}"))?;
    std::fs::write(
        fonts.join("arimo-latin-wght-italic.woff2"),
        FONT_ARIMO_ITALIC,
    )
    .map_err(|err| format!("could not write demo font: {err}"))?;
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<link rel=\"stylesheet\" href=\"theme.css\">\n</head>\n\
         <body>\n{body}\n</body>\n</html>\n",
        title = escape(title),
        body = body
    );
    let path = dir.join("index.html");
    std::fs::write(&path, html).map_err(|err| format!("could not write demo HTML: {err}"))?;
    Ok(path)
}

fn profile_card_body(card: &ProfileCard) -> String {
    let archived = if card.archived {
        r#"<span class="demo-badge is-archived">Archived</span>"#
    } else {
        ""
    };
    let chips = if card.form_codes.is_empty() {
        format!(
            "<p class=\"demo-empty\">No Forms Set stored for {}.</p>",
            escape(&card.year.to_string())
        )
    } else {
        let items: String = card
            .form_codes
            .iter()
            .map(|code| format!("<li class=\"demo-chip\">{}</li>", escape(code)))
            .collect();
        format!("<ul class=\"demo-chips\">{items}</ul>")
    };
    format!(
        "<main class=\"demo-shell\">\n\
         <p class=\"demo-kicker\">Identity card</p>\n\
         <h1 class=\"demo-title\">{name}{archived}</h1>\n\
         <article class=\"demo-card\">\n\
         <dl class=\"demo-dl\">\n\
         <dt>Registered name</dt><dd>{name}</dd>\n\
         <dt>TIN</dt><dd>{tin}</dd>\n\
         <dt>TIN last-4</dt><dd>{last4}</dd>\n\
         <dt>Taxpayer type</dt><dd>{taxpayer_type}</dd>\n\
         <dt>RDO</dt><dd>{rdo}</dd>\n\
         <dt>Line of business</dt><dd>{lob}</dd>\n\
         <dt>Registered address</dt><dd>{address}</dd>\n\
         <dt>ZIP</dt><dd>{zip}</dd>\n\
         <dt>Phone</dt><dd>{phone}</dd>\n\
         <dt>Email</dt><dd>{email}</dd>\n\
         <dt>Forms Set {year}</dt><dd>Active codes for the selected year</dd>\n\
         </dl>\n{chips}\n\
         </article>\n\
         <p class=\"demo-foot\">{note}</p>\n\
         </main>",
        name = escape(&card.full_name),
        archived = archived,
        tin = escape(&card.tin),
        last4 = escape(&card.last4),
        taxpayer_type = escape(&card.taxpayer_type),
        rdo = escape(&card.rdo_code),
        lob = escape(&card.line_of_business),
        address = escape(&card.registered_address),
        zip = escape(&card.zip_code),
        phone = escape(&card.phone),
        email = escape(&card.email),
        year = escape(&card.year.to_string()),
        chips = chips,
        note = HTML_NOTE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_refuses_to_emit_raw_markup() {
        assert_eq!(
            escape("<script>alert('x')</script> & \"q\""),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;q&quot;"
        );
    }

    #[test]
    fn write_profile_card_emits_theme_and_fonts_not_secrets() {
        let path = write_profile_card(&ProfileCard {
            full_name: "Acme <script>".into(),
            tin: "12345678900000".into(),
            last4: "0000".into(),
            rdo_code: "018".into(),
            line_of_business: "Software".into(),
            registered_address: "Olongapo".into(),
            zip_code: "2200".into(),
            phone: "09123456789".into(),
            email: "agent-fixture@example.com".into(),
            taxpayer_type: "Corporation".into(),
            archived: false,
            year: 2026,
            form_codes: vec!["1601C".into()],
        })
        .expect("write");
        assert!(path.is_absolute());
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("index.html")
        );
        let dir = path.parent().expect("dir");
        assert!(dir.join("theme.css").is_file());
        assert!(dir.join("fonts/arimo-latin-wght-normal.woff2").is_file());
        let html = std::fs::read_to_string(&path).expect("html");
        assert!(html.contains("Acme &lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("12345678900000"));
        assert!(html.contains("1601C"));
        assert!(!html.contains("profile_pin"));
        assert!(!html.contains("totp"));
        let theme = std::fs::read_to_string(dir.join("theme.css")).expect("theme");
        assert!(theme.contains("--bir-demo-bg"));
        assert!(!theme.contains(".page { position:relative"));
    }
}
