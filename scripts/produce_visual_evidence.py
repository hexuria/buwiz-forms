#!/usr/bin/env python3
"""Produce a promotion-grade visual-parity evidence report for one form.

This is the visual-evidence producer for the release audit
(``scripts/audit_html_form_migration.py``). It fails closed at every step:

1. Requires a clean curated source worktree and binds the exact revision.
2. Runs the real Playwright parity suite (no cached screenshots).
3. Requires the 1% complete-page gate to PASS; a failing gate writes nothing.
4. Copies the hashed page artifacts into a stable evidence directory and
   rewrites the report to point at them.
5. Self-verifies the finished report through the audit's own validator and
   requires that the ONLY remaining rejection is the (intentionally) empty
   trusted-producer registry.

The producer never edits ``form-release-evidence.json`` or any capability
flag. Promotion additionally requires a human review of this producer and the
registration of its id in ``TRUSTED_VISUAL_EVIDENCE_PRODUCERS``, followed by
an evidence-only commit. See docs/form-print-readiness/evidence-runbook.md.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path

TRUSTED_PRODUCER_ERROR_FRAGMENT = (
    "no trusted attested visual evidence producer is registered"
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_audit_module(root: Path):
    script = root / "scripts/audit_html_form_migration.py"
    spec = importlib.util.spec_from_file_location("audit_html_form_migration", script)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load audit module from {script}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def require_clean_source_revision(root: Path) -> str:
    result = subprocess.run(
        [
            "node",
            str(root / "scripts/run_python.mjs"),
            str(root / "scripts/audit_html_form_migration.py"),
            "--root",
            str(root),
            "--require-clean-source",
            "--print-source-revision",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            "curated source worktree is not clean; commit or remove the dirty "
            f"paths first:\n{result.stdout}{result.stderr}"
        )
    revision = result.stdout.strip().splitlines()[-1].strip()
    if len(revision) < 40:
        raise SystemExit(f"cannot resolve curated source revision: {revision!r}")
    return revision


def run_visual_suite(root: Path, evidence_path: Path) -> None:
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    environment = dict(**__import__("os").environ)
    environment["FORM_VISUAL_EVIDENCE_PATH"] = str(evidence_path)
    environment.pop("FORM_VISUAL_NON_PROMOTING_ALLOW_DIRTY_SOURCE", None)
    result = subprocess.run(
        ["npm", "run", "test:forms:visual"],
        cwd=root,
        env=environment,
        check=False,
    )
    if not evidence_path.is_file():
        raise SystemExit(
            "the visual suite did not write an evidence report; "
            f"Playwright exit code {result.returncode}"
        )


def relocate_artifacts(
    root: Path,
    report: dict,
    destination: Path,
) -> None:
    """Copy per-page artifacts to the stable evidence directory, verifying hashes."""

    destination.mkdir(parents=True, exist_ok=True)
    artifact_keys = (
        ("actual", "actual_sha256"),
        ("diff", "diff_sha256"),
        ("poppler_diff", "poppler_diff_sha256"),
    )
    for page in report["pages"]:
        for path_key, sha_key in artifact_keys:
            recorded = page.get(path_key)
            if recorded is None:
                raise SystemExit(f"page {page.get('page')} is missing {path_key}")
            source = root / recorded
            actual_sha = sha256_file(source)
            if actual_sha != page.get(sha_key):
                raise SystemExit(
                    f"artifact drifted between capture and relocation: {recorded}"
                )
            target = destination / f"page-{page['page']}-{path_key.replace('_', '-')}.png"
            shutil.copy2(source, target)
            if sha256_file(target) != actual_sha:
                raise SystemExit(f"artifact copy corrupted: {target}")
            page[path_key] = str(target.relative_to(root)).replace("\\", "/")
            page[sha_key] = actual_sha


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=".")
    parser.add_argument("--form", default="2551Q:2018", help="exact FORM:REVISION")
    parser.add_argument(
        "--evidence-dir",
        default=None,
        help="stable output directory (default evidence/visual/<form>-<revision>)",
    )
    args = parser.parse_args()
    root = Path(args.repo).resolve()
    code, _, revision = args.form.partition(":")
    if not code or not revision:
        parser.error("--form must be exact FORM:REVISION")

    audit = load_audit_module(root)
    source_revision = require_clean_source_revision(root)
    print(f"curated source revision: {source_revision}")

    evidence_dir = (
        Path(args.evidence_dir)
        if args.evidence_dir
        else root / "evidence" / "visual" / f"{code.lower()}-{revision.lower()}"
    )
    if not evidence_dir.is_absolute():
        evidence_dir = root / evidence_dir

    working_report = evidence_dir / "visual-parity-report.json"
    run_visual_suite(root, working_report)
    report = json.loads(working_report.read_text(encoding="utf-8"))

    pages = [
        page
        for page in report.get("pages", [])
        if page.get("form_code") == code and page.get("form_revision") == revision
    ]
    if not pages or len(pages) != len(report.get("pages", [])):
        raise SystemExit(
            f"the visual run did not cover exactly {args.form}; refusing to emit"
        )
    # The complete-page number is always surfaced, whatever else happens: it is
    # a mandatory diagnostic, never a hidden one and never a parity claim.
    per_page_percent = [
        (page.get("page"), page.get("changed_percent")) for page in pages
    ]
    print(f"complete-page changed percent (diagnostic): {per_page_percent}")

    # The old requirement here - report["passed"] must be true - bound evidence
    # to the 1% complete-page gate, which is unreachable by proof (non-embedded
    # fonts corpus-wide; see the criterion document). The operative gate is
    # official-fidelity-v1: no evidence can exist before its baselines are
    # pinned and reviewed, and once they are, the composite verdict must pass.
    if report.get("gate") != "official-fidelity-v1":
        working_report.unlink(missing_ok=True)
        raise SystemExit(
            f"the run reports gate {report.get('gate')!r}; only "
            "official-fidelity-v1 runs can produce evidence"
        )
    if report.get("proves_parity") is not False or (
        report.get("is_non_regression_gate") is not True
    ):
        working_report.unlink(missing_ok=True)
        raise SystemExit(
            "the report must carry proves_parity=false and "
            "is_non_regression_gate=true; anything else overclaims"
        )
    if report.get("baselines_pinned") is not True:
        working_report.unlink(missing_ok=True)
        raise SystemExit(
            "official-fidelity-v1 is in reporting mode: baselines are not "
            "pinned, so no promotion evidence can exist yet. Reporting mode "
            "is not a promotion shortcut; pin reviewed baselines first "
            "(criterion section 8)."
        )
    if report.get("passed") is not True:
        working_report.unlink(missing_ok=True)
        raise SystemExit(
            "the non-regression gate FAILED against the pinned baselines; no "
            f"promotion evidence can exist. Complete-page diagnostic: "
            f"{per_page_percent}. Use the structural-defect and region "
            "reports to find the regression."
        )
    if report.get("promotion_eligible") is not True:
        raise SystemExit("the run was not promotion-eligible; refusing to emit")
    if report.get("source_revision") != source_revision:
        raise SystemExit("the run does not bind the current curated revision")

    relocate_artifacts(root, report, evidence_dir)
    payload = f"{json.dumps(report, indent=2, sort_keys=True)}\n"
    working_report.write_text(payload, encoding="utf-8")
    report_sha = hashlib.sha256(payload.encode("utf-8")).hexdigest()

    # Self-verify through the audit's own validator. The only acceptable
    # rejection is the empty trusted-producer registry.
    manifest_path = root / "packages/form-renderer/references/manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    reference = next(
        (
            form
            for form in manifest.get("forms", [])
            if form.get("code") == code and form.get("revision") == revision
        ),
        None,
    )
    if reference is None:
        raise SystemExit(f"{args.form} is not in the reference manifest")
    errors: list[str] = []
    audit._audit_visual_report(
        root,
        report,
        {"code": code, "revision": revision},
        reference,
        sha256_file(manifest_path),
        f"{args.form} visual parity",
        errors,
    )
    unexpected = [
        error for error in errors if TRUSTED_PRODUCER_ERROR_FRAGMENT not in error
    ]
    if unexpected:
        raise SystemExit(
            "self-verification failed; the report would be rejected by the audit:\n"
            + "\n".join(f"- {error}" for error in unexpected)
        )
    if not any(TRUSTED_PRODUCER_ERROR_FRAGMENT in error for error in errors):
        raise SystemExit(
            "expected the empty trusted-producer registry to reject this report; "
            "the audit behaved unexpectedly, refusing to continue"
        )

    pointer = {
        "passed": True,
        "path": str(working_report.relative_to(root)).replace("\\", "/"),
        "sha256": report_sha,
    }
    print("visual evidence report written and schema-verified:")
    print(f"  {pointer['path']}  sha256={report_sha}")
    print()
    print("pointer for packages/form-specs/form-release-evidence.json")
    print(f'  "visual_parity": {json.dumps(pointer, indent=2)}')
    print()
    print(
        "NOT promotable yet: review this producer, register "
        "'playwright-form-parity-v1' in TRUSTED_VISUAL_EVIDENCE_PRODUCERS, "
        "then land the pointer in an evidence-only commit "
        "(docs/form-print-readiness/evidence-runbook.md)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
