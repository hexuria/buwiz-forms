# Pill radios ignore OS mouse clicks — main bcbaf324, Windows real window

Build under test: hexuria/buwiz-forms main @ bcbaf324187137010b19814d6ef6e59db4d1f5d9 (confirmed rev-parse, starts bcbaf324), own checkout `C:\Users\Administrator\370pills\buwiz-forms`, own build folder `C:\Users\Administrator\370pills\buwiz-forms\target\nsh-store` (CARGO_TARGET_DIR explicit, kit at pin 8c78cfd6 physically copied, RUSTC_WRAPPER empty). Real window, OS mouse clicks via user32 SetCursorPos+mouse_event, db read for ground truth.

## VERDICT: MAIN IGNORES PILL CLICKS — on every form tried. Keyboard works.

### 2551Q Q4 2026 (Amended Yes/No, Quarter)
- 10/10 clicks on Amended Yes/No FAILED to select: `is_amended` in the draft db stays `false` through five alternating Yes/No click rounds (m01 before, m02 after).
- Quarter pills (1st-4th): clicks dead too (visual, no selection change).
- CONTROL: clicks DO land in the same window — click into Number of Sheet/s + Ctrl+A + type 99 works (m05: box shows 99); sidebar/nav clicks navigate; card clicks open returns. So the window receives OS mouse input fine; the pill radios specifically do not respond.
- Click lands NOWHERE on a pill: no hover state (m14, mouse resting over Yes), no focus ring, no ripple, nothing.
- KEYBOARD: works. Tab reaches pill groups (focus ring visible); Left/Right arrows select — item 12 "Yes" selected by keyboard, pill filled and its conditional "If yes, specify" dropdown appeared (m10).

### 2550M Nov 2026 (Amended Yes/No)
- 4/4 clicks dead: `is_amended` stays false through two alternating Yes/No rounds (m17 open state).

### 1601FQ Q4 2026 (Amended Yes/No, Any Taxes Withheld Yes/No)
- clicks on both Yes pills: nothing selected visually (m22 before/after identical).

### Cross-build note
The SAME click method on the SAME machine takes 10/10 on PR #416's build (6c4ce772, based on an older main) — alternating Yes/No db-verified flips every round. So the deadness is specific to current main, not the rig, and the regression is recent.

### since-when
e5f7b42b (older main) result: see below when the build finishes.

## #416 case-(a) re-check (separate order) — gate CORRECT, original fault withdrawn
r16/r17: on PR #416's build (6c4ce772), a BRAND-NEW 2551Q Q3 2026 return (never opened before in that db): Item 16 shut with lock + "Item 16 applies only to an amended return." before anything is touched; click+type 42 takes nothing (db tax_paid_previous 0.0); fresh draft is_amended=false. The original case-(a) fault report was a rig artifact (reused draft whose Amended answer my own probing clicks had flipped); withdrawn.

## UPDATE — since-when: regression entered main between e5f7b42b and bcbaf324

| build | pill clicks | method |
|---|---|---|
| main bcbaf324 | **0/10 dead** (db-verified) | same |
| older main e5f7b42b | **10/10 taken** (db-verified alternating Yes/No flips, 5 rounds, 2551Q Q4; build folder `C:\Users\Administrator\370oldmain\buwiz-forms\target\nsh-store`) | same |
| #416 6c4ce772 (based on older main) | **10/10 taken** | same |

Same click method every row: python ctypes SetCursorPos + mouse_event down/up ~30 ms, positions from same-session screenshot ×1.4286, no pre-click dwell.

`git diff --stat e5f7b42b bcbaf324` (the regression window, 10 merges incl. #419 os_login, #414, #404, #363, #399, #366):
- nsh/screens/form: only FormPageNoRdo.tsx +26 (RDO box) — nothing pill-related; NO changes in nsh/ui/choice.tsx or any pill code
- BUT: nsh/src/door.rs +387, nsh/src/os_login*.rs +708 (#419 Windows Hello), nsh/src/lib.rs +34, nsh/src/agent.rs +13 — the input/window plumbing layer changed heavily
- crates/: 28 files, +4988/−320
- **kit pin IDENTICAL in both builds: 8c78cfd6** — so the RadioGroup pill control itself is byte-identical; the regression is in the repo side (door/input plumbing or window ownership are the suspect areas), not the kit
