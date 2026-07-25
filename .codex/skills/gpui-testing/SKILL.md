---
name: gpui-testing
description: Plan, implement, review, and debug tests for Rust applications built with GPUI. Use for #[gpui::test], TestAppContext, VisualTestContext, test-support, synthetic keyboard or mouse input, action and keybinding tests, deterministic async UI tests, fake services, flaky GPUI tests, native desktop smoke tests, or deciding between unit, integration, and process-level end-to-end coverage.
---

# GPUI Testing

Use this skill to create reliable, deterministic, version-correct tests for GPUI applications.

## Primary objective

Choose the smallest test layer that proves the behavior, then implement it using APIs that match the repository's exact GPUI revision.

A successful result should:

- exercise meaningful user or application behavior
- avoid real-time sleeps and unnecessary external services
- verify observable state and important side effects
- compile and run against the project's pinned GPUI version
- state clearly what remains outside the selected test layer

## Critical rule: resolve the exact GPUI source first

GPUI is pre-1.0 and its testing APIs can change. Never assume that an example from the latest documentation matches the repository.

When repository access is available, inspect in this order:

1. `Cargo.toml` and workspace dependency declarations
2. `Cargo.lock`
3. `[patch]`, Git revision, branch, or workspace overrides
4. existing GPUI tests in the repository
5. the locally downloaded GPUI source for that exact dependency
6. official GPUI or Zed source matching the revision

Useful commands:

```bash
cargo tree -i gpui
cargo metadata --format-version 1
rg -n 'gpui|test-support' -g 'Cargo.toml' .
rg -n 'gpui::test|TestAppContext|VisualTestContext' .
```

When the local dependency source is available, confirm any method before using it:

```bash
rg -n 'struct TestAppContext|struct VisualTestContext' "${CARGO_HOME:-$HOME/.cargo}/registry/src" "${CARGO_HOME:-$HOME/.cargo}/git/checkouts"
rg -n 'fn add_window_view|fn simulate_input|fn dispatch_action|fn condition|fn run_until_parked' \
  "${CARGO_HOME:-$HOME/.cargo}/registry/src" "${CARGO_HOME:-$HOME/.cargo}/git/checkouts"
```

Do not ask the user for the GPUI version when it can be determined from the repository.

When no repository is available, provide a version-neutral template, label assumptions, and tell the user which method signatures must be checked against their pinned dependency.

## Choose the test layer

Use this decision table before writing code.

| Layer | Use it for | Typical mechanism | Do not use it to prove |
|---|---|---|---|
| Unit test | reducers, parsing, validation, domain state, view-independent logic | ordinary Rust `#[test]` | focus, event routing, keymaps, rendering lifecycle |
| GPUI integration test | actions, focus, keyboard input, mouse input, entity updates, async UI flows, fake service integration | `#[gpui::test]`, `TestAppContext`, `VisualTestContext` | real GPU, compositor, accessibility bridge, native dialogs, packaging |
| Process-level E2E smoke test | startup, persistence across restart, native platform integration, packaging, crash behavior | launch the built binary and drive it through OS-level automation | broad business-logic coverage that is cheaper in-process |

Default to GPUI integration tests for application UI behavior. Reserve process-level E2E for a small set of critical smoke journeys.

Do not describe an in-process `TestAppContext` test as fully end-to-end. Call it a GPUI integration test or headless UI integration test.

## Repository inspection

Before implementing tests, locate:

- the GPUI dependency and feature configuration
- the application bootstrap and root view constructor
- actions and keybindings
- focusable views and input handlers
- service, filesystem, clock, network, and persistence boundaries
- existing test utilities and fixture conventions
- CI commands and supported platforms

Prefer the repository's existing test style over introducing a parallel framework.

## Make the app testable

Keep `main.rs` thin. Move reusable startup logic into library code so production and tests can construct the same root application state.

Prefer a shape like:

```text
src/
  lib.rs              application bootstrap and exported root constructor
  main.rs             Application::run wrapper only
  services.rs         injectable external boundaries
  testing.rs          optional test fixtures and fakes

tests/
  ui_flows.rs         GPUI integration tests
  native_smoke.rs     optional process-level smoke tests
```

Use dependency injection for external behavior. Good seams include:

- service traits
- filesystem abstractions or temporary directories
- deterministic clocks
- in-memory persistence
- fake HTTP clients
- explicit application configuration

Do not make real network calls from ordinary GPUI integration tests unless the user explicitly requests a live-system test.

