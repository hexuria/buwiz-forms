# form-review/check/354 — what every file shows

Independent check of draft PR #354 (fix/tin-check-digit-at-creation @ 29654be0) vs main 92b3e201. Report-only: nothing fixed, nothing pushed to the PR.

- REPORT.md — the full audit: entry-path table, edge probes, live behavior, test counts, faults ranked.
- results.tsv — one line per case in points 1, 2 and 4 of the order.
- tin_edge_scratch.rs.txt — the uncommitted scratch probe used for point 2 (never part of the PR).
- refusal-create-desktop-narrow.png — New taxpayer at head with 000-123-454 (one digit wrong): the exact refusal sentence in red inside the TIN cells after Create; typed input kept. Shows the narrow/phone-width layout (bottom tab bar). Does NOT show desktop-wide layout.
- created-after-correction.png — after correcting the last digit to 3 and pressing Create: "Juana Santos added", TIN 000-123-453-00000.
- create-and-edit.mp4 — screen recording of the head flow (see REPORT.md for scope and limits).
- main-creates-wrong-tin.png — (if present) main build accepting 000-123-454, proving behavior difference.
- Binary provenance: every GUI/test binary was verified by strings(1) for the presence (head) or absence (main) of the refusal sentence before its evidence was trusted; see REPORT.md "Build-folder provenance".

What this proof does NOT show: desktop-wide (un-narrowed) layout screenshots; the official BIR program's own behavior (not in scope — the check digit port itself was validated in the edge probes); anything transmitted anywhere (dry-run only).
