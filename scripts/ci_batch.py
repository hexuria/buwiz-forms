#!/usr/bin/env python3
"""Proves several pull requests together (docs/CI.md, "Proving several pull requests together").

  build  [system] [--dry-run] [--allow-unchecked <pr>] [--allow-unsuited <pr>] <pr> <pr> ...
                                  main plus the squash of each pull request, in that order, pushed to
                                  test/<system>/batch-<pr>-<pr>... (default system linux-windows)
  test-pr <pr> [system]           fetch and push the exact head GitHub reports to test/<system>/pr-<pr>
  status <branch>                 green or red per workflow run of the batch; for red, the failed jobs and a bisect
  merge  <branch>                 prints (never runs) the merge commands, after checking nothing moved
  verify <branch>                 after the merges: is main's tree the batch's tree?

Every pull request's required `ci-ok` must be `success` at its current head; pending, missing or not green refuses,
and nothing is pushed. Every pull request must also have ci.yml's `tests-ok` green at that head, per system the
batch asks for, from a pull_request suite run or a test/** push. When the planner says the changed files need the app
suite, each system must also have every `NSH app` shard green at that head, in the same two kinds of proof.
`--dry-run` proves the same batch without pushing. `--allow-unchecked <pr>` and `--allow-unsuited <pr>` (repeatable)
are the hand-run escape hatches.

The batch is built with git plumbing only (`git merge-tree`, `git commit-tree`) in the checkout's own object store:
no working tree, index or local branch is touched and nothing is written outside the repository. Each step merges a
pull request's head onto the running commit with the merge base git finds, then commits that tree on the running
commit: what `gh pr merge --squash` will do once the pull requests before it are merged.

The tip of the batch branch is a commit with the batch's tree whose message is the record: main's sha, each pull
request with its full head sha in order, the tree hash and the systems. `merge` and `verify` read only that record.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional, Sequence, Tuple

SCRIPT_DIRECTORY = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIRECTORY not in sys.path:
    sys.path.insert(0, SCRIPT_DIRECTORY)

import ci_plan  # noqa: E402

REPO = "hexuria/buwiz-forms"
SYSTEMS = ("linux-windows", "linux", "windows", "macos", "all")
DEFAULT_SYSTEM = "linux-windows"
CI_WORKFLOW = "CI"
CI_WORKFLOW_FILE = "ci.yml"
CI_WORKFLOW_PATH = ".github/workflows/ci.yml"
SUITE_WORKFLOW = "NSH app"
SUITE_WORKFLOW_FILE = "nsh.yml"
SUITE_WORKFLOW_PATH = ".github/workflows/nsh.yml"
# The workflows a push to test/** starts (ci.yml and nsh.yml).
WORKFLOWS = ((CI_WORKFLOW_FILE, CI_WORKFLOW), (SUITE_WORKFLOW_FILE, SUITE_WORKFLOW))
SHA = re.compile(r"^[0-9a-f]{40}$")
FAILED = ("failure", "timed_out", "cancelled", "startup_failure", "action_required", "stale")
# Where a batch's record commit is also kept locally, in case its branch was deleted (`just ci-test-clean`).
LOCAL_REF = "refs/ci-batch/"
SYSTEM_LABELS = {"macos": "macOS", "linux": "Linux", "windows": "Windows"}


class BatchError(Exception):
    pass


@dataclass
class Pull:
    number: int
    head: str
    title: str = ""
    suite_runs: List[int] = field(default_factory=list, compare=False)


@dataclass
class SuiteProof:
    state: str
    run_id: Optional[int] = None
    older_green: Optional[int] = None


@dataclass
class Record:
    main: str
    pulls: List[Pull]
    tree: str
    system: str
    allow_unsuited: List[int] = field(default_factory=list)

    def numbers(self) -> List[int]:
        return [pull.number for pull in self.pulls]


# ---------------------------------------------------------------------------------------------------------------
# The record in the tip commit's message.

def format_record(record: Record) -> str:
    numbers = " ".join(f"#{n}" for n in record.numbers())
    lines = [f"ci-batch: {numbers} onto main {record.main[:12]}", ""]
    lines.append("Squashes, in merge order, on main's sha below. Proof only: never merge this branch.")
    lines.append("")
    lines.append(f"Batch-Main: {record.main}")
    lines += [f"Batch-PR: {pull.number} {pull.head}" for pull in record.pulls]
    lines += [f"Batch-Allow-Unsuited: {pull.number} {pull.head}" for pull in record.pulls
              if pull.number in record.allow_unsuited]
    lines.append(f"Batch-Tree: {record.tree}")
    lines.append(f"Batch-Systems: {record.system}")
    return "\n".join(lines) + "\n"


def parse_record(message: str) -> Record:
    main = tree = system = ""
    pulls: List[Pull] = []
    unsuited: List[Tuple[int, str]] = []
    for line in message.splitlines():
        key, _, value = line.partition(": ")
        value = value.strip()
        if key == "Batch-Main":
            main = value
        elif key == "Batch-Tree":
            tree = value
        elif key == "Batch-Systems":
            system = value
        elif key in ("Batch-PR", "Batch-Allow-Unsuited"):
            number, _, head = value.partition(" ")
            if not number.isdigit() or not SHA.match(head):
                raise BatchError(f"bad {key} line in the record: {line}")
            if key == "Batch-PR":
                pulls.append(Pull(int(number), head))
            else:
                unsuited.append((int(number), head))
    if not SHA.match(main) or not SHA.match(tree) or not pulls or system not in SYSTEMS:
        raise BatchError("this commit carries no complete ci-batch record (Batch-Main, Batch-PR, Batch-Tree, "
                         "Batch-Systems)")
    heads = {pull.number: pull.head for pull in pulls}
    numbers = [number for number, _ in unsuited]
    if len(set(numbers)) != len(numbers) or any(heads.get(number) != head for number, head in unsuited):
        raise BatchError("bad Batch-Allow-Unsuited line in the record: it does not match a Batch-PR line")
    return Record(main, pulls, tree, system, numbers)


def branch_name(system: str, numbers: Sequence[int]) -> str:
    return f"test/{system}/batch-" + "-".join(str(n) for n in numbers)


def parse_build_args(args: Sequence[str]) -> Tuple[str, List[int], bool, List[int], List[int]]:
    def number(arg: str, label: str) -> int:
        value = arg.lstrip("#")
        if not value.isdigit():
            raise BatchError(f"not a {label}: {arg} (systems: {', '.join(SYSTEMS)}, given first)")
        return int(value)

    positional: List[str] = []
    allow_unchecked: List[int] = []
    allow_unsuited: List[int] = []
    dry_run = False
    args = list(args)
    i = 0
    while i < len(args):
        arg = args[i]
        if arg == "--dry-run":
            dry_run = True
        elif arg in ("--allow-unchecked", "--allow-unsuited"):
            flag = arg[2:]
            i += 1
            if i == len(args):
                raise BatchError(f"{arg} takes a pull request number")
            target = allow_unchecked if flag == "allow-unchecked" else allow_unsuited
            target.append(number(args[i], f"pull request number after {arg}"))
        elif arg.startswith("--allow-unchecked=") or arg.startswith("--allow-unsuited="):
            flag, _, value = arg.partition("=")
            if not value:
                raise BatchError(f"{flag} takes a pull request number")
            target = allow_unchecked if flag == "--allow-unchecked" else allow_unsuited
            target.append(number(value, f"pull request number after {flag}"))
        else:
            positional.append(arg)
        i += 1

    system = DEFAULT_SYSTEM
    if positional and positional[0] in SYSTEMS:
        system = positional.pop(0)
    numbers: List[int] = []
    for arg in positional:
        value = number(arg, "pull request number")
        if value in numbers:
            raise BatchError(f"#{value} is listed twice")
        numbers.append(value)
    if not numbers:
        raise BatchError("name at least one pull request: just ci-batch [system] <pr> <pr> ...")
    return system, numbers, dry_run, allow_unchecked, allow_unsuited


# ---------------------------------------------------------------------------------------------------------------
# GitHub. Tests replace this with a fake that returns the same shapes.

class GitHub:
    def __init__(self, repo: str = REPO):
        self.repo = repo

    def _gh(self, *args: str) -> str:
        result = subprocess.run(["gh", *args], capture_output=True, text=True, check=False)
        if result.returncode != 0:
            raise BatchError(f"gh {' '.join(args[:3])} failed: {result.stderr.strip()}")
        return result.stdout

    def pull(self, number: int) -> dict:
        """One read per pull request, so the head, base, draft flag and state belong together."""
        return json.loads(self._gh("pr", "view", str(number), "--repo", self.repo, "--json",
                                   "number,headRefOid,baseRefName,isDraft,state,title"))

    def check_runs(self, sha: str) -> List[dict]:
        """The check runs reported on one commit."""
        data = json.loads(self._gh("api", f"repos/{self.repo}/commits/{sha}/check-runs"
                                          "?check_name=ci-ok&filter=latest&per_page=100"))
        return data.get("check_runs", [])

    def workflow_runs(self, sha: str, workflow_file: str = SUITE_WORKFLOW_FILE) -> List[dict]:
        """The runs of one workflow file reported for one head sha."""
        path = f".github/workflows/{workflow_file}"
        data = json.loads(self._gh("api", f"repos/{self.repo}/actions/workflows/{workflow_file}/runs"
                                          f"?head_sha={sha}&per_page=100"))
        return [run for run in data.get("workflow_runs", [])
                if run.get("path") == path and run.get("head_sha") == sha]

    def suite_jobs(self, run_id: int) -> List[dict]:
        """Every job of one workflow run, one page at a time."""
        jobs: List[dict] = []
        page = 1
        while True:
            data = json.loads(self._gh("api", f"repos/{self.repo}/actions/runs/{run_id}/jobs"
                                              f"?per_page=100&page={page}"))
            batch = data.get("jobs", [])
            jobs.extend(batch)
            total = data.get("total_count")
            if len(batch) < 100 or (isinstance(total, int) and len(jobs) >= total):
                return jobs
            page += 1

    def job_log(self, job_id: int) -> str:
        return self._gh("api", f"repos/{self.repo}/actions/jobs/{job_id}/logs")

    def main(self) -> Tuple[str, str]:
        """main's commit sha and tree hash, in one read."""
        data = json.loads(self._gh("api", f"repos/{self.repo}/commits/main"))
        return data["sha"], data["commit"]["tree"]["sha"]

    def runs(self, branch: str, workflow: Optional[str] = None) -> List[dict]:
        args = ["run", "list", "--repo", self.repo, "--branch", branch, "--limit", "100"]
        if workflow is not None:
            args += ["--workflow", workflow]
        args += ["--json", "databaseId,workflowName,status,conclusion,headSha,url,event,createdAt"]
        return json.loads(self._gh(*args))

    def jobs(self, run_id: int) -> List[dict]:
        return json.loads(self._gh("run", "view", str(run_id), "--repo", self.repo, "--json", "jobs"))["jobs"]


