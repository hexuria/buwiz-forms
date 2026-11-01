#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    const APP_ID: &str = "dev.goldcoders.bir";

    #[test]
    fn desktop_entry_identity_matches_file_name_icon_and_window_class() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let entry_path = manifest.join("../../assets/linux/dev.goldcoders.bir.desktop");
        let entry = include_str!("../../../assets/linux/dev.goldcoders.bir.desktop");
        let id = entry_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("desktop entry ID");
        let values: HashMap<&str, &str> = entry
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect();
        let manifest = include_str!("../Cargo.toml");

        assert_eq!(id, APP_ID);
        assert_eq!(values.get("Type"), Some(&"Application"));
        assert_eq!(values.get("Name"), Some(&"eBIRForms"));
        assert_eq!(values.get("Exec"), Some(&"ebirforms"));
        assert_eq!(values.get("Icon"), Some(&id));
        assert_eq!(values.get("StartupWMClass"), Some(&id));
        assert!(
            values
                .get("Categories")
                .is_some_and(|categories| categories.ends_with(';'))
        );
        assert!(manifest.contains(
            r#"["../../assets/linux/dev.goldcoders.bir.desktop", "usr/share/applications/", "644"]"#
        ));
        assert!(manifest.contains(
            r#"["../../assets/linux/icons/hicolor/**/*", "usr/share/icons/hicolor/", "644"]"#
        ));
        assert!(!manifest.contains("ebirforms.desktop"));
    }
}
