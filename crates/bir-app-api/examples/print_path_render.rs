//! Print-path render: staticize(prepare_html_for(slug, &filled_document(..), true))
//! usage: print_path_render <slug> <out.html> key=value [key=value...]
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let slug = &args[1];
    let out = &args[2];
    let mut fields = BTreeMap::new();
    for pair in &args[3..] {
        let (k, v) = pair.split_once('=').expect("key=value");
        fields.insert(k.to_string(), v.to_string());
    }
    let filled = bir_print::frozen_html::filled_document(slug, &fields).expect("filled_document");
    let prepared = bir_app_api::print_sheet::prepare_html_for(slug, &filled, true);
    let staticized = bir_app_api::static_html::staticize(&prepared);
    std::fs::write(out, staticized).expect("write");
    println!("wrote {out}");
}
