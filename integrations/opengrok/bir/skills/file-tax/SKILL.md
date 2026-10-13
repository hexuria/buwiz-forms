---
name: file-tax
description: Prepare a draft BIR return (any of BIR's 43 forms, e.g. 2551Q, 1601C, 2550Q, 0619E, 1701) in the BIR desktop app with the user, using the bir-mcp tools. Use when the user types /file-tax <FORM> or asks to prepare, fill or draft a BIR return. Draft only - never submits, queues, files or pays; the user clicks Submit in BIR.
---

# /file-tax <FORM>

Prepare a **draft** of BIR form `<FORM>` (any of the 43 forms BIR has, for
example `2551Q` quarterly percentage tax, `1601C` monthly withholding on
compensation, `2550Q` quarterly VAT, `0619E` monthly expanded withholding, or
`1701` annual income tax) in the BIR desktop app, together with the user. The user watches the form in BIR while you work, and can type in
any box at any time.

The BIR app runs on **the user's own computer**, not on your box: the `bir_*`
tools reach it there. Say "on your computer" when you talk about it, never
"on my box".

Several BIR apps can be running at once (for example the user's real BIR
window and a demo on a test database). You work on exactly one, and **the
user picks it**.

You only have the `bir_*` tools from the `bir` MCP server. They are draft-only:
there is no way to submit, queue, file or pay, and you must never suggest that
you did. Filing is the user's own act, in BIR, after they have reviewed the
draft. When the draft validates, `bir_form_review` brings it up in BIR with
the Submit button highlighted; the user still clicks Submit themselves.

## Hard rules

1. **Never guess.** Fill a box only when its value is grounded in
   `bir_form_context` (profile, stored elections, past returns, uploaded
   documents) or the user told you in this chat. If you are unsure, leave it
   for the user.
2. **Never infer a tax election** (for example the 8% income tax option, or a
   tax-relief choice). Fill it only if `bir_form_context` shows it under
   `elections`; otherwise it is a needs-you box and the user decides.
3. **State the source honestly** on every `bir_form_fill`:
   `profile` (COR / profile facts), `past_return` (a previous return's value),
   `document` (uploaded document OCR), `ai` (anything you derived yourself,
   including values the user told you in chat that you then computed). Group
   fills by source; one call per source.
4. **Never overwrite the user.** BIR keeps boxes the user typed and lists them
   in `kept_user_boxes`. Do not try to fill them again or ask BIR to change
   them; if you think one is wrong, say so in chat and let the user decide.
5. **One form at a time.** If `bir_form_open` answers "form already open", do
   not retry. Ask the user whether to save that draft (`bir_form_save_draft`)
   or dismiss it (`bir_form_dismiss`), then open again.
6. **Never force a dismiss.** If `bir_form_dismiss` answers "unsaved edits",
   tell the user to press **Dismiss** in BIR (it asks them to confirm) or to
   let you save the draft first. Do not look for another way around it.
7. Never print or ask for passwords, tokens or the agent token. If an app's
   token is `missing`, tell the user to add it to their `BIR_MCP_TOKENS` file
   themselves; never ask them to paste it into the chat.
8. **The user picks the BIR app.** Never choose one yourself, not even when
   `bir_list_apps` shows only one. Never set `confirm_real` unless the user
   has just told you, in this chat, that they want to work on the app with
   the **real** taxpayer database.

## Flow

1. **List apps.** First call `bir_list_apps`. If it lists none, tell the user
   to open the BIR desktop app (agent enabled) and stop.
2. **Ask which app.** Show the user every listed app: its `id`, `mode`
   (desktop window or headless), `database` (`test` or `real`), its
   taxpayers, and whether the token is `known`. Ask which one to use, and
   wait for the answer, even when there is only one.
3. **Connect.** `bir_connect {id}` with the id the user chose, exactly as
   listed. If BIR answers that the app uses the REAL database, ask the user
   whether they really want to prepare a draft on real taxpayer data; only if
   they say yes, call `bir_connect {id, confirm_real: true}`. Each result
   after that names the app (`app.id`, `app.database`); the form data is
   under `result`. If a tool says no app is selected or the app is no longer
   running, go back to step 1.
4. **Taxpayer.** Call `bir_profiles_list` and ask the user which taxpayer this
   return is for (show registered name, TIN and branch). If there is exactly
   one profile, confirm it rather than assuming. Never invent a TIN.
5. **Period.** Ask for the taxable year and period: a month (1-12) for a
   monthly form (1601C, 0619E), a quarter (1-4) for a quarterly form (2551Q,
   2550Q, 1601EQ), nothing (0) for an annual return (1701, 1702RT), and the
   return number for an as-needed form (0605, 1707). `bir_dues_list` can
   suggest what is due; offer it, but let the user confirm.
6. **Open.** `bir_form_open {code, year, period, tin}` with the `tin` exactly
   as `bir_profiles_list` returned it. Handle "form already open" per rule 5.
7. **Learn.** Call `bir_form_context` and `bir_form_fields`. Use the official
   box keys from `bir_form_fields` (with their item numbers, labels and
   options); computed boxes and boxes owned by the profile or the period are
   not listed (BIR computes or keeps them). A radio item (Yes/No, Private or
   Government, ...) is one box: fill its key with one of its option values or
   labels, e.g. `{"frm0619E:optWithheld": "Yes"}`.
8. **Fill what is grounded.** For each source in the context, call
   `bir_form_fill {fields, source}` with only the boxes that source supports.
   Report what was filled, anything returned in `kept_user_boxes`, and any
   `not_shown` boxes (the form shows them only once an item such as Amended
   Return or Tax Relief is answered).
9. **Needs you.** Call `bir_form_needs_you`, or `bir_form_card` to show the
   open questions as a form in the chat (key, item, label, kind, options,
   value and BIR's error text per box). Ask the user for each box, or tell them
   they can type it directly in BIR. Send their answers from the card with
   `bir_form_fill {fields, source: "user", from_card: true}`; a value you
   worked out yourself goes with `source: "ai"`. If they say they typed it in
   BIR, do not fill it. Repeat until nothing is left or the user says to leave
   the rest.
10. **Validate.** Call `bir_form_validate`. For each entry in `field_errors`,
   explain in plain words what is wrong with that box and what would fix it.
   Fix only what is grounded (rule 1); otherwise ask the user. Validate again
   after fixes.
11. **Preview.** Offer `bir_form_preview`: it fills BIR's printed sheet and
   gives a local HTML path (in the desktop app it also opens in the browser).
   It is not a PDF and nothing is filed.
12. **Save, or hand over.** Call `bir_form_save_draft` and tell the user the
   draft is saved in BIR and what is still open. When `bir_form_validate` is
   clean and the user wants to file, call `bir_form_review`: BIR saves the
   draft, opens its page with Submit highlighted and "Prepared by the
   assistant. Review every box, then click Submit yourself." Tell the user to
   review every box and click Submit in BIR themselves. A headless BIR app
   refuses it; then the user opens the return in the desktop app. Never say
   the return was filed, and never offer to pay.

## Tool reference

| Tool | Use |
|---|---|
| `bir_list_apps` | Every running BIR app: `id`, `pid`, `mode`, `token` known/missing, and (token known) `database` test/real and `taxpayers`. Call first. |
| `bir_connect` | Select the app the user chose: `{id, confirm_real?}`. `confirm_real` only after the user confirmed real data. |
| `bir_status` | Which app is selected (id, mode, test/real) and whether it is reachable. |
| `bir_profiles_list` | Taxpayers saved in BIR. |
| `bir_dues_list` | Returns due (`filter`: upcoming/overdue/all; `scope`: profile/global). |
| `bir_form_open` | Open a draft of any of the 43 forms: `code`, `year`, `period` (month, quarter, 0 annual), `tin`. |
| `bir_form_fields` | Every fillable box: official key, item, label, kind, options, value, source. |
| `bir_form_context` | `{profile, elections, past_returns, documents}`. |
| `bir_form_fill` | `{fields: {key: value}, source, from_card?}` -> `{filled, kept_user_boxes, not_shown?}`. |
| `bir_form_needs_you` | Boxes still needing an answer: `{form, boxes: [...]}`. |
| `bir_form_card` | The needs-you and error boxes as chat form data; answers go back with `source: "user", from_card: true`. |
| `bir_form_validate` | The page's own Validate: `{ok, errors, field_errors: [{field, message}]}`. |
| `bir_form_preview` | Filled printed sheet as a local HTML `path` (not a PDF). Files nothing. |
| `bir_form_review` | Clean draft only: BIR opens its page with Submit highlighted for the user to click. Never files. |
| `bir_form_save_draft` | Save the draft. Never files. |
| `bir_form_dismiss` | Close without saving; refused with "unsaved edits". |
