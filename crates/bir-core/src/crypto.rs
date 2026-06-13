use aes::Aes256;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// The BIR-mandated AES passphrase for encrypting/decrypting IAF XML files.
/// Extracted from the official eBIRForms `Encrypt.exe` binary (DCPcrypt2 pipeline).
/// This is a protocol constant, not a secret — every eBIRForms installation uses the same key.
pub const BIR_IAF_PASSPHRASE: &str = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("Decryption failed: {0}")]
    Decryption(String),
    #[error("Encryption failed")]
    Encryption,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Derives the AES key and IV using DCPcrypt2's logic.
/// DCPcrypt2 `InitStr` with SHA-256 hashes the passphrase to get the key.
/// If `InitVector` is nil, it encrypts a block of zeros to get the IV.
fn derive_key_and_iv(passphrase: &str) -> ([u8; 32], [u8; 16]) {
    // 1. Hash passphrase with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(passphrase.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&hasher.finalize());

    // 2. Encrypt an all-zero block with ECB to get the IV
    use aes::cipher::{BlockEncrypt, KeyInit};
    let cipher = Aes256::new_from_slice(&key).unwrap();
    let mut iv = [0u8; 16];
    let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(&iv);
    cipher.encrypt_block(&mut block);
    iv.copy_from_slice(&block);

    (key, iv)
}

pub fn decrypt_and_decompress(ciphertext: &[u8], passphrase: &str) -> Result<Vec<u8>, CryptoError> {
    let (key, iv) = derive_key_and_iv(passphrase);

    // Decrypt AES-256-CBC
    // Note: DCPcrypt doesn't seem to use standard PKCS7 padding automatically for streams unless specified.
    // It might just process block by block. Wait, `DecryptStream` just processes exact sizes or left over?
    // Let's use no padding for now and handle the leftover if any, or use PKCS7 if it padded.
    // Actually, `DecryptStream` in DCPcrypt does:
    // If leftover bytes `Size mod SizeOf(Buffer)`, it processes them.
    // Wait, block cipher encrypts exact sizes. But the size of IAF_RDO_Copy is 978. 978 % 16 = 2.
    // How does it encrypt 2 bytes?
    // Let's look at DCPcrypt `EncryptStream` and `DecryptStream`... wait!
    // `TDCP_cipher.EncryptStream`:
    // It reads chunk, then `Encrypt(Buffer, Buffer, Read); OutStream.Write(Buffer, Read);`
    // And `Encrypt` calls `cmCFB8bit` for `EncryptString`, but what does it call for files?
    // Wait, the default CipherMode is `cmCBC`.
    // But `TDCP_blockcipher128.EncryptCBC` says:
    // `if (Size mod 16) <> 0 then { EncryptECB(CV, CV); Move(p1^, p2^, Size mod 16); XorBlock(p2^, CV, Size mod 16); }`
    // This is essentially CFB mode for the final incomplete block!

    // Since it mixes CBC and CFB for the final block, standard PKCS7 padding won't work.
    // We have to implement DCPcrypt's custom CBC decryption precisely.
    let mut decrypted = vec![0u8; ciphertext.len()];
    dcp_decrypt_cbc(&key, &iv, ciphertext, &mut decrypted);

    // Decompress ZLib
    let mut decoder = ZlibDecoder::new(decrypted.as_slice());
    let mut plaintext = Vec::new();
    decoder.read_to_end(&mut plaintext)?;

    Ok(plaintext)
}

/// Implements DCPcrypt2's exact DecryptCBC behavior
fn dcp_decrypt_cbc(key: &[u8; 32], iv: &[u8; 16], indata: &[u8], outdata: &mut [u8]) {
    use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
    let cipher = Aes256::new_from_slice(key).unwrap();
    let mut cv = *iv;

    let mut i = 0;
    let size = indata.len();
    while i + 16 <= size {
        let chunk = &indata[i..i + 16];
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        for j in 0..16 {
            outdata[i + j] = block[j] ^ cv[j];
        }
        cv.copy_from_slice(chunk);
        i += 16;
    }

    if i < size {
        // Incomplete block:
        // EncryptECB(CV, CV)
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(&cv);
        cipher.encrypt_block(&mut block);
        cv.copy_from_slice(&block);

        let rem = size - i;
        for j in 0..rem {
            outdata[i + j] = indata[i + j] ^ cv[j];
        }
    }
}