def checked_pull(number: int, data: dict) -> Tuple[Optional[Pull], Optional[str]]:
    """The pull request, or why it cannot be batched."""
    head = data.get("headRefOid") or ""
    if data.get("number") != number:
        return None, f"#{number}: GitHub answered for #{data.get('number')}"
    if data.get("state") != "OPEN":
        return None, f"#{number} is {str(data.get('state', 'not open')).lower()}"
    if data.get("isDraft"):
        return None, f"#{number} is a draft: mark it ready first"
    if data.get("baseRefName") != "main":
        return None, f"#{number} is based on {data.get('baseRefName')!r}, not main: retarget it to main first"
    if not SHA.match(head):
        return None, f"#{number}: GitHub gave no full head sha ({head!r})"
    return Pull(number, head, data.get("title") or ""), None


def ci_ok_state(check_runs: Sequence[dict], head: str) -> str:
    """The state of the newest ci-ok check run at exactly this head."""
    at_head = [run for run in check_runs if run.get("name") == "ci-ok" and run.get("head_sha") == head]
    if not at_head:
        return "missing"
    run = max(at_head, key=lambda r: (r.get("id") or 0, r.get("started_at") or "", r.get("completed_at") or ""))
    if run.get("status") != "completed":
        return "pending"
    return str(run.get("conclusion") or "no conclusion")


