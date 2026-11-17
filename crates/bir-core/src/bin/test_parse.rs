fn main() {
    let raw = r#"File name: 261708015000-2551Qv2018-122026Q1.xml
Date received by BIR: 26 April 2026
Time received by BIR: 02:43 PM
Penalties may be imposed for any violation of the provisions of the NIRC and issuances thereof.
"#;
    match bir_core::receipt::parse_bir_receipt_email(raw, None) {
        Ok(res) => println!("Success: {:?}", res),
        Err(e) => println!("Error: {:?}", e),
    }
}
