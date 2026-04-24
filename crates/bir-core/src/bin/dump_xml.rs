use std::fs;
use bir_core::crypto::decrypt_and_decompress;

fn main() {
    let iaf_path = "../IAF_RDO_Copy/010558054000-2551Qv2018-122026Q1#codeitlikemiley@gmail.com#.xml";
    let ciphertext = fs::read(iaf_path).expect("Missing IAF file");
    let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";
    let decrypted = decrypt_and_decompress(&ciphertext, passphrase).expect("Failed to decrypt");
    let xml_string = String::from_utf8_lossy(&decrypted);
    println!("{}", xml_string);
}
