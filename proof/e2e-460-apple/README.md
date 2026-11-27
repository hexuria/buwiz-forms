driver: gpui-agent-cli 0.4.1 (crates.io) — suite: smoke (agent-recipes/e2e/smoke.json)

# e2e-460-apple: the merged suite at PR #460 head 6b1097a0 on macOS and the iPhone Simulator

Head `6b1097a0c1099e4d12dce5ef5043d0d1e34a375a` of fix/e2e-merged-suite-results, confirmed with
`git rev-parse HEAD` (the job started at 2ce0c38c and moved with the branch; no money-boxes data
was produced at 2ce0c38c — the run was still in its cold build when the branch moved). Safety:
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, scratch database per run,
`--allow-send` inside the suite only. Stale-token container cleanup for iOS as before (#449). All
changes are in `agent-recipes/e2e/` only.

## The question that matters: does queue pass on macOS end to end?

**No — it now reaches the final wait and fails there, one step deeper than the Windows-proven path,
with a person-visible cause.** The recipe does everything validation asks: both profile prerequisites
through the banner Set buttons (business start date typed, election saved as 8%), then the form's own
to-dos (income type inline, telephone, email at `fp-txtEmail#occurrence-1`, tax relief No, Item 13
Not applicable), jumps to the top, presses Queue, the confirm dialog shows its two soft warnings and
Continue is pressed (s28 screenshot). Then validation refuses the queue with a toast a person would
also see: **"tax_paid_previous: Tax paid in a previously filed return must be zero unless Amended
Return is selected"** (`macos/07-queue/028-s28.png`), and `fp-send-line` never appears
(`macos/07-queue/failure.png`, tree: `fp-queue` still present, no send/cancel nodes).

The zeroing steps s12a/s12b in money-boxes-paced.json ran and reported `ok` — but both with
`delivered_unverified`, so on this adapter there is no evidence the zero actually landed in the data
model (the nodes themselves are culled from the tree at that point, so nothing reads back). Either
set_value's write does not reach validation's view of Items 15/16 on macOS, or validation disagrees
with a zero written this way. Windows proved the same zeroing works there — this is the one remaining
difference, and it needs either a data-plane read-back op or product knowledge of what validation
reads. Not fixable in recipe space with the ops that exist; documented here with exact evidence.

## What changed in the recipes (agent-recipes/e2e only)

1. `queue-desktop.json` (new, wired as the suite's `desktop` variant): the .wants format has no
   scroll verb (docs/proof/E2E.md documents this), and on macOS the income-type choices are occluded
   by the floating to-do pill without one (occluded_by evidence in the earlier iterations). JSON
   variant of the same flow plus `scroll form-scroll` after the attention jump and click-before-type
   for telNo/email (the strict adapter does not focus a box on bare `type`). This took macOS queue
   from the prerequisite banners all the way to the final send wait.
2. `queue.wants`: the same field-filling flow in wants syntax (8% election, Part I answers, fp-nav-0
   before Queue, fp-send-line wait) for the phone/linux paths; a scroll line briefly added here was
   removed again — .wants has no scroll op and the line broke the phone parse.

## Results

macOS: 7 pass (setup-start, sign-up, tax-year, navigate, open-2551q, money-boxes, birth-date),
queue fail at s29 as above, delete-row setup (no `fp-table-*-add` node at resolve time),
sign-out known gap (no lock gate under BIR_AUTH_BYPASS=1). See macos/report.md.

iOS: 5 pass (setup-start, sign-up, tax-year, navigate, open-2551q), money-boxes/birth-date/sign-out
known gaps (the documented phone gaps), queue fail at s5 — the branch editor's date box is occluded
on the phone (occluded_by 571, same gap-1 occlusion family as the sheets), delete-row setup as on
macOS. See ios/report.md.

One flake observed twice across the job's iterations: macOS sign-up s19 `nav-home` timeout in two
runs out of six; green in the final runs on both platforms.
