# Staging a new revision or package

`rules/UPDATING.md:33-36` records the hazard: builders write directly into a
historical `rules/forms/...` directory and mutate `rules/index.json`, so
pointing one at the canonical corpus for a **new** release overwrites the
snapshot that documents the **old** one. Evidence for a superseded package is
not reproducible — the installed package it came from is gone — so an
overwrite is unrecoverable.

## The rule

**Writing into the canonical corpus is only defensible for idempotent
regeneration of the snapshot that is already there.** That is the case today:
`project-2550q` and `build-2550q-bindings` reproduce
`2550q-v2024-p7.9.6.0` byte-for-byte, verified by `jq -S -c` equality, so
re-running them changes nothing.

Anything describing new official behaviour goes to a staging root first.

## How

```sh
rtk cargo run --locked -p bir-rules-codegen -- project-2550q \
    --staging-root tmp/staging-<snapshot-id>
```

The staging root is repository-relative and resolved under the repository root;
absolute paths and `..` are rejected. Output mirrors the corpus layout beneath
it, so `tmp/staging-x/rules/ir/v2/<id>/rule-set.json` is directly comparable to
the tracked file.

**Staging refuses to overwrite.** If a target already exists the run fails with
the offending path and writes nothing further. A second run into the same root
is an error, not an update — delete the root or choose another. That is the
fail-if-target-exists guard `UPDATING.md` step 5 asks for.

## Reviewing a staged snapshot

```sh
diff <(jq -S -c . rules/ir/v2/<id>/rule-set.json) \
     <(jq -S -c . tmp/staging-<id>/rules/ir/v2/<id>/rule-set.json)
```

Compare semantically with `jq -S -c`, never byte-wise. A byte difference with
semantic equality is a formatting change; only a semantic difference means the
evidence moved.

Then, before promoting a staged snapshot into the corpus:

1. `validate-v1` — the eight corpus counts must not move unexpectedly.
2. `audit` — declared source hashes and the source-set digest must reconcile.
3. `roll-pin` — any change to a declared source is a 123-file / 246-site atomic
   transaction. Never roll by hand.
4. `status` — every production boundary must still hold.

## What is not covered

The 65 archaeology scripts in this directory have no staging support and cannot
run from a clone at all; see `README.md`. They read an extracted installer, an
`.hta`, `atcCodes.xml` or savefile XML, none of which are tracked. If one is
ever revived, it needs the same treatment: input root, staging output root,
snapshot ID, expected input hashes, and this guard.