Do not expose broad mutable internals only for tests. Prefer narrow observable queries, recorded fake calls, emitted events, or test-only semantic helpers.

## Configure test support carefully

Confirm whether the pinned GPUI revision exposes testing APIs behind a feature such as `test-support`.

Prefer workspace-consistent configuration. For example, only when supported by the pinned version:

```toml
[dependencies]
gpui = { workspace = true }

[dev-dependencies]
gpui = { workspace = true, features = ["test-support"] }
```

Feature resolution and workspace layout can differ. Inspect the resolved dependency before editing manifests.

Do not add a test feature to the application's production default feature set unless required.

## Core GPUI integration workflow

Implement tests in this order:

1. Construct deterministic fakes and fixtures.
2. Create the root view through the same bootstrap path used by production.
3. Establish focus when keyboard or text input depends on it.
4. Drive logical actions for behavior tests.
5. Drive keystrokes separately when testing keybinding registration.
6. Use text input or mouse events only when those input paths matter.
7. Synchronize with GPUI's executor or entity notifications.
8. Assert user-observable state.
9. Assert important external side effects through fakes.
10. Run the narrow test, then the relevant test suite.

A representative shape is:

```rust
#[gpui::test]
async fn creates_project_from_the_ui(cx: &mut gpui::TestAppContext) {
    let projects = FakeProjects::default();

    let (root, cx) = cx.add_window_view({
        let projects = projects.clone();
        move |window, cx| AppView::new(projects, window, cx)
    });

    // Only when the target implements Focusable and the flow requires focus.
    cx.focus(&root);

    // Prefer logical actions for the main behavior test.
    cx.dispatch_action(OpenCreateProject);
    cx.simulate_input("buwiz");
    cx.dispatch_action(ConfirmCreateProject);

    cx.condition(&root, |view, _cx| view.has_project("buwiz"))
        .await;

    assert_eq!(projects.created_names(), ["buwiz"]);
}
```

Treat this as a structural pattern. Adapt imports, constructor signatures, entity access, and action types to the pinned GPUI revision and the application.

## Separate behavior tests from keybinding tests

Most flow tests should dispatch actions directly. This verifies application behavior without coupling every test to platform key syntax.

Add a smaller set of keybinding tests that:

- focus the correct view
- simulate the configured keystroke
- assert that the expected action effect occurred

This separation makes failures easier to diagnose:

- action test fails: application behavior or handler problem
- keybinding test fails: keymap, focus, context, or platform mapping problem

## Keyboard and text input

Before simulating input, verify:

- the intended entity or input handler is focused
- the required key context is active
- actions and keybindings were registered during test bootstrap
- the window has completed its initial update cycle

Use text input for actual text-entry paths. Use keystroke simulation for navigation, shortcuts, command sequences, and editing commands.

Do not type text through a sequence of physical key events when the behavior under test is only logical action handling.

## Mouse tests

Use mouse simulation when pointer routing, hit testing, hover, drag, or click behavior is itself important.

Avoid unexplained hardcoded coordinates. Prefer, in order:

1. semantic element selectors or debug bounds supported by the pinned GPUI version
2. bounds captured by the view during layout or paint
3. a narrow test-only helper returning a target's bounds or center
4. fixed coordinates only in a fixed-size fixture with an explanation

Run the necessary render or update cycle before querying bounds or sending pointer events.

Do not use mouse tests to cover flows that can be tested more robustly through actions.

## Deterministic async testing

Use GPUI's test executor and notification mechanisms. Prefer:

- methods that automatically drain scheduled work
- `run_until_parked` when all currently runnable work should complete
- an entity condition or notification when waiting for a specific state transition
- executor-provided timers or a fake clock when testing time-dependent behavior

Avoid:

```rust
std::thread::sleep(...);
smol::Timer::after(...).await; // when it is not driven by the test executor
poll_until_true_with_real_time(...);
```

A sleep is not synchronization. It makes tests slower and still allows races.

If a test hangs, inspect whether application work was spawned on:

- a runtime outside GPUI's test executor
- a real timer
- a detached task whose completion is never observed
- a channel or service fake that never responds

## Concurrency and flake testing

When supported by the pinned macro, use deterministic seeds or repeated iterations to explore task interleavings:

```rust
#[gpui::test(seeds(1, 7, 42, 99))]
async fn preserves_state_across_completion_orders(
    cx: &mut gpui::TestAppContext,
) {
    // ...
}
```

