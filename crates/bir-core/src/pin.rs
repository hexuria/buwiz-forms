//! The PIN hash (accounts-and-lock PLAN §B.1).
//!
//! The hash is Argon2id at the OWASP minimum (`m_cost = 19456` KiB, `t_cost = 2`, `p_cost = 1`), with a random
//! 16-byte salt from the operating system per PIN, stored as its PHC string
//! (`$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`). The PHC string carries its own parameters, so a hash
//! made at another cost still verifies, and [`check`] says when a match should be rehashed at today's cost.
//! Parameters beyond [`MAX_COST`] are refused on read, so a planted string cannot make a check huge or slow.
//!
//! Which PINs are Argon2id from B1 on:
//! - the owner PIN: hashed before it is stored, and only a PHC string is ever accepted for it ([`check_phc`]);
//! - a client or help-desk admin PIN set through the page's old synchronous verbs: stored in the old form for a
//!   moment (the verb answers at once and the engine never waits on the hash), then rehashed to Argon2id on its
//!   own thread straight after; if that is cut short, on its first match.
//!
//! The old form is SHA-256 of the PIN plus one fixed salt for everyone ([`crate::crypto::hash_pin`], 64 hex
//! characters). It is still read, so a client PIN set by an earlier version opens; a match against it says
//! [`Check::Match`] with `upgrade: true`, and the caller rehashes it (PLAN §B.1, client PINs).
//!
//! The hash is slow on purpose (about the PLAN's 100–500 ms target on a low-end phone): callers run it off the
//! engine thread.
//!
//! **The test cost.** In this crate's own tests, and in a debug build run with `EBIR_TEST_ENV` on (the other
//! crates' tests, `just nsh-test`), new hashes use the cheapest Argon2id cost instead ([`Cost::TEST`]), so a
//! test that types many PINs never waits on the real one. A release build never does. A PIN hashed at the
//! test cost still verifies anywhere, and a match outside the test environment rehashes it at the real cost.
//! The tests that are about the cost itself ask for [`Cost::PRODUCTION`] by name.

use argon2::password_hash::phc::PasswordHash;
use argon2::{Algorithm, Argon2, Params, PasswordHasher as _, PasswordVerifier as _, Version};
use rand::TryRng as _;

pub use zeroize::Zeroizing;

/// The bytes of salt each PIN gets.
pub const SALT_LEN: usize = 16;

/// An Argon2id cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Memory, in KiB.
    pub m_kib: u32,
    /// Passes.
    pub t: u32,
    /// Lanes.
    pub p: u32,
}

impl Cost {
    /// The OWASP minimum the PLAN picks: 19 MiB, 2 passes, 1 lane.
    pub const PRODUCTION: Cost = Cost {
        m_kib: 19_456,
        t: 2,
        p: 1,
    };

    /// The cheapest cost Argon2 allows (8 KiB, 1 pass): tests only (see the module notes).
    pub const TEST: Cost = Cost {
        m_kib: 8,
        t: 1,
        p: 1,
    };

    /// The cost new hashes use in this process.
    pub fn current() -> Cost {
        if cfg!(test) || (cfg!(debug_assertions) && crate::db::test_environment()) {
            Cost::TEST
        } else {
            Cost::PRODUCTION
        }
    }

    /// Within [`MAX_COST`] in every parameter.
    fn bounded(self) -> bool {
        self.m_kib <= MAX_COST.m_kib && self.t <= MAX_COST.t && self.p <= MAX_COST.p
    }

    fn params(self) -> Result<Params, PinError> {
        Params::new(self.m_kib, self.t, self.p, None).map_err(|error| PinError(error.to_string()))
    }

    /// Weaker than `other` in memory or passes: a match should be rehashed at `other`.
    fn below(self, other: Cost) -> bool {
        self.m_kib < other.m_kib || self.t < other.t
    }
}

/// The dearest cost a stored PHC string may name: 64 MiB, 8 passes, 4 lanes. Above the PLAN's other OWASP row
/// (46 MiB, 1 pass); anything beyond is refused unread (N6).
pub const MAX_COST: Cost = Cost {
    m_kib: 65_536,
    t: 8,
    p: 4,
};

/// The hash could not be made, or a stored value is not one this build reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinError(pub String);

impl std::fmt::Display for PinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PIN hash: {}", self.0)
    }
}

impl std::error::Error for PinError {}

