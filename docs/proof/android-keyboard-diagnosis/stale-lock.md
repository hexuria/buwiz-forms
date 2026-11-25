# Stale lock after job 10 / Part D — Back and swipe-away cycles

Build: `fe1a5f46` with `BIR_DATABASE_PATH=test-data/bir_data.db` (job-10 build),
instrumented NSH runtime (logging only — no behavior change).

## Stale lock after job 10 (the original incident, partly from memory — noted)

After job 10's force-stop/relaunch testing the app showed:
"eBIRForms is locked / This device's sign-in could not be checked, so nothing opens. /
eBIRForms is already open on this database. Close the other window and try again. /
Try again" — screenshot committed in job 10's proof as `E-access-error-real.png`.
Tapping "Try again" did NOT clear it (the same screen returned). A full
`adb shell am force-stop` + relaunch DID clear it — no files were removed by hand.
The original logcat error lines were already gone from the buffer when checked
(reported from memory, as the UI text above is verbatim from the screenshot and
uiautomator dump).

## Live reproduction captured during job 11 (this folder)

`stale-lock-screen.png` — the same locked screen, reproduced live. Sequence: the app
(pid 11495) had been used for case 1, then went to background; a launcher re-open
(monkey) started a SECOND AppActivity in the same task while the process never died
(logcat at the time shows `numActivities=2` in the task) — the new engine found the
database already owned. Pid before and after: **same pid 11495** — the lock fired in
the SAME process that owned the database, so this is not only a cross-process stale
file: a second engine in the same process can lock itself out.

`lock-ls.txt` — `ls -la` of test-data with the database live:
`bir_data.db` (4096), `bir_data.db-shm` (32768), `bir_data.db-wal` (918792),
`bir_data.db.owner.lock` (0 bytes), plus the cloud-account/install-id/device files.
`bir_data.db.owner.lock` exists while the database is open; the lock is an owner lock
on that file. Logcat grep for owner/lock/shutdown produced no app-side lines — the
app does not log the lock failure to logcat at this build.

## Part D results

### Back-exit series (press Back on Home until the app closes, wait 3 s, relaunch)

| cycle | pid before | pid after | opened |
|---|---|---|---|
| 1 | 14172 | 14172 | New taxpayer |
| 2 | 14172 | 14172 | locked |
| 3 | 14172 | 14172 | locked |
| 4 | 14172 | 14172 | locked |
| 5 | 14172 | 14172 | locked |
| 6 | 14172 | 14172 | locked |

**Lock first appears at cycle 2, and never clears again (cycles 2–6 all locked).**
The pid is the same (14172) in every cycle — the process never dies on a Back exit,
so each relaunch starts a new engine in the SAME process, and from the second engine
on the database is "already open". "Try again" does not clear it. Cycle 1 reopened on
the New taxpayer screen rather than Home — the first cycle's Back presses navigated
backward inside the app before exiting, so the exact count of exits per engine is
blurred by one; the invariant (second engine in one process → locked) is unaffected.
Locked-screen screenshot: `partd-lock-back-2.png` (and cycles 3–6 identical);
logcat grep at first lock in `partd-logcat-back-2.txt` — no app-side owner/lock/shutdown
lines are logged at this build.

### Cycle-1 anomaly settled (backpress-detail.txt)

One Back press on Home closes the app: after a single press the Nexus launcher is
already the resumed activity while the app's pid SURVIVES in the background (pid
16548 before and after every press; presses 2–5 land on the launcher and do nothing).
So in the Back series every cycle was a real close+relaunch. Cycle 1 reopening on
"New taxpayer" was session-state restore of engine 2, not a failed exit. The counts
then line up exactly with the shutdown_for_quit once-per-process guess:

- engine 1 (first launch) → exit releases the lock;
- engine 2 (cycle-1 relaunch) opens fine, its exit does NOT release (the once-guard
  already fired in this process);
- engine 3 (cycle-2 relaunch) finds the database still owned → locked, and every
  later engine the same.

### Swipe-away series (swipe the app away in Recents, wait 3 s, relaunch)

| cycle | pid before | pid after | opened |
|---|---|---|---|
| 1 | 15931 | 16026 | eBIRForms |
| 2 | 16026 | 16026 | eBIRForms |
| 3 | 16026 | 16187 | eBIRForms |
| 4 | 16187 | 16280 | eBIRForms |
| 5 | 16280 | 16376 | eBIRForms |
| 6 | 16376 | 16472 | eBIRForms |

**Never locked in six cycles.** Note the pids: they change in almost every cycle —
swipe-away kills the process, so each launch is a fresh process and the os releases
the dead owner's file lock. That is the mechanistic difference: Back-exit keeps the
process alive (same-process second engine locks itself out); swipe-away ends the
process (no live owner, no lock).

### D2 — the sender half (partial, honestly reported)

Engine 1: the return was never successfully queued — form 2551Q for Q4 2026 was
opened and Queue tapped, but the form only auto-saved ("✓ Saved"); validation still
showed missing required items (business start date was set to 2020-01-15 in the tax
profile; line of business and period amounts were not filled). So no sender lines
exist to compare across engines. What the cycles above already show: engines 2+ in
the back series never got a usable database at all (locked), so a second engine in
one process cannot even reach the point of having a sender — the lock fault gates
the sender question entirely in the Back-exit mode.

