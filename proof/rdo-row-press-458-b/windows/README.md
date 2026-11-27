driver: gpui-agent-cli 0.4.1
recipes: agent-recipes/e2e/rdo-row-press.json (rdo-row-press.wants, profile-pick-press.wants); smoke.json subset setup-start/sign-up/tax-year/open-2551q

Commits: R `6e73f1a90c75bd0b3bfb1a95aa3ff19d7008fa16` (recipes); AFTER `f73376e43acea3cb04f81123d83d717882d1fd5e` (PR #458 head). NSH pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.

No real mouse/keys, UI Automation, SendInput, or OS-input fallback was used. Videos are observe-only. Every launch used a fresh scratch database and the required dry-run/test safety environment. Paced primary recipes ran; unpaced fallback did not.

## Results

| folder | setup-start | rdo-row-press | sign-up | profile-pick-press |
|---|---|---|---|---|
| after-1 | PASS | PASS | PASS | FAIL |
| after-2 | PASS | PASS | PASS | FAIL |

Both AFTER runs failed `profile-pick-press` at `s90`: op `click`, target `tp-branch-save`, line `click tp-branch-save --screenshot`, error `occluded_by` (run 1 `13320`, run 2 `13672`). The box held `zzzz`; the failure tree/screenshots show `tp-branch-rdo_code-nothing` (`Nothing matches. Try a code or other words.`) overlapping/occluding Save. The RDO row/chevron/click-away rounds completed before this step; the ZIP rounds did not run.

## 2551Q smoke and SearchPicks

The corrected smoke subset passed in order: `setup-start`, `sign-up`, `tax-year`, `open-2551q` (4 pass). Queue was not pressed. The 2551Q form snapshot is `after-form/form-snapshot.json`. Analyzer outputs are `after-form/form-searchpicks.json` and `.md`: `searchpick_count: 0`. Therefore the small form SearchPick rounds recipe is `known_gap` and recorded in `after-form/form-searchpick-rounds.wants`; there was no true form SearchPick to drive.

## Files

- `after-1/`, `after-2/`: suite `report.json`, `report.md`, `run.log`, receipts, screenshots and failure trees.
- `after-form/`: smoke evidence, form snapshot, SearchPick analysis, and known-gap recipe note.
- `after.mp4`: observe-only recording.
- Copies of the relevant recipe files are beside this README.
