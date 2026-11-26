# Paste trace evidence

The ordered P1 failure trace could not be produced because P1 does not fail at bf52. The substituted P2 retest also did not reproduce as a persistent failure: the first long-press only completed a keyboard/focus transition, and a second real long-press showed Paste; Paste changed `12` to `12345`.

Included: `p2-bf52-actual-pass.mov`, transition/menu/result screenshots, and `README-TRACE.md`. No fake failure recording was produced.
