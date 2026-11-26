# TIN check-digit refusal — proof in the running app

Proof for this pull request, driven by hand in the real desktop app built from
this branch's working tree (fresh scratch database, the four development
safety variables, invented taxpayers only, real OS key events at human speed,
Queue never pressed). `NOTES.txt` says exactly what was done.

| File | Shows | Does not show |
|---|---|---|
| `proof.mp4` | One continuous annotated recording (41 s) of both refusals and the saves, in the app's own window. | Not a full-speed capture of every click — the raw careful-clicking pass ran about four minutes; the edit is the proof steps. |
| `refusal-create-desktop.png` | New taxpayer: TIN typed as `000 123 454` (spaces strip as typed; `000-123-454` lands), Create refused — "TIN 000-123-454 fails the official check digit. Compare each digit with the registration and type it again." — typed input kept in red cells. Desktop width (1400×900, the screen's maximum). | A 1440×900 layout — the screen does not have one; the window clamped at 1400. |
| `refusal-create-phone.png` | The same refusal at the narrow layout. | 390 pt width — the app enforces a 420 pt minimum, so this is 420×844. |
| `refusal-edit-desktop.png` | The profile's TIN edit: wrong digit `000123454` typed, save refused with the same sentence (the edit surface prefixes it: "Name and type saved. The TIN did not change: …"), wrong value kept in red cells. | — |
| `refusal-edit-phone.png` | The same edit refusal at narrow width, where the edit is a sheet overlay rather than inline. | — |
| `corrected-edit-saved.png` | The correction to a valid unused TIN (`000123461`) saving: toast "TIN changed to 000-123-461. Every branch, tax year, return and template moved with it." | — |
| `NOTES.txt` | The exact steps, launch command, and honest caveats. | — |

What the proof does not cover: anything beyond the two host surfaces — the
form pages' own TIN validation is unchanged and not re-proven here; the
spaced-TIN input cannot reach the field with spaces intact because the cells
strip them as typed, so the spaces normalization is proven by the unit tests
(`a_wrong_digit_spaced_like_the_store_allows_is_still_refused`), not by these
pictures.

One behavior to read, not a fault of this change: on the edit surface a save
that includes a wrong TIN still saves the name and type edits in the same
form, and only the TIN move is refused — the prefix "Name and type saved.
The TIN did not change:" says exactly that. The two operations were always
separate; the check digit now guards the second one.
