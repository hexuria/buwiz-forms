# Back after creating a taxpayer (PR #340)

The desktop app in a real macOS window (`nsh test --target macos`, 1280 x 800), an empty database, a made-up taxpayer.
Steps: Home, New taxpayer, type the TIN and the name, Create taxpayer, then the title bar's Back.

| Picture | What it shows |
| --- | --- |
| `1-taxpayer-created-forms.png` | After Create: the new taxpayer's Forms, the same before and after the fix. |
| `2-before-fix-back-reopens-the-form.png` | `main`: Back shows the New taxpayer form again. |
| `3-after-fix-back-goes-home.png` | This branch: Back shows Home, where the person was before opening the form. |
