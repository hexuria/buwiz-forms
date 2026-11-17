# Updating the official eBIRForms rule corpus

The corpus is versioned as immutable official snapshots. A new Offline
eBIRForms release must add a snapshot; it must not rewrite the behavior against
which an old draft or compatibility test was created.

## Snapshot identity

Each executable snapshot is selected by all of:

- form code;
- printed form revision;
- official package/build version;
- normalized source-set SHA-256.

`form_code` or printed year alone is not a sufficient identity. If an official
package changes behavior without changing the printed form revision, use a
package-qualified rule-set ID such as `1701q-v2018-p7.9.7`. Existing directories
need not be renamed merely to adopt this convention; the v2 normalized manifest
will assign their unambiguous runtime IDs.

## Update procedure

1. Record the official package version and retrieval provenance.
2. Discover the current temporary extraction directory. Never assume a prior
   GUID path still exists.
3. Hash every relevant HTML, JavaScript, VBScript, PDF, guide, savefile, and
   observed runtime asset. Do not add wholesale third-party source merely for
   convenience.
4. Copy the nearest previous rule package to a **new** revision/package
   directory. Never edit the old snapshot to describe new official behavior.
5. Before reusing an existing builder, parameterize it with an input root,
   staging output root, snapshot ID, expected input hashes, and a
   fail-if-target-exists guard. Current builders commonly write directly to a
   historical `rules/forms/...` directory and mutate `rules/index.json`; do not
   point them at the canonical corpus for a new release until this is fixed.
6. Generate into staging with the applicable scripts under `rules/tools/`, then
   review the candidate against the nearest previous snapshot.
7. Re-run dummy-profile Save, reopen, Validate, and Final Copy observations for
   every changed or uncertain branch. Do not use online submission for
   discovery.
8. Preserve exact official behavior and message ordering. Record any safer
   recommendation independently; never replace the official branch silently.
9. Add concrete accepted, rejected, calculation-boundary, serialization, and
   workflow fixtures for every changed executable rule. Fixture evidence is a
   promotion contract, not an example set:

   - pin the legacy negative-fixture and calculation-fixture documents as
     `legacy-v1-negative-fixtures` and `legacy-v1-calculation-fixtures` sources
     with exact SHA-256 values and matching legacy `form_id`/schema version;
   - preserve the declared legacy counts exactly;
   - translate every negative source case through one canonical
     `#/cases/N` locator, exactly once, into an official evaluation fixture
     that isolates one violation and preserves the source rule ID, canonical
     phase, and exact selected/official message;
   - cover every legacy calculation `#/cases/N` locator with a fixture that
     expects an output from that calculation, with one legacy case per
     executable calculation;
   - for both profiles, make each fixture's expected rule list exactly match
     the executable rules for its phase, cover every executable rule, violate
     every issue-emitting rule at least once, and cover every expected/derived
     calculation output in reviewed order;
   - include a zero-violation fixture that exercises executable behavior for
     each profile, at least the declared number of official negative fixtures,
     and at least one independently evidenced filing-safe negative fixture.
10. Promote the reviewed snapshot additively, then update the form manifest,
    evidence, audit, gaps, and `rules/index.json`.
11. Run the corpus validator:

    ```powershell
    rtk powershell -NoProfile -ExecutionPolicy Bypass -File rules/validate.ps1 -RequireJsonSchema
    ```

12. Run the v2 audit and deterministic drift check:

    ```text
    npm run rules:check
    ```

    The compiler emits deterministic executable Rust for the audited safe
    subset; it does not package canonical JSON for runtime interpretation.
    Every reviewed snapshot must have independently reviewed, source-bound
    executable `evaluation_policy` branches. Generation must fail if a
    referenced field, dependency, state, rule, selected behavior profile,
    evaluation policy, or required fixture is unresolved.

    Generation also fails explicitly for executable nodes whose semantics are
    not yet safely bound: regex `matches`, decimal binary `divide` without
    expression-level division/rounding policy, and `set-derived`,
    `normalize-field`, or `set-workflow-state` effects. Do not change review
    state or encode metadata-only substitutes to bypass these failures.
13. Run both compatibility suites:

    - the previous compiled snapshot against its pinned fixtures;
    - the new snapshot against the new official evidence and differential
      observations.

14. Update the application selection table only after review. New drafts may
    select the newly supported snapshot; existing drafts retain their stored
    rule-set ID and digest until an explicit migration is accepted.

## Release invariants

- Research `status: complete` never enables application capabilities.
- Ambiguous, unverified, obsolete, or prose-only behavior is not executable.
- Both official-compatibility and filing-safe profiles compile and test
  independently.
- Official and filing-safe effect-evaluation policies are independently
  reviewed and never supplied by a runtime default.
- Submission always revalidates in `bir-core`; a GPUI report is never trusted as
  proof that a draft is still valid.
- A checked adapter manifest must bijectively cover evaluation values and XML.
  Until v2 serialized occurrences and repeated XML keys are preserved
  end-to-end, Phase 5 Final Copy/queue integration remains closed.
- Old snapshots and their fixtures remain testable for as long as the
  application can reopen drafts that reference them.
