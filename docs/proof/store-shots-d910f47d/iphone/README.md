driver: gpui-agent-cli 0.4.1

device: iphone

| file | pixels | visual check |
|---|---:|---|
| 01-signin.png | 1206x2622 | pass visually; real sign-in gate, but captured at end through Settings > Account > Sign in because BIR_AUTH_BYPASS skips it at launch |
| 02-home.png | 1206x2622 | wrong: black transient “Preparing the print preview…” toast; otherwise correct home page |
| 03-form-library.png | 1206x2622 | pass visually; no toast, clipping, keyboard, debug banner, or old logo |
| 04-2551Q-filling.png | 1206x2622 | pass visually; integer amount 12345 in row editor, tax due 370.35 while editing |
| 05-2551Q-totals.png | 1206x2622 | pass visually; total 388.15 includes both ATC rows |
| 06-print-preview.png | 1206x2622 | pass visually; native two-page print preview |
| 07-profile.png | 1206x2622 | pass visually |
| 08-settings.png | 1206x2622 | pass visually |
