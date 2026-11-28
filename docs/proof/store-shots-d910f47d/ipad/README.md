driver: gpui-agent-cli 0.4.1

device: ipad

| file | pixels | visual check |
|---|---:|---|
| 01-signin.png | missing | missing: run stopped at back-tab-home after print preview did not close |
| 02-home.png | missing | missing: run stopped at back-tab-home after print preview did not close |
| 03-form-library.png | missing | missing: run stopped at back-tab-home after print preview did not close |
| 04-2551Q-filling.png | 2064x2752 | pass visually; amount editor sheet shown |
| 05-2551Q-totals.png | 2064x2752 | pass visually; total 388.15 |
| 06-print-preview.png | 2064x2752 | wrong page: captured before native print preview appeared; later failure.png shows the preview |
| 07-profile.png | missing | missing: run stopped at back-tab-home after print preview did not close |
| 08-settings.png | missing | missing: run stopped at back-tab-home after print preview did not close |
| 09-home-landscape.png | missing | not run; exact rotate error not reached because store-form failed |
