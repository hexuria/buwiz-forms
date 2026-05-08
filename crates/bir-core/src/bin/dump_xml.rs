use bir_core::crypto::decrypt_and_decompress;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: dump_xml <path-to-iaf-file>");
        return;
    }
    let iaf_path = &args[1];
    let ciphertext = fs::read(iaf_path).expect("Missing IAF file");
    let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";
    match decrypt_and_decompress(&ciphertext, passphrase) {
        Ok(decrypted) => {
            let xml_string = String::from_utf8_lossy(&decrypted);
            println!("{}", xml_string);
        }
        Err(e) => eprintln!("Failed to decrypt: {:?}", e),
    }
}
