#!/usr/bin/env bash
# The A2 rehearsal's undo by a previous release's own code (accounts-and-lock PLAN, "A2 rehearsal", The run 4):
# each pre-rooms safety zip the rehearsal kept is restored by that release's own Import path
# (`import::stage_database_zip`, which runs its `check_restorable_database`, then `StagedDatabase::install`), the
# restored file's raw SHA-256 is compared with the one the claim recorded, and the release then opens it.
#
# This is the "test built from the tag, with no UI" the plan allows where no UI driver works. bir_module's
# `restore_staged_database` wraps the same two calls with its safety copy and connection swap.
#
# Usage: scripts/a2_old_release_restore.sh <tag> <zips folder> <work folder>
#   <zips folder>: what `A2_REHEARSAL_ZIPS` named for `cargo test -p bir-core --lib a2_rehearsal -- --ignored`.
#   <work folder>: where the tag's tree is unpacked (on the build volume, never the system disk).
# The tag's own Cargo.lock is used (--locked). Set CARGO_TARGET_DIR to a build folder on the same volume.
set -euo pipefail
tag="$1" zips="$(cd "$2" && pwd)" work="$3"
repo="$(cd "$(dirname "$0")/.." && pwd)"
src="$work/$tag"
rm -rf "$src"
mkdir -p "$src"
git -C "$repo" archive "$tag" | tar -x -C "$src"
mkdir -p "$src/crates/bir-core/tests"
cat >"$src/crates/bir-core/tests/a2_restore.rs" <<'RS'
//! A2's safety zips, restored by this release's own Import code.
use sha2::{Digest, Sha256};

#[test]
fn this_release_restores_each_a2_safety_zip() {
    let dir = std::path::PathBuf::from(std::env::var("A2_ZIPS").expect("A2_ZIPS names the zips"));
    let mut zips: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zip"))
        .collect();
    zips.sort();
    assert!(!zips.is_empty(), "no zips in {}", dir.display());
    for zip in zips {
        let raw = std::fs::read_to_string(zip.with_extension("sha256")).unwrap();
        let folder = tempfile::tempdir().unwrap();
        let live = folder.path().join("bir_data.db");
        // The live database the import replaces, as on a person's device.
        drop(bir_core::db::Database::open(&live).unwrap());
        let staged = bir_core::import::stage_database_zip(&zip, &live)
            .unwrap_or_else(|error| panic!("{}: refused: {error}", zip.display()));
        staged.install().unwrap();
        let restored = hex::encode(Sha256::digest(std::fs::read(&live).unwrap()));
        assert_eq!(restored, raw.trim(), "{}: not byte for byte", zip.display());
        let db = bir_core::db::Database::open(&live).unwrap();
        println!(
            "A2 restore: {} by bir-core {}: raw SHA-256 equal ({}), opens with {} profiles",
            zip.file_name().unwrap().to_string_lossy(),
            env!("CARGO_PKG_VERSION"),
            &restored[..12],
            db.list_profiles().unwrap().len()
        );
    }
}
RS
cd "$src"
export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
unset BIR_SFTP_LIVE BIR_DATABASE_PATH
A2_ZIPS="$zips" cargo test --locked -p bir-core --test a2_restore -- --nocapture