def enforce_ci_ok(github, pulls: Sequence[Pull], allow_unchecked: Sequence[int],
                  say: Callable[[str], None]) -> None:
    unchecked = set(allow_unchecked)
    batched = {pull.number for pull in pulls}
    for number in sorted(unchecked - batched):
        say(f"*** WARNING: --allow-unchecked #{number} is not in the batch; the build goes on ***")
    refusals = []
    for pull in pulls:
        if pull.number in unchecked:
            say(f"*** ALLOW-UNCHECKED: #{pull.number} at {pull.head}; skipping the ci-ok gate ***")
            continue
        state = ci_ok_state(github.check_runs(pull.head), pull.head)
        if state != "success":
            wording = ("ci-ok is not reported at this head (a run still going, or none)" if state == "missing"
                       else f"ci-ok is {state}")
            refusals.append(f"#{pull.number} at {pull.head}: {wording}; nothing was pushed")
    if refusals:
        raise BatchError("refused by the ci-ok gate:\n  " + "\n  ".join(refusals))


def needs_suite(changed: Sequence[str]) -> bool:
    """True when the planner would run the NSH app jobs for these changed files."""
    return bool(ci_plan.what_changed(list(changed))["nsh"])


def batch_systems(system: str) -> List[str]:
    """The systems a test/<system>/... branch asks the planner for."""
    return ci_plan.branch_systems(f"refs/heads/test/{system}/batch")


def shard_jobs(jobs: Sequence[dict], system: str) -> Dict[int, dict]:
    label = re.escape(SYSTEM_LABELS[system])
    pattern = re.compile(rf"^NSH app \({label}, shard (\d+)\)$")
    found = {}
    for job in jobs:
        match = pattern.match(job.get("name") or "")
        if match:
            found[int(match.group(1))] = job
    return found


def infer_job_systems(jobs: Sequence[dict]) -> List[str]:
    return [system for system in ci_plan.SYSTEMS if shard_jobs(jobs, system)]


def plan_log_lines(log: str) -> List[str]:
    """A plan job log's lines with GitHub's timestamps and terminal colours stripped."""
    lines = []
    for line in log.splitlines():
        text = line.lstrip("\ufeff")
        text = re.sub(r"^\d{4}-\d{2}-\d{2}T\S+Z\s+", "", text)
        text = re.sub(r"\x1b\[[0-9;]*m", "", text).strip()
        if text:
            lines.append(text)
    return lines


def plan_log_values(log: str) -> Dict[str, str]:
    """The `name: value` lines ci_plan prints, read from a plan job log with its timestamps stripped."""
    values = {}
    for text in plan_log_lines(log):
        key, colon, value = text.partition(":")
        if colon and re.fullmatch(r"[A-Za-z_]+", key):
            values[key] = value.strip()
    return values


def read_plan_output(github, jobs: Sequence[dict], job_name: str) -> Tuple[Dict[str, str], List[str]]:
    """The planner's output values and summary lines from one run's plan job log, or empty when it cannot be read."""
    plan_job = next((job for job in jobs if job.get("name") == job_name), None)
    if plan_job is None or not isinstance(plan_job.get("id"), int):
        return {}, []
    try:
        log = github.job_log(plan_job["id"])
    except BatchError:
        return {}, []
    return plan_log_values(log), plan_log_lines(log)


def parse_system_names(value: str) -> List[str]:
    """A comma or JSON list of system names, in the planner's canonical order."""
    names: List[str]
    try:
        decoded = json.loads(value)
    except (TypeError, json.JSONDecodeError):
        decoded = None
    if isinstance(decoded, list):
        names = [str(name) for name in decoded]
    else:
        names = [name for name in re.split(r"\s*,\s*|\s+", value.strip()) if name]
    return [system for system in ci_plan.SYSTEMS if system in names]


def planned_chosen_systems(values: Dict[str, str], lines: Sequence[str]) -> List[str]:
    """The systems a test lane or suite-label run was asked to cover, before changed-file narrowing."""
    if "chosen_systems" in values:
        return parse_system_names(values["chosen_systems"])
    for line in lines:
        match = re.fullmatch(r"systems from (?:the label|the branch name): (.+)", line)
        if match:
            return parse_system_names(match.group(1))
    return []


def tests_ok_job(jobs: Sequence[dict]) -> Optional[dict]:
    return next((job for job in jobs if job.get("name") == "tests-ok"), None)


def job_state(run: dict, job: dict) -> str:
    """success, failed, running or missing for one job inside one workflow run."""
    if run.get("status") != "completed" or job.get("status") != "completed":
        return "running"
    conclusion = job.get("conclusion")
    if conclusion == "success":
        return "success"
    if conclusion in (None, "skipped", "cancelled"):
        return "missing"
    return "failed"


