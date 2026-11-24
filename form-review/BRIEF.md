# Form layout review: brief for the four agents

You are one of four agents (neon-1 to neon-4) fixing how tax forms look in the desktop app in hexuria/buwiz-forms. An orchestrator (me) collects your work and shows it to the owner, who passes or fails each form from before-and-after pictures. If your machine is still running its setup, finish that first, then start this.

YOUR FORMS
- neon-1: 1700, 1701, 1701A, 1701MS (the annual income tax returns for individuals)
- neon-2: 2551Q, 2550Q, 2000, 2000OT (percentage tax, VAT and documentary stamp tax)
- neon-3: 1706, 1707, 1707A, 1606 (capital gains and the withholding on property transfers)
- neon-4: 1800, 1801, 1600WP, 1602Q, 2552 (donor's and estate tax, two withholding returns and the stock transaction tax)
The other three agents have the other forms. Touch only files that belong to your forms (see SHARED FILES).

THE GOAL, IN THE OWNER'S WORDS
- The app marked 17 forms "Draft layout" from an automatic score. The owner does not trust the score and is removing the label, so the forms themselves must be made right.
- "No text should overlap, no text should exceed its border. We need to tweak the line height, thickness or whatever option is available. Usually the issue is the text, and the placement of boxes."
- "The best way to trust it is to compare with the real form."
- "Our forms are dynamic: certain pages can grow in different sections." A fix must hold when a schedule has more rows than the paper shows.
- It will not match the paper 100%: some forms were moved from the 9-digit TIN to the 14-digit one, so TIN boxes differ from an old official sheet on purpose.
How to improve each form is yours to work out. What follows is the map, the rules and what to hand back.

WHERE A FORM LIVES (two surfaces; check both)
1. The form page: the screen where a return is filled in. Layout data: forms/forms-schema.json (one entry per form: parts, rows, labels; `q` is the distrusted score), forms/schema-overrides.json (reviewed corrections per form code; the right place for most screen fixes), forms/screen-1700-v2018.json (1700 only), forms/conditions.json, forms/condition-labels.json, forms/radio-name-groups.json. Built into screens by crates/bir-app-api/src/screen_form_module.rs and drawn by nsh/screens/form/.
2. The printed sheet and its preview: html-frozen/<folder>/ per form (index.html, form.css, writer-cells.json, provenance.json, guide.html), shared html-frozen/base.css and fonts, filled by crates/bir-print (frozen_html.rs; text fitting has golden files in crates/bir-print/src/fixtures), cell maps in forms/print-cells.json, shown by nsh/screens/print/. Folder names are in docs/FORMS.md ("Notes on printed sheets") and html-frozen/inventory.json; provenance.json names the official PDF each sheet was made from and its growable sections.
Read first: docs/FORMS.md, html-frozen/README.md, nsh/AGENTS.md, docs/agent.md (driving the running app and taking screenshots with the gpui-agent command line), tools/form_findings.py (an older automatic findings tool; useful ideas, not the judge).
The real form: the official BIR PDF named in provenance.json. The sheets were generated from those PDFs in the repository hexuria/buwiz-forms-html (tools/formgen, forms/, forms-corrected/); use it if you can read it. Otherwise get the same form revision from the BIR's public website. Render it to images at the same scale as your screenshots so they can be laid side by side.

WHAT TO DO, PER FORM
A. BEFORE. From a clean checkout of main, with the demo taxpayer and a return filled so that every box has a realistic value, including the worst cases: a long registered name and address, large amounts (hundreds of millions with centavos), every schedule row filled, and one growable section pushed past the rows printed on paper. Capture: every section of the form page, and every page of the print preview. Use the running app (`just nsh-run-agent ...` with a scratch database, driven through gpui-agent) so the pictures are what a user sees; say so if you had to render the sheet another way.
B. COMPARE with the official PDF page by page and list every defect: text over text, text crossing or leaving its box, clipped text, wrong line height or weight, boxes in the wrong place or of the wrong size, a value in the wrong box, a box the paper has and we lack (or the reverse), labels that differ from the paper, a page that breaks badly when a section grows. Skip differences that come from the 14-digit TIN.
C. VERDICT for the form: "tweak" (fixable in its own files), "rebuild part" (name the part), or "remake" (say why tweaking cannot work). For "remake", stop and report; the orchestrator decides.
D. FIX what a tweak or a part rebuild covers. Prefer the real cause over a one-off nudge (for example a fitting rule or a cell size, not a hand-shrunk font on one value). Keep every existing test passing; update a golden file only when the new output is the correct one, and say which.
E. AFTER. The same pictures, same data, same sizes, same file names as BEFORE.

SHARED FILES: ASK FIRST
Four agents work at once. You may edit: your forms' entries in forms/schema-overrides.json and forms/print-cells.json (keep the edit inside your form's block so the four branches merge), and your forms' html-frozen/<folder>/ files. Do NOT edit without my yes: html-frozen/base.css and fonts, crates/ (any Rust, including the fitting code), nsh/ (any screen code), forms/forms-schema.json outside your forms' entries, tools/. If the right fix is in one of those, describe it (file, change, which forms it helps) in your report and wait; I will have one agent make it for everyone.
Also know: CI checks html-frozen/ byte for byte against the generator repository hexuria/buwiz-forms-html (tag v1-frozen), where layout corrections officially live (forms-corrected/). Make and prove your sheet fix in html-frozen/ here; if you can read that repository, also say exactly what must change there. Landing it is my job.
Do not touch anything about sending returns, sign-in or versions; another team is changing crates/bir-core and the queue right now.

RULES FOR EVERY RUN
- Set BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1. Never set BIR_SFTP_LIVE. Nothing is ever sent to the BIR. Do not press Queue.
- Run the app only through the `nsh-*` recipes in the justfile with a fresh scratch database. Never `just run`.
- Only the demo taxpayer (Juana Santos, TIN 000-123-453) and invented values. No real names, TINs or credentials.
- Never print, copy or upload a key, token, certificate or .env file.
- No pull requests, no merges, no pushes to main or to anyone else's branch.

WHAT TO HAND BACK
Work on a branch named try/forms-neon-<your number>, made from origin/main, and push it to hexuria/buwiz-forms (a try/ branch runs no CI). If plain `git push` is refused by your machine's git proxy, create the commits through the GitHub git-data API with the token your machine already has; say which way worked. One commit per form for the fix, so a form can be taken or redone alone.
In the same branch, under form-review/<CODE>/:
- before/ and after/ with matching names: screen-01-<section>.png, screen-02-..., print-p1.png, print-p2.png, plus official-p1.png ... once. PNG, about 1400 px wide, no real data.
- report.json: {"code", "verdict": "tweak|rebuild part|remake", "summary": one plain sentence, "defects": [{"id", "surface": "screen|print", "where", "what", "before", "after", "status": "fixed|open|needs shared change|by design"}], "files_changed": [], "shared_changes_wanted": [], "tests_run": [], "open_questions": []}
Order: do BEFORE and the defect list for ALL your forms first and push that (so the owner sees the starting point early), then fix form by form, pushing after each one.
Report to me in this chat: (1) within your first reply, your system (OS, and whether the app window and print preview run on it), and whether you can read hexuria/buwiz-forms-html; (2) when the BEFORE set is pushed, the commit id and a one-line verdict per form; (3) after each fixed form, the commit id and what is still open. Ask when something here is unclear; do not guess about shared files.
