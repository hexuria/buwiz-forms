# A4 issue #528 UI proof report

Repo/worktree: `/Users/devin/jobs/a4screens/app`  
Branch intended: `try/a4-screens-reset`  
HEAD verified: `af85bd1964564693d1c96417892fab62baca8d89` (see `git-head.txt`)  
Final worktree status: clean (`git-status-final.txt` is empty)  
No commit or push was made. No files were deleted.

## Safety

Every launch used `just nsh-run-agent ... A4Proof.tsx` through `proof/run-a4.sh` with:

`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`

`BIR_SFTP_LIVE` and real relay/account/SFTP variables were unset. Databases were scratch files under `proof/scratch/`. Input was gpui-agent-cli 0.5.0 only; no OS input, simctl tap, osascript, or CGEvent was used.

## Important recipe adaptation/known gap

The supplied recipes type into `gate-code-boxes`, but in the real gpui window that test id is a generic OTP wrapper, not a text input. Direct execution failed with `node ... is not a text input`; the inner textbox has only generated ids (`n3`, then varying ids such as `n237`/`n261`) that are not stable across processes/flows.

Evidence captured here therefore uses two recipe layers:

1. **Desktop AFTER full sign-in proof**: a runtime copy of the supplied desktop recipe with the two generated textbox ids observed in that deterministic fresh run (`n3`, `n237`). This passed and is the run in the mp4.
2. **Apples-to-apples BEFORE/AFTER reset proof**: `a4-screens-reset-desktop-switch.wants`, which signs in Juana, leaves the same state, signs out, then uses the fixture's stable test-only `demo-switch-account` seam instead of retyping Pedro's OTP. This avoids measuring a gpui-agent/OTP targeting limitation rather than the screen-reset behavior. The final assertions are unchanged.

The adapted recipes and receipts are preserved. This deviation is a known gap against the exact supplied recipe text, although the proof still exercises a real account switch through the fixture and uses only gpui-agent input.

## Commands

Representative exact commands:

```sh
/Users/devin/jobs/a4screens/proof/run-a4.sh desktop-after3 macos \
  /Users/devin/jobs/a4screens/proof/scratch/after-desktop3.db 17521 a4mac

GPUI_AGENT_SESSION=a4mac GPUI_AGENT_TOKEN=a4proof-local-token \
/Users/devin/.cargo/bin/gpui-agent recipe run \
  /Users/devin/jobs/a4screens/proof/a4-screens-reset-desktop-runtime.wants \
  --screenshot-dir /Users/devin/jobs/a4screens/proof/desktop-after \
  --screenshot-flagged \
  --receipt-out /Users/devin/jobs/a4screens/proof/receipts/desktop-after-receipt.json
```

BEFORE overlay, exactly as requested:

```sh
cd /Users/devin/jobs/a4screens/app
git show 49bf61aa5b3ea240262d0ef90d2d40736d6cd6b0:nsh/shell/app.ts > /tmp/a4-before-app.ts
cp /tmp/a4-before-app.ts nsh/shell/app.ts
# build/run/capture
git checkout HEAD -- nsh/shell/app.ts
```

The overlay was applied only for BEFORE builds/runs and restored immediately afterward. Overlay/restore events are in `logs/before-overlay-state.txt`; final `git status --short` is empty.

Switch-proof commands were the same shape, with `a4-screens-reset-desktop-switch.wants` and screenshot dirs `desktop-before-switch` / `desktop-after-switch`.

## Results

### Desktop AFTER, full supplied-flow adaptation: PASS

Receipt: `receipts/desktop-after-receipt.json` — `ok=true`, 29 steps.  
Video: `evidence/af85bd19-macos-desktop-after-proof.mp4`  
Final screenshot: `evidence/af85bd19-macos-full-recipe-final.png`

The run signed in Juana, left form/Filings/profile state, signed out, signed in Pedro, captured the final frame, and verified absence of `form-page`, `filings-search`, and `tp-sec-taxpayer-edit`.

### Desktop BEFORE overlay, apples-to-apples switch proof: expected FAIL after screenshot

Receipt: `receipts/desktop-before-switch-receipt.json` — `ok=false` as designed. It captured the final frame at step 21, then failed at step 24 because `tp-sec-taxpayer-edit` still existed. Screenshots:

- `evidence/49bf61aa-macos-state-left.png` — Juana state left before sign-out/switch
- `evidence/49bf61aa-macos-after-switch-stale.png` — BEFORE behavior: profile/panel state persists after account switch

### Desktop AFTER, apples-to-apples switch proof: PASS

Receipt: `receipts/desktop-after-switch-receipt.json` — `ok=true`, 24 steps. Screenshots:

- `evidence/af85bd19-macos-state-left.png` — same Juana state before sign-out/switch
- `evidence/af85bd19-macos-after-switch-fresh.png` — AFTER behavior: fresh Home frame, no stale form/Filings/profile panel

## iPhone simulator result: CAPTURED AFTER RETRY

After the owner's compression pass, the iOS host build completed. One stale-token retry was needed; relaunching after the handoff token was already present fixed `invalid automation token`. No deletion was performed.

Phone recipe adaptation: the phone UI has no desktop `context-pill`/profile panel surface. `a4-screens-reset-phone-runtime.wants` therefore leaves form + Filings-search state only; final assertions cover `form-page` and `filings-search`. This adaptation is explicit in the recipe copy.

### iPhone BEFORE overlay: expected FAIL after screenshot

Receipt: `receipts/ios-before-runtime-receipt.json` — `ok=false` as designed. It captured the final frame at step 17, then failed because `filings-search` still existed.

- `evidence/49bf61aa-iphone-state-left.png`
- `evidence/49bf61aa-iphone-after-switch-stale.png` — BEFORE behavior: Filings state persists after account switch

### iPhone AFTER: PASS

Receipt: `receipts/ios-after-runtime-receipt.json` — `ok=true`.

- `evidence/af85bd19-iphone-state-left.png`
- `evidence/af85bd19-iphone-after-switch-fresh.png` — AFTER behavior: fresh Home frame after account switch

## File index

- `evidence/af85bd19-macos-desktop-after-proof.mp4` — one desktop AFTER mp4
- `evidence/49bf61aa-macos-state-left.png`
- `evidence/49bf61aa-macos-after-switch-stale.png`
- `evidence/af85bd19-macos-state-left.png`
- `evidence/af85bd19-macos-after-switch-fresh.png`
- `evidence/af85bd19-macos-full-recipe-final.png`
- `receipts/` — recipe receipts/stdout/stderr/exit codes
- `logs/` — launch logs, overlay notes, disk and worktree status
- `recipes/a4-screens-reset-desktop-runtime.wants` — generated-id adaptation of supplied desktop recipe
- `recipes/a4-screens-reset-desktop-before-runtime.wants` — attempted BEFORE generated-id adaptation
- `recipes/a4-screens-reset-desktop-switch.wants` — stable apples-to-apples desktop proof recipe
- `recipes/a4-screens-reset-phone-switch.wants` — stable apples-to-apples phone proof recipe

## Bottom line

Desktop evidence demonstrates the bug/fix clearly: BEFORE keeps the profile/panel state after account switch; AFTER returns to a fresh Home frame and passes all absence assertions. iPhone evidence was captured after the compression pass; BEFORE retains Filings state and AFTER resets to Home.
