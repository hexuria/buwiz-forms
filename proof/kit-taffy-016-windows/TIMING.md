# Timing: five runs per state, milliseconds, median (range)

Measured from the runner's own receipts and run.log timestamps. Same window size (1440x900), same
machine, sequential runs, BEFORE first then AFTER. cold_start: `nsh-run-agent` launch to the window
plane answering. home_visible: launch to the Home screen's tiles confirmed (includes the sign-up the
fresh scratch database requires: it is the same work in both states). open_2551q: the open-2551q
recipe end to end (click New return to the form page visible with its first checks). scroll: the
eight fixed scroll steps of the form tour, top towards the end — note these are paced one wheel
event per display frame by the host, so they measure event pacing more than layout speed.

| metric | BEFORE median | BEFORE range | AFTER median | AFTER range | reading |
|---|---|---|---|---|---|
| cold start | 7527 | 7515-7575 | 7544 | 7514-8728 | identical (the 8728 outlier is first-run noise) |
| home visible | 19695 | 19579-19778 | 19137 | 18590-20261 | inside the noise |
| open 2551Q | 9214 | 9196-9478 | 8720 | 8701-8759 | AFTER consistently ~490 ms (5 %) faster, outside the noise |
| scroll form | 13511 | 13425-13522 | 13556 | 13460-13793 | identical (event-paced, not layout-bound) |
