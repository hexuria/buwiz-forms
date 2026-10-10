---
name: file-tax
description: Prepare a draft BIR return (2551Q or 1601C) in the BIR desktop app with the user, using the bir-mcp tools. Use when the user types /file-tax <FORM> or asks to prepare, fill or draft a 2551Q or 1601C. Draft only - never submits, queues, files or pays.
---

# /file-tax <FORM>

Prepare a **draft** of BIR form `<FORM>` (`2551Q` quarterly percentage tax, or
`1601C` monthly withholding on compensation) in the BIR desktop app, together
with the user. The user watches the form in BIR while you work, and can type in
any box at any time.

You only have the `bir_*` tools from the `bir` MCP server. They are draft-only:
there is no way to submit, queue, file or pay, and you must never suggest that
you did. Filing is the user's own act, in BIR, after they have reviewed the
draft.

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
7. Never print or ask for passwords, tokens or the agent token.

## Flow

1. **Connect.** Call `bir_status`. If BIR is not reachable, tell the user to
   open the BIR desktop app (agent enabled) and stop.
2. **Taxpayer.** Call `bir_profiles_list` and ask the user which taxpayer this
   return is for (show registered name, TIN and branch). If there is exactly
   one profile, confirm it rather than assuming. Never invent a TIN.
3. **Period.** Ask for the taxable year and period: a quarter (1-4) for 2551Q,
   a month (1-12) for 1601C. `bir_dues_list` can suggest what is due; offer
   it, but let the user confirm.
4. **Open.** `bir_form_open {code, year, period, tin}` with the `tin` exactly
   as `bir_profiles_list` returned it. Handle "form already open" per rule 5.
5. **Learn.** Call `bir_form_context` and `bir_form_fields`. Use the field keys
   from `bir_form_fields`; skip computed boxes (BIR computes them).
6. **Fill what is grounded.** For each source in the context, call
   `bir_form_fill {fields, source}` with only the boxes that source supports.
   Report what was filled and anything returned in `kept_user_boxes`.
7. **Needs you.** Call `bir_form_needs_you`. For each box, ask the user in chat
   (show the box label, and why you could not fill it), or tell them they can
   type it directly in BIR. When they answer in chat, fill it with
   `source: "ai"` only if they gave you the value; if they say they typed it in
   BIR, do not fill it. Repeat `bir_form_needs_you` until it is empty or the
   user says to leave the rest.
8. **Validate.** Call `bir_form_validate`. For each entry in `field_errors`,
   explain in plain words what is wrong with that box and what would fix it.
   Fix only what is grounded (rule 1); otherwise ask the user. Validate again
   after fixes.
9. **Save.** Call `bir_form_save_draft` and tell the user the draft is saved in
   BIR, what is still open (needs-you boxes or field errors), and that
   reviewing and filing happen in BIR by them. Do not offer to submit or pay.

## Tool reference

| Tool | Use |
|---|---|
| `bir_status` | Is BIR running; which instance (desktop or headless). |
| `bir_profiles_list` | Taxpayers saved in BIR. |
| `bir_dues_list` | Returns due (`filter`: upcoming/overdue/all; `scope`: profile/global). |
| `bir_form_open` | Open a draft: `code` 2551Q/1601C, `year`, `period`, `tin`. |
| `bir_form_fields` | Every box: key, label, value, computed, source. |
| `bir_form_context` | `{profile, elections, past_returns, documents}`. |
| `bir_form_fill` | `{fields: {key: value}, source}` -> `{filled, kept_user_boxes}`. |
| `bir_form_needs_you` | Required boxes still empty: `{form, boxes: [{field, label}]}`. |
| `bir_form_validate` | `{ok, errors, field_errors: [{field, message}]}`. |
| `bir_form_save_draft` | Save the draft. Never files. |
| `bir_form_dismiss` | Close without saving; refused with "unsaved edits". |
