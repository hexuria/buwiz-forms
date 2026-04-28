use bir_core::{bir_xml, crypto, transport};
use chrono::Local;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let xml_path = "../savefile/010558054000-2551Qv2018-122026Q1.xml";
    let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";

    println!("Reading and decrypting source IAF from: {}", xml_path);
    let ciphertext = fs::read(xml_path)?;

    // Decrypt and decompress to get the pseudo-XML
    let plaintext_bytes = crypto::decrypt_and_decompress(&ciphertext, passphrase)?;
    let plaintext_str = String::from_utf8_lossy(&plaintext_bytes);

    // Parse using the robust parser
    let mut fields = bir_xml::parse_bir_xml(&plaintext_str);

    // Inject dynamic date
    let now = Local::now();
    let dynamic_date = now.format("%m/%d/%Y %H:%M:%S").to_string();
    println!("Injecting heartbeat: {}", dynamic_date);
    fields.insert("txtDateIssue".to_string(), dynamic_date);

    // Re-generate the pseudo-XML and encrypt
    println!("Encrypting payload...");
    let new_xml = bir_xml::generate_bir_xml(&fields);
    let encrypted = crypto::compress_and_encrypt(new_xml.as_bytes(), passphrase)?;

    let form_type = "2551Qv2018";
    let email = "codeitlikemiley@gmail.com";
    let tin = "010558054000";
    let period = "122026Q1";

    // Format the filename correctly for the BIR FTP server
    let filename = format!("{}-{}-{}#{}#.xml", tin, form_type, period, email);

    println!("Transmitting {} to Remote Gateway...", filename);
    transport::submit_iaf(form_type, &filename, &encrypted).await?;

    println!(
        "Dispatch successful! Confirmation will be sent to {}",
        email
    );
    Ok(())
}
