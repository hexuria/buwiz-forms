# Linux candidate certification collector/verifier foundation

Status: operator-only, non-promotional foundation. It does not register trusted
evidence, set `release_ready`, or certify a public Linux package.

## What the workflow candidate proves

The manually dispatched `html-candidate-certification.yml` Linux job uploads a
portable x86-64 tarball, `candidate-manifest.json`, and the renderer build
identity. The manifest binds the archive to a clean source revision and the
offline renderer hash. It permanently records:

```json
{
  "promotion_eligible": false,
  "trusted_producer": false
}
```

Inspect and securely extract those exact bytes with:

```sh
python3 scripts/linux_candidate_certification.py inspect \
  --candidate-manifest candidate-manifest.json \
  --candidate-archive eBIRForms-Linux-x64-COMMIT.tar.gz \
  --renderer-identity form-renderer-build-identity.json \
  --output-dir target/linux-candidate-inspection
```

The binder rejects archive traversal, links, special files, duplicate paths,
oversized payloads, a non-executable binary, changed renderer bytes, and a
source revision mismatch. It hashes the securely extracted candidate root,
binary, assets, renderer, and build identity.

This is a portable workflow candidate installed by secure extraction. Its
hashes are not the hashes of the public `.deb` or release tarball. The binding
therefore retains all three final-package statements as `false`:

```json
{
  "final_release_deb_verified": false,
  "final_release_tarball_verified": false,
  "release_package_signature_verified": false
}
```

## Optional network-denied startup probes

On Linux with `bubblewrap`, launch the real non-development binary in a fresh
network namespace. The X11 probe requires `DISPLAY`; the Wayland probe requires
`WAYLAND_DISPLAY`.

```sh
python3 scripts/linux_candidate_certification.py probe \
  --candidate-manifest candidate-manifest.json \
  --candidate-archive eBIRForms-Linux-x64-COMMIT.tar.gz \
  --renderer-identity form-renderer-build-identity.json \
  --output-dir target/linux-x11-probe \
  --backend x11

python3 scripts/linux_candidate_certification.py probe \
  --candidate-manifest candidate-manifest.json \
  --candidate-archive eBIRForms-Linux-x64-COMMIT.tar.gz \
  --renderer-identity form-renderer-build-identity.json \
  --output-dir target/linux-wayland-probe \
  --backend wayland
```

A startup probe is diagnostic only. It does not prove preview readiness,
printing, export, rollback, Xvfb/Weston parity, or final package installation.

## Closed dual-host attestation

The external collector contract is closed by:

- `packages/form-specs/schema/linux-candidate-certification-attestation-v1.schema.json`
- `packages/form-specs/schema/linux-candidate-certification-report-v1.schema.json`

One attestation must contain both native runs:

1. **X11/Xvfb** — `GpuiWryChild`, the GPUI-owned child WebView, no external
   browser.
2. **Wayland/Weston** — `GtkTopLevel`, the app-owned GTK/WebKitGTK top-level
   window, no external browser.

Each run is independent and must retain:

- the expected display variable, compositor version, GTK3 version, and
  WebKitGTK 4.1 version;
- an isolated `bubblewrap --unshare-net` namespace whose inode differs from
  the host namespace;
- open, readiness, close/reopen, and clean-shutdown evidence;
- the immutable envelope hash, one-use nonce, and two identical measurements
  of exactly two 612 x 936 point pages with zero clipping or overflow;
- the actual **Export PDF** control and native save chooser;
- a completed CUPS system-print job;
- a direct WebKitGTK PDF export validated by the Rust-owned verifier as exactly
  two nonempty 612 x 936 point pages;
- unchanged installed-root, pre-existing destination, and draft hashes;
- no temporary files and all eleven rollback/failure drills.

The verifier also checks the current Xvfb and Weston endpoints, installed
binary/tree hashes, GTK/WebKitGTK versions, and completed CUPS jobs. Build the
owned PDF verifier and replay the complete attestation with:

```sh
cargo build --locked --release -p bir-print \
  --features native-output-evidence \
  --bin verify_certification_pdf

python3 scripts/linux_candidate_certification.py verify-attestation \
  --candidate-manifest candidate-manifest.json \
  --candidate-archive eBIRForms-Linux-x64-COMMIT.tar.gz \
  --renderer-identity form-renderer-build-identity.json \
  --attestation linux-candidate-attestation.json \
  --pdf-verifier target/release/verify_certification_pdf \
  --report target/linux-candidate-certification-report.json
```

The PDF verifier is invoked with the explicit `linux` platform scope and
re-runs `bir-print::html_output::validate_pdf_file` for both exports.

## Why a successful report still cannot promote Linux

Every report still has:

```json
{
  "promotion_eligible": false,
  "trusted_producer": false,
  "operator_only": true,
  "promotion_satisfied": false
}
```

The collector producer is not trusted and the candidate is not the final
public `.deb` or tarball. A later, separately reviewed milestone must construct
the final release packages, bind their exact hashes/signature or publisher
lineage, install the `.deb` on a clean Linux machine, exercise both X11 and
Wayland from the installed bytes, audit both final payloads offline, and record
that evidence in a dedicated evidence-only change. This foundation never
writes `form-release-evidence.json` or any trusted-producer registry.
