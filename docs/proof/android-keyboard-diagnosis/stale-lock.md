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

## Part D cycles — six Back exits, six swipe-aways

(table filled as the cycles run; see report message)
