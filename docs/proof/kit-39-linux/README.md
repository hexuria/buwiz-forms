# Linux proof for hexuria/nsh PR #39 (fix/otp-input-burst @ 62e436ff)

Independent Linux verification of hexuria/nsh pull request #39, "otp input:
Keep fast typing in order". The PR was written and tested on macOS; this
machine is the Linux proof. Report only: no fixes, no comments, nothing pushed
to the PR branch.

## What is here

- `REPORT.md` — the full report: per-step table (step, commit, command,
  PASS/FAIL, counts, full failure texts) and the ways this machine differs
  from a standard Linux build machine.
- `otp-head-runNN.png` — the gallery's OtpInput demo ("One-time code") after
  nine digits were typed with real X key events (xdotool, 10 ms apart) at the
  PR head 62e436ff, run NN of 10.
- `otp-base-runNN.png` — the same at the base branch buwiz-next (8c78cfd6).
- `otp-base-window.png` — the demo window for context.

## What the screenshots show

Each run: the six boxed cells of the demo OtpInput after the digit string
`123456789` was sent with `xdotool type --delay 10` (the input caps at its
length of six). In order, the cells must read 1 2 3 4 5 6. The pre-fix fault
lands digits out of order (permutations, never wrong or lost digits).

## What they do not show

- They do not prove anything about paste, composition or selection editing —
  those are covered by the harness tests quoted in REPORT.md, not by these
  pictures.
- The base-branch pictures are evidence of the fault as it reproduces on this
  machine; the fault is timing-dependent, so a passing base run would not
  prove the base correct either. The decisive evidence is the headless
  harness's modeled burst (see REPORT.md step 3), which fails deterministically
  before the fix.
