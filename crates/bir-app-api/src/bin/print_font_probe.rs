use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rows = bir_app_api::print_fonts::probe_all(&root).expect("print font probe");
    let below: Vec<_> = rows
        .iter()
        .filter(|row| row.effective_pt + 0.005 < bir_app_api::print_fonts::MIN_PRINTED_PT)
        .collect();
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
    for slug in bir_app_api::print_fonts::FLOOR_SLUGS {
        let own: Vec<_> = rows.iter().filter(|row| row.slug == *slug).collect();
        let low = own
            .iter()
            .filter(|row| row.effective_pt + 0.005 < bir_app_api::print_fonts::MIN_PRINTED_PT)
            .count();
        let min = own
            .first()
            .map(|row| row.effective_pt)
            .unwrap_or(f64::INFINITY);
        eprintln!(
            "{slug}: minimum {min:.2}pt effective; {low} below 7.5pt of {} filled values",
            own.len()
        );
    }
    if !below.is_empty() {
        eprintln!("{} filled values print below 7.5pt effective", below.len());
        std::process::exit(1);
    }
}
