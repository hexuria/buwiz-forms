//! Every sheet's filled print document is self-contained in what decides how
//! it looks: no stylesheet link the inliner must have consumed survives in any
//! attribute order, and no image still points at `../assets/`. A correction
//! round-trip once reordered one sheet's link attributes
//! (`<link href=... rel="stylesheet"/>`), escaping the exact-byte inlining and
//! printing 2200T unstyled; this test is the guard against any recurrence.
use bir_print::frozen_html::filled_document;
use std::collections::BTreeMap;

fn sheet_slugs() -> Vec<String> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../html-frozen");
    let mut slugs: Vec<String> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .join("writer-cells.json")
                .exists()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    slugs.sort();
    slugs
}

#[test]
fn filled_documents_reference_nothing_outside_themselves() {
    let slugs = sheet_slugs();
    assert_eq!(slugs.len(), 43, "the frozen corpus is 43 sheets");
    let mut offenders = Vec::new();
    for slug in &slugs {
        let doc = filled_document(slug, &BTreeMap::new()).unwrap();
        // A stylesheet link must never survive: inlining replaces it, whatever
        // its attribute order or self-closing.
        let mut scan = 0;
        while let Some(at) = doc[scan..].find("<link").map(|i| scan + i) {
            let end = doc[at..].find('>').map(|i| at + i + 1).unwrap_or(doc.len());
            let tag = &doc[at..end];
            if tag.contains("stylesheet") {
                offenders.push(format!("{slug}: stylesheet link survives: {tag}"));
            }
            scan = end;
        }
        for needle in [
            "../assets/",
            "href=\"form.css\"",
            "href='form.css'",
            "href=\"../base.css\"",
            "href='../base.css'",
        ] {
            if doc.contains(needle) {
                offenders.push(format!("{slug}: {needle}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "external references remain:\n{}",
        offenders.join("\n")
    );
}
