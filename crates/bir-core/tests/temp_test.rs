use bir_core::db::Database;
#[test]
fn dump_profiles() {
    let db_path = bir_core::platform::data_dir().join("bir_data.db");
    if let Ok(mut db) = Database::open(&db_path) {
        if let Ok(profiles) = db.list_profiles() {
            for p in profiles {
                println!(
                    "Profile {}: Type: {:?}, Class: {:?}",
                    p.full_name, p.taxpayer_type, p.tax_classification
                );
            }
        }
    }
}
