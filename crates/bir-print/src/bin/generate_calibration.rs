use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use bir_print::{PrintRequest, render_flat_pdf, formtype::FormType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let form_id = "2551Qv2018";
    
    // Find the project root correctly
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let formtypes_dir = workspace_root.join("formtypes");
    let target_dir = formtypes_dir.join(form_id);
    
    let temp_dir = std::env::temp_dir().join("typst_calib_gen");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let layout_str = fs::read_to_string(target_dir.join("formtype.json"))?;
    let formtype: FormType = serde_json::from_str(&layout_str)?;

    let mut fields = BTreeMap::new();
    for field in formtype.fields {
        fields.insert(field.key, "X".to_string());
    }

    println!("Rendering flat PDF to generate Typst code...");
    let req = PrintRequest::new(form_id, fields, &temp_dir)
        .with_formtypes_dir(&formtypes_dir);
    
    let result = render_flat_pdf(req)?;
    
    let generated_typ = result.typ_path;
    let target_typ = target_dir.join("calibration.typ");
    
    // Read and fix SVG paths: the generator writes "pages/" but the source uses "pages/" too,
    // so this should be consistent now. Just copy.
    println!("Copying generated.typ to {:?}", target_typ);
    fs::copy(&generated_typ, &target_typ)?;
    
    println!("Successfully generated calibration file at formtypes/{}/calibration.typ", form_id);
    
    Ok(())
}
