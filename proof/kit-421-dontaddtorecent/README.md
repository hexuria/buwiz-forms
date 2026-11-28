# Proof for the kit patch of hexuria/buwiz-forms#421 (FOS_DONTADDTORECENT)

What this is: the Windows kit patch that keeps the shell's open and save dialogs from adding their files to the shell's recent items (Quick Access, the Jump List), proven in a scratch clone of hexuria/nsh. Nothing here pushed yet.

## Build line

hexuria/nsh scratch clone, work done at 17e51a8e9cfae5be7d0b1c846c8923415070b591 (buwiz-next as ordered; buwiz-next head moved to d969849df0a46752e0da9d1a6a4cadcc1c390e2d during the job and the branch was made from that head). cargo 1.99.0, CARGO_BUILD_JOBS=4, one fresh CARGO_TARGET_DIR inside the scratch clone. The branch fix/file-dialogs-not-recent carries the same final patch.

## What each file shows — and does not

- `421-fos-dontaddtorecent.patch` — the final patch (after the rustfmt reflow of the import block, and with one deviation forced by the compiler, see below). It does not by itself prove the flag survives a real dialog; that is what the test is for.
- `step3-fmt.log` — `cargo fmt --check -p nsh-platform-windows` after formatting: the only remaining diff is the pre-existing violation in `examples/transparent.rs`, which is not part of the patch and was not touched. rustfmt did change the patch as given: the import block is reflowed to the crate's width.
- `step4-clippy.log` — `cargo clippy -p nsh-platform-windows --all-targets -- -D warnings`, finished rc=0. It does not cover the crates depending on this one.
- `step5-test-new.log` — `cargo test -p nsh-platform-windows file_requests -- --nocapture`: `file_requests::tests::no_dialog_adds_its_file_to_recent_items ... ok`, 1 passed / 0 failed. The test instantiates real COM open and save dialogs (never shown) and reads back that FOS_DONTADDTORECENT and FOS_FORCEFILESYSTEM survive SetOptions/GetOptions for files, several files, folders and save.
- `step5-test-all.log` — one full `cargo test -p nsh-platform-windows`: lib 343 passed / 0 failed / 1 ignored in the first binary block, doctests 2 passed; one unrelated failure `audio::tests::a_sound_decodes_on_the_decoding_thread` in a parallel run block — a sound-decoding timing test that passes in isolation twice in a row afterwards (see the tail of the log); nothing about file dialogs. Treat it as flaky, not as caused by the patch: the patch touches only dialog option flags.
- `step6-control.log` — the control: with only the two `NOT_RECENT` uses reverted (test kept), the same test FAILS: `open OpenDialog { kind: Files, multiple: false, ... }: FILEOPENDIALOGOPTIONS(6216)` — the flag is absent without the patch lines. After restoring, the test passes again.
- This README does not show a real dialog opened by a person: no human clicked anything, no dialog was ever shown on screen; the test exercises the same COM objects the app uses, unshown.

## The one deviation from the patch as dictated

`OpenDialog` is `#[non_exhaustive]` in nsh-core, so the test's struct-update literal `OpenDialog { multiple: true, ..OpenDialog::files() }` does not compile outside nsh-core (E0639). The test constructs the same request with the crate's own builder instead: `OpenDialog::files().multiple(true)`. Everything else is exactly as dictated (modulo the rustfmt import reflow).

## Applies at the app's pin

`git apply --check` of the final patch passes on a checkout of 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf (the app's kit pin today), in a scratch worktree that was removed after the check.
