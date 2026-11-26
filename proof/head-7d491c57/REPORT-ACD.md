# Money acceptance A/C/D at 7d491c57

Exact commit `7d491c57dd073a86d933361088500ab03bc74837`; `e5f7b42b` is an ancestor. Worktree `/home/ubuntu/forms-validation/money-accept-7d491c57`, target `/home/ubuntu/forms-validation/target-money-accept-7d491c57`, proof `/home/ubuntu/forms-validation/money-accept-proof`. Real maximized Linux X11 window; real xdotool keys at 120 ms unless stated; scratch SQLite `/home/ubuntu/forms-validation/money-accept-proof/money-accept-validation.db`. Safety env only; no Queue/live/token; Android untouched.

Machine rows: `accept-acd-results.jsonl`. Recording: `money-accept-acd-validation.mp4`.

## A

RIGHT: Tab into `1,234.56` then `15000.50` => after keys `15000.50`, after leaving `15,000.50`, saved `15000.5`.
RIGHT: Shift+Tab same.
RIGHT: click Ctrl+A type same.
RIGHT: click Ctrl+A BackSpace type same.
FAULT/behavior: one click then type did **not** select the existing amount. After keys `15000.501,234.56`; after leaving `1,234.56`; saved `1234.56`. Thus click places a caret/at least does not select; the typed text was inserted before the held amount and then the unreadable/oversize input reverted.
RIGHT: into `0.00`, click type and Tab type both produced `15000.50` / `15,000.50` / saved `15000.5`.
RIGHT: click with first key at once, 10/10 runs produced `15000.50` / `15,000.50` / saved `15000.5`.

## C

For each Ctrl+A unreadable typed case, after keys showed the typed string; after Tab the box showed `1,234.56`, draft held `1234.56`, and clicking the box again removed the sentence. Cases: `1500,50`; no-delay `15000..50`; no-delay `150a00.50`; `12a3`; `1.234,56`; `1,2,3`; lone `,`.

Exact sentence shape observed, for example:
`Taxable Amount: 1500,50 is not an amount. The box kept 1,234.56.`

This is a mismatch from the requested shape `Item N: <typed> is not an amount. The box kept <amount>.`: the prefix is `Taxable Amount:` rather than an item number. The underlying restore/save behavior is correct.

Pastes into `0.00` with real Ctrl+V:
- `₱ 15,000.50`: immediately after paste visible raw `₱ 15,000.50`; after leaving `15,000.50`; saved `15000.5`.
- ` 15000.50 `: immediately raw spaces visible; after leaving `15,000.50`; saved `15000.5`.
- `PHP 15000.50`: immediately raw `PHP 15000.50`; after leaving `15,000.50`; saved `15000.5`.
- `15000.50.25`: after paste raw malformed text; after leaving `0.00`; saved `0.0`; sentence `Taxable Amount: 15000.50.25 is not an amount. The box kept 0.00.`

Twenty digits `12345678901234567890`: after keys the digits remained visible; after leaving box restored `1,234.56`; saved `1234.56`; no sentence was captured/visible in semantic evidence. This misses the requested “sentence why” evidence.

Negative amounts in a negative-capable box: UNTESTED in this first A/C/D round. The seeded forms did not include the negative-capable 1604C box, and priority was to report A/C/D immediately.

## D

RIGHT: 2551Q Item 20 Surcharge was `1,500.00`; visible Check/to-do moved focus into it; typing `2500` at 120 ms showed `2500`, after leaving `2,500.00`, saved `2500.0`. The prior append fault did not reproduce.

## Caveats

No explicit Save button was clicked; persistence was judged from the scratch SQLite draft after the app's normal save. Schedule row 1 initially required opening the row drawer manually; the validation driver then used the visible row/control surface.
