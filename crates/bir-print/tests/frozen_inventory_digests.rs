//! html-frozen/inventory.json records each sheet as buwiz-forms-html has it.
//!
//! Each bundle's `index_sha256` is the sha256 of its sheet's `index.html`
//! with the product's `<script type="application/json"
//! id="formgen-page-layout">` block removed (the one part the html
//! repository does not carry; its `check_product_sync.py` compares the two
//! trees the same way). The two repositories' inventories are then equal.
//!
//! After a sheet changes, `BIR_PRINT_BLESS_INVENTORY=1 cargo test -p
//! bir-print --test frozen_inventory_digests` rewrites the stale digests in
//! place (only the digest strings, so the file stays byte-comparable with
//! the html repository's).

use sha2::{Digest, Sha256};
use std::path::PathBuf;

const PAGE_LAYOUT_OPEN: &str = "<script type=\"application/json\" id=\"formgen-page-layout\">";

fn frozen_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../html-frozen")
}

/// `html` without its formgen-page-layout blocks (and the newline after
/// each), as buwiz-forms-html's `without_page_layout`.
fn without_page_layout(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find(PAGE_LAYOUT_OPEN) {
        let Some(close) = rest[open..].find("</script>") else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &rest[open + close + "</script>".len()..];
        rest = rest.strip_prefix('\n').unwrap_or(rest);
    }
    out.push_str(rest);
    out
}

/// The inventory digest of sheet `slug`'s `index.html`.
fn sheet_digest(slug: &str) -> String {
    let html = std::fs::read_to_string(frozen_root().join(slug).join("index.html")).unwrap();
    format!(
        "{:x}",
        Sha256::digest(without_page_layout(&html).as_bytes())
    )
}

#[test]
fn every_inventory_digest_is_its_sheet_without_the_page_layout() {
    let path = frozen_root().join("inventory.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let inventory: serde_json::Value = serde_json::from_str(&text).unwrap();
    let bundles = inventory["bundles"].as_array().unwrap();
    assert_eq!(bundles.len(), 43);
    let mut stale = Vec::new();
    for bundle in bundles {
        let slug = bundle["frozen_slug"].as_str().unwrap();
        let recorded = bundle["index_sha256"].as_str().unwrap();
        let now = sheet_digest(slug);
        if recorded != now {
            stale.push((slug.to_string(), recorded.to_string(), now));
        }
    }
    if std::env::var_os("BIR_PRINT_BLESS_INVENTORY").is_some() {
        // Blessing only ever rewrites; it never passes: a run that inherits the switch (in CI or
        // anywhere) must not count as proof that the inventory was right.
        assert!(
            std::env::var_os("CI").is_none(),
            "BIR_PRINT_BLESS_INVENTORY is refused under CI: it rewrites inventory.json instead of checking it"
        );
        let mut blessed = text.clone();
        for (slug, recorded, now) in &stale {
            assert_eq!(blessed.matches(recorded.as_str()).count(), 1, "{slug}");
            blessed = blessed.replace(recorded.as_str(), now);
        }
        std::fs::write(&path, blessed).unwrap();
        panic!(
            "inventory digests rewritten ({}): re-run without BIR_PRINT_BLESS_INVENTORY to check them",
            stale.len()
        );
    }
    assert!(
        stale.is_empty(),
        "inventory digests not their sheets' ({}; BIR_PRINT_BLESS_INVENTORY=1 rewrites them):\n{}",
        stale.len(),
        stale
            .iter()
            .map(|(slug, recorded, now)| format!("{slug}: recorded {recorded}, sheet {now}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_page_layout_block_alone_is_left_out() {
    let sheet = format!("<p>a</p>\n{PAGE_LAYOUT_OPEN}{{\"pages\":[]}}</script>\n<p>b</p>\n");
    assert_eq!(without_page_layout(&sheet), "<p>a</p>\n<p>b</p>\n");
    let other = "<script type=\"application/json\" id=\"formgen-bands\">[]</script>\n";
    assert_eq!(without_page_layout(other), other);
}
