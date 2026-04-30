use bir_print::{render_flat_pdf, PrintRequest};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let mut fields = BTreeMap::new();
    fields.insert("frm2551Qv2018:forThe_1".to_string(), "1".to_string());
    fields.insert("frm2551Qv2018:registeredName".to_string(), "TEST COMPANY".to_string());
    
    let req = PrintRequest {
        form_id: "2551Qv2018".to_string(),
        fields,
        output_dir: PathBuf::from("tmp/test_print"),
        formtypes_dir: Some(PathBuf::from("formtypes")),
    };
    
    match render_flat_pdf(req) {
        Ok(res) => println!("Success! PDF at {:?}", res.pdf_path),
        Err(e) => println!("Error: {:?}", e),
    }
}
