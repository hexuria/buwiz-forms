# Per-form UI audit (read code; ground every claim with file:line)
Repo: /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/forms (branch forms/integration).
Page: crates/bir-desktop/src/views/form_<code lowercase>_view.rs (kit pages also use views/event_form_kit.rs; shared header in components/form_engine.rs).
Model: crates/bir-core/src/forms/form_<code lowercase>*.rs. Official field list: crates/bir-core/data/official-xml/<layout>.json (layout id from the form's QueueableForm LAYOUT_ID / official_xml).

Write ONE file per form: /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/audit/forms/<CODE>.json (valid JSON, UTF-8), exactly this shape:
{
 "code": "0619E", "title": "official form title", "page": "views/form_0619e_view.rs", "page_kind": "kit|handrolled|bespoke",
 "period": [ {"label": "Item 1 — For the month", "control": "button-row|select|combobox|text|date|segmented", "options": ["Jan","Feb"], "prefilled_from_library": true, "editable": true, "where": "file:line"} ],
 "choice_rows": [ {"label": "...", "options": ["Yes","No"], "control": "button-row|chips|radio|select|checkbox", "where": "file:line"} ],
 "atc": [ {"label": "...", "control": "chips|button-row|select|combobox|text|table", "multi": true, "official_slots": 0, "options_count": 0, "where": "file:line"} ],
 "validation": {"inline": false, "needs_review_panel": true, "how": "one line: where errors render", "where": "file:line"},
 "lists": [ {"label": "Schedule 1 — payees", "growable_officially": true, "add_remove_in_ui": false, "rows_in_ui": "fixed N | dynamic", "official_rows": 0, "where": "file:line"} ],
 "print_preview": {"slug": "...", "how": "launch_frozen_form_preview|custom|none", "where": "file:line"},
 "findings": [ {"id": "F1", "issue": "period-buttons|period-editable|atc|validation|lists|choice-buttons|other", "severity": "high|medium|low",
   "field": "exact label", "now": "one sentence, what the user sees", "why": "one sentence",
   "options": [ {"name": "Combobox", "detail": "one sentence"}, {"name": "Read-only chip", "detail": "..."} ], "where": "file:line"} ]
}
Rules:
- period: every month/quarter/year/"for the period" control. prefilled_from_library = the draft is created from the library tile with the period already set (check how the draft gets its period: new_from_profile(year, month/quarter) etc.). editable = the user can still change it on the page.
- choice_rows: every control rendered as a row of buttons/chips for picking ONE value (yes/no, categories, months...). Recommend combobox when > 4 options; keep segmented for 2-3.
- atc: how ATC(s) are picked. multi=true if the official form allows several ATCs (several ATC rows/slots in the official layout JSON or schedules). official_slots = number of ATC slots in the official layout. Recommend: multi → multi-select combobox; single → combobox.
- validation: are errors shown next to the field (inline) or only in a summary/"Needs Review" panel? Quote the render call.
- lists: any repeating group the official form lets grow (schedules, payee rows, ATC rows, tax-debit rows). Can the UI add/remove rows? How many rows does the official layout have (count numbered keys in the layout JSON)?
- findings: only real, grounded problems (with file:line). Severity: high = blocks filing or loses data; medium = confusing/wasteful; low = polish. Give 2-3 options each.
Keep labels exactly as the page shows them. No prose outside the JSON file. Reply to your caller with one line per form: CODE, number of findings, highest severity.
