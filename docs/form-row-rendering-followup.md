# 1601-C radio-toggle rendering follow-up

2026-10-16, on codex/pr141-preview-integration.

## Cause and change

The form's rows were ordinary functions inside the application's one script
View. A Private/Government toggle called fill and then validation. Both starting
and finishing validation notified that View, reconstructing all visible rows and
briefly inserting/removing the Checking chip.

Each form item is now a retained GPUI Shell View. A dependency snapshot covers
its values, locks, options, validation text, picker/dropdown state, profile marker
and available width. Only affected items receive new props. Header/navigation
updates compare their own visible state. The busy chip appears only when a
validation lasts at least 150 ms; validation itself still starts immediately.

BirRoot owns a retained BirApp handle so child-row callbacks can explicitly
invalidate shared application chrome when needed. A no-op answer or unchanged
validation result no longer invalidates that chrome. Toast notifications use the
same handle because they can originate in a row callback.

The existing viewport poll refreshes row layout only when the available width
changes. Opening another return releases its old row views. InputState handles
continue to own the input value/focus machinery; elements are never cached or
reused across renders.

## Evidence

The command rtk proxy just shell-ui-test drives the actual radio click handlers,
renderer, FormPageActions and real 1601-C schema with a lightweight GPUI/host
test adapter. Before the change six toggles rebuilt 108 rows across nine distinct
items. Afterwards they rebuild six rows, all item 1:6. Additional assertions cover
unchanged answers, dependent values, validation messages, parent redraw,
resizing, folded sections and return reload. This check is now part of shell-check.

A separate native run used the compiled GPUI Fast host, the real Rust form
functions, a copy of the existing synthetic scratch fixture, and an open live
PDF preview. Six toggles were driven through the same FormPageActions.choose
path as the radio handlers, one second apart.

| Native counter | Before | After |
| --- | ---: | ---: |
| Application View rebuilds | 11 | 1 |
| Form row rebuilds | 99 | 7 |
| Distinct form rows rebuilt | 9 | 1 |

The first toggle removes a validation error, so the corrected run legitimately
updates the navigation badge and the category row twice. The remaining five
toggles rebuild only that row, with no application View rebuilds. These measure
script render scope, not GPU frame damage or a claim that only two pixels change.

Native logs, instrumentation restricted to copied app files, and comparison
JSON are under tmp/form-row-perf/. The owned test processes were stopped.
No debug instrumentation remains in application source.

The command just shell-check passed: the new JS regression, Rust formatting,
upstream and GPUI Fast compile checks, and strict clippy. JavaScript syntax and
diff whitespace checks also passed. All functional edits are JavaScript;
rerunning the existing integration binary loads them without a Rust rebuild.
