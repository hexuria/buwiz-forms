#!/usr/bin/env python3
"""Stage 2 of the three-stage model: build a corrected tree from a NAMED batch.

    STAGE 1  GENERATE   pinned PDF -> IR -> lattice -> emit -> HTML   (forms/)
    STAGE 2  CORRECT    forms-corrected/ = copy(batch vN) + correction records
    STAGE 3  MAP        fields -> eBIRForms XML payload keys

Stage 2 exists for facts the SOURCE cannot tell us. The one true record known
today is 2550M's TIN branch code: BIR's own eBIRForms HTA runtime declares
`txtBranchCode` at max_length 5, while the Feb-2007 artwork prints three
compartments. The artwork is correct AND out of date, and no rule derives "BIR
widened this in 2018" from 2007 ink -- which is exactly why it is a declared
record here and not a producer fix in stage 1. A taxpayer with a 4- or 5-digit
branch code cannot enter it today, so the return files against the wrong branch.

Four rules from ARCHITECTURE.md drive every decision below. Where the code
makes a structural choice to enforce one of them, the comment names the rule.

 1. A BATCH IS IMMUTABLE once a sighted gate scores it. This tool opens the
    stage-1 tree read-only and refuses any output path that would put a byte
    inside it -- or inside `forms/` at all, whatever `--source-tree` says.
 2. ONE GENERATOR VERSION PER BATCH. `--batch` names the batch (`corpus/r23`);
    the manifest records the name AND a Merkle hash of the tree, because a name
    is a label and only the hash is a binding.
 3. NO RECORD => BYTE-IDENTICAL COPY. Enforced by construction, not by care: a
    file is rewritten only when some record's edit matched a span inside it.
    Every other file is copied byte-for-byte, and the manifest carries the
    sha256 of both sides so the claim is checkable without rerunning anything.
 4. A CORRECTION NEVER HIDES A DIVERGENCE. Three structural properties, none of
    which depends on the operator's good intentions:
      a. The record schema is CLOSED and contains no key that can suppress,
         allowlist, waive, or exclude a check. There is no way to spell it, so
         "the override became the way to silence the check" cannot happen here
         -- it would have to happen in the checker, in the open.
      b. Every record MUST name, in `diverges_from`, the check it still fails,
         and the manifest publishes the sentence a report has to print:
         "diverges by declared override <id>, authorised by <authority>",
         with `still_diverges_from_official: true`. The divergence gets louder,
         never quieter.
      c. A record can only REWRITE declared byte spans. It cannot add a file,
         delete a file, or rename one -- the applier compares the written file
         set against the source's and refuses on any difference. A correction
         therefore cannot make an inconvenient artefact disappear.

Independence, stated plainly because the project keeps paying for the opposite:
this tool checks each record's `expected_effect` against the bytes it actually
wrote, which is necessary and NOT sufficient. It shares a producer with the
correction. `verified_by` names the independent producer that must re-derive
the effect downstream, and the manifest carries `independently_verified: false`
for every record until that happens. A tool that certified its own corrections
would be the `?debug=fields` failure (233/233 OK on a visibly wrong page) built
one layer higher.

Failure is refusal. A correction that cannot be applied is a failure, never a
skip -- the house rule that a check which cannot be evaluated is a failure,
restated for the applier. Everything is staged in a temporary directory and
verified there; the output tree appears only when every record has landed and
every declared effect has been re-derived from the written bytes. There is no
half-corrected tree to find later.

Usage:
    python3 tools/formgen/correct.py --batch corpus/r23 \
        --source-tree forms --records tools/formgen/corrections \
        --out forms-corrected

    python3 tools/formgen/correct.py --verify \
        --manifest forms-corrected.manifest.json

    python3 tools/formgen/correct.py --self-test
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import os
import pathlib
import re
import shutil
import sys
import tempfile
from typing import Any, Iterable, Sequence

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

MANIFEST_VERSION = 1
RECORD_VERSION = 1

# The stage-1 corpus. Rule 1: immutable once scored. This path is a hard floor
# -- no combination of flags may cause a write at or below it, and the guard is
# by resolved path so a symlink or a `..` cannot walk into it.
STAGE1_TREE = REPO / "forms"

# A generated bundle is a directory carrying batch.py's provenance record. Used
# only to report a bundle count; nothing keys off it.
BUNDLE_MARKER = "provenance.json"

# --- the correction record schema -------------------------------------------
# CLOSED on purpose (rule 4a). An unknown key is a refusal, not a warning, so
# nobody can introduce `suppress`, `waive`, `expected_failures` or `allowlist`
# by writing it into a record file and hoping the applier ignores what it does
# not understand.
REQUIRED_RECORD_KEYS = ("id", "form", "subject", "what", "reason", "authority",
                        "diverges_from", "expected_effect", "verified_by", "edits")
OPTIONAL_RECORD_KEYS = ("record_version", "notes")
EDIT_KEYS = ("file", "find", "replace", "occurrences")

# `expected_effect` vocabulary. Each kind is re-derived from bytes on disk:
# `count_delta` reads the SOURCE and the WRITTEN output and demands both
# numbers, which is the only kind that states something the edit itself does
# not trivially imply -- so at least one non-`sha256` effect is required.
EFFECT_KEYS = {
    "occurs":      ("kind", "file", "text", "count"),
    "absent":      ("kind", "file", "text"),
    "count_delta": ("kind", "file", "text", "before", "after"),
    "sha256":      ("kind", "file", "value"),
}
SEMANTIC_EFFECT_KINDS = ("occurs", "absent", "count_delta")

RECORD_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


class Refusal(Exception):
    """Every failure mode in the contract. Carries a stable machine code.

    Raised before or during staging, never after the output tree is in place,
    so a Refusal always means nothing was written to `--out`.
    """

    def __init__(self, code: str, detail: str) -> None:
        super().__init__(f"{code}: {detail}")
        self.code = code
        self.detail = detail


# --------------------------------------------------------------------------
# hashing
# --------------------------------------------------------------------------

def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_path(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_sha256(files: dict[str, str], dirs: Sequence[str]) -> str:
    """A Merkle over the whole tree: content AND shape.

    Directories are folded in because an empty directory is part of the copy;
    a tree that lost one is not byte-identical in the sense rule 3 means, even
    though every file still matches.
    """
    lines = [f"{sha}  f {path}" for path, sha in sorted(files.items())]
    lines += [f"{'0' * 64}  d {path}" for path in sorted(dirs)]
    return sha256_bytes(("\n".join(lines) + "\n").encode("utf-8"))


# --------------------------------------------------------------------------
# tree scanning
# --------------------------------------------------------------------------

def scan_tree(root: pathlib.Path, what: str) -> tuple[dict[str, str], list[str]]:
    """Every regular file's sha256 plus every directory, both by POSIX relpath.

    Refuses on a symlink or any other non-regular entry: a copy that follows a
    link is not a copy, and one that preserves it is not checkable by content.
    Neither is worth guessing about, so the tree has to be plain.
    """
    if not root.is_dir():
        raise Refusal(f"{what}-missing", f"{root} is not a directory")
    files: dict[str, str] = {}
    dirs: list[str] = []
    stack = [(root, "")]
    while stack:
        here, prefix = stack.pop()
        try:
            entries = sorted(os.scandir(here), key=lambda e: e.name)
        except OSError as exc:                                  # pragma: no cover
            raise Refusal(f"{what}-unreadable", f"{here}: {exc}") from exc
        for entry in entries:
            rel = f"{prefix}{entry.name}"
            if entry.is_symlink():
                raise Refusal(f"{what}-symlink",
                              f"{rel} is a symlink; the tree must be plain files")
            if entry.is_dir():
                dirs.append(rel)
                stack.append((pathlib.Path(entry.path), rel + "/"))
            elif entry.is_file():
                files[rel] = sha256_path(pathlib.Path(entry.path))
            else:
                raise Refusal(f"{what}-not-regular", f"{rel} is not a regular file")
    return files, sorted(dirs)


def tree_label(path: pathlib.Path) -> str:
    """A stable label for the manifest: repo-relative when possible.

    Paths label, hashes bind. An absolute path would make the manifest differ
    between two checkouts of the same batch, which is not a fact about the
    corrected tree.
    """
    resolved = path.resolve()
    try:
        return resolved.relative_to(REPO).as_posix()
    except ValueError:
        return resolved.name


# --------------------------------------------------------------------------
# record loading and validation
# --------------------------------------------------------------------------

def _require_str(obj: dict[str, Any], key: str, where: str, *,
                 code: str = "record-field-invalid") -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value.strip():
        raise Refusal(code, f"{where}: '{key}' must be a non-empty string")
    return value


def _safe_relpath(value: str, where: str, key: str, *, allow_dot: bool) -> str:
    """A record may only name a path inside the tree, spelled POSIX-style.

    No absolute paths, no `..`, no backslashes, no drive letters. A record that
    could point outside the batch could correct a file the batch does not own.
    """
    if value.startswith("/") or "\\" in value or ":" in value:
        raise Refusal("record-path-unsafe", f"{where}: '{key}' must be a relative POSIX path")
    parts = [p for p in value.split("/") if p]
    if value == "." or value == "":
        if not allow_dot:
            raise Refusal("record-path-unsafe", f"{where}: '{key}' must name a file")
        return "."
    if any(p == ".." or p == "." for p in parts):
        raise Refusal("record-path-unsafe", f"{where}: '{key}' must not contain '.' or '..'")
    if not parts:
        raise Refusal("record-path-unsafe", f"{where}: '{key}' is empty")
    return "/".join(parts)


def validate_record(raw: Any, where: str) -> dict[str, Any]:
    """Reject anything the applier cannot fully account for.

    Rule 4a lives here: the key set is closed, so there is no spelling of
    "and stop checking this". The only thing a record can say is what changed,
    where, why, on whose authority, which check it still fails, and what a
    reader should be able to measure afterwards.
    """
    if not isinstance(raw, dict):
        raise Refusal("record-not-object", f"{where}: a record must be a JSON object")

    keys = set(raw)
    unknown = sorted(keys - set(REQUIRED_RECORD_KEYS) - set(OPTIONAL_RECORD_KEYS))
    if unknown:
        raise Refusal("record-unknown-key",
                      f"{where}: unknown key(s) {unknown}; the schema is closed so that "
                      f"no record can express a suppression")
    missing = sorted(set(REQUIRED_RECORD_KEYS) - keys)
    if missing:
        raise Refusal("record-missing-key", f"{where}: missing required key(s) {missing}")

    version = raw.get("record_version", RECORD_VERSION)
    if version != RECORD_VERSION:
        raise Refusal("record-version",
                      f"{where}: record_version {version!r} != {RECORD_VERSION}")

    ident = _require_str(raw, "id", where)
    if not RECORD_ID_RE.match(ident):
        raise Refusal("record-id-invalid",
                      f"{where}: id {ident!r} must match {RECORD_ID_RE.pattern}")
    for key in ("subject", "what", "reason", "authority", "diverges_from", "verified_by"):
        _require_str(raw, key, where)
    form = _safe_relpath(_require_str(raw, "form", where), where, "form", allow_dot=True)

    edits = raw.get("edits")
    if not isinstance(edits, list) or not edits:
        raise Refusal("record-edits-empty",
                      f"{where}: 'edits' must be a non-empty list -- a record that changes "
                      f"nothing is a claim with no subject")
    clean_edits = []
    for index, edit in enumerate(edits):
        tag = f"{where} edit[{index}]"
        if not isinstance(edit, dict):
            raise Refusal("edit-not-object", f"{tag}: must be a JSON object")
        extra = sorted(set(edit) - set(EDIT_KEYS))
        if extra:
            raise Refusal("edit-unknown-key", f"{tag}: unknown key(s) {extra}")
        short = sorted(set(EDIT_KEYS) - set(edit))
        if short:
            raise Refusal("edit-missing-key", f"{tag}: missing key(s) {short}")
        target = _safe_relpath(_require_str(edit, "file", tag), tag, "file", allow_dot=False)
        find = edit["find"]
        replace = edit["replace"]
        if not isinstance(find, str) or not find:
            raise Refusal("edit-find-empty", f"{tag}: 'find' must be a non-empty string")
        if not isinstance(replace, str):
            raise Refusal("edit-replace-type", f"{tag}: 'replace' must be a string")
        if find == replace:
            # A no-op edit would pass every count check and leave the tree
            # unchanged while the manifest advertised a correction. That is a
            # false claim, so it is a refusal rather than a tolerated nicety.
            raise Refusal("edit-no-op", f"{tag}: 'find' equals 'replace'")
        occurrences = edit["occurrences"]
        if not isinstance(occurrences, int) or isinstance(occurrences, bool) or occurrences < 1:
            raise Refusal("edit-occurrences-invalid",
                          f"{tag}: 'occurrences' must be an integer >= 1")
        clean_edits.append({"file": target, "find": find,
                            "replace": replace, "occurrences": occurrences})

    effects = raw.get("expected_effect")
    if not isinstance(effects, list) or not effects:
        raise Refusal("record-effect-empty",
                      f"{where}: 'expected_effect' must be a non-empty list -- a correction "
                      f"that cannot state its effect in advance cannot land")
    clean_effects = []
    for index, effect in enumerate(effects):
        tag = f"{where} expected_effect[{index}]"
        if not isinstance(effect, dict):
            raise Refusal("effect-not-object", f"{tag}: must be a JSON object")
        kind = effect.get("kind")
        if kind not in EFFECT_KEYS:
            raise Refusal("effect-kind-unknown",
                          f"{tag}: kind {kind!r} not in {sorted(EFFECT_KEYS)}")
        expected_keys = set(EFFECT_KEYS[kind])
        if set(effect) != expected_keys:
            raise Refusal("effect-keys",
                          f"{tag}: kind {kind} takes exactly {sorted(expected_keys)}")
        _safe_relpath(_require_str(effect, "file", tag), tag, "file", allow_dot=False)
        if kind in ("occurs", "absent", "count_delta"):
            text = effect.get("text")
            if not isinstance(text, str) or not text:
                raise Refusal("effect-text-empty", f"{tag}: 'text' must be a non-empty string")
        for numeric in ("count", "before", "after"):
            if numeric in effect:
                value = effect[numeric]
                if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                    raise Refusal("effect-count-invalid",
                                  f"{tag}: '{numeric}' must be an integer >= 0")
        if kind == "count_delta" and effect["before"] == effect["after"]:
            raise Refusal("effect-delta-zero",
                          f"{tag}: before == after states no change")
        if kind == "sha256":
            value = _require_str(effect, "value", tag)
            if len(value) != 64 or any(c not in "0123456789abcdef" for c in value.lower()):
                raise Refusal("effect-sha-invalid", f"{tag}: 'value' is not a sha256 hex digest")
        clean_effects.append(dict(effect))
    if not any(e["kind"] in SEMANTIC_EFFECT_KINDS for e in clean_effects):
        # `sha256` of the post-image is derivable from the edit itself, so a
        # record carrying only that has declared nothing it did not already
        # know. At least one effect must be a statement about the document.
        raise Refusal("record-effect-tautological",
                      f"{where}: at least one expected_effect must be one of "
                      f"{list(SEMANTIC_EFFECT_KINDS)}; a post-image sha256 restates the edit")

    record = {
        "record_version": RECORD_VERSION,
        "id": ident,
        "form": form,
        "subject": raw["subject"],
        "what": raw["what"],
        "reason": raw["reason"],
        "authority": raw["authority"],
        "diverges_from": raw["diverges_from"],
        "verified_by": raw["verified_by"],
        "expected_effect": clean_effects,
        "edits": clean_edits,
    }
    if "notes" in raw:
        record["notes"] = raw["notes"]
    return record


def load_records(records_dir: pathlib.Path) -> list[dict[str, Any]]:
    """Read every `*.json` under the ledger directory, sorted by file name.

    A MISSING ledger directory is a refusal, not an empty ledger. "There were
    no corrections" and "I could not find out whether there were corrections"
    are different statements and only one of them is a pass.
    """
    if not records_dir.is_dir():
        raise Refusal("records-dir-missing",
                      f"{records_dir} is not a directory; an absent ledger is not an empty one")
    paths = sorted(p for p in records_dir.iterdir()
                   if p.is_file() and p.suffix == ".json" and not p.name.startswith("."))
    for path in records_dir.iterdir():
        if path.is_symlink():
            raise Refusal("records-symlink", f"{path.name} is a symlink")
    records: list[dict[str, Any]] = []
    seen: dict[str, str] = {}
    for path in paths:
        raw_bytes = path.read_bytes()
        try:
            parsed = json.loads(raw_bytes.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise Refusal("record-unparseable", f"{path.name}: {exc}") from exc
        entries = parsed if isinstance(parsed, list) else [parsed]
        for index, raw in enumerate(entries):
            where = path.name if not isinstance(parsed, list) else f"{path.name}[{index}]"
            record = validate_record(raw, where)
            if record["id"] in seen:
                raise Refusal("record-duplicate-id",
                              f"{where}: id {record['id']} already declared in {seen[record['id']]}")
            seen[record["id"]] = where
            record["_source_file"] = path.name
            record["_source_sha256"] = sha256_bytes(raw_bytes)
            records.append(record)
    # Records are applied in id order, not directory order, so the result does
    # not depend on how a ledger happens to be split across files.
    records.sort(key=lambda r: r["id"])
    return records


# --------------------------------------------------------------------------
# edit application
# --------------------------------------------------------------------------

def find_spans(text: str, needle: str) -> list[tuple[int, int]]:
    """Non-overlapping left-to-right matches, the same rule `str.count` uses."""
    spans: list[tuple[int, int]] = []
    start = 0
    while True:
        at = text.find(needle, start)
        if at < 0:
            return spans
        spans.append((at, at + len(needle)))
        start = at + len(needle)


def apply_edits(original: str, edits: Sequence[tuple[str, int, dict[str, Any]]],
                relpath: str) -> str:
    """Rewrite declared spans of `original`. Order-independent by construction.

    Every span is located in the ORIGINAL text and the result is assembled once
    from disjoint spans. Sequential search-and-replace would let edit A create
    or destroy edit B's anchor, which would make the output depend on record
    order -- and determinism would then be a property of the ledger's file
    names rather than of the tool.
    """
    placed: list[tuple[int, int, str, str]] = []
    for record_id, index, edit in edits:
        tag = f"{record_id} edit[{index}] on {relpath}"
        spans = find_spans(original, edit["find"])
        if len(spans) != edit["occurrences"]:
            raise Refusal("edit-occurrence-mismatch",
                          f"{tag}: declared {edit['occurrences']} occurrence(s), found "
                          f"{len(spans)}")
        for start, end in spans:
            placed.append((start, end, edit["replace"], tag))
    placed.sort(key=lambda item: (item[0], item[1]))
    for (a_start, a_end, _r, a_tag), (b_start, _b_end, _r2, b_tag) in zip(placed, placed[1:]):
        if b_start < a_end:
            raise Refusal("edit-overlap",
                          f"{a_tag} and {b_tag} match overlapping spans "
                          f"[{a_start},{a_end}) and [{b_start},...)")
    out: list[str] = []
    cursor = 0
    for start, end, replacement, _tag in placed:
        out.append(original[cursor:start])
        out.append(replacement)
        cursor = end
    out.append(original[cursor:])
    return "".join(out)


# --------------------------------------------------------------------------
# effect checking
# --------------------------------------------------------------------------

def _read_text(path: pathlib.Path, relpath: str, side: str) -> str:
    data = path.read_bytes()
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise Refusal("target-not-utf8",
                      f"{side} {relpath} is not valid UTF-8; an edit is a text operation "
                      f"and a binary target cannot be one") from exc


def check_effects(record: dict[str, Any], src_root: pathlib.Path,
                  out_root: pathlib.Path, out_files: dict[str, str]) -> list[dict[str, Any]]:
    """Re-derive every declared effect FROM THE BYTES THAT WERE WRITTEN.

    Not from the in-memory string the edit produced: a number measured on a
    tree that was not written is the failure this project has already paid for
    more than once. Everything here reads back off disk.
    """
    results = []
    for index, effect in enumerate(record["expected_effect"]):
        tag = f"{record['id']} expected_effect[{index}]"
        relpath = effect["file"] if record["form"] == "." else f"{record['form']}/{effect['file']}"
        if relpath not in out_files:
            raise Refusal("effect-target-missing",
                          f"{tag}: {relpath} is not in the corrected tree")
        out_path = out_root / relpath
        kind = effect["kind"]
        if kind == "sha256":
            got: Any = out_files[relpath]
            want: Any = effect["value"].lower()
        elif kind == "occurs":
            got = _read_text(out_path, relpath, "output").count(effect["text"])
            want = effect["count"]
        elif kind == "absent":
            got = _read_text(out_path, relpath, "output").count(effect["text"])
            want = 0
        else:  # count_delta -- the only kind that also measures the SOURCE
            before = _read_text(src_root / relpath, relpath, "source").count(effect["text"])
            after = _read_text(out_path, relpath, "output").count(effect["text"])
            got = {"before": before, "after": after}
            want = {"before": effect["before"], "after": effect["after"]}
        if got != want:
            raise Refusal("effect-unmet",
                          f"{tag}: declared {want!r}, measured {got!r} on {relpath}")
        results.append({"index": index, "kind": kind, "file": relpath,
                        "measured": got, "declared": want, "met": True})
    return results


# --------------------------------------------------------------------------
# the build
# --------------------------------------------------------------------------

def guard_paths(src_root: pathlib.Path, out_root: pathlib.Path,
                manifest_path: pathlib.Path) -> None:
    """Rule 1. The stage-1 tree is read-only and `forms/` is a hard floor."""
    src = src_root.resolve()
    out = out_root.resolve()
    manifest = manifest_path.resolve()
    stage1 = STAGE1_TREE.resolve()
    for label, candidate in (("output tree", out), ("manifest", manifest)):
        if candidate == stage1 or stage1 in candidate.parents:
            raise Refusal("writes-into-stage1",
                          f"{label} {candidate} is at or under {stage1}; a published batch is "
                          f"immutable and this tool never writes into it")
        if candidate == src or src in candidate.parents:
            raise Refusal("writes-into-source",
                          f"{label} {candidate} is at or under the source tree {src}")
    if out in src.parents:
        raise Refusal("output-contains-source",
                      f"output tree {out} contains the source tree {src}")


def build_tree(src_root: pathlib.Path, records: Sequence[dict[str, Any]],
               staging: pathlib.Path) -> dict[str, Any]:
    """Materialise copy(batch) + records into `staging` and measure the result.

    Rule 3 is structural here: `edited` is keyed by the files some record's
    edit actually names. Every other path goes through a byte copy. There is no
    branch in which an unnamed file is rewritten, reformatted, or normalised.
    """
    src_files, src_dirs = scan_tree(src_root, "source-tree")

    # Group edits by target path, across records, so two records touching the
    # same file are checked for overlap against each other and not just against
    # themselves.
    by_target: dict[str, list[tuple[str, int, dict[str, Any]]]] = {}
    for record in records:
        form = record["form"]
        if form != ".":
            if form not in src_dirs:
                raise Refusal("record-form-missing",
                              f"{record['id']}: form directory {form!r} is not in the batch; "
                              f"a correction that cannot be applied is a failure, not a skip")
        for index, edit in enumerate(record["edits"]):
            relpath = edit["file"] if form == "." else f"{form}/{edit['file']}"
            if relpath not in src_files:
                raise Refusal("record-target-missing",
                              f"{record['id']} edit[{index}]: {relpath} is not in the batch")
            by_target.setdefault(relpath, []).append((record["id"], index, edit))

    edited: dict[str, bytes] = {}
    for relpath in sorted(by_target):
        original = _read_text(src_root / relpath, relpath, "source")
        updated = apply_edits(original, by_target[relpath], relpath)
        if updated == original:                                  # pragma: no cover - defensive
            raise Refusal("edit-no-effect",
                          f"{relpath}: the declared edits left the file unchanged")
        edited[relpath] = updated.encode("utf-8")

    staging.mkdir(parents=True, exist_ok=True)
    for rel in src_dirs:
        (staging / rel).mkdir(parents=True, exist_ok=True)
    for relpath in sorted(src_files):
        destination = staging / relpath
        destination.parent.mkdir(parents=True, exist_ok=True)
        if relpath in edited:
            destination.write_bytes(edited[relpath])
        else:
            # Byte copy, not a re-serialisation. `copyfile` moves content only;
            # `copymode` keeps the permission bits so the corrected tree is the
            # batch in every respect the batch defines.
            shutil.copyfile(src_root / relpath, destination)
        shutil.copymode(src_root / relpath, destination)

    out_files, out_dirs = scan_tree(staging, "output-tree")

    # Rule 4c: a correction cannot add, delete or rename. Anything else here
    # would mean the applier had a path that makes an artefact disappear.
    if set(out_files) != set(src_files):
        added = sorted(set(out_files) - set(src_files))
        lost = sorted(set(src_files) - set(out_files))
        raise Refusal("file-set-changed",
                      f"corrected tree file set differs from the batch: added={added[:5]} "
                      f"missing={lost[:5]}")
    if out_dirs != src_dirs:
        raise Refusal("dir-set-changed", "corrected tree directory set differs from the batch")

    # And the copy path is proved per file, not assumed: every file no record
    # named must hash equal on both sides.
    for relpath, sha in src_files.items():
        if relpath not in edited and out_files[relpath] != sha:
            raise Refusal("uncorrected-file-changed",
                          f"{relpath} has no correction record but its bytes changed")
        if relpath in edited and out_files[relpath] == sha:
            raise Refusal("corrected-file-unchanged",
                          f"{relpath} carries a correction record but its bytes did not change")

    return {"src_files": src_files, "src_dirs": src_dirs,
            "out_files": out_files, "out_dirs": out_dirs,
            "edited": sorted(edited), "by_target": by_target}


def build_manifest(*, batch: str, source_commit: str | None,
                   labels: dict[str, str], built: dict[str, Any],
                   records: Sequence[dict[str, Any]],
                   effects: dict[str, list[dict[str, Any]]],
                   tool_sha: str) -> dict[str, Any]:
    """The ledger. One file that answers "what changed, and under whose authority".

    Deterministic by construction: no timestamps, no absolute paths, no run
    ids, every list sorted by a stable key.
    """
    src_files = built["src_files"]
    out_files = built["out_files"]
    record_by_file: dict[str, list[str]] = {}
    for relpath, edits in built["by_target"].items():
        record_by_file[relpath] = sorted({rid for rid, _i, _e in edits})

    files = []
    for relpath in sorted(src_files):
        entry = {"path": relpath,
                 "source_sha256": src_files[relpath],
                 "output_sha256": out_files[relpath],
                 "state": "corrected" if relpath in record_by_file else "copied"}
        if relpath in record_by_file:
            entry["records"] = record_by_file[relpath]
        files.append(entry)

    manifest_records = []
    divergences = []
    for record in records:
        public = {k: v for k, v in record.items() if not k.startswith("_")}
        manifest_records.append({
            "id": record["id"],
            "record_file": record["_source_file"],
            "record_sha256": record["_source_sha256"],
            "record": public,
            "applied_to": sorted({(edit["file"] if record["form"] == "."
                                   else f"{record['form']}/{edit['file']}")
                                  for edit in record["edits"]}),
            "effects_re_derived": effects[record["id"]],
        })
        divergences.append({
            "record_id": record["id"],
            "form": record["form"],
            "subject": record["subject"],
            "diverges_from": record["diverges_from"],
            "authority": record["authority"],
            "still_diverges_from_official": True,
            # The exact sentence a fidelity report has to print. Generated
            # here so a downstream reporter cannot paraphrase the divergence
            # into a pass (ARCHITECTURE.md, "Rules the user set" #1).
            "report": (f"diverges by declared override {record['id']}, "
                       f"authorised by {record['authority']}"),
            "independent_verifier": record["verified_by"],
            "independently_verified": False,
        })

    bundles = sorted({relpath.rsplit("/", 1)[0] for relpath in src_files
                      if relpath.endswith("/" + BUNDLE_MARKER)})

    return {
        "manifest_version": MANIFEST_VERSION,
        "tool": "tools/formgen/correct.py",
        "tool_sha256": tool_sha,
        "source_batch": batch,
        "source_commit": source_commit,
        "source_tree": labels["source_tree"],
        "source_tree_sha256": tree_sha256(src_files, built["src_dirs"]),
        "records_dir": labels["records_dir"],
        "output_tree": labels["output_tree"],
        "output_tree_sha256": tree_sha256(out_files, built["out_dirs"]),
        "counts": {
            "files": len(src_files),
            "directories": len(built["src_dirs"]),
            "bundles": len(bundles),
            "records": len(records),
            "files_corrected": len(record_by_file),
            "files_copied_byte_identical": len(src_files) - len(record_by_file),
        },
        "bundle_marker": BUNDLE_MARKER,
        # Honesty flags, in the house style. None of these is a claim the tool
        # cannot support: the first three are structural properties of this
        # file's schema, and the fourth is the limit of what a producer may say
        # about its own output.
        "divergence_count": len(divergences),
        "hides_divergence": False,
        "suppressions_expressible": False,
        "self_check_is_not_independent_verification": True,
        "divergences": divergences,
        "records": manifest_records,
        "files": files,
    }


def manifest_bytes(manifest: dict[str, Any]) -> bytes:
    return (json.dumps(manifest, indent=2, ensure_ascii=True) + "\n").encode("utf-8")


def tool_sha256() -> str:
    return sha256_path(pathlib.Path(__file__).resolve())


def run_build(*, src_root: pathlib.Path, records_dir: pathlib.Path,
              staging: pathlib.Path, batch: str, source_commit: str | None,
              labels: dict[str, str]) -> tuple[dict[str, Any], dict[str, Any]]:
    records = load_records(records_dir)
    built = build_tree(src_root, records, staging)
    effects = {r["id"]: check_effects(r, src_root, staging, built["out_files"])
               for r in records}
    manifest = build_manifest(batch=batch, source_commit=source_commit, labels=labels,
                              built=built, records=records, effects=effects,
                              tool_sha=tool_sha256())
    return manifest, built


# --------------------------------------------------------------------------
# commands
# --------------------------------------------------------------------------

def cmd_apply(args: argparse.Namespace) -> int:
    src_root = args.source_tree or STAGE1_TREE
    records_dir = args.records or (HERE / "corrections")
    out_root = args.out or (REPO / "forms-corrected")
    manifest_path = args.manifest or out_root.parent / (out_root.name + ".manifest.json")

    if not args.batch or not args.batch.strip():
        raise Refusal("batch-unnamed",
                      "--batch must name the stage-1 batch this tree is built from")
    guard_paths(src_root, out_root, manifest_path)
    if out_root.exists():
        if not out_root.is_dir():
            raise Refusal("output-not-a-directory", f"{out_root} exists and is not a directory")
        if any(out_root.iterdir()) and not args.force:
            raise Refusal("output-not-empty",
                          f"{out_root} exists and is not empty; pass --force to replace it")

    labels = {"source_tree": tree_label(src_root),
              "records_dir": tree_label(records_dir),
              "output_tree": tree_label(out_root)}

    # Everything is built and checked in a temporary directory first. The output
    # tree therefore never exists in a half-corrected state: it is either
    # absent, or it is a tree every record landed in and every declared effect
    # was re-derived from. A real run stages in a sibling of the output so the
    # final step is a rename on one filesystem; a dry run stages in the system
    # temp dir, because it must not create so much as the output's parent.
    if args.dry_run:
        parent = None
    else:
        parent = out_root.parent
        parent.mkdir(parents=True, exist_ok=True)
    holding = pathlib.Path(tempfile.mkdtemp(prefix=".correct-staging-", dir=parent))
    staging = holding / "tree"
    trash: pathlib.Path | None = None
    try:
        manifest, built = run_build(src_root=src_root, records_dir=records_dir,
                                    staging=staging, batch=args.batch,
                                    source_commit=args.source_commit, labels=labels)
        payload = manifest_bytes(manifest)
        if args.dry_run:
            summary = _summary(manifest, dry_run=True)
            json.dump(summary, sys.stdout, indent=2)
            sys.stdout.write("\n")
            print(f"dry run: nothing written to {out_root}", file=sys.stderr)
            return 0
        if out_root.exists():
            trash = holding / "replaced"
            os.replace(out_root, trash)
        os.replace(staging, out_root)
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        manifest_path.write_bytes(payload)
    finally:
        shutil.rmtree(holding, ignore_errors=True)
        if trash is not None and trash.exists():                 # pragma: no cover - defensive
            shutil.rmtree(trash, ignore_errors=True)

    summary = _summary(manifest, dry_run=False)
    summary["manifest"] = tree_label(manifest_path)
    json.dump(summary, sys.stdout, indent=2)
    sys.stdout.write("\n")
    return 0


def _summary(manifest: dict[str, Any], *, dry_run: bool) -> dict[str, Any]:
    return {
        "ok": True,
        "dry_run": dry_run,
        "source_batch": manifest["source_batch"],
        "source_tree_sha256": manifest["source_tree_sha256"],
        "output_tree_sha256": manifest["output_tree_sha256"],
        "counts": manifest["counts"],
        "records_applied": [r["id"] for r in manifest["records"]],
        "divergences": [d["report"] for d in manifest["divergences"]],
        "hides_divergence": manifest["hides_divergence"],
        "self_check_is_not_independent_verification":
            manifest["self_check_is_not_independent_verification"],
    }


def cmd_verify(args: argparse.Namespace) -> int:
    """Re-derive that the tree on disk is exactly copy(batch) + records.

    This is what lets a corrected tree be checked WITHOUT trusting the run that
    made it: the inputs are the batch, the ledger and the manifest, and the
    output tree is treated as a suspect. A byte edited by hand afterwards, a
    file added, a file removed, a record quietly rewritten, or a manifest
    number touched up all land as failures here.
    """
    manifest_path = args.manifest
    if manifest_path is None or not manifest_path.is_file():
        raise Refusal("manifest-missing", f"{manifest_path} is not a file")
    given_bytes = manifest_path.read_bytes()
    try:
        given = json.loads(given_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise Refusal("manifest-unparseable", f"{manifest_path}: {exc}") from exc
    if given.get("manifest_version") != MANIFEST_VERSION:
        raise Refusal("manifest-version",
                      f"manifest_version {given.get('manifest_version')!r} != {MANIFEST_VERSION}")

    current_tool = tool_sha256()
    if given.get("tool_sha256") != current_tool and not args.allow_tool_drift:
        raise Refusal("tool-drift",
                      f"manifest was written by correct.py {str(given.get('tool_sha256'))[:12]} "
                      f"and this is {current_tool[:12]}; re-run the applier, or pass "
                      f"--allow-tool-drift to verify the tree against a different version")

    # Paths label, hashes bind: the manifest's own labels are reused so a
    # relocated checkout still re-derives byte-identical manifest bytes.
    src_root = args.source_tree or (REPO / given["source_tree"])
    records_dir = args.records or (REPO / given["records_dir"])
    out_root = args.out or (REPO / given["output_tree"])
    if not out_root.is_dir():
        raise Refusal("output-tree-missing", f"{out_root} is not a directory")

    labels = {"source_tree": given["source_tree"],
              "records_dir": given["records_dir"],
              "output_tree": given["output_tree"]}

    holding = pathlib.Path(tempfile.mkdtemp(prefix=".correct-verify-"))
    problems: list[str] = []
    try:
        staging = holding / "tree"
        rebuilt, _built = run_build(src_root=src_root, records_dir=records_dir,
                                    staging=staging, batch=given["source_batch"],
                                    source_commit=given.get("source_commit"), labels=labels)
        if args.allow_tool_drift:
            rebuilt["tool_sha256"] = given.get("tool_sha256")
        rebuilt_bytes = manifest_bytes(rebuilt)
        if rebuilt_bytes != given_bytes:
            first = _first_difference(rebuilt, given)
            problems.append(f"manifest does not re-derive: {first}")

        have_files, have_dirs = scan_tree(out_root, "output-tree")
        want_files, want_dirs = scan_tree(staging, "rebuilt-tree")
        for relpath in sorted(set(want_files) - set(have_files)):
            problems.append(f"missing from the corrected tree: {relpath}")
        for relpath in sorted(set(have_files) - set(want_files)):
            problems.append(f"present in the corrected tree but not derivable: {relpath}")
        for relpath in sorted(set(have_files) & set(want_files)):
            if have_files[relpath] != want_files[relpath]:
                problems.append(f"bytes differ from copy(batch)+records: {relpath}")
        if have_dirs != want_dirs:
            problems.append("directory shape differs from copy(batch)+records")
    finally:
        shutil.rmtree(holding, ignore_errors=True)

    result = {
        "ok": not problems,
        "verified": "the corrected tree is exactly copy(batch) + declared records",
        "source_batch": given["source_batch"],
        "records": [r["id"] for r in given.get("records", [])],
        "divergences": [d["report"] for d in given.get("divergences", [])],
        "problems": problems[:50],
        "problem_count": len(problems),
    }
    json.dump(result, sys.stdout, indent=2)
    sys.stdout.write("\n")
    return 0 if not problems else 1


def _first_difference(rebuilt: Any, given: Any, path: str = "") -> str:
    """Name the first key whose value does not re-derive, for a usable message."""
    if isinstance(rebuilt, dict) and isinstance(given, dict):
        for key in sorted(set(rebuilt) | set(given)):
            here = f"{path}.{key}" if path else key
            if key not in rebuilt:
                return f"{here} present in the manifest, not re-derived"
            if key not in given:
                return f"{here} re-derived, absent from the manifest"
            if rebuilt[key] != given[key]:
                return _first_difference(rebuilt[key], given[key], here)
        return f"{path or '<root>'} differs only in key order"
    if isinstance(rebuilt, list) and isinstance(given, list):
        if len(rebuilt) != len(given):
            return f"{path}: {len(given)} entries in the manifest, {len(rebuilt)} re-derived"
        for index, (a, b) in enumerate(zip(rebuilt, given)):
            if a != b:
                return _first_difference(a, b, f"{path}[{index}]")
    return f"{path or '<root>'}: manifest {given!r} != re-derived {rebuilt!r}"


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

_PNG = bytes.fromhex("89504e470d0a1a0a") + bytes(range(256)) * 3

_INDEX_A = """<!doctype html><html><body>
<div class=tin>{slots}</div>
<p class=note>BIR Form 2550M &mdash; February 2007 (ENCS)</p>
</body></html>
"""


def _slots(count: int) -> str:
    return "".join(f'<input name=p1c2s{i} maxlength=1>' for i in range(count))


def _make_batch(root: pathlib.Path) -> None:
    """A miniature stage-1 batch: two bundles, shared assets, an empty dir."""
    (root / "alpha-2007").mkdir(parents=True)
    (root / "alpha-2007" / "index.html").write_text(
        _INDEX_A.format(slots=_slots(12)), encoding="utf-8")
    (root / "alpha-2007" / "form.css").write_text("body{margin:0}\n", encoding="utf-8")
    (root / "alpha-2007" / BUNDLE_MARKER).write_text('{"slug":"alpha-2007"}\n', encoding="utf-8")
    (root / "extra" / "beta-2019").mkdir(parents=True)
    (root / "extra" / "beta-2019" / "index.html").write_text(
        "<html><body>beta</body></html>\n", encoding="utf-8")
    (root / "extra" / "beta-2019" / BUNDLE_MARKER).write_text('{"slug":"beta"}\n', encoding="utf-8")
    (root / "assets").mkdir()
    (root / "assets" / "art.png").write_bytes(_PNG)
    (root / "base.css").write_text("*{box-sizing:border-box}\n", encoding="utf-8")
    (root / "empty-dir").mkdir()


def _good_record() -> dict[str, Any]:
    """The shape of C01, on the miniature batch: 12 TIN slots -> 14."""
    return {
        "id": "C01",
        "form": "alpha-2007",
        "subject": "alpha-2007 p1c2 TIN comb, trailing branch-code group",
        "what": "branch code 3 digits -> 5: two further comb slots",
        "reason": ("The Feb-2007 artwork prints 3 compartments and is out of date; a "
                   "taxpayer with a 4- or 5-digit branch code cannot file correctly."),
        "authority": "official-hta-runtime#control:L409 (frm2550m:txtBranchCode max_length 5)",
        "diverges_from": "official-fidelity/cell-edge-f1-v1 on the TIN comb",
        "verified_by": "comb_referee.py re-derivation from the pinned PDF",
        "expected_effect": [
            {"kind": "count_delta", "file": "index.html",
             "text": "<input name=p1c2s", "before": 12, "after": 14},
            {"kind": "occurs", "file": "index.html",
             "text": "<input name=p1c2s13 maxlength=1>", "count": 1},
        ],
        "edits": [{"file": "index.html",
                   "find": '<input name=p1c2s11 maxlength=1></div>',
                   "replace": ('<input name=p1c2s11 maxlength=1>'
                               '<input name=p1c2s12 maxlength=1>'
                               '<input name=p1c2s13 maxlength=1></div>'),
                   "occurrences": 1}],
    }


def _write_record(records: pathlib.Path, record: dict[str, Any],
                  name: str = "c01.json") -> pathlib.Path:
    path = records / name
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return path


_LAST_STDERR = ""


def _run(argv: Sequence[str]) -> int:
    """Drive the CLI the way an operator does, so exit codes are under test.

    The summary and the refusal message are captured rather than printed: the
    self-test's own PASS/FAIL lines are the report, and a failing case quotes
    the captured refusal in its detail so nothing is lost.
    """
    global _LAST_STDERR
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = main(list(argv))
    _LAST_STDERR = err.getvalue().strip()
    return rc


class _Checker:
    def __init__(self) -> None:
        self.failures = 0

    def __call__(self, name: str, ok: bool, detail: Any = "") -> bool:
        self.failures += not ok
        suffix = "" if ok else f": {detail}"
        print(f"  {'PASS' if ok else 'FAIL'}  {name}{suffix}")
        return bool(ok)


def _dirs_identical(a: pathlib.Path, b: pathlib.Path) -> tuple[bool, str]:
    a_files, a_dirs = scan_tree(a, "a")
    b_files, b_dirs = scan_tree(b, "b")
    if set(a_files) != set(b_files):
        return False, f"file sets differ: {sorted(set(a_files) ^ set(b_files))[:4]}"
    if a_dirs != b_dirs:
        return False, "directory sets differ"
    differing = [p for p in a_files if a_files[p] != b_files[p]]
    if differing:
        return False, f"{len(differing)} file(s) differ, first {differing[0]}"
    # Hash equality is the claim, but the copy path is the one thing worth
    # proving by the bytes themselves rather than by a digest of them.
    for relpath in sorted(a_files):
        if (a / relpath).read_bytes() != (b / relpath).read_bytes():
            return False, f"bytes differ at {relpath} despite equal digests"
    return True, ""


def self_test() -> int:  # noqa: C901 - a contract with many clauses, listed flat
    check = _Checker()
    with tempfile.TemporaryDirectory() as tmp:
        base = pathlib.Path(tmp)
        src = base / "batch"
        _make_batch(src)
        empty_records = base / "records-empty"
        empty_records.mkdir()
        src_files_before, src_dirs_before = scan_tree(src, "src")
        src_sha_before = tree_sha256(src_files_before, src_dirs_before)

        # ------------------------------------------------------------------
        # 1. No record => byte-identical copy. The user's rule, asserted on
        #    bytes rather than on similarity.
        # ------------------------------------------------------------------
        out = base / "corrected"
        manifest_path = base / "corrected.manifest.json"
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                   "--records", str(empty_records), "--out", str(out),
                   "--manifest", str(manifest_path)])
        check("an empty ledger applies cleanly", rc == 0, f"rc={rc}")
        identical, why = _dirs_identical(src, out)
        check("no record => byte-identical copy of every file", identical, why)
        manifest = json.loads(manifest_path.read_text())
        check("no record => identical tree digest",
              manifest["source_tree_sha256"] == manifest["output_tree_sha256"],
              manifest["output_tree_sha256"])
        check("the empty-dir survives the copy", (out / "empty-dir").is_dir())
        check("a binary asset copies byte-for-byte",
              (out / "assets" / "art.png").read_bytes() == _PNG)

        # ------------------------------------------------------------------
        # 2. The manifest names batch, records, and the sha of every file.
        # ------------------------------------------------------------------
        check("the manifest names the source batch",
              manifest["source_batch"] == "corpus/rTEST", manifest["source_batch"])
        check("the manifest hashes every input and output file",
              len(manifest["files"]) == len(src_files_before)
              and all(entry["source_sha256"] == src_files_before[entry["path"]]
                      and entry["output_sha256"] == src_files_before[entry["path"]]
                      for entry in manifest["files"]),
              len(manifest["files"]))
        check("every file is reported copied when no record exists",
              all(entry["state"] == "copied" for entry in manifest["files"]))
        check("the manifest counts bundles by their provenance marker",
              manifest["counts"]["bundles"] == 2, manifest["counts"])
        check("the manifest carries no absolute path and no timestamp",
              str(base) not in manifest_path.read_text())

        # ------------------------------------------------------------------
        # 3. The applier never writes into the source tree.
        # ------------------------------------------------------------------
        src_files_after, src_dirs_after = scan_tree(src, "src")
        check("the batch is untouched by a run",
              tree_sha256(src_files_after, src_dirs_after) == src_sha_before)

        # ------------------------------------------------------------------
        # 4. Determinism: two runs, byte-identical tree and manifest.
        # ------------------------------------------------------------------
        out2 = base / "corrected2"
        manifest2 = base / "corrected2.manifest.json"
        _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
              "--records", str(empty_records), "--out", str(out2),
              "--manifest", str(manifest2)])
        same, why = _dirs_identical(out, out2)
        check("two runs produce byte-identical trees", same, why)
        a = json.loads(manifest_path.read_text())
        b = json.loads(manifest2.read_text())
        a["output_tree"] = b["output_tree"] = ""      # the only intended difference
        check("two runs produce byte-identical manifests",
              manifest_bytes(a) == manifest_bytes(b),
              _first_difference(a, b))

        # ------------------------------------------------------------------
        # 5. A record applies, and changes ONLY what it declared.
        # ------------------------------------------------------------------
        records = base / "records"
        records.mkdir()
        _write_record(records, _good_record())
        cout = base / "corrected-c01"
        cmanifest = base / "corrected-c01.manifest.json"
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                   "--records", str(records), "--out", str(cout),
                   "--manifest", str(cmanifest)])
        check("a declared record applies", rc == 0, f"rc={rc}")
        cm = json.loads(cmanifest.read_text())
        corrected = [e for e in cm["files"] if e["state"] == "corrected"]
        check("exactly the declared file is corrected",
              [e["path"] for e in corrected] == ["alpha-2007/index.html"],
              [e["path"] for e in corrected])
        untouched = [e for e in cm["files"] if e["state"] == "copied"]
        check("every other file stays byte-identical under a record",
              all(e["source_sha256"] == e["output_sha256"] for e in untouched)
              and all((cout / e["path"]).read_bytes() == (src / e["path"]).read_bytes()
                      for e in untouched))
        check("the corrected file really changed",
              corrected[0]["source_sha256"] != corrected[0]["output_sha256"])
        check("the correction landed in the written bytes",
              (cout / "alpha-2007/index.html").read_text().count("<input name=p1c2s") == 14)
        check("the manifest records which record touched which file",
              corrected[0]["records"] == ["C01"], corrected[0].get("records"))
        check("the manifest re-derives the declared effects",
              all(e["met"] for e in cm["records"][0]["effects_re_derived"])
              and len(cm["records"][0]["effects_re_derived"]) == 2)
        check("the manifest hashes the record file itself",
              cm["records"][0]["record_sha256"]
              == sha256_bytes((records / "c01.json").read_bytes()))

        # ------------------------------------------------------------------
        # 6. A correction never hides a divergence.
        # ------------------------------------------------------------------
        divergence = cm["divergences"][0]
        check("the divergence is published, not suppressed",
              divergence["still_diverges_from_official"] is True
              and cm["hides_divergence"] is False
              and cm["divergence_count"] == 1)
        check("the divergence report names the record and the authority",
              divergence["report"].startswith("diverges by declared override C01, authorised by "),
              divergence["report"])
        check("the divergence names the check it still fails",
              divergence["diverges_from"] == "official-fidelity/cell-edge-f1-v1 on the TIN comb")
        check("the applier does not claim independent verification",
              divergence["independently_verified"] is False
              and cm["self_check_is_not_independent_verification"] is True
              and cm["suppressions_expressible"] is False)

        # ------------------------------------------------------------------
        # 7. --verify re-derives the tree without trusting the run that made it.
        # ------------------------------------------------------------------
        rc = _run(["--verify", "--manifest", str(cmanifest), "--source-tree", str(src),
                   "--records", str(records), "--out", str(cout)])
        check("--verify accepts a tree that re-derives", rc == 0, f"rc={rc}")

        tampered = base / "tampered"
        shutil.copytree(cout, tampered)
        victim = tampered / "extra" / "beta-2019" / "index.html"
        victim.write_text(victim.read_text().replace("beta", "gamma"), encoding="utf-8")
        rc = _run(["--verify", "--manifest", str(cmanifest), "--source-tree", str(src),
                   "--records", str(records), "--out", str(tampered)])
        check("--verify rejects a hand-edited byte", rc != 0, f"rc={rc}")

        added = base / "added"
        shutil.copytree(cout, added)
        (added / "alpha-2007" / "sneaked.html").write_text("x\n", encoding="utf-8")
        rc = _run(["--verify", "--manifest", str(cmanifest), "--source-tree", str(src),
                   "--records", str(records), "--out", str(added)])
        check("--verify rejects an added file", rc != 0, f"rc={rc}")

        removed = base / "removed"
        shutil.copytree(cout, removed)
        (removed / "base.css").unlink()
        rc = _run(["--verify", "--manifest", str(cmanifest), "--source-tree", str(src),
                   "--records", str(records), "--out", str(removed)])
        check("--verify rejects a deleted file", rc != 0, f"rc={rc}")

        bent = base / "bent.manifest.json"
        bent_obj = json.loads(cmanifest.read_text())
        bent_obj["divergences"][0]["still_diverges_from_official"] = False
        bent.write_bytes(manifest_bytes(bent_obj))
        rc = _run(["--verify", "--manifest", str(bent), "--source-tree", str(src),
                   "--records", str(records), "--out", str(cout)])
        check("--verify rejects a manifest edited to soften a divergence", rc != 0, f"rc={rc}")

        swapped = base / "records-swapped"
        swapped.mkdir()
        rogue = _good_record()
        rogue["authority"] = "because I said so"
        _write_record(swapped, rogue)
        rc = _run(["--verify", "--manifest", str(cmanifest), "--source-tree", str(src),
                   "--records", str(swapped), "--out", str(cout)])
        check("--verify rejects a record rewritten after the fact", rc != 0, f"rc={rc}")

        rc = _run(["--verify", "--manifest", str(base / "nope.json")])
        check("--verify refuses when the manifest is missing", rc != 0, f"rc={rc}")

        # ------------------------------------------------------------------
        # 8. Refusals. Each one exits non-zero and leaves no output tree.
        # ------------------------------------------------------------------
        def refusal(name: str, mutate, *, records_dir: pathlib.Path | None = None,
                    extra_argv: Sequence[str] = ()) -> None:
            rdir = records_dir or (base / f"r-{abs(hash(name)) % 10**8}")
            if records_dir is None:
                rdir.mkdir()
                record = _good_record()
                mutate(record)
                _write_record(rdir, record)
            target = base / f"out-{abs(hash(name)) % 10**8}"
            argv = ["--batch", "corpus/rTEST", "--source-tree", str(src),
                    "--records", str(rdir), "--out", str(target),
                    "--manifest", str(target.with_suffix(".manifest.json"))]
            rc = _run([*argv, *extra_argv])
            check(name, rc != 0 and not target.exists(),
                  f"rc={rc} out_exists={target.exists()} {_LAST_STDERR}")

        refusal("refuses a record naming a form the batch does not have",
                lambda r: r.__setitem__("form", "no-such-form"))
        refusal("refuses a record naming a file the bundle does not have",
                lambda r: r["edits"][0].__setitem__("file", "nope.html"))
        refusal("refuses when the anchor count does not match the declaration",
                lambda r: r["edits"][0].__setitem__("occurrences", 2))
        refusal("refuses an anchor that does not match at all",
                lambda r: r["edits"][0].__setitem__("find", "not in the document"))
        refusal("refuses an unknown record key",
                lambda r: r.__setitem__("suppress", ["cell-edge-f1-v1"]))
        refusal("refuses a missing record key",
                lambda r: r.pop("authority"))
        refusal("refuses an empty authority",
                lambda r: r.__setitem__("authority", "   "))
        refusal("refuses a record that names no failing check",
                lambda r: r.__setitem__("diverges_from", ""))
        refusal("refuses a record with no independent verifier",
                lambda r: r.__setitem__("verified_by", ""))
        refusal("refuses a record with no declared effect",
                lambda r: r.__setitem__("expected_effect", []))
        refusal("refuses a record whose only effect restates its own edit",
                lambda r: r.__setitem__("expected_effect",
                                        [{"kind": "sha256", "file": "index.html",
                                          "value": "0" * 64}]))
        refusal("refuses an effect the written bytes do not satisfy",
                lambda r: r["expected_effect"][0].__setitem__("after", 15))
        refusal("refuses a count_delta that states no change",
                lambda r: r["expected_effect"][0].update({"before": 12, "after": 12}))
        refusal("refuses an unknown effect kind",
                lambda r: r["expected_effect"][0].__setitem__("kind", "looks_fine"))
        refusal("refuses a no-op edit",
                lambda r: r["edits"][0].__setitem__("replace", r["edits"][0]["find"]))
        refusal("refuses an edit with no edits at all",
                lambda r: r.__setitem__("edits", []))
        refusal("refuses a path that escapes the bundle",
                lambda r: r["edits"][0].__setitem__("file", "../base.css"))
        refusal("refuses an absolute path in a record",
                lambda r: r["edits"][0].__setitem__("file", "/etc/passwd"))
        refusal("refuses an unknown edit key",
                lambda r: r["edits"][0].__setitem__("regex", True))
        refusal("refuses a record with an unusable id",
                lambda r: r.__setitem__("id", "../../escape"))
        refusal("refuses a future record_version",
                lambda r: r.__setitem__("record_version", 99))

        # Two records, one of which cannot apply: nothing is written, so there
        # is no half-corrected tree. This is the atomicity clause.
        pair = base / "records-pair"
        pair.mkdir()
        _write_record(pair, _good_record(), "a.json")
        broken = _good_record()
        broken["id"] = "C02"
        broken["edits"][0]["find"] = "absent anchor"
        _write_record(pair, broken, "b.json")
        refusal("one unapplicable record refuses the whole tree",
                None, records_dir=pair)

        # Duplicate ids across two files in the ledger.
        dup = base / "records-dup"
        dup.mkdir()
        _write_record(dup, _good_record(), "a.json")
        _write_record(dup, _good_record(), "b.json")
        refusal("refuses duplicate record ids", None, records_dir=dup)

        # Two records whose anchors overlap in the same file.
        overlap = base / "records-overlap"
        overlap.mkdir()
        _write_record(overlap, _good_record(), "a.json")
        second = _good_record()
        second["id"] = "C03"
        second["edits"][0]["find"] = '<input name=p1c2s11 maxlength=1></div>\n<p class=note>'
        second["edits"][0]["replace"] = '<input name=p1c2s11 maxlength=1></div>\n<p class=x>'
        second["expected_effect"] = [{"kind": "occurs", "file": "index.html",
                                      "text": "class=x", "count": 1}]
        _write_record(overlap, second, "b.json")
        refusal("refuses two records whose anchors overlap", None, records_dir=overlap)

        # Malformed ledger entries.
        bad_json = base / "records-badjson"
        bad_json.mkdir()
        (bad_json / "c.json").write_text("{not json", encoding="utf-8")
        refusal("refuses an unparseable record file", None, records_dir=bad_json)

        missing_dir = base / "records-that-do-not-exist"
        refusal("refuses a missing ledger directory (absent is not empty)",
                None, records_dir=missing_dir)

        # A binary target cannot take a text edit.
        binary = base / "records-binary"
        binary.mkdir()
        bin_record = _good_record()
        bin_record["form"] = "assets"
        bin_record["edits"] = [{"file": "art.png", "find": "\x89PNG",
                                "replace": "\x89PNH", "occurrences": 1}]
        bin_record["expected_effect"] = [{"kind": "occurs", "file": "art.png",
                                          "text": "PNH", "count": 1}]
        _write_record(binary, bin_record)
        refusal("refuses a text edit against a non-UTF-8 file", None, records_dir=binary)

        # ------------------------------------------------------------------
        # 9. The applier refuses to write into forms/ or into the batch.
        # ------------------------------------------------------------------
        def path_refusal(name: str, out_path: pathlib.Path,
                         source: pathlib.Path = src) -> None:
            before = out_path.exists()
            rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(source),
                       "--records", str(empty_records), "--out", str(out_path),
                       "--manifest", str(base / "ignored.manifest.json")])
            check(name, rc != 0 and out_path.exists() == before, f"rc={rc}")

        path_refusal("refuses to write the corrected tree into forms/", STAGE1_TREE)
        path_refusal("refuses to write inside forms/", STAGE1_TREE / "corrected")
        path_refusal("refuses an output equal to the source tree", src)
        path_refusal("refuses an output inside the source tree", src / "corrected")
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                   "--records", str(empty_records), "--out", str(base / "x"),
                   "--manifest", str(STAGE1_TREE / "m.json")])
        check("refuses to write the manifest into forms/", rc != 0, f"rc={rc}")
        rc = _run(["--batch", "", "--source-tree", str(src),
                   "--records", str(empty_records), "--out", str(base / "y")])
        check("refuses an unnamed batch", rc != 0 and not (base / "y").exists(), f"rc={rc}")
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(base / "no-batch-here"),
                   "--records", str(empty_records), "--out", str(base / "z")])
        check("refuses a source tree that does not exist", rc != 0, f"rc={rc}")

        # ------------------------------------------------------------------
        # 10. An existing output is not silently replaced.
        # ------------------------------------------------------------------
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                   "--records", str(empty_records), "--out", str(out),
                   "--manifest", str(manifest_path)])
        check("refuses to overwrite a non-empty output tree", rc != 0, f"rc={rc}")
        check("the refused overwrite left the existing tree intact",
              _dirs_identical(src, out)[0])
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                   "--records", str(records), "--out", str(out),
                   "--manifest", str(manifest_path), "--force"])
        check("--force replaces the tree wholesale", rc == 0, f"rc={rc}")
        check("the replaced tree is the corrected one",
              (out / "alpha-2007/index.html").read_text().count("<input name=p1c2s") == 14)

        # ------------------------------------------------------------------
        # 11. --dry-run writes nothing at all.
        # ------------------------------------------------------------------
        dry = base / "dry"
        rc = _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                   "--records", str(records), "--out", str(dry),
                   "--manifest", str(base / "dry.manifest.json"), "--dry-run"])
        check("--dry-run succeeds without writing a tree",
              rc == 0 and not dry.exists() and not (base / "dry.manifest.json").exists(),
              f"rc={rc} tree={dry.exists()}")

        # ------------------------------------------------------------------
        # 12. Order independence: two records editing DIFFERENT spans of the
        #     SAME file, split across two ledger files in either name order,
        #     produce the same bytes. Spans are located in the original text
        #     and assembled once, so record order cannot reach the output.
        # ------------------------------------------------------------------
        order_a = base / "order-a"
        order_b = base / "order-b"
        for target, (first, second_name) in ((order_a, ("aa.json", "zz.json")),
                                             (order_b, ("zz.json", "aa.json"))):
            target.mkdir()
            one = _good_record()
            two = _good_record()
            two["id"] = "C04"
            two["edits"] = [{"file": "index.html", "find": "<p class=note>",
                             "replace": "<p class=n>", "occurrences": 1}]
            two["expected_effect"] = [{"kind": "occurs", "file": "index.html",
                                       "text": "<p class=n>", "count": 1}]
            _write_record(target, one, first)
            _write_record(target, two, second_name)
        outs = []
        for index, rdir in enumerate((order_a, order_b)):
            target = base / f"order-out-{index}"
            _run(["--batch", "corpus/rTEST", "--source-tree", str(src),
                  "--records", str(rdir), "--out", str(target),
                  "--manifest", str(base / f"order-{index}.manifest.json")])
            outs.append(target)
        same, why = _dirs_identical(outs[0], outs[1])
        check("ledger file order cannot change the output", same, why)

    print(f"self-test: {check.failures} failure(s)")
    return 1 if check.failures else 0


# --------------------------------------------------------------------------
# entry point
# --------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--batch", default="",
                        help="the stage-1 batch this tree is built from, e.g. corpus/r23")
    parser.add_argument("--source-commit", default=None,
                        help="optional commit or tag object id for the named batch")
    parser.add_argument("--source-tree", type=pathlib.Path, default=None,
                        help="the stage-1 tree to copy from (default: forms/)")
    parser.add_argument("--records", type=pathlib.Path, default=None,
                        help="the correction ledger directory (default: tools/formgen/corrections)")
    parser.add_argument("--out", type=pathlib.Path, default=None,
                        help="the corrected tree to write (default: forms-corrected/)")
    parser.add_argument("--manifest", type=pathlib.Path, default=None,
                        help="manifest path (default: <out>.manifest.json, a sibling of the "
                             "tree so that the tree itself stays exactly copy(batch)+records)")
    parser.add_argument("--force", action="store_true",
                        help="replace a non-empty output tree")
    parser.add_argument("--dry-run", action="store_true",
                        help="build and check everything, write nothing")
    parser.add_argument("--verify", action="store_true",
                        help="re-derive that an existing corrected tree is exactly "
                             "copy(batch) + records")
    parser.add_argument("--allow-tool-drift", action="store_true",
                        help="--verify only: accept a manifest written by another version "
                             "of this file")
    parser.add_argument("--self-test", action="store_true",
                        help="prove the contract on a miniature batch, then stop")
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    if args.self_test:
        return self_test()
    # Defaults are resolved per command, not here: --verify takes its paths
    # from the manifest when the operator did not name them, and a default
    # applied up front would silently override the manifest's own labels.
    try:
        if args.verify:
            return cmd_verify(args)
        return cmd_apply(args)
    except Refusal as exc:
        # A refusal is the designed outcome of every contract failure, so it
        # reports its code and stops. Nothing was written to --out.
        print(f"REFUSED {exc.code}: {exc.detail}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
