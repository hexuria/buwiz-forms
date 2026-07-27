# HTML-Only Renderer Retirement Evidence

The production renderer path now contains only Rust render contracts, semantic
HTML/CSS assets, platform WebView output, PDF validation/merge helpers, and
small discrete artwork.

## Removed payload

The pre-retirement Git tree contained:

- 55 runtime layout-pack files totaling 42,002,348 bytes (40.06 MiB);
- 5 crate-local template/calibration files totaling 2,374,914 bytes (2.26 MiB);
- a separately downloaded document-compiler executable in every assembled
  platform package.

The prior assembled payload was approximately 106 MB uncompressed. A fresh
2026-07-19 ad-hoc universal macOS diagnostic package built from source revision
`111cce2f72e5e1fd7bdd38a4c74afb1880069d0f` contains 38,151,966 bytes (36.38
MiB) of regular files, about 64% less than that prior assembled payload. Its
DMG is 22,349,838 bytes, and the compiled offline HTML renderer directory is
1,217,872 bytes (1.16 MiB) after adding the ten renderer components. The
renderer tree hash is
`4a6bd595670be82efd9b0fe2680091edc21b82866327f789fe022daac7e2b471`.

These measurements remain diagnostic rather than release evidence: final net
package reduction must be recorded from the signed macOS, Windows, and Linux
artifacts because executable formats, signing, and archive compression differ.

## Enforced absence

`scripts/audit_no_legacy.py` fails on production source, packaging, or an
assembled package that contains the retired compiler, layout packs, full-page
runtime backgrounds, fallback viewer routes, or a Node runtime. CI and release
jobs run the source audit. Each platform packaging flow scans both the staged
payload and the payload extracted from the final deliverable: mounted DMG on
macOS, final EXE and MSI installers on Windows, and final DEB and tarball on
Linux. A clean staging directory cannot hide a legacy file introduced by the
installer recipe.

Official form rasters remain only under the renderer reference directory for
visual tests. They are not copied into the runtime document or used as page
backgrounds.