/// What a stored PIN value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// An Argon2id PHC string.
    Phc,
    /// The old SHA-256 hex of [`crate::crypto::hash_pin`].
    LegacySha,
}

/// The form of `stored`, or `None` for a value this build does not read.
pub fn stored_kind(stored: &str) -> Option<Stored> {
    if stored.starts_with("$argon2id$") {
        Some(Stored::Phc)
    } else if stored.len() == 64 && stored.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(Stored::LegacySha)
    } else {
        None
    }
}

/// What [`check`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Miss,
    /// The PIN matches. `upgrade`: the stored value is the old SHA-256 form or a cheaper Argon2id cost than
    /// this process's ([`Cost::current`]); the caller should store [`hash`] of the same PIN in its place.
    Match {
        upgrade: bool,
    },
}

impl Check {
    pub fn matched(self) -> bool {
        matches!(self, Check::Match { .. })
    }
}

/// The Argon2id PHC string of `pin` at [`Cost::current`], with a fresh salt from the operating system.
pub fn hash(pin: &str) -> Result<String, PinError> {
    hash_with(pin, Cost::current())
}

/// [`hash`] at `cost`.
pub fn hash_with(pin: &str, cost: Cost) -> Result<String, PinError> {
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::SysRng
        .try_fill_bytes(&mut salt)
        .map_err(|error| PinError(format!("no random salt from the system: {error}")))?;
    let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, cost.params()?);
    // The PHC string holds no secret; the salt is wiped all the same once used.
    let salt = Zeroizing::new(salt);
    hasher
        .hash_password_with_salt(pin.as_bytes(), salt.as_slice())
        .map(|phc| phc.to_string())
        .map_err(|error| PinError(error.to_string()))
}

/// The cost a PHC string was made at.
pub fn phc_cost(stored: &str) -> Result<Cost, PinError> {
    let phc = PasswordHash::new(stored).map_err(|error| PinError(error.to_string()))?;
    let params = Params::try_from(&phc).map_err(|error| PinError(error.to_string()))?;
    Ok(Cost {
        m_kib: params.m_cost(),
        t: params.t_cost(),
        p: params.p_cost(),
    })
}

/// `pin` against an Argon2id PHC string only: the owner PIN's check (N2). The old SHA-256 form is refused.
pub fn check_phc(stored: &str, pin: &str) -> Result<Check, PinError> {
    if stored_kind(stored) != Some(Stored::Phc) {
        return Err(PinError("the stored PIN is not an Argon2id string".into()));
    }
    check(stored, pin)
}

/// `pin` against `stored`, in either form. Only Argon2id is accepted as a PHC string, and only within
/// [`MAX_COST`].
pub fn check(stored: &str, pin: &str) -> Result<Check, PinError> {
    match stored_kind(stored) {
        Some(Stored::Phc) => {
            let phc = PasswordHash::new(stored).map_err(|error| PinError(error.to_string()))?;
            let cost = phc_cost(stored)?;
            if !cost.bounded() {
                return Err(PinError(format!(
                    "the stored PIN names a cost beyond what this version checks ({cost:?})"
                )));
            }
            match Argon2::default().verify_password(pin.as_bytes(), &phc) {
                Ok(()) => Ok(Check::Match {
                    upgrade: cost.below(Cost::current()),
                }),
                Err(argon2::password_hash::Error::PasswordInvalid) => Ok(Check::Miss),
                Err(error) => Err(PinError(error.to_string())),
            }
        }
        Some(Stored::LegacySha) => {
            let typed = crate::crypto::hash_pin(pin);
            Ok(if same_bytes(typed.as_bytes(), stored.as_bytes()) {
                Check::Match { upgrade: true }
            } else {
                Check::Miss
            })
        }
        None => Err(PinError(
            "the stored PIN is in no form this version reads".into(),
        )),
    }
}