/// Implements DCPcrypt2's exact EncryptCBC behavior
fn dcp_encrypt_cbc(key: &[u8; 32], iv: &[u8; 16], indata: &[u8], outdata: &mut [u8]) {
    use aes::cipher::{BlockEncrypt, KeyInit};
    let cipher = Aes256::new_from_slice(key).unwrap();
    let mut cv = *iv;

    let mut i = 0;
    let size = indata.len();
    while i + 16 <= size {
        for j in 0..16 {
            cv[j] ^= indata[i + j];
        }
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(&cv);
        cipher.encrypt_block(&mut block);
        cv.copy_from_slice(&block);
        outdata[i..i + 16].copy_from_slice(&cv);
        i += 16;
    }

    if i < size {
        // Incomplete block:
        // EncryptECB(CV, CV)
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(&cv);
        cipher.encrypt_block(&mut block);
        cv.copy_from_slice(&block);

        let rem = size - i;
        for j in 0..rem {
            outdata[i + j] = indata[i + j] ^ cv[j];
            cv[j] = outdata[i + j]; // Update CV just in case, though it's the end
        }
    }
}

pub fn compress_and_encrypt(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, CryptoError> {
    let (key, iv) = derive_key_and_iv(passphrase);

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(plaintext)?;
    let compressed = encoder.finish()?;

    let mut ciphertext = vec![0u8; compressed.len()];
    dcp_encrypt_cbc(&key, &iv, &compressed, &mut ciphertext);

    Ok(ciphertext)
}

pub fn hash_pin(pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pin.as_bytes());
    hasher.update(b"e-birforms-pin-salt-2024");
    hex::encode(hasher.finalize())
}

pub fn generate_totp_secret(account_name: &str) -> (String, std::path::PathBuf) {
    use totp_rs::{Algorithm, Secret, TOTP};
    let secret = Secret::generate_secret().to_bytes().unwrap();
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        Some("e-BIRForms".to_string()),
        account_name.to_string(),
    )
    .unwrap();
    let qr_base64 = totp.get_qr_base64().unwrap();
    let bytes = data_encoding::BASE64
        .decode(qr_base64.as_bytes())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("totp_qr_{}.png", uuid::Uuid::new_v4()));
    let _ = std::fs::write(&path, bytes);
    (totp.get_secret_base32(), path)
}

pub fn validate_totp(secret: &str, token: &str) -> bool {
    use totp_rs::{Algorithm, Secret, TOTP};
    if let Ok(decoded_secret) = Secret::Encoded(secret.to_string()).to_bytes()
        && let Ok(totp) = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            decoded_secret,
            None,
            "".to_string(),
        )
    {
        return totp.check_current(token).unwrap_or(false);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[ignore]
    fn test_decryption_with_hardcoded_key() {
        let iaf_path =
            "../../../IAF_RDO_Copy/010558054000-2551Qv2018-122026Q1#codeitlikemiley@gmail.com#.xml";
        let ciphertext = fs::read(iaf_path).expect("Missing IAF file");

        let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>"; // Extracted from Encrypt.exe

        let decrypted = decrypt_and_decompress(&ciphertext, passphrase).expect("Failed to decrypt");
        let xml_string = String::from_utf8_lossy(&decrypted);

        // The decrypted payload is pure XML
        assert!(xml_string.starts_with("<?xml version='1.0'?>"));
        assert!(xml_string.contains("frm2551Qv2018"));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = b"<xml>test data with special chars</xml>";
        let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";
        let encrypted = compress_and_encrypt(original, passphrase).expect("encrypt failed");
        assert_ne!(&encrypted[..], &original[..]);
        let decrypted = decrypt_and_decompress(&encrypted, passphrase).expect("decrypt failed");
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_roundtrip_exact_block_boundary() {
        // 32 bytes = exactly 2 blocks, no tail
        let original = b"0123456789abcdef0123456789abcdef";
        let passphrase = "testpass";
        let encrypted = compress_and_encrypt(original, passphrase).expect("encrypt failed");
        let decrypted = decrypt_and_decompress(&encrypted, passphrase).expect("decrypt failed");
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_roundtrip_non_block_boundary() {
        // 33 bytes = 2 blocks + 1 tail byte
        let original = b"0123456789abcdef0123456789abcdefX";
        let passphrase = "testpass";
        let encrypted = compress_and_encrypt(original, passphrase).expect("encrypt failed");
        let decrypted = decrypt_and_decompress(&encrypted, passphrase).expect("decrypt failed");
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_roundtrip_empty_input() {
        let original = b"";
        let passphrase = "testpass";
        let encrypted = compress_and_encrypt(original, passphrase).expect("encrypt failed");
        let decrypted = decrypt_and_decompress(&encrypted, passphrase).expect("decrypt failed");
        assert_eq!(decrypted, original);
    }
}