def manual_suite_context(github, jobs: Sequence[dict]) -> dict:
    """What a workflow_dispatch run planned, as far as its plan job log proves it."""
    values, _lines = read_plan_output(github, jobs, "plan")
    if all(f"nsh_{system}" in values for system in ci_plan.SYSTEMS):
        systems = [system for system in ci_plan.SYSTEMS if values.get(f"nsh_{system}") == "true"]
    else:
        systems = infer_job_systems(jobs)
    try:
        shard_count = int(values.get("shard_count") or ci_plan.SHARDS)
    except ValueError:
        shard_count = ci_plan.SHARDS

    try:
        selected_shards = json.loads(values.get("shards") or "[]")
    except (TypeError, json.JSONDecodeError):
        selected_shards = []
    every_shard = (isinstance(selected_shards, list)
                   and sorted(shard for shard in selected_shards if isinstance(shard, int)) == list(range(shard_count))
                   and len(selected_shards) == shard_count)
    complete_outputs = all(values.get(name) == "true" for name in ("screens", "build", "nsh_rust", "validation"))
    if "windows" in systems:
        complete_outputs = complete_outputs and values.get("installer") == "true"
    complete = (complete_outputs and every_shard and not values.get("screens_filter", "").strip())
    return {"systems": systems, "shard_count": shard_count, "partial": not complete}


def push_suite_context(run: dict, changed: Sequence[str]) -> dict:
    branch = run.get("head_branch") or ""
    if not branch.startswith("test/"):
        return {"systems": [], "shard_count": ci_plan.SHARDS, "partial": False}
    plan = ci_plan.decide(event="push", ref=f"refs/heads/{branch}", branch=branch, changed=list(changed))
    return {"systems": plan["nsh_systems"], "shard_count": plan["shard_count"], "partial": False}


def suite_run_context(github, run: dict, jobs: Sequence[dict], changed: Sequence[str]) -> dict:
    if run.get("event") == "push":
        return push_suite_context(run, changed)
    if run.get("event") == "workflow_dispatch":
        return manual_suite_context(github, jobs)
    if run.get("event") == "schedule":
        plan = ci_plan.decide(event="schedule", ref="refs/heads/main")
        return {"systems": plan["nsh_systems"], "shard_count": plan["shard_count"], "partial": False}
    return {"systems": [], "shard_count": ci_plan.SHARDS, "partial": False}


def classify_suite_run(run: dict, system: str, jobs: Sequence[dict], context: dict) -> str:
    """success, failed, running, missing or partial for one system in one run."""
    if run.get("status") != "completed":
        return "running"
    conclusion = run.get("conclusion")
    if conclusion in ("skipped", "cancelled"):
        return "missing"
    if conclusion != "success":
        return "failed"
    if context["partial"]:
        return "partial"

    shards = shard_jobs(jobs, system)
    if not shards:
        return "missing"
    missing = False
    for shard in range(int(context["shard_count"])):
        job = shards.get(shard)
        if job is None or job.get("conclusion") in (None, "skipped"):
            missing = True
            continue
        if job.get("conclusion") != "success":
            return "failed"
    return "missing" if missing else "success"


def shard_run_covers(run: dict, jobs: Sequence[dict], context: dict, system: str) -> bool:
    if system not in context["systems"]:
        return False
    # A finished run without one of the system's app jobs is not a run of that system at all.
    return run.get("status") != "completed" or bool(shard_jobs(jobs, system))


def newest_proof_states(github, runs: Sequence[dict], systems: Sequence[str], context_for, covers, classify
                        ) -> Dict[str, SuiteProof]:
    """The proof state of each needed system, judging the newest run covering that system."""
    ordered = sorted((run for run in runs if isinstance(run.get("id"), int)), key=lambda run: run["id"], reverse=True)
    cache: Dict[int, Tuple[List[dict], dict]] = {}

    def info(run: dict) -> Tuple[List[dict], dict]:
        run_id = run["id"]
        if run_id not in cache:
            jobs = github.suite_jobs(run_id)
            cache[run_id] = (jobs, context_for(run, jobs))
        return cache[run_id]

    proofs: Dict[str, SuiteProof] = {}
    for system in systems:
        proof = SuiteProof("missing")
        for index, run in enumerate(ordered):
            jobs, context = info(run)
            if not covers(run, jobs, context, system):
                continue
            state = classify(run, system, jobs, context)
            older_green = None
            if state == "running":
                for older in ordered[index + 1:]:
                    older_jobs, older_context = info(older)
                    if covers(older, older_jobs, older_context, system) and classify(
                            older, system, older_jobs, older_context) == "success":
                        older_green = older["id"]
                        break
            proof = SuiteProof(state, run["id"], older_green)
            break
        proofs[system] = proof
    return proofs


def suite_states(github, pull: Pull, systems: Sequence[str], changed: Sequence[str]) -> Dict[str, SuiteProof]:
    """The NSH proof state from pushes, manual runs and the nightly run. Pull-request label proof is separate."""
    runs = [run for run in github.workflow_runs(pull.head) if run.get("event") != "pull_request"]
    return newest_proof_states(
        github, runs, systems,
        lambda run, jobs: suite_run_context(github, run, jobs, changed), shard_run_covers, classify_suite_run)


def pull_request_suite_context(github, jobs: Sequence[dict]) -> dict:
    """What a pull_request suite-label run of nsh.yml planned, from its own plan log or job names."""
    values, _lines = read_plan_output(github, jobs, "plan")
    if values and values.get("lane") != "suite":
        return {"systems": [], "shard_count": ci_plan.SHARDS, "partial": False}
    if all(f"nsh_{system}" in values for system in ci_plan.SYSTEMS):
        systems = [system for system in ci_plan.SYSTEMS if values.get(f"nsh_{system}") == "true"]
    else:
        systems = infer_job_systems(jobs)
    try:
        shard_count = int(values.get("shard_count") or ci_plan.SHARDS)
    except ValueError:
        shard_count = ci_plan.SHARDS
    return {"systems": systems, "shard_count": shard_count, "partial": False}


def pull_request_shard_covers(run: dict, jobs: Sequence[dict], context: dict, system: str) -> bool:
    return system in context["systems"] and tests_ok_job(jobs) is not None


