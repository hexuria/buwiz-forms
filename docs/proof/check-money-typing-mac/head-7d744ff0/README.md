# Typing into zero boxes — macOS real-window proof at 7d744ff0

Build: hexuria/buwiz-forms pull request #403 at exactly `7d744ff028eb88c5c3ea4bac53a852fdc3849377`
("forms: Show editable zeros as placeholders [Z]"), kit sibling ../nsh at the pin 8c78cfd6, real windows
on macOS, 2551Q, the demo taxpayer, fresh returns, window 1440x900. Probes were untracked test files
(nsh/shots/ztype*.test.tsx plus the fixture nsh/ZTypeShot.tsx, a copy of FormAnyShot with a probe text);
nothing tracked was edited. The full probe output with every run's values is `ztype-run.log`.

## Files

- `z-a<N>-while.png` / `z-a<N>-left.png` — probe (a), ten runs: a click into Item 17's zero amount box,
  the keys 1 5 0 0 0 . 5 0 as fast as the rig sends them. `-while` is the box still focused with the typed
  text, `-left` after leaving into Item 15's box (grouped, saved).
- `z-c-focused.png` / `z-c-typed.png` — probe (c) first attempt: after the Check button. Check only reveals
  messages and toasts; it does not move the keys anywhere.
- `z-c2-jumped.png` / `z-c2-typed.png` — probe (c) with the rail's attention stepper: the jump lands in
  Part I (income type radio, business start date — not amount boxes); typing 4 2 lands nowhere visible.
- `z-d-after.png` — probe (d): a typed 0 left in the zero box — the grey 0.00 placeholder is back, the
  return still holds 0.00, no note.
- `z-e-after.png` — probe (e): `1500x` left in the zero box — the placeholder is back with the sentence
  "Item 17: 1500x is not an amount. The box kept 0.00." under the box.
- `z-g-items-light.png` — probe (g): locked/computed zeros (Items 14, 18, 19) black 0.00 on their grey
  fill; editable zeros (Items 15, 16, 17) grey placeholders in white boxes. Light.
- `z-h-items-dark.png` — probe (h): the same part in dark mode.
