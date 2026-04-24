//! ZLib compression + AES-128 Rijndael encryption pipeline.
//!
//! Replicates the behavior of `Encrypt.exe` (DCPcrypt2 library).
//! Pipeline: Plain XML → ZLib Compress → AES-128 Encrypt → IAF File

// TODO: Implement DCPcrypt2-compatible InitStr key derivation
// TODO: Verify cipher mode (CBC/ECB) against known plaintext/ciphertext pairs
// TODO: Test with hardcoded key candidates from Encrypt.exe binary
