#!/usr/bin/env python3
"""Done-condition of the fix-order build goal (.claude/GOAL.md).

Exit 0 only when every pull-request group in tools/form_findings_priority.json
has shipped and every finding of every form has a closed status:

1. Ledger (tools/form_findings_status/<group branch>.json, one file per PR
   group so parallel branches never edit the same file; each maps
   form code -> finding id -> entry): every finding of every form
   (automatic findings not set aside, plus the hand-added `manual` ones, keyed
   `manual-<n>`) has a status:
   - `fixed`: names its `pr` (a group id such as "PR 3") and a `proof`
     (a test name, or a screenshot or command that shows it fixed);
   - `not_a_bug`: carries the `reason` and the `reviewer` verdict that agreed;
   - `deferred`: needs `approved_by_maintainer: true` (never set by an agent).
2. Per group: its branch is on `public`, a PR from it into main exists (open
   or merged), the PR's `ci-ok` check passed, the PR body has a
   "Validated locally:" line, and the branch tip is inside the integration
   branch.
3. Integration: `integration/findings-fixes` is on `public`, a PR from it into
   main exists, and its `ci-ok` check passed.

Reads docs/form-findings/<CODE>/findings.json (gitignored, present locally).
Prints what is missing otherwise.

    python3 scripts/check_fix_order_goal.py [--offline]

`--offline` skips the GitHub checks (2 and 3) for a quick ledger count; it
always exits non-zero, because it does not check everything.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FINDINGS = ROOT / "docs" / "form-findings"
PRIORITY = ROOT / "tools" / "form_findings_priority.json"
LEDGER = ROOT / "tools" / "form_findings_status"
REPO = "hexuria/buwiz-forms"
REMOTE = "public"
INTEGRATION = "integration/findings-fixes"


def finding_ids(code):
    data = json.loads((FINDINGS / code / "findings.json").read_text())
    dropped = data.get("dropped", {})
    ids = [f["id"] for r in data["rules"].values() for f in r["findings"] if f["id"] not in dropped]
    ids += [f"manual-{i}" for i in range(len(data.get("manual", [])))]
    return ids


def check_ledger(plan, ledger):
    problems = []
    groups = {p["id"] for p in plan["prs"]}
    counts = {"fixed": 0, "not_a_bug": 0, "deferred": 0, "open": 0}
    for p in plan["prs"]:
        for f in p["forms"]:
            code = f["code"]
            entries = ledger.get(code, {})
            for fid in finding_ids(code):
                e = entries.get(fid)
                status = e.get("status") if isinstance(e, dict) else None
                if status == "fixed" and e.get("pr") in groups and str(e.get("proof", "")).strip():
                    counts["fixed"] += 1
                elif status == "not_a_bug" and str(e.get("reason", "")).strip() and str(e.get("reviewer", "")).strip():
                    counts["not_a_bug"] += 1
                elif status == "deferred" and e.get("approved_by_maintainer") is True:
                    counts["deferred"] += 1
                else:
                    counts["open"] += 1
                    if counts["open"] <= 40:
                        problems.append(f"{code} {fid}: {status or 'no status'}"
                                        + (" (incomplete)" if status in ("fixed", "not_a_bug", "deferred") else ""))
    if counts["open"] > 40:
        problems.append(f"... and {counts['open'] - 40} more open findings")
    return problems, counts


def run(*cmd):
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    return r.returncode, r.stdout.strip()


def pr_for(branch):
    code, out = run("gh", "pr", "list", "-R", REPO, "--head", branch, "--base", "main", "--state", "all",
                    "--json", "number,state,body", "--limit", "5")
    if code != 0 or not out:
        return None
    prs = [p for p in json.loads(out) if p["state"] in ("OPEN", "MERGED")]
    return prs[0] if prs else None


def ci_ok(number):
    code, out = run("gh", "pr", "checks", str(number), "-R", REPO, "--json", "name,bucket")
    if not out:
        return False
    try:
        checks = json.loads(out)
    except ValueError:
        return False
    return any(c["name"] == "ci-ok" and c["bucket"] == "pass" for c in checks)


def remote_tip(branch):
    code, out = run("git", "ls-remote", REMOTE, f"refs/heads/{branch}")
    return out.split()[0] if code == 0 and out else None


def check_github(plan):
    problems = []
    run("git", "fetch", "-q", REMOTE)
    integ_tip = remote_tip(INTEGRATION)
    if not integ_tip:
        problems.append(f"integration: {INTEGRATION} is not on {REMOTE}")
    else:
        pr = pr_for(INTEGRATION)
        if not pr:
            problems.append(f"integration: no PR from {INTEGRATION} into main")
        elif not ci_ok(pr["number"]):
            problems.append(f"integration: PR #{pr['number']} ci-ok has not passed")
    for p in plan["prs"]:
        branch, gid = p["branch"], p["id"]
        tip = remote_tip(branch)
        if not tip:
            problems.append(f"{gid}: branch {branch} is not on {REMOTE}")
            continue
        pr = pr_for(branch)
        if not pr:
            problems.append(f"{gid}: no PR from {branch} into main")
            continue
        if not ci_ok(pr["number"]):
            problems.append(f"{gid}: PR #{pr['number']} ci-ok has not passed")
        if "Validated locally:" not in (pr.get("body") or ""):
            problems.append(f"{gid}: PR #{pr['number']} body has no 'Validated locally:' line")
        if integ_tip:
            code, _ = run("git", "merge-base", "--is-ancestor", tip, integ_tip)
            if code != 0:
                problems.append(f"{gid}: {branch} tip {tip[:8]} is not in {INTEGRATION}")
    return problems


def main():
    offline = "--offline" in sys.argv[1:]
    plan = json.loads(PRIORITY.read_text())
    ledger = {}
    for f in sorted(LEDGER.glob("*.json")) if LEDGER.is_dir() else []:
        for code, entries in json.loads(f.read_text()).items():
            for fid, e in entries.items():
                # A finding closed in any group's file is closed.
                if fid not in ledger.setdefault(code, {}) or e.get("status") in ("fixed", "not_a_bug"):
                    ledger[code][fid] = e
    problems, counts = check_ledger(plan, ledger)
    if not offline:
        problems += check_github(plan)
    total = sum(counts.values())
    print(f"findings: {counts['fixed']} fixed, {counts['not_a_bug']} not a bug, "
          f"{counts['deferred']} deferred (approved), {counts['open']} open, of {total}")
    for line in problems:
        print(f"  - {line}")
    if offline:
        print("offline: GitHub checks skipped, so this is never a pass")
        return 1
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
