# 1702MX official-samples print audit — 3f37b225 vs main 92b3e201

mxaudit-scratch-test.rs.txt — the scratch test (never committed to the PR; compiled in each
worktree's own build folder: target-p5 for the head, target-p4main for main). It builds the three
official sample drafts exactly as sample_payloads_test does (the builder code is copied verbatim),
takes Form1702MXDraft::to_print_field_map(), renders with bir-print's own filled_document, and dumps
one line per writer-cells.json join of every type (joins, xbox_joins, peso_joins, date_joins):
stem, key, cell, official value (.official.xml parsed with official_xml::read), print-map value,
rendered cell text.

audit-head-3f37b225.tsv / audit-main-92b3e201.tsv — the dumps.
1702MX-1225-rendered.pdf (+ page1 png) — the head render of the calendar sample, rasterized with
headless chrome, so Part II 14-21 and Schedule 2 can be looked at with real numbers.

What it does not show: gui interaction; centavo formatting (1702MX is whole-peso); the derived
comb keys (tin digits etc.) which have no official-xml counterpart by design.