def classify_pull_request_suite_run(run: dict, system: str, jobs: Sequence[dict], context: dict) -> str:
    state = classify_suite_run(run, system, jobs, context)
    if state != "success":
        return state
    gate = tests_ok_job(jobs)
    return "missing" if gate is None else job_state(run, gate)


def pull_request_suite_states(github, pull: Pull, systems: Sequence[str]) -> Dict[str, SuiteProof]:
    """The NSH proof state from pull_request suite-label runs, newest tests-ok run first per system."""
    runs = [run for run in github.workflow_runs(pull.head, SUITE_WORKFLOW_FILE) if run.get("event") == "pull_request"]
    return newest_proof_states(github, runs, systems, lambda run, jobs: pull_request_suite_context(github, jobs),
                               pull_request_shard_covers, classify_pull_request_suite_run)


def empty_ci_context() -> dict:
    return {"systems": [], "required": {system: [] for system in ci_plan.SYSTEMS}, "lane": ""}


def required_from_ci_plan(plan: dict) -> Dict[str, List[str]]:
    required = {system: [] for system in ci_plan.SYSTEMS}
    for system in plan["check_systems"]:
        required[system].append("check")
    if plan["installer"]:
        required["windows"].append("installer")
    return required


def required_from_ci_values(values: Dict[str, str]) -> Optional[Dict[str, List[str]]]:
    if "check_matrix" not in values:
        return None
    try:
        matrix = json.loads(values["check_matrix"])
        entries = matrix.get("include", []) if isinstance(matrix, dict) else []
    except json.JSONDecodeError:
        return None
    if not isinstance(entries, list):
        return None
    os_to_system = {settings["os"]: system for system, settings in ci_plan.CHECK_OS.items()}
    required = {system: [] for system in ci_plan.SYSTEMS}
    for entry in entries:
        if not isinstance(entry, dict) or entry.get("os") not in os_to_system:
            return None
        required[os_to_system[entry["os"]]].append("check")
    if values.get("installer") == "true":
        required["windows"].append("installer")
    return required


def ci_run_context(github, run: dict, jobs: Sequence[dict], changed: Sequence[str]) -> dict:
    """What one ci.yml test-lane or suite-label run was asked to prove, read from its own planner output."""
    event = run.get("event")
    if event not in ("push", "pull_request"):
        return empty_ci_context()
    values, lines = read_plan_output(github, jobs, "changes")
    if event == "push":
        branch = run.get("head_branch") or ""
        if not branch.startswith("test/") or (values and values.get("lane") != "tests"):
            return empty_ci_context()
        chosen = ci_plan.branch_systems(branch)
        required = required_from_ci_values(values)
        if required is None:
            plan = ci_plan.decide(event="push", ref=f"refs/heads/{branch}", branch=branch, changed=list(changed))
            required = required_from_ci_plan(plan)
        return {"systems": chosen, "required": required, "lane": "tests"}
    if values.get("lane") != "suite":
        return empty_ci_context()
    chosen = planned_chosen_systems(values, lines)
    required = required_from_ci_values(values)
    if required is None:
        required = {system: [] for system in ci_plan.SYSTEMS}
    return {"systems": chosen, "required": required, "lane": "suite"}


def ci_system_job(jobs: Sequence[dict], system: str, kind: str) -> Optional[dict]:
    if kind == "check":
        name = f"workspace tests ({ci_plan.CHECK_OS[system]['os']})"
    else:
        name = "Build and audit unsigned Windows Setup"
    return next((job for job in jobs if job.get("name") == name), None)


def ci_run_covers(run: dict, jobs: Sequence[dict], context: dict, system: str) -> bool:
    return system in context["systems"] and tests_ok_job(jobs) is not None


def classify_ci_run(run: dict, system: str, jobs: Sequence[dict], context: dict) -> str:
    """success, failed, running or missing for one system in one ci.yml test run."""
    if run.get("status") != "completed":
        return "running"
    conclusion = run.get("conclusion")
    if conclusion in ("skipped", "cancelled"):
        return "missing"
    if conclusion != "success":
        return "failed"
    gate = tests_ok_job(jobs)
    if gate is None:
        return "missing"
    gate_state = job_state(run, gate)
    if gate_state != "success":
        return gate_state
    missing = False
    for kind in context["required"].get(system, []):
        job = ci_system_job(jobs, system, kind)
        if job is None:
            missing = True
            continue
        state = job_state(run, job)
        if state == "failed":
            return "failed"
        if state == "running":
            return "running"
        if state == "missing":
            missing = True
    return "missing" if missing else "success"


def ci_tests_states(github, pull: Pull, systems: Sequence[str], changed: Sequence[str], event: str
                    ) -> Dict[str, SuiteProof]:
    """ci.yml tests-ok proof per system from either pull_request suite runs or test/** pushes."""
    runs = [run for run in github.workflow_runs(pull.head, CI_WORKFLOW_FILE) if run.get("event") == event]
    return newest_proof_states(github, runs, systems, lambda run, jobs: ci_run_context(github, run, jobs, changed),
                               ci_run_covers, classify_ci_run)


def combine_proof_sets(systems: Sequence[str], *proof_sets: Dict[str, SuiteProof]) -> Dict[str, SuiteProof]:
    """Judge the newest covering run across all proof sources; only its own state may satisfy the system."""
    combined = {}
    for system in systems:
        choices = [proofs[system] for proofs in proof_sets]
        combined[system] = max(choices, key=lambda proof: proof.run_id or -1)
    return combined


def proof_problems(proofs: Dict[str, SuiteProof], systems: Sequence[str]) -> List[str]:
    problems = []
    for name in systems:
        proof = proofs[name]
        if proof.state == "success":
            continue
        if proof.run_id is None:
            where = "no run"
        else:
            where = f"run {proof.run_id}"
            if proof.older_green is not None:
                where += f", older green run {proof.older_green}"
        problems.append(f"{name} {proof.state} ({where})")
    return problems


