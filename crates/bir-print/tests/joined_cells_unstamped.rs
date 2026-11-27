//! No input inside a joined cell carries a name other than its cell's id.
//!
//! The engine fills a joined cell (`frozen_html::joined_cell_ids`: the
//! sheet's writer-cells joins plus the engine's own joins, `SHEET_JOINS` and
//! the donor and estate joins) through its join, and it also fills any input
//! whose name is a filing key by that name. An input of a joined cell
//! stamped with a filing key would be filled twice, so every input inside a
//! joined cell must be named after the cell.
//!
//! buwiz-forms-html's freeze refuses such stamps too, but it reads only the
//! writer-cells.json joins. This test covers every join the engine knows,
//! including the ones that file never lists (2550M's TIN cells are
//! `SHEET_JOINS`) and any the product adds later. Band rows kept in a
//! `<template>` are not the sheet's own cells and are skipped, as there.

use std::path::PathBuf;

fn frozen_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../html-frozen")
}

/// The value of attribute `name` itself, not of one whose name ends in it
/// (`id` against `data-cell-id`).
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let mut from = 0;
    while let Some(rel) = tag[from..].find(&needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && !tag[..at].ends_with([' ', '\t', '\n']) {
            continue;
        }
        let rest = &tag[from..];
        return rest.find('"').map(|end| &rest[..end]);
    }
    None
}

/// `html` without its `<template>` elements.
fn without_templates(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find("<template") {
        out.push_str(&rest[..open]);
        match rest[open..].find("</template>") {
            Some(close) => rest = &rest[open + close + "</template>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Each joined cell of `html` with the names of the inputs inside it (up to
/// the next cell's tag): `(cell id, input names)`.
fn joined_cell_inputs(
    html: &str,
    joined: &std::collections::BTreeSet<String>,
) -> Vec<(String, Vec<String>)> {
    let html = without_templates(html);
    let mut tags = Vec::new();
    let mut from = 0;
    while let Some(rel) = html[from..].find("<div ") {
        let start = from + rel;
        let Some(len) = html[start..].find('>') else {
            break;
        };
        let end = start + len + 1;
        let tag = &html[start..end];
        if tag.contains("data-cell-kind=") {
            tags.push((start, end, attr(tag, "id").map(str::to_string)));
        }
        from = end;
    }
    let mut out = Vec::new();
    for (at, (_, end, id)) in tags.iter().enumerate() {
        let Some(id) = id.as_ref().filter(|id| joined.contains(*id)) else {
            continue;
        };
        let stop = tags.get(at + 1).map_or(html.len(), |next| next.0);
        let body = &html[*end..stop];
        let mut names = Vec::new();
        let mut from = 0;
        while let Some(rel) = body[from..].find("<input") {
            let start = from + rel;
            let len = body[start..].find('>').unwrap_or(body.len() - start);
            if let Some(name) = attr(&body[start..start + len], "name") {
                names.push(name.to_string());
            }
            from = start + len;
        }
        out.push((id.clone(), names));
    }
    out
}

/// Every input of a joined cell of sheet `slug` (its markup `html`) named
/// other than the cell, as `"<cell>: input named <name>"`.
fn stamped_joined_inputs(slug: &str, html: &str) -> Vec<String> {
    let joined = bir_print::frozen_html::joined_cell_ids(slug);
    joined_cell_inputs(html, &joined)
        .into_iter()
        .flat_map(|(cell, names)| {
            names
                .into_iter()
                .filter(|name| *name != cell)
                .map(|name| format!("{cell}: input named {name:?}"))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn sheets() -> Vec<String> {
    let mut slugs: Vec<String> = std::fs::read_dir(frozen_root())
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .join("index.html")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    slugs.sort();
    slugs
}

fn sheet_html(slug: &str) -> String {
    std::fs::read_to_string(frozen_root().join(slug).join("index.html")).unwrap()
}

#[test]
fn no_input_of_a_joined_cell_is_named_other_than_its_cell() {
    let slugs = sheets();
    assert_eq!(slugs.len(), 43, "{slugs:?}");
    let (mut cells, mut inputs, mut stamped) = (0, 0, Vec::new());
    for slug in &slugs {
        let html = sheet_html(slug);
        let joined = bir_print::frozen_html::joined_cell_ids(slug);
        for (_, names) in joined_cell_inputs(&html, &joined) {
            cells += 1;
            inputs += names.len();
        }
        stamped.extend(
            stamped_joined_inputs(slug, &html)
                .into_iter()
                .map(|p| format!("{slug} {p}")),
        );
    }
    eprintln!("joined cells checked: {cells}, their inputs: {inputs}");
    assert!(cells > 0 && inputs > 0, "no joined cell found to check");
    assert!(
        stamped.is_empty(),
        "inputs of joined cells carry another name than their cell \
         (the engine would fill them twice) ({}):\n{}",
        stamped.len(),
        stamped.join("\n")
    );
}

/// A stamp put back on 2550M's TIN (1-3) cell, a `SHEET_JOINS` cell that the
/// sheet's writer-cells.json does not list (so buwiz-forms-html's refusal
/// cannot see it), fails; the real sheet passes.
#[test]
fn a_stamped_input_in_a_sheet_joins_cell_fails() {
    let slug = "2550m-2007";
    let html = sheet_html(slug);
    let writer_cells =
        std::fs::read_to_string(frozen_root().join(slug).join("writer-cells.json")).unwrap();
    assert!(
        !writer_cells.contains("\"p1c127\""),
        "p1c127 is now in writer-cells.json; pick a cell only the engine joins"
    );
    assert!(bir_print::frozen_html::joined_cell_ids(slug).contains("p1c127"));
    assert_eq!(stamped_joined_inputs(slug, &html), Vec::<String>::new());

    let unstamped = "id=\"p1c127-s0\" name=\"p1c127\"";
    assert_eq!(html.matches(unstamped).count(), 1);
    let stamped = html.replacen(unstamped, "id=\"p1c127-s0\" name=\"frm2550m:txtTIN1\"", 1);
    assert_eq!(
        stamped_joined_inputs(slug, &stamped),
        vec!["p1c127: input named \"frm2550m:txtTIN1\"".to_string()]
    );
}
