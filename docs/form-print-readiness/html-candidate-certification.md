# HTML candidate certification bootstrap

Status: candidate construction only. This path does not create trusted release
evidence, promote a capability, publish a package, or set `release_ready`.

## Why this path exists

Native preview, system print, PDF export, and packaged-offline evidence must be
collected from an exact non-development application candidate. Requiring
`release_ready` before that candidate can be built or can open 2551Q would make
the evidence gate circular.

The runtime and distribution decisions are therefore separate:

- `route: html_only` lets a normal non-`dev-tools` candidate open the owned HTML
  renderer. This currently applies to exactly 2551Q revision 2018.
- `route: experimental` remains available only in a `dev-tools` build.
- `release_ready` remains the public distribution gate. It is not inferred from
  a successful candidate build or from the presence of an HTML component.

The tagged release workflow remains unchanged and continues to run:

```sh
npm run audit:forms:migration -- --require-release-ready 2551Q:2018
```

before any public package is constructed or published.

## Non-publishing candidate workflow

Run **HTML Candidate Certification Inputs** manually from the GitHub Actions UI.
The workflow is `workflow_dispatch` only, has read-only repository permission,
and has no release job. It checks out the selected exact SHA, requires a clean
curated source revision, runs the full Rust format, check, clippy, and workspace
test gates, and builds normal release binaries without `dev-tools` and without
`--require-release-ready`.

GitHub exposes a manually dispatched workflow only after that workflow file
exists on the repository's default branch. Land the non-promotional workflow,
its manifest/binder scripts, schemas, and policy tests on `main` as a dedicated
infrastructure commit before trying to certify the still-unmerged renderer
branch. Once that bootstrap is present, push a clean source commit and dispatch
the exact branch revision:

```sh
gh workflow run html-candidate-certification.yml \
  --repo codeitlikemiley/ebirforms \
  --ref codex/print-preview-parity
```

The selected revision remains subject to the workflow's clean-source and
non-promotional gates. Merging this bootstrap does not make any form
release-ready.

Archive names retain the exact workflow checkout SHA. The manifest and renderer
identity use the migration audit's curated source revision, so a later
documentation-only commit cannot make an otherwise identical renderer bundle
fail its own identity check. Any change under the curated renderer, Rust,
packaging, workflow, or verification paths advances that source revision.

It uploads three short-lived candidate bundles:

- a Developer-ID-signed, notarized, and stapled universal macOS `.app` archive;
- a portable Windows x86-64 archive;
- a portable Linux x86-64 archive.

Each artifact includes `candidate-manifest.json` and the separately generated
renderer build identity. The candidate manifest hashes the archive and binds it
to the clean source revision and renderer bundle while permanently recording:

```json
{
  "promotion_eligible": false,
  "trusted_producer": false
}
```

These bundles are inputs for the platform collectors. Candidate signing alone
is not platform evidence, and none of these archives is installer-level package
evidence required for promotion. The macOS job fails closed unless all five
existing signing/notarization secrets are available; it creates the final
manifest only after stapling and re-verifying the exact archived application.

The operator-only platform binders and strict attestation contracts are
documented in:

- [macOS candidate certification collector/verifier foundation](macos-candidate-certification.md)
- [Windows candidate certification collector/verifier foundation](windows-candidate-certification.md)

The macOS workflow can now provide the exact Developer-ID-signed, notarized,
and stapled archive required by its strict verifier. It still cannot create UI,
printer, rollback, or trusted-producer evidence. The current Windows portable
archive is unsigned, so its strict verifier correctly cannot pass
Authenticode. Windows public EXE/MSI installers and Store-only MSIX remain
separate artifact tracks.

The Linux portable candidate binder and closed X11/Xvfb plus Wayland/Weston
attestation contract are documented in
[Linux candidate certification collector/verifier foundation](linux-candidate-certification.md).
It hashes the securely extracted portable candidate and requires both
application-owned hosts, CUPS print completion, direct PDF export, network
denial, and rollback evidence while explicitly retaining that the final `.deb`
and release tarball have not been certified.

## Promotion remains a separate milestone

The next collector milestone must run the unchanged candidate on its native
platform and produce independently reviewable evidence for preview readiness,
actual toolbar and native chooser operation, system-print completion, PDF
validation, network-denied operation, package identity, and rollback behavior.
macOS needs an operator with Accessibility and printer access to exercise the
new signed candidate; Windows needs the signed installer path; Linux needs both
X11/Xvfb and Wayland/Weston.

Only after a collector and verifier are reviewed may its producer be added to a
trusted producer registry. Curated reports and readiness flags then land in a
separate evidence-only commit bound to the candidate source revision. Do not
copy candidate manifests into `packages/form-specs/form-release-evidence.json`.