```rust
#[gpui::test(iterations = 50)]
async fn does_not_lose_concurrent_updates(
    cx: &mut gpui::TestAppContext,
) {
    // ...
}
```

Useful diagnostic commands, when supported:

```bash
SEED=42 cargo test test_name -- --nocapture
ITERATIONS=100 cargo test test_name -- --nocapture
```

Record the failing seed in the failure output or report.

Do not use retries to hide a flaky test. Retries are acceptable only as a temporary diagnostic aid while the root cause is being fixed.

## Assertions

Prefer assertions at two boundaries:

1. observable application state or emitted UI event
2. recorded external side effect

Examples:

- dialog opened and the expected action was dispatched
- status changed to saved and the fake repository recorded the correct payload
- validation message appeared and no backend request was made
- loading state transitioned to success after the fake completed
- window title changed after opening a document

Avoid assertions that only prove a private helper was called.

## Process-level E2E smoke tests

Use process-level E2E only when the behavior crosses the in-process test boundary.

A good native smoke harness should:

- build or locate the exact application binary
- launch it with an isolated temporary home, config, and data directory
- use deterministic fixtures and test-mode service endpoints
- wait for readiness without an arbitrary sleep
- drive the app through platform accessibility or other OS-level automation
- capture stdout, stderr, exit status, screenshots, and crash artifacts on failure
- terminate and clean up the process reliably

Good smoke journeys include:

- application launches and opens its main window
- create, save, restart, and verify persistence
- open a real file through supported native integration
- clipboard, drag and drop, or accessibility bridge behavior
- packaged binary contains required assets

Keep this suite small. Do not duplicate all GPUI integration coverage at the process level.

## Common failure modes

### Testing APIs are unavailable

Check the resolved GPUI revision and feature configuration. Do not assume the latest crate's feature names apply.

### Simulated input does nothing

Check focus, key context, input handler registration, and whether bootstrap registered actions and keybindings.

### Action dispatch does nothing

Confirm the action handler is attached to the focused dispatch path and that the correct window is being used.

### Mouse click misses

Ensure the view rendered, query stable bounds, set an explicit test window size when layout matters, and avoid stale coordinates.

### Async test hangs

Look for external runtimes, real timers, detached tasks, unfulfilled fake responses, or a condition that is never notified.

### Test flakes only in CI

Check global mutable state, shared fixture paths, real time, unordered background completion, platform-specific key syntax, and tests that depend on execution order.

### Test passes but proves too little

Add an assertion on user-visible state and a separate assertion on the fake service or persistence boundary.

## Verification workflow

When code access and execution are available:

1. format changed Rust files
2. compile the narrow test target
3. run the new test alone
4. rerun with a fixed seed if concurrency is involved
5. run the containing crate's test suite
6. run Clippy or repository-specific checks when practical

Typical commands:

```bash
cargo fmt --all -- --check
cargo test -p <crate> <test_name> -- --nocapture
cargo test -p <crate>
cargo nextest run -p <crate>
cargo clippy -p <crate> --tests -- -D warnings
```

Use the repository's documented commands when they differ.

Never claim a code example compiles unless it was compiled or its exact API signatures were verified against the pinned source. State any unverified assumption explicitly.

## Output contract

When applying this skill, return:

1. **Selected layer**: unit, GPUI integration, or process-level E2E, with a brief reason
2. **Testability changes**: any bootstrap or dependency seams introduced
3. **Coverage added**: behaviors and failure paths tested
4. **Verification**: exact commands run and results
5. **Boundary**: what the test intentionally does not prove
6. **Flake diagnostics**: failing seed or remaining nondeterminism, when relevant

For implementation requests, prefer concrete patches over general advice.

For design-only requests, provide a test matrix with the smallest useful set of cases.

## Quality checklist

Before finishing, confirm:

- the exact GPUI revision was resolved
- the selected layer matches the claim being tested
- production startup is reused rather than reimplemented in the test
- external dependencies are deterministic
- focus and key context are explicit where needed
- behavior tests and keybinding tests are separated
- no arbitrary sleep is used
- mouse targets are stable and semantic where possible
- assertions cover observable state and side effects
- targeted tests were run, or unverified assumptions were disclosed
- process-level gaps are stated clearly

## Additional recipes

Read `references/recipes.md` for reusable patterns covering action tests, keybindings, async fakes, mouse targets, multi-context tests, and native smoke harnesses.
