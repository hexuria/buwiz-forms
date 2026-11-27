#!/usr/bin/env python3
"""Proves several pull requests together (docs/CI.md, "Proving several pull requests together").

  build  [system] [--dry-run] [--allow-unchecked <pr>] [--allow-unsuited <pr>] <pr> <pr> ...
                                  main plus the squash of each pull request, in that order, pushed to
                                  test/<system>/batch-<pr>-<pr>... (default system linux-windows)
  status <branch>                 green or red per workflow run of the batch; for red, the failed jobs and a bisect
  merge  <branch>                 prints (never runs) the merge commands, after checking nothing moved
  verify <branch>                 after the merges: is main's tree the batch's tree?

Every pull request's required `ci-ok` must be `success` at its current head; pending, missing or not green refuses,
and nothing is pushed. A pull request that changes `nsh/`, `crates/`, top-level `Cargo.toml`, `Cargo.lock` or
`justfile` must also have a green `NSH app` run at that head. `--dry-run` proves the same batch without pushing.
`--allow-unchecked <pr>` and `--allow-unsuited <pr>` (repeatable) are the hand-run escape hatches.

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

REPO = "hexuria/buwiz-forms"
SYSTEMS = ("linux-windows", "linux", "windows", "macos", "all")
DEFAULT_SYSTEM = "linux-windows"
SUITE_WORKFLOW = "NSH app"
# The workflows a push to test/** starts (ci.yml and nsh.yml).
WORKFLOWS = ("CI", SUITE_WORKFLOW)
SHA = re.compile(r"^[0-9a-f]{40}$")
FAILED = ("failure", "timed_out", "cancelled", "startup_failure", "action_required", "stale")
# Where a batch's record commit is also kept locally, in case its branch was deleted (`just ci-test-clean`).
LOCAL_REF = "refs/ci-batch/"
SUITE_TOP_LEVEL = {"Cargo.toml", "Cargo.lock", "justfile"}
SUITE_PREFIXES = ("nsh/", "crates/")


class BatchError(Exception):
    pass


@dataclass
class Pull:
    number: int
    head: str
    title: str = ""
    suite_run: Optional[int] = field(default=None, compare=False)


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

    def workflow_runs(self, sha: str) -> List[dict]:
        """The workflow runs reported for one head sha."""
        data = json.loads(self._gh("api", f"repos/{self.repo}/actions/runs?head_sha={sha}&per_page=100"))
        return data.get("workflow_runs", [])

    def pull_files(self, number: int) -> List[dict]:
        """Every changed file of one pull request, one page at a time."""
        files: List[dict] = []
        page = 1
        while True:
            data = json.loads(self._gh("api", f"repos/{self.repo}/pulls/{number}/files"
                                              f"?per_page=100&page={page}"))
            files.extend(data)
            if len(data) < 100:
                return files
            page += 1

    def main(self) -> Tuple[str, str]:
        """main's commit sha and tree hash, in one read."""
        data = json.loads(self._gh("api", f"repos/{self.repo}/commits/main"))
        return data["sha"], data["commit"]["tree"]["sha"]

    def runs(self, branch: str) -> List[dict]:
        return json.loads(self._gh("run", "list", "--repo", self.repo, "--branch", branch, "--limit", "100",
                                   "--json", "databaseId,workflowName,status,conclusion,headSha,url,event,createdAt"))

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


def suite_path(path: str) -> bool:
    return path.startswith(SUITE_PREFIXES) or path in SUITE_TOP_LEVEL


def needs_suite(github, pull: Pull) -> bool:
    """True when the pull request changes something the NSH app suite proves."""
    for entry in github.pull_files(pull.number):
        names = [entry.get("filename") or ""]
        if entry.get("previous_filename"):
            names.append(entry["previous_filename"])
        if any(suite_path(name) for name in names):
            return True
    return False


def suite_state(github, pull: Pull) -> Tuple[str, Optional[int]]:
    """(success, failure, running or missing, run id) of the newest NSH app run at the pull request's head."""
    runs = [run for run in github.workflow_runs(pull.head)
            if run.get("name") == SUITE_WORKFLOW and run.get("head_sha") == pull.head]
    if not runs:
        return "missing", None
    run = max(runs, key=lambda r: r.get("id") or 0)
    run_id = run.get("id")
    if run.get("status") != "completed":
        return "running", run_id
    conclusion = run.get("conclusion")
    if conclusion == "success":
        return "success", run_id
    if conclusion in ("skipped", "cancelled"):
        return "missing", run_id
    return "failure", run_id


def enforce_suite(github, pulls: Sequence[Pull], allow_unsuited: Sequence[int],
                  say: Callable[[str], None]) -> None:
    unsuited = set(allow_unsuited)
    batched = {pull.number for pull in pulls}
    for number in sorted(unsuited - batched):
        say(f"*** WARNING: --allow-unsuited #{number} is not in the batch; the build goes on ***")
    refusals = []
    for pull in pulls:
        pull.suite_run = None
        if pull.number in unsuited:
            say(f"*** ALLOW-UNSUITED: #{pull.number} at {pull.head}; skipping the NSH app suite gate ***")
            continue
        if not needs_suite(github, pull):
            continue
        state, run_id = suite_state(github, pull)
        pull.suite_run = run_id
        if state != "success":
            run = f" (run {run_id})" if run_id is not None else ""
            refusals.append(f"#{pull.number} at {pull.head}: NSH app suite is {state}{run}; "
                            "start it with: just ci-test; nothing was pushed")
    if refusals:
        raise BatchError("refused by the NSH app suite gate:\n  " + "\n  ".join(refusals))


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
    enforce_suite(github, pulls, allow_unsuited, say)

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
        if pull.suite_run is not None:
            line += f"  suite {pull.suite_run}"
        say(f"{line}  {pull.title}")
    say(f"tree     {record.tree}")
    if push:
        say(f"runs     https://github.com/{getattr(github, 'repo', REPO)}/actions?query=branch%3A"
            + branch.replace("/", "%2F"))
        say(f"then     just ci-batch-status {branch}")
    return branch, sha, record


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
    runs = [run for run in github.runs(branch) if run.get("headSha") == sha and run.get("event") == "push"]
    latest: Dict[str, dict] = {}
    for run in sorted(runs, key=lambda r: r.get("createdAt", "")):
        latest[run["workflowName"]] = run
    verdicts = []
    say(f"{branch} at {sha}")
    for name in WORKFLOWS:
        run = latest.get(name)
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
    if not argv or argv[0] not in ("build", "status", "merge", "verify"):
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
