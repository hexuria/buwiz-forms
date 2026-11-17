use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

pub fn digest_entries<'a>(
    domain: &str,
    entries: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> String {
    let mut digest = Sha256::new();
    update_framed(&mut digest, domain.as_bytes());
    for (path, bytes) in entries {
        update_framed(&mut digest, path.as_bytes());
        update_framed(&mut digest, bytes);
    }
    let digest = digest.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn update_framed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::{digest_entries, sha256_hex};

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn entry_framing_distinguishes_boundaries() {
        let left = digest_entries("test", [("a", b"bc".as_slice())]);
        let right = digest_entries("test", [("ab", b"c".as_slice())]);
        assert_ne!(left, right);
    }
}
