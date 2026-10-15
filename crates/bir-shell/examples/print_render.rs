//! Render frozen print sheets filled with worst-case data (longest names,
//! full combs, big amounts) through the same pipeline the app uses:
//! fill -> A4 prepare (+ print-overrides/<slug>.css) -> static HTML.
//!
//!   cargo run --example print_render -- [--only <slug>] [--out <dir>] [--no-override]
//!
//! Used by scripts/preview-form.sh to check one form's layout and overrides.
use std::collections::BTreeMap;
/// Field type inferred from writer key + printed caption (+ comb slots).
fn classify(key: &str, printed: &str) -> &'static str {
    let k = key.rsplit(':').next().unwrap_or(key).to_lowercase();
    let c = printed.to_lowercase();
    let has = |w: &str| k.contains(w) || c.contains(w);
    if has("branch") { "branch" }
    else if has("tin") && !has("tint") { "tin" }
    else if has("email") { "email" }
    else if has("telephone") || has("phone") || has("contact no") || has("tel no") { "phone" }
    else if has("zip") { "zip" }
    else if has("rdo") { "rdo" }
    else if has("atc") { "atc" }
    else if has("address") { "address" }
    else if has("date") { "date" }
    else if has("year") { "year" }
    else if has("month") { "month" }
    else if has("quarter") { "quarter" }
    else if has("name") || has("payee") || has("payor") || has("taxpayer") || has("withholding agent") || has("representative") { "name" }
    else if has("rate") || has("%") { "rate" }
    else if has("type code") || has("tax type") || has("code") && !has("amount") { "code" }
    else if has("line of business") || has("nature of") || has("occupation") || has("description") || has("particulars") { "text" }
    else if ["amount","amt","tax","due","income","sales","total","payable","credit","penalt","surcharge","interest","compromise","gross","net","deduct","receipt","value","purchase","withheld","exempt","paid","payment","cost","expense","revenue","refund","excess","balance","price","fee"].iter().any(|w| has(w)) { "amount" }
    else if has("no.") || has("number") || has("code") || k.contains("num") { "code" }
    else { "unclassified" }
}

fn value_for(class: &str, slots: usize) -> String {
    // Comb: exactly one character per box, right type, full length.
    if slots > 0 && class == "amount" {
        // Same shape the app's print maps hand over: plain decimal string.
        return std::env::var("AUDIT_AMOUNT").unwrap_or_else(|_| "1234567.89".into());
    }
    if slots > 0 {
        let digits = matches!(class, "rate" | "tin" | "branch" | "zip" | "rdo" | "date" | "year" | "month" | "quarter" | "amount" | "phone" | "code");
        let pat = if digits { "9876543210" } else { "MWQBXZKHGD" };
        return pat.chars().cycle().take(slots).collect();
    }
    match class {
        "tin" => "123-456-789-00000",
        "branch" => "00000",
        "email" => "maria.delossantos-villanueva@goldcoders-holdings.com.ph",
        "phone" => "(02) 8888-8888 / 0917-123-4567",
        "zip" => "1234",
        "rdo" => "050",
        "atc" => "WC158",
        "address" => "Unit 4512 Tower B, The Residences at Greenbelt, 123 Esperanza St. cor. Makati Ave., Brgy. San Lorenzo, Makati City, Metro Manila",
        "date" => "12/31/2026",
        "year" => "2026",
        "month" => "12",
        "quarter" => "4",
        "name" => "MARIA CONSOLACION DE LOS SANTOS-VILLANUEVA Y GUTIERREZ",
        "text" => "WHOLESALE AND RETAIL OF COMPUTER HARDWARE AND SOFTWARE",
        "amount" => "999,999,999.99",
        "rate" => "12.50%",
        "code" => "1234567890",
        _ => "AB12",
    }
    .to_string()
}

/// Comb slot count per name= in the frozen sheet (0 = free text input).
fn slot_counts(html: &str) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for tag in html.split("<input").skip(1) {
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        let attr = |a: &str| tag.find(&format!(" {a}=\"")).map(|i| { let r = &tag[i + a.len() + 3..]; r[..r.find('"').unwrap_or(0)].to_string() });
        if let Some(n) = attr("name") {
            let e = m.entry(n).or_insert(0usize);
            if attr("data-slot-index").is_some() { *e += 1; }
        }
    }
    m
}

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let args: Vec<String> = std::env::args().collect();
    let arg = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
    let frozen = arg("--frozen").map(std::path::PathBuf::from).unwrap_or(root.join("html-frozen"));
    let out = arg("--out").map(std::path::PathBuf::from).unwrap_or(root.join("target/print-render"));
    let only = arg("--only");
    let apply_override = !args.iter().any(|a| a == "--no-override");
    std::fs::create_dir_all(&out).unwrap();
    let mut ok = 0;
    for e in std::fs::read_dir(&frozen).unwrap() {
        let dir = e.unwrap().path();
        let wc = dir.join("writer-cells.json");
        if !wc.exists() { continue; }
        let slug = dir.file_name().unwrap().to_string_lossy().to_string();
        if only.as_ref().is_some_and(|o| o != &slug) { continue; }
        let cells: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&wc).unwrap()).unwrap();
        let s = |j: &serde_json::Value, k: &str| j[k].as_str().unwrap_or("").to_string();
        let sheet = std::fs::read_to_string(dir.join("index.html")).unwrap_or_default();
        let slots = slot_counts(&sheet);
        let mut f = BTreeMap::new();
        for j in cells["joins"].as_array().into_iter().flatten() {
            let k = s(j, "writer_key");
            let n = slots.get(&s(j, "html_id")).copied().unwrap_or(0);
            f.insert(k.clone(), value_for(classify(&k, &s(j, "printed")), n));
        }
        for j in cells["money_joins"].as_array().into_iter().flatten() { f.insert(s(j, "writer_key"), "999999999.99".into()); }
        for j in cells["peso_joins"].as_array().into_iter().flatten() { f.insert(s(j, "writer_key"), "999999999".into()); }
        for j in cells["date_joins"].as_array().into_iter().flatten() { f.insert(s(j, "writer_key"), "12/31/2026".into()); }
        for j in cells["xbox_joins"].as_array().into_iter().flatten() { f.insert(s(j, "writer_key"), "true".into()); }
        match bir_print::frozen_html::filled_document(&slug, &f) {
            Ok(html) => {
                let html = bir_shell::static_html::staticize(&bir_shell::print_sheet::prepare_html_for(&slug, &html, apply_override));
                std::fs::write(out.join(format!("{slug}.html")), html).unwrap();
                ok += 1;
            }
            Err(err) => eprintln!("SKIP {slug}: {err}"),
        }
    }
    println!("rendered {ok} into {}", out.display());
}
