# accounts-and-lock: Windows probes

Standalone checks of the Windows facts that features/accounts-and-lock/PLAN.md relies on, run on a real Windows
machine before the code that needs them exists. Not shipped; not part of any workspace.

| Probe | Plan | What it settles |
|---|---|---|
| `fsync <dir>` | A.3.2 step 4 | Whether `FlushFileBuffers` on a folder handle opened with `FILE_FLAG_BACKUP_SEMANTICS` is accepted, with a read-only handle and with write access. |
| `move <dir>` | A.3.2 step 4, A.3.3 (N2) | `hard_link` never replaces (183), both names share one volume serial and file index, the unlink leaves one link, `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` fails onto an existing file. |
| `holder <a> <b>` | A.2 (X1) | Today's second `share_mode(0)` open of the same lock in one process is a sharing violation (32); a holder keyed by path holds two locks side by side and returns the held handle when asked again; another process is refused while held and gets it after release. |
| `boot` | B.2 (L4, X3) | Within one boot, setting the clock forward moves "now minus `GetTickCount64`" but not the registry `BootId` or the boot GUID of `NtQuerySystemInformation(SystemBootEnvironmentInformation)`. |

Build and run (elevated PowerShell, on a data drive):

```
cargo build --release --manifest-path spikes/windows-probes/Cargo.toml
cd spikes/windows-probes
./run-probes.ps1 -Data D:\probe-data -Out .\probe-results
```

The script sets the clock forward 3 hours and back, with the Windows Time service stopped meanwhile and resynced at
the end. Results: `probe-results/results.jsonl`.

The holder probe repeats the plan's design, not A1's code. Once A1 is pushed, the same checks run against it.