def proof_run_ids(proofs: Dict[str, SuiteProof]) -> List[int]:
    return sorted({proof.run_id for proof in proofs.values() if proof.state == "success" and proof.run_id is not None})


def enforce_suite(github, pulls: Sequence[Pull], system: str, changed_by_pr: Dict[int, List[str]],
                  allow_unsuited: Sequence[int], say: Callable[[str], None]) -> None:
    unsuited = set(allow_unsuited)
    batched = {pull.number for pull in pulls}
    for number in sorted(unsuited - batched):
        say(f"*** WARNING: --allow-unsuited #{number} is not in the batch; the build goes on ***")
    needed = batch_systems(system)
    refusals = []
    for pull in pulls:
        pull.suite_runs = []
        if pull.number in unsuited:
            say(f"*** ALLOW-UNSUITED: #{pull.number} at {pull.head}; skipping the test suite gate ***")
            continue
        changed = changed_by_pr[pull.number]
        ci_label = ci_tests_states(github, pull, needed, changed, "pull_request")
        ci_push = ci_tests_states(github, pull, needed, changed, "push")
        ci_proofs = combine_proof_sets(needed, ci_label, ci_push)
        problems = []
        ci_problems = proof_problems(ci_proofs, needed)
        if ci_problems:
            problems.append("ci.yml tests-ok is not green: " + "; ".join(ci_problems))
        run_ids = proof_run_ids(ci_proofs)

        if needs_suite(changed):
            nsh_label = pull_request_suite_states(github, pull, needed)
            nsh_push = suite_states(github, pull, needed, changed)
            nsh_proofs = combine_proof_sets(needed, nsh_label, nsh_push)
            nsh_problems = proof_problems(nsh_proofs, needed)
            if nsh_problems:
                problems.append("NSH app suite is not green: " + "; ".join(nsh_problems))
            run_ids += proof_run_ids(nsh_proofs)

        if problems:
            system_arg = "" if system == DEFAULT_SYSTEM else f" {system}"
            refusals.append(f"#{pull.number} at {pull.head}: " + "; ".join(problems)
                            + f"; start it with: just ci-suite {pull.number}{system_arg}; nothing was pushed")
        else:
            pull.suite_runs = sorted(set(run_ids))
    if refusals:
        raise BatchError("refused by the test suite gate:\n  " + "\n  ".join(refusals))


def read_pulls(github, numbers: Sequence[int]) -> List[Pull]:
    pulls, refusals = [], []
    for number in numbers:
        pull, why = checked_pull(number, github.pull(number))
        if why:
            refusals.append(why)
        else:
            pulls.append(pull)
    if refusals:
        raise BatchError("refused:\n  " + "\n  ".join(refusals))
    return pulls


# ---------------------------------------------------------------------------------------------------------------
# git.

class Git:
    def __init__(self, cwd: str, remote: str):
        self.cwd, self.remote = cwd, remote

    def run(self, *args: str, ok: Sequence[int] = (0,), stdin: str = "") -> Tuple[int, str]:
        result = subprocess.run(["git", *args], cwd=self.cwd, capture_output=True, text=True, input=stdin or None,
                                check=False)
        if result.returncode not in ok:
            raise BatchError(f"git {' '.join(args[:2])} failed: {result.stderr.strip() or result.stdout.strip()}")
        return result.returncode, result.stdout

    def out(self, *args: str) -> str:
        return self.run(*args)[1].strip()

    def remote_heads(self, refs: Sequence[str]) -> Dict[str, str]:
        listing = self.out("ls-remote", self.remote, *refs)
        return {ref: sha for sha, ref in (line.split("\t", 1) for line in listing.splitlines() if "\t" in line)}

    def fetch(self, refs: Sequence[str]) -> None:
        # No destination and no FETCH_HEAD: only the objects arrive; no ref of the checkout changes.
        self.run("fetch", "--quiet", "--no-tags", "--no-write-fetch-head", self.remote, *refs)

    def has_commit(self, sha: str) -> bool:
        return self.run("cat-file", "-e", f"{sha}^{{commit}}", ok=(0, 1, 128))[0] == 0

    def changed_files(self, base: str, head: str) -> List[str]:
        """The full diff's paths, with both names of every rename."""
        span = f"{base}...{head}"
        names = [part for part in self.run("diff", "--name-only", "-M", "-z", span)[1].split("\0") if part]
        status = self.run("diff", "--name-status", "-M", "-z", "--diff-filter=R", span)[1]
        parts = [part for part in status.split("\0") if part]
        old_names = []
        index = 0
        while index < len(parts):
            marker = parts[index]
            index += 1
            if marker.startswith("R"):
                if index + 1 >= len(parts):
                    raise BatchError(f"git diff gave a bad rename entry for {span}")
                old_names.append(parts[index])
                index += 2
            else:
                index += 1
        changed = []
        for name in names + old_names:
            if name not in changed:
                changed.append(name)
        return changed

    def squash(self, onto: str, head: str) -> Tuple[Optional[str], List[str]]:
        """Merges head onto `onto` (merge base found by git, as GitHub does). Returns (tree, []) or (None, files)."""
        code, output = self.run("merge-tree", "--write-tree", "--name-only", "--no-messages", "-z", onto, head,
                                ok=(0, 1))
        parts = [part for part in output.split("\0") if part]
        if code == 0:
            return parts[0].strip(), []
        return None, parts[1:]

    def commit(self, tree: str, parent: str, message: str) -> str:
        return self.run("commit-tree", tree, "-p", parent, "-F", "-", stdin=message)[1].strip()


# ---------------------------------------------------------------------------------------------------------------
# The four commands.

