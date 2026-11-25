# Print audit of every official sample — main 75bd78b8

printaudit-scratch-test.rs.txt — scratch integration test (worktree-scratch only, never committed to a PR).
Built every sample draft with the suite's own builder functions (modules included via the same
mod sample_payloads tree), took each form's print field map (the same map the print preview consumes:
inherent to_print_field_map, or the QueueableForm trait's print_field_map), dumped to JSON.
Official values parsed tolerantly from the .official.xml pseudo-format (layout-independent), so
extended-layout samples are covered. Comparison exact and case-sensitive, normalized only for comb
spacing, thousands separators, and checkbox/radio truth marks. Build folder: ~/BuwizNsh/target-audit
(RUSTC_WRAPPER empty, toolchain cargo 1.100.0), used by nothing else; tell: item 14 of 1702MX prints
nothing at main — confirmed in the data.

per-key.tsv — one line per sample/key/cell with verdict. table.md — the per-form rollup, worst first.
diffs.json / held.json / notjoined.json — the detail behind the table.

Limits: a "same" verdict means the official value reaches the input of the right cell — NOT that it is
readable, uncut or inside the box on paper. Drafts that mutate the base builder inside the test body
(e.g. joint-many variants) are approximated by the base builder. 0619E/0619F are not covered (their
print map is private to the crate and they do not implement QueueableForm). Forms with no official
sample at all are not covered by construction. 2550M variant-to-stem pairing (072022/082022/092022)
is by test declaration order.
