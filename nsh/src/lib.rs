//! The Rust side of the NSH app: what TSX imports from `ebirforms/native`.
//!
//! This wave holds only the bridge proof, `Bridge.ping()`. The door to `bir-app-api` joins it later; its
//! TypeScript declarations are generated from these types into `.nsh/types/ebirforms/native.d.ts`.

use nsh_core::prelude::*;

/// The crate's UI set (`nsh.json` `"rust"`): everything TSX can import from `ebirforms/native`.
#[native_module("ebirforms/native")]
fn native(registry: &mut Registry) {
    registry.module::<Bridge>();
}

/// Proof that TSX reaches this crate.
#[module]
pub struct Bridge;

// A module: `Bridge.ping()`. Not `async`, no `cx`: it runs inside the TSX call and returns its string.
#[module]
impl Bridge {
    /// Answers `"pong"`, so a screen or a test can tell the native module is linked.
    pub fn ping() -> String {
        "pong".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::Bridge;

    #[test]
    fn ping_answers_pong() {
        assert_eq!(Bridge::ping(), "pong");
    }
}