def build(git: Git, github, system: str, numbers: Sequence[int], *, push: bool = True,
          allow_unchecked: Sequence[int] = (), allow_unsuited: Sequence[int] = (),
          say: Callable[[str], None] = print) -> Tuple[str, str, Record]:
    """Returns (branch, record commit sha, record)."""
    main_sha, _ = github.main()
    if not SHA.match(main_sha):
        raise BatchError(f"GitHub gave no full sha for main ({main_sha!r})")
    pulls = read_pulls(github, numbers)
    enforce_ci_ok(github, pulls, allow_unchecked, say)

    # What the remote serves must be what GitHub just said; otherwise something moved between the two reads.
    refs = ["refs/heads/main", *[f"refs/pull/{pull.number}/head" for pull in pulls]]
    served = git.remote_heads(refs)
    moved = []
    if served.get("refs/heads/main") != main_sha:
        moved.append(f"main: GitHub said {main_sha}, {git.remote} serves {served.get('refs/heads/main')}")
    for pull in pulls:
        got = served.get(f"refs/pull/{pull.number}/head")
        if got != pull.head:
            moved.append(f"#{pull.number}: GitHub said {pull.head}, {git.remote} serves {got}")
    if moved:
        raise BatchError("something moved while reading; run again:\n  " + "\n  ".join(moved))
    git.fetch(refs)
    for sha in [main_sha, *[pull.head for pull in pulls]]:
        if not git.has_commit(sha):
            raise BatchError(f"fetched, but commit {sha} is not here")
    changed_by_pr = {pull.number: git.changed_files(main_sha, pull.head) for pull in pulls}
    enforce_suite(github, pulls, system, changed_by_pr, allow_unsuited, say)

    tip = main_sha
    for pull in pulls:
        tree, conflicts = git.squash(tip, pull.head)
        if tree is None:
            done = ", ".join(f"#{p.number}" for p in pulls[:pulls.index(pull)]) or "nothing"
            raise BatchError(f"#{pull.number} conflicts with main plus {done}, in: {', '.join(conflicts)}\n"
                             "Nothing was pushed and nothing changed on GitHub. Reorder the list, leave it out, or "
                             "have it rebased.")
        title = pull.title or f"Pull request {pull.number}"
        tip = git.commit(tree, tip, f"{title} (#{pull.number})\n\nci-batch squash of {pull.head}\n")
    record = Record(main_sha, pulls, git.out("rev-parse", f"{tip}^{{tree}}"), system,
                    [pull.number for pull in pulls if pull.number in set(allow_unsuited)])
    sha = git.commit(record.tree, tip, format_record(record))
    branch = branch_name(system, numbers)
    if push:
        git.run("push", "--force", "--quiet", git.remote, f"{sha}:refs/heads/{branch}")
        git.run("update-ref", LOCAL_REF + branch, sha)
        say(f"pushed {sha} to {branch}")
    else:
        say("dry-run: nothing was pushed")
    say(f"main     {main_sha}")
    for pull in pulls:
        line = f"#{pull.number:<7} {pull.head}"
        if pull.suite_runs:
            line += "  suite " + ",".join(str(run_id) for run_id in pull.suite_runs)
        say(f"{line}  {pull.title}")
    say(f"tree     {record.tree}")
    if push:
        say(f"runs     https://github.com/{getattr(github, 'repo', REPO)}/actions?query=branch%3A"
            + branch.replace("/", "%2F"))
        say(f"then     just ci-batch-status {branch}")
    return branch, sha, record


def parse_test_pr_args(args: Sequence[str]) -> Tuple[int, str]:
    words = [arg for arg in args if arg]
    if len(words) not in (1, 2):
        raise BatchError("test-pr takes a pull request number and optionally a system")
    value = words[0].lstrip("#")
    if not value.isdigit():
        raise BatchError(f"not a pull request number: {words[0]}")
    system = words[1] if len(words) == 2 else DEFAULT_SYSTEM
    if system not in SYSTEMS:
        raise BatchError(f"unknown system {system}: use {', '.join(SYSTEMS)}")
    return int(value), system


def test_pr(git: Git, github, number: int, system: str = DEFAULT_SYSTEM,
            say: Callable[[str], None] = print) -> str:
    """Push the exact head GitHub reports for one pull request to its test branch."""
    if system not in SYSTEMS:
        raise BatchError(f"unknown system {system}: use {', '.join(SYSTEMS)}")
    data = github.pull(number)
    head = data.get("headRefOid") or ""
    if not SHA.match(head):
        raise BatchError(f"#{number}: GitHub gave no full head sha ({head!r})")
    remote_ref = f"refs/pull/{number}/head"
    local_ref = f"refs/ci-test-pr/{number}"
    try:
        git.run("fetch", "--quiet", "--no-tags", git.remote, f"+{remote_ref}:{local_ref}")
        fetched = git.out("rev-parse", local_ref)
    finally:
        git.run("update-ref", "-d", local_ref, ok=(0, 1, 128))
    if fetched != head:
        raise BatchError(f"#{number}: fetched {fetched}, which is not the head GitHub reports ({head}); "
                         "nothing was pushed")
    branch = f"test/{system}/pr-{number}"
    git.run("push", "--force", "--quiet", git.remote, f"{head}:refs/heads/{branch}")
    say(f"{head} on {branch}")
    say(f"https://github.com/{getattr(github, 'repo', REPO)}/actions?query=branch%3A" + branch.replace("/", "%2F"))
    return head


def read_batch(git: Git, branch: str) -> Tuple[str, Record]:
    """The batch's record commit: the branch tip on the remote, else the copy kept locally by `build`."""
    ref = f"refs/heads/{branch}"
    sha = git.remote_heads([ref]).get(ref)
    if sha:
        if not git.has_commit(sha):
            git.fetch([ref])
    else:
        code, local = git.run("rev-parse", "--verify", "-q", LOCAL_REF + branch, ok=(0, 1))
        if code != 0:
            raise BatchError(f"no branch {branch} on {git.remote} and no local copy of its record; run the batch again")
        sha = local.strip()
    record = parse_record(git.out("log", "-1", "--format=%B", sha))
    if git.out("rev-parse", f"{sha}^{{tree}}") != record.tree:
        raise BatchError(f"{branch}: the tip's tree is not the recorded tree; this is not a ci-batch record")
    return sha, record


