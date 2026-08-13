"""Reviewed ledger decisions: pure data, shape-validated, shipped empty.

R2b of the comb-referee campaign. The subject ledger's designed review paths
(`eligible-for-reviewed-resolution` for active-unresolved subjects whose
four-way evidence agrees, `explicit-transition-required` for retained ones)
have checkers on every side but no review INPUT anywhere. This module is that
input, and nothing else: two registries of decisions a named reviewer made,
each entry carrying its provenance, plus the shape validation both consumers
run before trusting a single byte of it.

Doctrine, unchanged from the reviewed-topology registry this mirrors:

* the registries ship EMPTY and grow only after the user reviews the
  evidence panels generated for each subject;
* a producer never certifies its own promotion -- lattice.py consumes an
  entry and publishes the transitioned state WITH a certificate naming it,
  and comb_referee.py independently validates that certificate against this
  module AND against its own current-run evidence (four-way agreement for a
  resolution, a TRUE source corroboration for a transition).  Review cannot
  overrule the paper: an entry the referee's own evidence contradicts is an
  ERROR, not a stronger review;
* this module carries no geometry and no measurement.  It is data with a
  schema, importable by producer and adjudicator alike without breaching the
  referee's independence, and its bytes join both attested closures.

`REVIEWED_LEDGER_RESOLUTIONS` -- (slug, page, cell_id) -> decision for an
ACTIVE_UNRESOLVED subject: the reviewer confirms the four-way-agreed comb is
the sheet's comb, and the subject resolves.

`REVIEWED_LEDGER_TRANSITIONS` -- (slug, page, legacy_cell_id) -> decision for
a RETAINED_UNRESOLVED subject: the reviewer confirms the suppressed legacy
comb claim, corroborated from the source (R2a), and names the transition.
`active_composite` keeps the subject in the ledger as the composite of its
mapped partition cells; `retired_proven_false` exists for a subject whose
partition is not real, and is expected to stay unused.
"""
from __future__ import annotations

from typing import Any

RESOLUTION_CRITERION = "reviewed-ledger-resolution-v1"
TRANSITION_CRITERION = "reviewed-ledger-transition-v1"
PERMITTED_TRANSITIONS = ("active_composite", "retired_proven_false")

REVIEWED_LEDGER_RESOLUTIONS: dict[tuple[str, int, str], dict[str, Any]] = {
    # (slug, page, cell_id): {
    #     "subject_key": ...,          # the exact ledger subject key
    #     "source_sha256": ...,        # pinned source PDF this was reviewed on
    #     "four_way": {"lattice": n, "audit": n, "emitted": n, "referee": n},
    #     "reviewer": ..., "date": ..., "citation": ...,
    # }
}

REVIEWED_LEDGER_TRANSITIONS: dict[tuple[str, int, str], dict[str, Any]] = {
    # (slug, page, legacy_cell_id): {
    #     "subject_key": ...,
    #     "source_sha256": ...,
    #     "transition": "active_composite",
    #     "suppression_criterion": ..., # the R2a criterion that corroborated
    #     "reviewer": ..., "date": ..., "citation": ...,
    # }
}


def _entry_errors(key: Any, value: Any, kind: str) -> list[str]:
    errors: list[str] = []
    if (not isinstance(key, tuple) or len(key) != 3
            or not isinstance(key[0], str) or not key[0]
            or not isinstance(key[1], int) or key[1] < 1
            or not isinstance(key[2], str) or not key[2]):
        return [f"{kind} registry key is malformed: {key!r}"]
    label = f"{kind}:{key[0]}/p{key[1]}/{key[2]}"
    if not isinstance(value, dict):
        return [f"{label} entry is not a dict"]
    for field in ("subject_key", "source_sha256", "reviewer", "date",
                  "citation"):
        item = value.get(field)
        if not isinstance(item, str) or not item:
            errors.append(f"{label} {field} is missing or empty")
    sha = value.get("source_sha256")
    if isinstance(sha, str) and (
            len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha)):
        errors.append(f"{label} source_sha256 is not a lowercase sha256")
    if kind == "resolution":
        four_way = value.get("four_way")
        if (not isinstance(four_way, dict)
                or set(four_way) != {"lattice", "audit", "emitted", "referee"}
                or not all(isinstance(item, int) and item >= 2
                           for item in four_way.values())
                or len(set(four_way.values())) != 1):
            errors.append(
                f"{label} four_way must be four equal counts >= 2")
        extra = set(value) - {
            "subject_key", "source_sha256", "four_way",
            "reviewer", "date", "citation"}
        if extra:
            errors.append(f"{label} carries unknown fields: {sorted(extra)}")
    else:
        if value.get("transition") not in PERMITTED_TRANSITIONS:
            errors.append(f"{label} transition is not permitted")
        criterion = value.get("suppression_criterion")
        if not isinstance(criterion, str) or not criterion:
            errors.append(f"{label} suppression_criterion is missing")
        extra = set(value) - {
            "subject_key", "source_sha256", "transition",
            "suppression_criterion", "reviewer", "date", "citation"}
        if extra:
            errors.append(f"{label} carries unknown fields: {sorted(extra)}")
    return errors


