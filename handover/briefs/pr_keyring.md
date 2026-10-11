**Linux data loss:** every relaunch quarantined the user's database.

`crates/bir-core/Cargo.toml` gave Linux `keyring = { workspace = true }` with no backend feature. keyring 3 has no default store, so the build silently used its **in-memory mock**: `set_password` "succeeded", the SQLCipher master key vanished at exit, the next launch generated a new key, and the existing database was moved aside as unreadable (`Recovered unreadable database … .corrupt-*.bak`). The comment above the Windows section describes exactly this. Windows was fixed, Linux never was. Reproduced on Linux aarch64 under Xvfb.

## Change
- **Backend:** Linux keyring now uses `sync-secret-service` + `crypto-rust` in both `bir-core` and `bir-desktop`. `bir-desktop` stores the Gemini API key in the keyring too.
- **Key lookup** (`get_or_create_master_key_keyring`):
  1. A valid `bir_key.txt` in the data dir is used first. It only exists because an earlier launch couldn't use the OS store, so it's the key the database was encrypted with.
  2. Otherwise the keyring is read. Found → use it. No entry → generate and store; if the store refuses, fall back to the file. **`PlatformFailure`** (no Secret Service on the bus, e.g. headless/minimal desktops) → file fallback (`0600`). Any other error (locked collection, dismissed prompt) still refuses rather than minting a key that would make the database unreadable.
- **Test:** `keyring_read_falls_back_only_when_store_is_unreachable`.

## Trade-offs / notes
- **New runtime dependency:** the Linux app needs `libdbus-1.so.3`. Desktop distros ship it. Debian packaging may want `libdbus-1-3` in `depends`.
- **Secret Service down at launch:** if a user's key is in the keyring but the Secret Service is down at launch, `PlatformFailure` is treated as "no store", a new file key is minted, and the database is quarantined (the `.bak` is kept). That's the price of supporting headless machines. A stricter variant would refuse when a database already exists and no key file is present. Say if you prefer that.
- **Cargo.lock:** only `dbus`, `dbus-secret-service` and `libdbus-sys` are newly compiled. The other lock additions are feature-gated entries that aren't built.

## Verification
- `cargo test -p bir-core --lib`: 807 passed
- `cargo clippy -p bir-core -- -D warnings`: clean
- `cargo clippy -p bir-desktop --all-targets --features dev-tools,agent -- -D warnings`: clean
- `cargo fmt --all`: clean

🤖 Generated with [Claude Code](https://claude.com/claude-code)