def status(git: Git, github, branch: str, say: Callable[[str], None] = print) -> str:
    """'green', 'red' or 'running'."""
    sha, record = read_batch(git, branch)
    latest: Dict[str, dict] = {}
    for workflow_file, _name in WORKFLOWS:
        runs = [run for run in github.runs(branch, workflow_file)
                if run.get("headSha") == sha and run.get("event") == "push"]
        for run in sorted(runs, key=lambda r: r.get("createdAt", "")):
            latest[workflow_file] = run
    verdicts = []
    say(f"{branch} at {sha}")
    for workflow_file, name in WORKFLOWS:
        run = latest.get(workflow_file)
        if run is None:
            verdicts.append("running")
            say(f"  {name}: no run yet (a run can take a minute to appear)")
            continue
        if run["status"] != "completed":
            verdicts.append("running")
            say(f"  {name}: {run['status']}  {run['url']}")
        elif run.get("conclusion") == "success":
            verdicts.append("green")
            say(f"  {name}: green  {run['url']}")
        else:
            verdicts.append("red")
            say(f"  {name}: red ({run.get('conclusion')})  {run['url']}")
            for job in github.jobs(run["databaseId"]):
                if job.get("conclusion") in FAILED:
                    say(f"    failed job: {job.get('name')} ({job.get('conclusion')})  {job.get('url', '')}")
    verdict = "red" if "red" in verdicts else "running" if "running" in verdicts else "green"
    say(f"batch: {verdict}")
    if verdict == "red":
        numbers = record.numbers()
        say("This does not say which pull request broke it.")
        if len(numbers) == 1:
            say(f"The batch is #{numbers[0]} alone: that pull request fails on main {record.main[:12]}.")
        else:
            half = (len(numbers) + 1) // 2
            say("Bisect: run both halves (they run side by side on their own branches):")
            for part in (numbers[:half], numbers[half:]):
                say(f"  just ci-batch {record.system} " + " ".join(str(n) for n in part))
    return verdict


def merge_commands(git: Git, github, branch: str, *, require_green: bool = True,
                   say: Callable[[str], None] = print) -> List[str]:
    """Prints the merge commands; raises if the batch is not green or anything moved since it was built."""
    _, record = read_batch(git, branch)
    if require_green:
        verdict = status(git, github, branch, say=lambda _line: None)
        if verdict != "green":
            raise BatchError(f"{branch} is {verdict}, not green: nothing to merge (just ci-batch-status {branch})")
    moved = []
    main_sha, _ = github.main()
    if main_sha != record.main:
        moved.append(f"main moved: the batch is on {record.main}, main is now {main_sha}")
    for pull in record.pulls:
        current, why = checked_pull(pull.number, github.pull(pull.number))
        if why:
            moved.append(why)
        elif current.head != pull.head:
            moved.append(f"#{pull.number} moved: the batch has {pull.head}, its head is now {current.head}")
    if moved:
        raise BatchError("the batch's proof no longer holds; run the batch again:\n  " + "\n  ".join(moved)
                         + f"\n  just ci-batch {record.system} " + " ".join(str(n) for n in record.numbers()))
    repo = getattr(github, "repo", REPO)
    commands = [f"gh pr merge {pull.number} --repo {repo} --squash --match-head-commit {pull.head}"
                for pull in record.pulls]
    say(f"# {branch} is green, and main ({record.main}) and every head are unchanged.")
    say("# Run these in this order, one after another, with nothing else merged in between:")
    for command in commands:
        say(command)
    say(f"# Then: just ci-batch-verify {branch}")
    return commands


def verify(git: Git, github, branch: str, say: Callable[[str], None] = print) -> bool:
    sha, record = read_batch(git, branch)
    main_sha, main_tree = github.main()
    if main_tree == record.tree:
        say(f"equal: main {main_sha} has tree {main_tree}, the tree {branch} proved.")
        return True
    say(f"not equal: main {main_sha} has tree {main_tree}; {branch} proved tree {record.tree}.")
    say("The batch's proof does not cover main. Run the test suites on main (just ci-main).")
    try:
        git.fetch(["refs/heads/main"])
        if git.has_commit(main_sha):
            say(git.out("diff", "--stat", sha, main_sha))
    except BatchError:
        pass
    return False


def main(argv: Sequence[str]) -> int:
    if not argv or argv[0] not in ("build", "test-pr", "status", "merge", "verify"):
        print(__doc__.strip().split("\n\n")[1], file=sys.stderr)
        return 2
    command, args = argv[0], list(argv[1:])
    git = Git(os.getcwd(), os.environ.get("CI_TEST_REMOTE") or "origin")
    github = GitHub(REPO)
    try:
        if command == "build":
            system, numbers, dry_run, allow_unchecked, allow_unsuited = parse_build_args(args)
            build(git, github, system, numbers, push=not dry_run, allow_unchecked=allow_unchecked,
                  allow_unsuited=allow_unsuited)
            return 0
        if command == "test-pr":
            number, system = parse_test_pr_args(args)
            test_pr(git, github, number, system)
            return 0
        if len(args) != 1:
            raise BatchError(f"{command} takes the batch branch, test/<system>/batch-...")
        if command == "status":
            return 0 if status(git, github, args[0]) == "green" else 1
        if command == "merge":
            merge_commands(git, github, args[0])
            return 0
        return 0 if verify(git, github, args[0]) else 1
    except BatchError as error:
        print(f"ci-batch: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
