# Playbook: proof of anything on a screen

A claim about a screen (a page, a dialog, a print, a layout, a flow on a phone, tablet or desktop) is proven by driving the real app through gpui-agent. `docs/proof/E2E.md` is the full description of the runner.

## The proof ladder

1. **A gpui-agent recipe drives the real app.**
   ```sh
   just e2e check <suite>                       # no app: is every recipe sound?
   just e2e <target> <suite> [--record]          # target: macos, linux, windows, ios, android
   ```
   The runner launches a debug build in the dry-run test environment on a scratch database, finds each element in the app's own tree, and proves with a hit test that nothing covers it. Typing goes one key at a time (`type` with `key_delay_ms`). A new flow is a new recipe in `agent-recipes/e2e/`.
2. **gpui-agent cannot do a step.** Do not fall back to OS-level input: no real mouse, CGEvent, `adb shell input`, uiautomator, osascript, simctl taps, xdotool, UI Automation or a script of your own. Mark the step `known_gap` in the suite with the issue number, add your evidence to that issue, and send it to the `driver` role. The driver is patched, and the proof runs again.
3. **The app's own tree ends.** An OS file picker, a system permission sheet, a store's web page: only there may an OS-level tool act.

A fault seen only under injected input at machine speed, or only from a driver of your own, and contradicted by real use, is not a finding. Re-run it through `just e2e` before reporting it.

Queue, Send and Submit run only with `--allow-send`, in the dry-run test environment, against an app that reports its test database. Never in a release build.

## Pictures and a recording

Every UI task also returns:
- screenshots of each state it changed, before and after where there is a before;
- one screen recording of the flow from start to end, as a person would do it, under two minutes, `.mp4`. Phones: `--record`. Desktops: the machine's own recorder. A recorder is a camera, not a hand, so any recorder is fine.

Never record the maintainer's own screen: a system recording captures whatever else is on it, and the file goes into a repository.

## Print proofs

Render print proofs through the app's own print path (`staticize` in `crates/bir-app-api/src/static_html.rs`, then `print_sheet.rs`), never from the raw frozen HTML: the raw sheet is not what the filer prints. Use the demo taxpayer and invented values only.

## The acceptance rule for a proof branch

A proof is accepted only when its branch (a `try/` branch or the pull request's own) holds all of these:

1. The recipe files that drove the app, under `agent-recipes/`.
2. The `report.json` and `report.md` that `just e2e` or `gpui-agent-cli` wrote.
3. A README whose first line is `driver: gpui-agent-cli <version>`, which names the recipe, the app commit, and for phones the NSH commits and runtime hash; then one line per file saying what it shows and what it does not.
4. The pictures and the recording, under `proof/` or `evidence/`.

A README that mentions real mouse, real keys, CGEvent, Simulator input, `adb shell input`, uiautomator, osascript, simctl taps, xdotool, UI Automation or any tool of the machine's own is not accepted, whatever it shows.

The role that ordered the proof looks at the pictures and the recording itself before it accepts the work. A text report alone is not proof for UI work.