/// Equal, in a time that does not depend on where they differ.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_hash_is_argon2id_with_its_own_salt() {
        let first = hash_with("0817", Cost::PRODUCTION).unwrap();
        let second = hash_with("0817", Cost::PRODUCTION).unwrap();
        assert_ne!(
            first, second,
            "two equal PINs give two different PHC strings"
        );
        for phc in [&first, &second] {
            assert!(phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{phc}");
            assert_eq!(stored_kind(phc), Some(Stored::Phc));
            assert_eq!(phc_cost(phc).unwrap(), Cost::PRODUCTION);
            // The salt is 16 bytes: 22 characters of unpadded base64.
            let salt = phc.split('$').nth(4).unwrap();
            assert_eq!(salt.len(), 22, "{phc}");
            assert_eq!(check(phc, "0817").unwrap(), Check::Match { upgrade: false });
            assert_eq!(check(phc, "0818").unwrap(), Check::Miss);
        }
        let salts: Vec<&str> = [&first, &second]
            .iter()
            .map(|phc| phc.split('$').nth(4).unwrap())
            .collect();
        assert_ne!(salts[0], salts[1]);
    }

    #[test]
    fn the_old_sha_form_still_opens_and_asks_for_an_upgrade() {
        let old = crate::crypto::hash_pin("0817");
        assert_eq!(stored_kind(&old), Some(Stored::LegacySha));
        assert_eq!(check(&old, "0817").unwrap(), Check::Match { upgrade: true });
        assert_eq!(check(&old, "0818").unwrap(), Check::Miss);
    }

    #[test]
    fn a_cheaper_hash_asks_for_an_upgrade_and_a_dearer_one_does_not() {
        // This crate's tests hash at the test cost: a production hash is not "upgraded" down to it.
        assert_eq!(Cost::current(), Cost::TEST);
        let dear = hash_with("1234", Cost::PRODUCTION).unwrap();
        assert_eq!(
            check(&dear, "1234").unwrap(),
            Check::Match { upgrade: false }
        );
        let cheap = hash_with("1234", Cost::TEST).unwrap();
        assert!(Cost::TEST.below(Cost::PRODUCTION));
        assert!(!Cost::PRODUCTION.below(Cost::TEST));
        assert_eq!(
            check(&cheap, "1234").unwrap(),
            Check::Match { upgrade: false }
        );
    }

    /// N6: a planted PHC string that names a huge or slow cost is refused before any hashing (this test would run
    /// out of memory, or for minutes, otherwise). N2: the owner's check takes only a PHC string.
    #[test]
    fn absurd_costs_are_refused_unread_and_the_owner_check_takes_only_argon2id() {
        let real = hash_with("0817", Cost::TEST).unwrap();
        let tail = real.splitn(5, '$').nth(4).unwrap();
        for params in [
            "m=4194304,t=2,p=1",
            "m=19456,t=100000,p=1",
            "m=19456,t=2,p=255",
        ] {
            let planted = format!("$argon2id$v=19${params}${tail}");
            let started = std::time::Instant::now();
            assert!(check(&planted, "0817").is_err(), "{params}");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(1),
                "{params}"
            );
        }
        let at_the_bound = hash_with(
            "0817",
            Cost {
                m_kib: 65_536,
                t: 1,
                p: 1,
            },
        )
        .unwrap();
        assert!(check(&at_the_bound, "0817").unwrap().matched());

        let old = crate::crypto::hash_pin("0817");
        assert!(
            check(&old, "0817").unwrap().matched(),
            "a client's old form still opens"
        );
        assert!(check_phc(&old, "0817").is_err(), "never for the owner");
        assert!(check_phc(&real, "0817").unwrap().matched());
    }

    #[test]
    fn values_of_no_known_form_are_refused() {
        for stored in [
            "",
            "1234",
            "$argon2i$v=19$m=8,t=1,p=1$c2FsdHNhbHQ$aGFzaA",
            "zz",
        ] {
            assert!(check(stored, "1234").is_err(), "{stored:?}");
        }
    }

    /// The PLAN's target is 100–500 ms on a low-end Android phone. This measures the production cost on the
    /// machine the tests run on and prints it (`--nocapture`). The low-end Android number is not this: it is
    /// taken on the device for the B1 device proof.
    #[test]
    fn argon2_timing_at_the_production_cost() {
        let runs = 5;
        let mut hashes = Vec::new();
        let start = std::time::Instant::now();
        for _ in 0..runs {
            hashes.push(hash_with("0817", Cost::PRODUCTION).unwrap());
        }
        let hash_ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
        let start = std::time::Instant::now();
        for phc in &hashes {
            assert!(check(phc, "0817").unwrap().matched());
        }
        let verify_ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
        println!(
            "argon2id m=19456 KiB t=2 p=1 on {} {} ({} build): hash {hash_ms:.1} ms, verify {verify_ms:.1} ms \
             (mean of {runs}); the low-end Android figure is for the device proof",
            std::env::consts::OS,
            std::env::consts::ARCH,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        );
        assert!(hash_ms > 0.0 && verify_ms > 0.0);
    }
}
