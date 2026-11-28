driver: gpui-agent-cli 0.4.1

#465 sheet press-through proof at buwiz-forms d910f47d8964d9350520a1c3bdf653dc4e964ce2.

Result: 10/10 rounds passed (run1 5/5, run2 5/5). In every round the ZIP sheet opened on the immediate first tap after the RDO pick; no known_gap occurred. The executable recipe is the JSON copy. The .wants copy is the readable companion; iOS keeps a closed sheet as a zero-height tree node, so the JSON uses a fixed gpui-agent wait after ZIP selection instead of an absence assertion.

Round screenshots are named `rN-press-zip-at-once.png` and `rN-zip-opened.png` under each run. No failing-round screenshot exists because no round failed.
