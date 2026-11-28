driver: none (headless kit only; the app was not driven with gpui-agent in this job)

# Amount notes on macOS: the evidence

The fault: macOS CI shard 0 failed 3 tests in nsh/screens/form/FormPageAmountNotes.test.tsx
(run 46111412550, main e9f34205): after a refused over-long amount and Tab the cell showed
truncated typed digits ("12,345,678,901.00" for 20 typed, "123,456,789,012.00" for 13)
instead of the amount the return held. Linux and Windows passed.

## Verdict: PRODUCT, proven without any kit trick

The session's flush (VALIDATE_DELAY = 300 ms, nsh/screens/form/session.ts) runs 300 ms after
the last edit. Typing is staged per keystroke, so keystrokes wider apart than 300 ms flush
mid-typing. Each flush validates AND commits: the still-valid prefix of the amount becomes an
accepted edit, autosaved. When the typing crosses the official 12-character input limit the
host refuses the final amount, but the refusal can only keep the draft as it is - already
holding the last fitting prefix, a number the filer never meant. On fast machines keys land
about 175 ms apart, the flush never fires mid-typing, the whole amount is refused as one edit
and the test passes. The macOS CI runner (3-6x slower) crosses 300 ms per key; Linux and
Windows runners do not. Any slow-typing filer on any platform hits the same.

## Evidence files

- evlog1.log - stock driving (wait 40 ms per key), instrumented: ONE focus, all digits land
  cleanly, ONE blur at the end. No mid-typing blur exists on a fast machine. (An early theory
  of focus loss was tested and excluded.)
- evlog2.log - the SAME stock driving with waits raised to 450 ms per key: still no blur,
  but the three CI failures reproduce exactly, with CI's exact values (probe
  "123,456,789,012.00" for the 13-digit typing).
- fix1.log - with the fix, the slow-typing probe passes all 15 tests.
- final-file.log - the real test file (16/16, including the new slow-typing regression test).
- repro2.log / repro-load.log - the stock file passing on this machine before the fix,
  also under CPU load (machine speed alone does not reproduce it; the typing cadence does).
- probe-test-slow-typing.tsx - the instrumented probe copy used for the reproduction.

## The fix (branch fix/amount-notes-macos)

nsh/screens/form/session.ts (+22 lines): the session remembers what a number box held when
its focus began (typeBase, captured in focus()); a refused non-empty amount whose draft value
now diverges from that base is filled back to the base in the same flush, following fill()'s
existing blank-chain pattern. The typing is one gesture: box, return and note all settle on
the pre-typing amount. The failing-first test types 13 digits 450 ms apart.

## Not in this evidence

- The bisect: the fault is inherent to the 300 ms flush design since its introduction, not a
  single-commit regression; not run.
- The real-Mac gpui-agent e2e step: not run (session budget). The headless reproduction is
  exact and needs no platform part: the mechanism is wall-clock timing in the session logic.