def registry_errors(
        resolutions: dict[Any, Any] | None = None,
        transitions: dict[Any, Any] | None = None,
        ) -> list[str]:
    """Every shape defect in both registries; empty registries are valid."""
    resolutions = (REVIEWED_LEDGER_RESOLUTIONS
                   if resolutions is None else resolutions)
    transitions = (REVIEWED_LEDGER_TRANSITIONS
                   if transitions is None else transitions)
    errors: list[str] = []
    for key, value in resolutions.items():
        errors.extend(_entry_errors(key, value, "resolution"))
    for key, value in transitions.items():
        errors.extend(_entry_errors(key, value, "transition"))
    overlap = set(resolutions) & set(transitions)
    if overlap:
        errors.append(
            "a subject may carry a resolution or a transition, never both: "
            f"{sorted(overlap)[:3]}")
    return errors


def self_test() -> int:
    """The registry validation, proven able to fail."""
    assert registry_errors({}, {}) == []
    good_resolution = {
        ("0605-1999", 1, "p1c66"): {
            "subject_key": "p1@1,2,3,4",
            "source_sha256": "0" * 64,
            "four_way": {"lattice": 3, "audit": 3,
                         "emitted": 3, "referee": 3},
            "reviewer": "self-test", "date": "2026-08-14",
            "citation": "self-test",
        },
    }
    good_transition = {
        ("0605-1999", 1, "p1c54"): {
            "subject_key": "p1@1,2,3,4",
            "source_sha256": "0" * 64,
            "transition": "active_composite",
            "suppression_criterion": "source-partition-edge-in-final-picture-v1",
            "reviewer": "self-test", "date": "2026-08-14",
            "citation": "self-test",
        },
    }
    assert registry_errors(good_resolution, good_transition) == []
    import copy

    def broken(kind: str, mutate) -> None:
        resolutions = copy.deepcopy(good_resolution)
        transitions = copy.deepcopy(good_transition)
        mutate(resolutions if kind == "resolution" else transitions)
        found = registry_errors(resolutions, transitions)
        assert found, f"{kind} forgery was accepted: {mutate.__doc__}"

    def no_reviewer(registry):
        """empty reviewer"""
        next(iter(registry.values()))["reviewer"] = ""
    broken("resolution", no_reviewer)
    broken("transition", no_reviewer)

    def bad_sha(registry):
        """uppercase sha"""
        next(iter(registry.values()))["source_sha256"] = "A" * 64
    broken("resolution", bad_sha)

    def unequal_four_way(registry):
        """four-way disagreement smuggled in"""
        next(iter(registry.values()))["four_way"]["referee"] = 4
    broken("resolution", unequal_four_way)

    def one_slot(registry):
        """a one-compartment comb is no comb"""
        next(iter(registry.values()))["four_way"] = {
            "lattice": 1, "audit": 1, "emitted": 1, "referee": 1}
    broken("resolution", one_slot)

    def unknown_transition(registry):
        """invented transition"""
        next(iter(registry.values()))["transition"] = "retired_quietly"
    broken("transition", unknown_transition)

    def extra_field(registry):
        """unknown field"""
        next(iter(registry.values()))["evil"] = True
    broken("resolution", extra_field)
    broken("transition", extra_field)

    def bad_key(registry):
        """page zero"""
        value = next(iter(registry.values()))
        registry.clear()
        registry[("0605-1999", 0, "p1c66")] = value
    broken("resolution", bad_key)

    both = registry_errors(good_resolution, {
        ("0605-1999", 1, "p1c66"): dict(
            next(iter(good_transition.values())))})
    assert any("never both" in error for error in both)
    print("review_registry self-test: pass")
    return 0


if __name__ == "__main__":
    raise SystemExit(self_test())
