# 2551Q Linux screen-suite proof at a8aeaca8

This proof branch preserves the validation artifacts that could not be added to `fix/2551q-two-codes-save` after that branch was frozen at `05e1cc204bc197bb1b8c9d47b1fb1b8517113482`.

## Whole screen suite

Commit under test: `a8aeaca8a35c9e15ae62c57c4283dab4a72f4fbc`.
NSH pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.

The valid run used the workflow's Linux shard selection exactly: the sorted `*.test.ts`/`*.test.tsx` list excluding `verify`, file `k` assigned to shard `k % 3`, one `just test <file>` run per file, one log per file. The list has 195 files and each shard ran 65 files.

Results:

| Shard | Files | Failed |
|---|---:|---:|
| 0 | 65 | 0 |
| 1 | 65 | 1 |
| 2 | 65 | 0 |
| Total | 195 | 1 |

The only failing file was `./screens/form/atc2551q.test.tsx` on shard 1. Its failure begins:

```text
✗ screens/form/atc2551q.test.tsx › phone: row 1's ATC changes and row 2 takes a pick (16.2s)
    `text:✓ Saved` showing never happened in 8000 ms; on screen: ["BIR Form 2551Q", "Revision 2018", "Quarterly Percentage Tax Return", "Quarterly", "2 pages", "85 fields", "Print preview", "Check", "Queue", "Unsaved changes", "Return details", "5 items", "Part I · Background Information", "8 items", "Part II · Total Tax Payable", "13 items", "Part III · Details of Payment", "4 items", "Schedule 1 – Computation of Tax, Items 1 to 6", "2 rows", "1", "PT040 · 3.0", "2", "PT060 · 2.0", "Add row", "7", "Total Tax Due (Sum of Items 1 to 6) (To Part II Item 14)", "₱", "0.00", "Schedule 1 · Computation of Tax (Attach additional sheet/s, if necessary)", "2 items", "3 to do · Page 1", "Return details", "3"]
error 1 test: 0 passed, 1 failed (18.1s)
error: recipe `nsh-test` failed with exit code 1
error: recipe `test` failed on line 12 with exit code 1
```

## Focused validation logs

- `validation/fix-atc-run1.log` through `fix-atc-run3.log`: the frozen fix branch's `atc2551q.test.tsx` passed 1/1 in each of three Linux runs.
- `validation/08-sessionfixes-valid1.log` through `valid3.log`: `screens/form/session.fixes.test.ts` at `08ffd5c6` passed 13/13 in each of three Linux runs.
- `validation/fix-sessionfixes-run1.log`: the same file at the frozen fix branch head passed 13/13 once.

## Layout

- `full-suite-a8/files.txt`: the exact 195-file list.
- `full-suite-a8/shard0..2/summary.txt`: per-file exit statuses.
- `full-suite-a8/shard0..2/*.log`: one preserved log per test file.
- `full-suite-a8/summary.txt`: merged sorted statuses.
- `run_full_suite.sh`: the driver used for the valid run.
- `superseded-suite-attempts/`: the three earlier invalid/partial infrastructure attempts, retained rather than deleted.
- `NOTES.from-fix-branch.md`: the investigation notes exactly as committed on the frozen fix branch.

## Environment note

The workflow's dependency-cache step uses the `huacnlee/cargo` fork. The same approach was needed here after ordinary Cargo rebuilds exceeded the available time; the fork still reports Cargo 1.102.0 while `RUSTUP_TOOLCHAIN=1.99.0` selects the toolchain's rustc. `RUSTC_WRAPPER` was empty, every commit workspace had its own home/build area, and the shared `/home/ubuntu/BuwizNsh/target-nsh` was not used for these screen-test builds.
