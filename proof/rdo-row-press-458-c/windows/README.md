driver: gpui-agent-cli 0.4.1
recipes: agent-recipes/e2e/rdo-row-press.json (rdo-row-press.wants, profile-pick-press.wants)

Commits: R `a9bede3e844797436027612309aaaf7503dc03e4`; AFTER `f73376e43acea3cb04f81123d83d717882d1fd5e`. NSH pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.

No real mouse/keys, UI Automation, SendInput, or OS-input fallback was used. Video is observe-only. Each run used a fresh app and scratch database with the required dry-run/test safety environment. gpui-agent-cli 0.4.1 used the paced primary recipes; fallback/unpaced was not used.

## Results

| folder | setup-start | rdo-row-press | sign-up | profile-pick-press |
|---|---|---|---|---|
| after-1 | PASS | PASS | PASS | PASS |
| after-2 | PASS | PASS | PASS | PASS |

Both runs passed all recipes. `profile-pick-press` passed 182/182 receipt steps in each run, including the RDO and ZIP error-line Save rounds and ZIP row rounds for `1200` and `1224`.

Files: `after-1/` and `after-2/` contain `report.json`, `report.md`, `run.log`, receipts, screenshots and trees; `after.mp4` is the observe-only recording; recipe copies are beside this README.
