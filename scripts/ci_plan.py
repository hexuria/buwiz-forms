#!/usr/bin/env python3
"""Decides what a CI run does: the `changes` job of ci.yml and the `plan` job of nsh.yml both run this, so the two
workflows always agree. docs/CI.md says the same in words.

Six lanes:
  none     a pull request that closed (or merged), one that is a draft (except an explicit suite-label request), or a
           labeled event whose label is not a suite label: nothing. ci.yml's jobs carry the closed/draft rule in their
           `if`, so those events take no runner at all; marking a draft ready starts the quick lane.
  quick    a pull request, or a push to main or codex/**: the quick checks on Linux, plus the Python script tests on
           Windows when the pull request changes a script input (ci.yml).
  tests    a push to test/**: the test suites; the branch name picks the systems.
  suite    a pull_request labeled event whose label is ci:suite*: the quick checks and the test suites; the label
           picks the systems. Only the labeled event asks for this lane, so a later push plans quick alone.
  manual   workflow_dispatch: the systems, suites, screen-test filter and shard the inputs name.
  nightly  the schedule: everything, on all three systems.

Within a lane, the changed files decide which suites are needed. A path class may only list files that no crate,
build script or test reads; everything unlisted is app code and runs everything. An unknown or empty change range
runs everything too.

Reads the event from the environment (see `main`), writes `name=value` lines to $GITHUB_OUTPUT and a summary to
$GITHUB_STEP_SUMMARY.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys

SYSTEMS = ("macos", "linux", "windows")
SUITES = ("rust", "screens", "build", "installer", "validation")
# A test/** push and a manual run on a branch split the screen tests over this many runners.
SHARDS = 3

# The pull-request labels that ask for the suites, and the systems each asks for. GitHub's expression equality
# is ordinal-ignore-case (it upper-cases), so the planner compares upper-case keys too; anything else plans nothing.
SUITE_LABEL_SYSTEMS = {
    "ci:suite": ("linux", "windows"),
    "ci:suite-linux": ("linux",),
    "ci:suite-windows": ("windows",),
    "ci:suite-macos": ("macos",),
    "ci:suite-all": SYSTEMS,
}
SUITE_LABEL_SYSTEMS_BY_UPPER = {name.upper(): systems for name, systems in SUITE_LABEL_SYSTEMS.items()}

# The `check` job's runner and toolchain target for each system, in the matrix order ci.yml has always used.
CHECK_OS = {
    "macos": {"os": "macos-15", "target": "aarch64-apple-darwin"},
    "linux": {"os": "ubuntu-22.04", "target": "x86_64-unknown-linux-gnu"},
    "windows": {"os": "windows-latest", "target": "x86_64-pc-windows-msvc"},
}

# Docs a Python script or script test reads are inputs to those tests, not plain docs. One list owns the rule;
# scripts/tests/test_ci_plan.py scans the scripts and fails when one reads a docs path missing here. A trailing
# slash is a prefix: check_fix_order_goal.py reads the local findings files under docs/form-findings/.
SCRIPT_INPUT_DOCS = (
    "docs/platforms/devin-blueprint.yml",
    "docs/CI.md",
    "docs/platforms/devin-environment.md",
    "docs/form-findings/",
)


def script_input_doc(path: str) -> bool:
    return any(path == item or (item.endswith("/") and path.startswith(item)) for item in SCRIPT_INPUT_DOCS)


# Read by people only. assets/brand and assets/badges feed local recipes (`just icons`, `just badges`), not a build;
# assets/readme holds the pictures README.md shows and nothing builds from it (a README picture change used to run
# every quick check as app code, #494). Markdown in nsh/ is not bundled or type checked. Markdown under rules/ or scripts/ and crate README files are NOT
# here: they feed code generation or policy checks.
DOCS = re.compile(r"^(docs/|[^/]+\.md$|crates/[^/]+/docs/|\.github/[^/]+\.md$|assets/(brand|badges|readme)/|nsh/.*\.md$)")
# The Windows installer. nsh.yml owns the NSH pin that generates the installer source.
WINDOWS_PKG = re.compile(
    r"^(assets/windows/|packaging/windows/|scripts/(audit_final_setup|package_msix|package_nsh_windows|smoke_msix)\.ps1$"
    r"|scripts/audit_installer_sources\.py$|\.github/workflows/(nsh|release-windows)\.yml$)"
)
# App files the installer also packs: they stay app code, and they also ask for the installer audit.
WINDOWS_PKG_INPUTS = re.compile(r"^(nsh/nsh\.json$|print-overrides/|assets/icon[^/]*\.png$|scripts/audit_no_legacy\.py$)")
MACOS_PKG = re.compile(r"^(assets/macos/|scripts/(package_macos_release|setup_macos_release_secrets)\.sh$|[^/]*entitlements[^/]*\.plist$)")
LINUX_PKG = re.compile(r"^packaging/linux/")
# Files only the Python script tests read (and workflows nothing here runs).
SCRIPTS_ONLY = re.compile(
    r"^(scripts/tests/|scripts/setup-(linux|linux-nsh|macos|impeccable)\.sh$|scripts/setup-windows\.ps1$"
    r"|scripts/impeccable\.pin$|\.github/workflows/(release[^/]*|bir|html-candidate-certification)\.yml$)"
)
SCRIPTS_WINDOWS_INPUTS = re.compile(r"^(scripts/|\.githooks/|\.github/workflows/|justfile$)")
# The NSH (TSX) app is a workspace of its own; nothing the workspace tests read is under nsh/. nsh/src/ and the
# app's Cargo files stay app code: the production-boundary check scans nsh/src (bir-rules-codegen status.rs,
# SHELL_SOURCE_ROOTS).
NSH_APP = re.compile(r"^nsh/(src/|Cargo\.(toml|lock)$)")
SCREEN_TEST = re.compile(r"^nsh/.*\.test\.tsx?$")
# What the NSH app's jobs read (nsh.yml's path filter before the fast lane): the app, the crates it is built on, the
# workspace it builds in, and the files that run it.
NSH_INPUTS = re.compile(
    r"^(nsh/|forms/|crates/bir-(app-api|core|host-shim|print|rules)/|Cargo\.(toml|lock)$|justfile$"
    r"|rust-toolchain\.toml$|\.github/workflows/nsh\.yml$|scripts/(ci_plan\.py|nsh_screen_tests\.sh)$)"
)
# The quick NSH type check lives in ci.yml.
NSH_CHECK_INPUTS = re.compile(NSH_INPUTS.pattern + r"|^\.github/workflows/ci\.yml$")
# The plug pin, plus every nsh source or test file whose text carries `feature = "agent"` gates. The policy test in
# scripts/tests/test_ci_plan.py keeps this set in step with the tree.
NSH_AGENT_CHECK_INPUTS = re.compile(
    r"^nsh/(Cargo\.(toml|lock)|src/agent\.rs|tests/(agent|agent_send|window_plane)\.rs)$"
)
# The extracted validation-rules corpus and what audits it.
VALIDATION = re.compile(
    r"^(rules/|crates/bir-rules(-codegen|-platform)?/|crates/bir-core/src/form_rules/"
    r"|crates/bir-core/src/validation_summary\.rs|evidence/validation-rules/|rust-toolchain"
    r"|\.github/workflows/ci\.yml$|scripts/ci_plan\.py$)"
)


class PlanError(ValueError):
    pass


def classify(path: str) -> str:
    if script_input_doc(path):
        return "scripts"
    if DOCS.match(path):
        return "docs"
    if WINDOWS_PKG.match(path):
        return "windows_pkg"
    if MACOS_PKG.match(path):
        return "macos_pkg"
    if LINUX_PKG.match(path):
        return "linux_pkg"
    if SCRIPTS_ONLY.match(path):
        return "scripts"
    if path.startswith("nsh/") and not NSH_APP.match(path):
        return "nsh_ui"
    return "app"


def what_changed(changed: list[str] | None) -> dict[str, bool]:
    """What the change can affect. None (unknown range) and an empty list mean everything."""
    if not changed:
        return dict(app=True, windows_pkg=True, other=True, nsh=True, nsh_check=True, nsh_agent_check=True,
                    nsh_beyond_tests=True, core_inputs=True, validation_paths=False, known=False, docs_only=False,
                    scripts_windows=True)
    classes = [classify(path) for path in changed]
    nsh_paths = [path for path in changed if NSH_INPUTS.match(path) and not DOCS.match(path)]
    return dict(
        app="app" in classes,
        windows_pkg="windows_pkg" in classes or any(WINDOWS_PKG_INPUTS.match(path) for path in changed),
        other=bool({"windows_pkg", "macos_pkg", "linux_pkg", "scripts"} & set(classes)),
        nsh=bool(nsh_paths),
        nsh_check=any(NSH_CHECK_INPUTS.match(path) and not DOCS.match(path) for path in changed),
        nsh_agent_check=any(NSH_AGENT_CHECK_INPUTS.match(path) for path in changed),
        nsh_beyond_tests=any(not SCREEN_TEST.match(path) for path in nsh_paths),
        # The plain-SQLite core tests: the crates, forms and workspace files, not the app in nsh/.
        core_inputs=any(not path.startswith("nsh/") for path in nsh_paths),
        validation_paths=any(VALIDATION.match(path) for path in changed),
        scripts_windows=any(SCRIPTS_WINDOWS_INPUTS.match(path) for path in changed),
        known=True,
        docs_only=all(class_name == "docs" for class_name in classes),
    )


def parse_names(value: str, allowed: tuple[str, ...], default: str, what: str) -> list[str]:
    names = [name.strip() for name in (value or default).split(",") if name.strip()]
    if names == ["all"]:
        return list(allowed)
    unknown = [name for name in names if name not in allowed]
    if unknown or not names:
        raise PlanError(f"unknown {what}: {', '.join(unknown) or '(none)'}; use all or a comma list of {', '.join(allowed)}")
    return [name for name in allowed if name in names]


def branch_systems(ref: str) -> list[str]:
    """test/linux/**, test/windows/**, test/macos/**: that system; test/all/**: all three; any other: Linux and Windows."""
    first = ref.removeprefix("refs/heads/test/").split("/", 1)[0]
    return {"linux": ["linux"], "windows": ["windows"], "macos": ["macos"], "all": list(SYSTEMS)}.get(first, ["linux", "windows"])


def has_label(labels: tuple[str, ...], wanted: str) -> bool:
    """Label membership with GitHub's ordinal-ignore-case name comparison."""
    wanted_key = wanted.upper()
    return any(label.upper() == wanted_key for label in labels)


def plan_quick_checks(plan: dict, *, event: str, branch: str, labels: tuple[str, ...], seen: dict[str, bool],
                      reasons: list[str]) -> None:
    """The quick lane's part of a plan."""
    if not seen["known"]:
        reasons.append("change range unknown or empty: every quick check")
    plan["check_systems"] = ["linux"] if seen["app"] else []
    plan["nsh_check"] = seen["nsh_check"]
    # Compile-proof the agent plug on pull requests only: the pin must be green before merge, while main and the
    # test lane already have the complete NSH suite's agent build.
    plan["nsh_agent_check"] = event == "pull_request" and seen["nsh_agent_check"]
    # `check` runs the Python script tests itself.
    plan["scripts"] = not seen["app"] and (seen["other"] or seen["windows_pkg"])
    plan["scripts_windows"] = event == "pull_request" and seen["scripts_windows"]
    # The validation-rules audits: Linux only, and only when asked for by a path, the label or the branch. An
    # unknown range does not start them (a new branch or a force push must not start six jobs).
    if seen["validation_paths"]:
        plan["validation_systems"] = ["linux"]
        reasons.append("validation-rules audits: validation-rules paths changed")
    elif event == "pull_request" and has_label(labels, "run-validation"):
        plan["validation_systems"] = ["linux"]
        reasons.append("validation-rules audits: run-validation label")
    elif branch.startswith("ebirforms-"):
        plan["validation_systems"] = ["linux"]
        reasons.append(f"validation-rules audits: branch {branch}")


def plan_test_suites(plan: dict, *, chosen: list[str], seen: dict[str, bool], reasons: list[str], source: str) -> None:
    """The test suites the changed files ask for on the chosen systems. test/** pushes and suite labels share this."""
    if not seen["known"]:
        reasons.append("change range against main unknown or empty: every suite")
    plan["chosen_systems"] = list(chosen)
    plan["check_systems"] = chosen if seen["app"] else []
    plan["check_tests"] = True
    plan["installer"] = seen["windows_pkg"] and "windows" in chosen
    plan["screens"] = seen["nsh"]
    plan["build"] = plan["nsh_rust"] = seen["nsh_beyond_tests"]
    plan["core_plain"] = seen["core_inputs"] and "linux" in chosen
    plan["nsh_systems"] = chosen if seen["nsh"] else []
    plan["shards"], plan["shard_count"] = list(range(SHARDS)), SHARDS
    reasons.append(f"systems from {source}: {', '.join(chosen)}")


def decide(
    *,
    event: str,
    ref: str,
    action: str = "",
    label: str = "",
    state: str = "",
    draft: bool = False,
    branch: str = "",
    sync_lane: str = "quick",
    changed: list[str] | None = None,
    labels: tuple[str, ...] = (),
    systems: str = "",
    suites: str = "",
    screens: str = "",
    shard: str = "all",
) -> dict:
    """The whole decision. Returns the outputs (Python values) plus `reasons`, lines for the run summary."""
    on_main = ref == "refs/heads/main"
    reasons: list[str] = []
    test_push = event == "push" and ref.startswith("refs/heads/test/")
    if sync_lane not in ("quick", "none"):
        raise PlanError(f"unknown pull-request synchronize lane {sync_lane}: use quick or none")

    suite_systems = (SUITE_LABEL_SYSTEMS_BY_UPPER.get(label.upper())
                     if event == "pull_request" and action == "labeled" else None)
    closed = action == "closed" or state.lower() == "closed"

    if event == "pull_request" and closed:
        lane = "none"
        reasons.append("closed pull request: nothing runs")
    elif event == "pull_request" and action == "labeled" and suite_systems is None:
        lane = "none"
        reasons.append("pull request label is not a suite label: nothing runs")
    elif event == "pull_request" and action == "synchronize" and sync_lane == "none":
        lane = "none"
        reasons.append("pull request synchronize: nothing runs")
    elif event == "pull_request" and draft and suite_systems is None:
        lane = "none"
        reasons.append("draft pull request: nothing runs until it is marked ready")
    elif suite_systems is not None:
        lane = "suite"
        reasons.append(f"suite label: {label}")
    elif event in ("pull_request", "push") and not test_push:
        lane = "quick"
    elif test_push:
        lane = "tests"
    elif event == "workflow_dispatch":
        lane = "manual"
    elif event == "schedule":
        lane = "nightly"
    else:
        raise PlanError(f"no lane for event {event}")

    plan = dict(lane=lane, chosen_systems=[], check_systems=[], check_tests=False, nsh_check=False,
                nsh_agent_check=False, scripts=False, scripts_windows=False, installer=False, validation_systems=[],
                nsh_systems=[], screens=False, build=False, nsh_rust=False, core_plain=False, shards=[0], shard_count=1,
                screens_filter="", docs_only=False, legacy_in_changes=False, legacy_guard=False)

    if lane in ("quick", "suite"):
        seen = what_changed(changed)
        plan["docs_only"] = seen["docs_only"]
        plan_quick_checks(plan, event=event, branch=branch, labels=labels, seen=seen, reasons=reasons)

    if lane == "tests":
        chosen = branch_systems(ref)
        seen = what_changed(changed)
        plan["docs_only"] = seen["docs_only"]
        plan_test_suites(plan, chosen=chosen, seen=seen, reasons=reasons, source="the branch name")
    elif lane == "suite":
        chosen = list(suite_systems)
        plan_test_suites(plan, chosen=chosen, seen=seen, reasons=reasons, source="the label")
    elif lane == "manual":
        chosen = parse_names(systems, SYSTEMS, "linux,windows", "system")
        wanted = parse_names(suites, SUITES, "all", "suite")
        if on_main and (wanted != list(SUITES) or screens or shard != "all"):
            # Only main's runs save the caches every branch restores, and the first run to save a cache entry defines
            # it for good: a partial run on main could save a cache missing what the full jobs need.
            reasons.append("main: suites, screen filter and shard ignored, so the caches main saves stay complete")
            wanted, screens, shard = list(SUITES), "", "all"
        plan["chosen_systems"] = list(chosen)
        plan["check_systems"] = chosen if "rust" in wanted else []
        plan["check_tests"] = True
        plan["installer"] = "installer" in wanted and "windows" in chosen
        plan["validation_systems"] = chosen if "validation" in wanted else []
        plan["screens"] = "screens" in wanted
        plan["build"] = "build" in wanted
        plan["nsh_rust"] = "rust" in wanted
        plan["core_plain"] = "rust" in wanted and "linux" in chosen
        plan["nsh_systems"] = chosen if (plan["screens"] or plan["build"] or plan["nsh_rust"]) else []
        plan["screens_filter"] = screens.strip()
        if shard not in ("all", *map(str, range(SHARDS))):
            raise PlanError(f"unknown shard {shard}: use all or 0 to {SHARDS - 1}")
        # main runs each system's screen tests on one runner, so the cache it saves holds what every shard needs.
        if not on_main:
            plan["shards"] = list(range(SHARDS)) if shard == "all" else [int(shard)]
            plan["shard_count"] = SHARDS
        reasons.append(f"manual run: systems {', '.join(chosen)}; suites {', '.join(wanted)}")

    elif lane == "nightly":
        plan.update(chosen_systems=list(SYSTEMS), check_systems=list(SYSTEMS), check_tests=True, installer=True,
                    validation_systems=list(SYSTEMS), screens=True, build=True, nsh_rust=True, core_plain=True,
                    nsh_systems=list(SYSTEMS), shards=[0])
        reasons.append("nightly run: everything on all three systems")

    plan["legacy_in_changes"] = lane in ("quick", "suite") and plan["docs_only"]
    plan["legacy_guard"] = (not ref.startswith("refs/heads/test/") and lane in ("quick", "suite", "manual", "nightly")
                            and not plan["legacy_in_changes"])
    if not plan["screens"]:
        # Without screen tests only the lead shard has work.
        plan["shards"], plan["shard_count"] = [0], 1
    # The Linux release packages (release-linux.yml, x64 and arm64): built only by the nightly run and a manual run
    # that asks for the build on Linux; never by a pull request or a test/** push.
    plan["linux_packages"] = lane in ("manual", "nightly") and plan["build"] and "linux" in plan["nsh_systems"]
    plan["reasons"] = reasons
    return plan


def outputs(plan: dict) -> dict[str, str]:
    """The plan as the string outputs the workflows read."""
    def flag(value: bool) -> str:
        return "true" if value else "false"

    check = [dict(CHECK_OS[name], tests=plan["check_tests"]) for name in SYSTEMS if name in plan["check_systems"]]
    validation = [CHECK_OS[name]["os"] for name in SYSTEMS if name in plan["validation_systems"]]
    nsh = plan["nsh_systems"]
    shards = plan["shards"]
    return {
        "lane": plan["lane"],
        "chosen_systems": ",".join(plan["chosen_systems"]),
        "docs_only": flag(plan["docs_only"]),
        "legacy_in_changes": flag(plan["legacy_in_changes"]),
        "legacy_guard": flag(plan["legacy_guard"]),
        "check": flag(bool(check)),
        "check_matrix": json.dumps({"include": check}, separators=(",", ":")),
        "nsh_check": flag(plan["nsh_check"]),
        "nsh_agent_check": flag(plan["nsh_agent_check"]),
        "scripts": flag(plan["scripts"]),
        "scripts_windows": flag(plan["scripts_windows"]),
        "installer": flag(plan["installer"]),
        "validation": flag(bool(validation)),
        "validation_matrix": json.dumps(validation, separators=(",", ":")),
        "nsh_macos": flag("macos" in nsh),
        "nsh_linux": flag("linux" in nsh),
        "nsh_windows": flag("windows" in nsh),
        "screens": flag(plan["screens"]),
        "build": flag(plan["build"]),
        "nsh_rust": flag(plan["nsh_rust"]),
        "core_plain": flag(plan["core_plain"]),
        "linux_packages": flag(plan["linux_packages"]),
        "shards": json.dumps(shards),
        "shard_count": str(plan["shard_count"]),
        "lead_shard": str(shards[0]),
        "screens_filter": plan["screens_filter"],
    }


def changed_files(env: dict[str, str]) -> list[str] | None:
    """The files this run's change touches, or None when the range is unknown."""
    event, ref = env.get("EVENT", ""), env.get("REF", "")
    base, head = env.get("PR_BASE", ""), env.get("PR_HEAD", "")
    before, after = env.get("PUSH_BEFORE", ""), env.get("PUSH_AFTER", "")
    if event == "pull_request" and base and head:
        span = f"{base}...{head}"
    elif event == "push" and ref.startswith("refs/heads/test/"):
        # A test branch carries a pull request's head: test what the pull request changes, whatever was there before.
        span = "origin/main...HEAD"
    elif event == "push" and before and after and not re.fullmatch(r"0+", before):
        span = f"{before}..{after}"
    else:
        return None
    # --no-renames: with rename detection git prints only a moved file's NEW path, so a script moved into docs/
    # would read as one docs file while a code input is deleted. Both paths must be classified.
    result = subprocess.run(["git", "diff", "--name-only", "--no-renames", span], capture_output=True, text=True,
                            check=False)
    if result.returncode != 0:
        return None
    return [line for line in result.stdout.splitlines() if line]


def pull_request_labels(env: dict[str, str]) -> tuple[str, ...]:
    # Read now, not from the event payload, so adding the label and then "Re-run all jobs" picks it up.
    if env.get("EVENT") != "pull_request" or not env.get("PR_NUMBER"):
        return ()
    result = subprocess.run(
        ["gh", "pr", "view", env["PR_NUMBER"], "--json", "labels", "--jq", ".labels[].name"],
        capture_output=True, text=True, check=False,
    )
    return tuple(result.stdout.split()) if result.returncode == 0 else ()


def main() -> int:
    env = dict(os.environ)
    try:
        plan = decide(
            event=env.get("EVENT", ""),
            ref=env.get("REF", ""),
            action=env.get("ACTION", ""),
            label=env.get("LABEL", ""),
            state=env.get("PR_STATE", ""),
            draft=env.get("DRAFT", "") == "true",
            branch=env.get("BRANCH", ""),
            sync_lane=env.get("PR_SYNC_LANE", "quick"),
            changed=changed_files(env) if env.get("EVENT") in ("pull_request", "push") else None,
            labels=pull_request_labels(env),
            systems=env.get("IN_SYSTEMS", ""),
            suites=env.get("IN_SUITES", ""),
            screens=env.get("IN_SCREENS", ""),
            shard=env.get("IN_SHARD", "") or "all",
        )
    except PlanError as error:
        print(f"::error::{error}")
        return 1
    values = outputs(plan)
    with open(env["GITHUB_OUTPUT"], "a", encoding="utf-8") as out:
        for name, value in values.items():
            out.write(f"{name}={value}\n")
    lines = [f"lane: {plan['lane']}", *plan["reasons"]]
    lines += [f"{name}: {value}" for name, value in values.items()]
    summary = "\n".join(lines) + "\n"
    print(summary, end="")
    if env.get("GITHUB_STEP_SUMMARY"):
        with open(env["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as out:
            out.write("```\n" + summary + "```\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
