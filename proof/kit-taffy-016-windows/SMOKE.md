# `just e2e windows smoke --allow-send` summaries

- BEFORE: `e2e smoke on windows: 1 known gap, 10 pass.` — the known gap is sign-out s4
  (`wait_until timed out: node locked-screen not found`), the same pre-existing known gap the
  suite has everywhere. Queue passed in the dry-run test environment.
- AFTER: `e2e smoke on windows: 1 known gap, 10 pass.` — the identical known gap, nothing else.

No failure at either state, so nothing needed comparing further: the smoke outcome is the same
before and after the move.
