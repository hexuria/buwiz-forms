# r1-2551q-schedule1-invalid run1

verdict: `known_gap`
known_gap: `hexuria/nsh-gpui-agent#13 sheet press`

- **exact_sentence**: `null` (untested; recipe stopped before typing `abc`)
- **row_exists_in_draft**: `False`
- **failure**: `error: `node 31078` is offscreen`
- **draft_row2**: `null`

## steps
- opened 2551Q 2021 Q1 through UI recipe steps s1-s20
- added a new Schedule 1 row (row 2)
- stopped at s21 `click fp-drpATC2-trigger` because gpui-agent reported the sheet node offscreen
- no fallback used; exact sentence and typed value untested
