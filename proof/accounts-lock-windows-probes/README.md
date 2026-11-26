# accounts-lock Windows probe proof

Probe source: `spikes/windows-probes/` at base commit `83fc03062cbd3b1656837ffb709339b7c106942d`, run on `DEVINBOX`, Windows Server 2022 build 20348.

Build: `cargo build --release` finished without warnings after the probe target directory was removed and the pinned toolchain's `rustc` was passed explicitly. The first build attempt stopped with E0514 (`windows_sys` compiled by an incompatible rustc); no source change was needed or made.

## `results.jsonl`

The elevated NTFS run on `C:\probe-data` and the administrator's LocalAppData, plus the holder and boot/clock probes.

What it shows:

- `fsync`: opening a directory read-only and calling `FlushFileBuffers` fails with OS error 5; opening it read+write succeeds. This is true both in a top-level NTFS data folder and in a user's LocalAppData.
- `move` on NTFS: `hard_link` creates the second name, both names report the same volume serial and file index, the second identical `hard_link` fails with 183, unlinking the legacy name leaves one link, the bytes survive, and `MoveFileExW` without replace fails onto an existing name with 183.
- `holder`: a raw second `share_mode(0)` open in the same process fails with 32. A path-keyed holder holds A and B side by side, returns the same handle for a repeated take of A, blocks another process while held (probe exit 3), and another process can open A after release (probe exit 0).
- `boot`: registry `BootId` 17 and the boot GUID `3a01280527d8f111b7fbf6f9664c97bf` are unchanged before, during, and after the three-hour clock jump. `wall_minus_tick_ms` moves by about 10,800,000 ms while the clock is forward and returns after the clock is restored.

What it does not show:

- It does not test the eBIRForms app or any production account/lock implementation.
- It does not prove behavior across a reboot; all boot checks are deliberately within one boot.
- It does not prove that a successful folder `FlushFileBuffers` provides every durability ordering guarantee an application protocol may need; it only proves the API acceptance/failure behavior and the move/link facts measured here.
- The generated `filesystem of C:\probe-data` note is blank because the script's `Get-Volume -FilePath` query errored on this Server 2022 machine. Independent `Get-Volume` output in `machine-facts.txt` shows C: is NTFS.

## `step3/`

Temporary 1 GB virtual disks were created with diskpart, formatted exFAT and ReFS, probed, detached, and deleted.

- exFAT: folder fsync matches NTFS (read handle 5, read+write success). `hard_link` is unsupported with OS error 1, so the subsequent move-chain facts do not hold there; because the hard link never existed, the later MoveFileExW lines are not evidence about replacement onto a real hard-linked target.
- ReFS: folder fsync and every move/link fact match NTFS, including same file identity, hard-link refusal 183, one link after unlink, and MoveFileExW no-replace refusal 183.
- These runs do not test parity/storage spaces, cluster sizes, or nondefault ReFS versions.

## `step4/`

A temporary standard local user ran `fsync` and `move` under its own `%LOCALAPPDATA%`; its outputs match the elevated user's NTFS results. The user was deleted afterwards. The random password was generated locally, never printed, and is not in these files. This does not test a domain user, roaming profile, or a user without permission to create a profile.

## Clock recovery

`clock-check.txt` shows the machine within about 0.02-0.03 seconds of `time.windows.com` after the script restored and resynchronized the clock.
