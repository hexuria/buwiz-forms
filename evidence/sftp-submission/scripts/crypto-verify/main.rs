// Independent Rust verification of the ebfSFTP dispatcher-field crypto.
// Must reproduce the .NET/PowerShell result: DEV/UAT `srv` blob -> "ftp2.birgovph.com".
use aes::Aes256;
use aes::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use pbkdf2::pbkdf2_hmac;
use sha1::Sha1;

type Aes256CbcDec = cbc::Decryptor<Aes256>;

const PASS: &str = "Carlo*TSSD2!018";
const ROUNDS: u32 = 100_000;

fn unwrap_field(b64: &str) -> String {
    let raw = data_encoding::BASE64.decode(b64.trim().as_bytes()).expect("base64");
    let (salt, rest) = raw.split_at(32);
    let (iv, ct) = rest.split_at(16);
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha1>(PASS.as_bytes(), salt, ROUNDS, &mut key);
    let mut buf = ct.to_vec();
    let plain = Aes256CbcDec::new((&key).into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .expect("decrypt");
    String::from_utf8(plain.to_vec()).expect("utf8")
}

fn main() {
    let blob = "25s+rBZx/AO+YuDjzPzIBx81hOVx4fhdnOHNysmXar3RpmRnduhtuxoasmUEANldVjKUeaebvHyefVvj5aJQ/+hCjBdF+xwd7GFdWWSbqL8=";
    let got = unwrap_field(blob);
    println!("unwrapped = {:?} (len {})", got, got.len());
    assert_eq!(got, "ftp2.birgovph.com", "Rust crypto must match .NET result");
    assert_eq!(got.len(), 17, "plaintext must be 17 bytes (no BOM)");
    println!("PASS: Rust port reproduces the .NET dispatcher-field decrypt byte-for-byte");
}
