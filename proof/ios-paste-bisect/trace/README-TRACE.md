# P2 trace retest report

## Result

The requested bf52 failure **did not reproduce** in the exact comparable retest. I did not create a fake `p2-bf52-fail.mov`.

Exact verified environment for bf52:

- app `19e4b0e84cfc2f7b84a504a7729b363eecfcee59` (PR398)
- nsh source `bf52b6fe79f7abf0139cec81d760f0d0dca7949e`
- CLI `nsh 0.6.1 (bf52b6f)`
- iPad Pro 13-inch simulator `41A2AFE3-9E35-4EE8-BEF0-21AA02B68BC3`
- scratch DB `/Users/devin/ios-paste-bisect/trace/bf52.db`
- safety variables set; no Queue, no real service, no `BIR_SFTP_LIVE`

## bf52 P2 exact retest evidence

The authenticator-code field was seeded to `12` by real software-keyboard input while the floating numeric keypad was visible. Pasteboard fixture was `345` set only with `xcrun simctl pbcopy`.

Observed sequence:

1. First real long-press on the nonempty authenticator-code field: **no callout/menu appeared**. The floating numeric keypad was replaced by the full software keyboard. No selection/caret movement was visible. Value remained `12`.
2. Second real long-press: an edit callout **did appear** with `Paste`, `Select`, `Select All`, and `AutoFill`.
3. Real tap on `Paste`: value changed from `12` to **`12345`**. Thus Paste succeeded.

Primary video:

- `/Users/devin/ios-paste-bisect/trace/p2-bf52-actual-pass.mov`

Screenshots:

- first long-press/no menu: `/Users/devin/ios-paste-bisect/trace/bf52-first-longpress-no-menu.png`
- second long-press/menu: `/Users/devin/ios-paste-bisect/trace/bf52-second-longpress-menu.png`
- after Paste/value `12345`: `/Users/devin/ios-paste-bisect/trace/bf52-after-paste-12345.png`
- cheap P1 note image, TIN filled by Paste: `/Users/devin/ios-paste-bisect/trace/bf52-p1-tin-pass-auth-empty.png`

Supplementary control recordings retained outside the trace directory:

- paste-seeded control: `/Users/devin/screencasts/p2-bf52-fail/p2-bf52-fail-edited.mp4`
- full-keyboard typed-seed control: `/Users/devin/screencasts/p2-bf52-fail-typed/p2-bf52-fail-typed-edited.mp4`

Both controls also resulted in successful nonempty-field Paste. The exact floating-keypad retest is the authoritative artifact above.

## P1 note

P1/TIN Paste passed in this bf52 environment: the scratch TIN was pasted into the TIN boxes and the authenticator-code page was reached. Evidence: `bf52-p1-tin-pass-auth-empty.png`.

## Parent 9fb status and app-base caveat

A fresh 9fb target was built using exact source commit `9fb1f7f8047c248b82fdcb94138c2a41743a3650` on app main `219da036370d46afcc4e0814a653912436d066d0`, and the simulator connected. However the final pass video was not completed: TIN setup became unreliable after repeated real input attempts, and the run was stopped rather than risking fabricated evidence. Prior bisect still-image evidence already showed 9fb passing P2, but this trace stage does **not** add a new 9fb video.

Because the bf52 failure did not reproduce, there was no valid fail/pass pair to record. The requested app-base caveat remains important if older results are compared: bf52 evidence came from PR398 app `19e4b0e`, whereas 9fb bisect evidence came from main app `219da036`; they are not the same app base.

## Interpretation

The earlier statement that repeated real long-presses never opened the menu at bf52 is not supported by this retest. The reproducible visible sequence here is: first long-press no menu/keyboard expansion, second long-press menu appears, Paste succeeds. This may explain the earlier observation as a first-focus/keyboard-transition artifact, but that is inference rather than proof.

## Cleanup

- `/Users/devin/BuwizNsh/target-bf52b6fe` deleted after bf52 testing.
- `/Users/devin/BuwizNsh/target-9fb1f7f8` deleted after stopping the incomplete 9fb trace attempt.
