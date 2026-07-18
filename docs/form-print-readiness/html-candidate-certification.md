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

It uploads three short-lived candidate bundles:

- an ad-hoc-signed universal macOS `.app` archive;
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

These bundles are inputs for the platform collectors. They are not the signed,
notarized, or installer-level package evidence required for promotion.

The current operator-only macOS candidate binder and strict attestation
contract are documented in
[macOS candidate certification collector/verifier foundation](macos-candidate-certification.md).
The current macOS archive is ad-hoc signed, so that strict verifier correctly
cannot pass Developer ID, notarization, or stapling for the exact workflow
artifact yet.

## Promotion remains a separate milestone

The next collector milestone must run the unchanged candidate on its native
platform and produce independently reviewable evidence for preview readiness,
actual toolbar and native chooser operation, system-print completion, PDF
validation, network-denied operation, package identity, and rollback behavior.
macOS additionally needs Developer ID signing and notarization; Windows needs
the signed installer path; Linux needs both X11/Xvfb and Wayland/Weston.

Only after a collector and verifier are reviewed may its producer be added to a
trusted producer registry. Curated reports and readiness flags then land in a
separate evidence-only commit bound to the candidate source revision. Do not
copy candidate manifests into `packages/form-specs/form-release-evidence.json`.
