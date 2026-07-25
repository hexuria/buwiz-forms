# GPUI Testing Recipes

These recipes are structural examples. Verify every GPUI method and signature against the repository's pinned dependency before applying them.

## 1. Thin production bootstrap

Keep application construction reusable and keep process startup in `main.rs`.

```rust
// src/lib.rs
pub struct AppServices {
    pub projects: std::sync::Arc<dyn ProjectRepository>,
}

pub fn build_root(
    services: AppServices,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<AppView>,
) -> AppView {
    AppView::new(services, window, cx)
}
```

```rust
// src/main.rs
fn main() {
    gpui::Application::new().run(|cx| {
        let services = production_services();
        open_main_window(services, cx);
    });
}
```

The integration test should call the same `build_root` or application bootstrap path.

## 2. Fake service with observable calls

Use a fake that can be controlled by the test and inspected afterward.

```rust
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct FakeProjects {
    inner: Arc<Mutex<FakeProjectsState>>,
}

#[derive(Default)]
struct FakeProjectsState {
    created: Vec<String>,
    result: Option<anyhow::Result<Project>>,
}

impl FakeProjects {
    pub fn succeed_with(&self, project: Project) {
        self.inner.lock().unwrap().result = Some(Ok(project));
    }

    pub fn created_names(&self) -> Vec<String> {
        self.inner.lock().unwrap().created.clone()
    }
}
```

The actual async return type should match the project's service trait and GPUI version.

Avoid fakes that immediately mutate the view behind the test's back. Let the application observe a normal service result.

## 3. Action-driven behavior test

Use actions to verify application behavior without coupling the test to a keymap.

```rust
#[gpui::test]
async fn creates_a_project(cx: &mut gpui::TestAppContext) {
    let projects = FakeProjects::default();
    projects.succeed_with(Project::named("buwiz"));

    let (root, cx) = cx.add_window_view({
        let projects = projects.clone();
        move |window, cx| AppView::new(projects, window, cx)
    });

    cx.focus(&root);
    cx.dispatch_action(OpenCreateProject);
    cx.simulate_input("buwiz");
    cx.dispatch_action(ConfirmCreateProject);

    cx.condition(&root, |view, _| view.status() == Status::Ready)
        .await;

    assert_eq!(projects.created_names(), vec!["buwiz"]);
}
```

Add a user-visible assertion such as dialog state, selected project, status text, or emitted event.

## 4. Keybinding test

Keep keybinding registration coverage separate from the main behavior test.

```rust
#[gpui::test]
async fn new_project_shortcut_opens_the_dialog(
    cx: &mut gpui::TestAppContext,
) {
    let (root, cx) = cx.add_window_view(|window, cx| {
        AppView::new(test_services(), window, cx)
    });

    cx.focus(&root);
    cx.simulate_keystrokes("cmd-shift-n");

    let is_open = root.read_with(cx, |view, _| view.create_dialog_is_open());
    assert!(is_open);
}
```

Use the repository's platform abstraction or conditional key syntax when shortcuts differ by operating system.

## 5. Validation failure

Verify both visible feedback and the absence of a backend effect.

```rust
#[gpui::test]
async fn rejects_an_empty_project_name(cx: &mut gpui::TestAppContext) {
    let projects = FakeProjects::default();

    let (root, cx) = cx.add_window_view({
        let projects = projects.clone();
        move |window, cx| AppView::new(projects, window, cx)
    });

    cx.focus(&root);
    cx.dispatch_action(OpenCreateProject);
    cx.dispatch_action(ConfirmCreateProject);

    let message = root.read_with(cx, |view, _| view.validation_message());
    assert_eq!(message.as_deref(), Some("Project name is required"));
    assert!(projects.created_names().is_empty());
}
```

## 6. Controlled async completion

For loading-state tests, let the fake pause until the test completes it.

A useful fake shape is:

```text
request arrives
  -> fake records request
  -> fake exposes a responder to the test
  -> application remains loading
  -> test completes responder
  -> GPUI task updates entity
  -> test waits on entity condition
```

Test the intermediate state before completing the response:

```rust
assert_eq!(root.read_with(cx, |view, _| view.status()), Status::Loading);

pending.complete(Ok(expected_project));

cx.condition(&root, |view, _| view.status() == Status::Ready)
    .await;
```

This proves both loading and completion behavior without a sleep.

## 7. Stable mouse target

Prefer semantic bounds:

```rust
let bounds = cx
    .debug_bounds("create-project-button")
    .expect("button should have rendered");

cx.simulate_click(bounds.center(), gpui::Modifiers::default());
```

If the pinned version does not expose a selector or debug-bounds API, use a narrow test-only helper:

```rust
#[cfg(any(test, feature = "test-support"))]
impl AppView {
    pub fn create_button_bounds(&self) -> gpui::Bounds<gpui::Pixels> {
        self.create_button_bounds
    }
}
```

Avoid fixed coordinates unless the test owns a fixed-size layout fixture.

## 8. Resize behavior

Use an explicit size when responsive layout is under test:

```rust
cx.simulate_resize(gpui::size(gpui::px(640.0), gpui::px(480.0)));

let compact = root.read_with(cx, |view, _| view.uses_compact_layout());
assert!(compact);
```

Wait for the update or render cycle required by the pinned version before reading layout-derived state.

## 9. Deterministic interleavings

Use seeds to reproduce and expand concurrency coverage:

```rust
#[gpui::test(seeds(3, 11, 42), iterations = 25)]
async fn latest_search_result_wins(cx: &mut gpui::TestAppContext) {
    // Arrange two controlled requests.
    // Complete them in scheduler-dependent order.
    // Assert stale completion cannot overwrite the newest result.
}
```

On failure, rerun with the printed seed:

```bash
SEED=<failing-seed> cargo test latest_search_result_wins -- --nocapture
```

## 10. Multiple app contexts

When supported by the pinned macro, multiple `TestAppContext` parameters can model separate clients:

```rust
#[gpui::test]
async fn propagates_shared_state(
    alice: &mut gpui::TestAppContext,
    bob: &mut gpui::TestAppContext,
) {
    // Build two app instances against a deterministic shared fake transport.
    // Perform an action in Alice's app.
    // Complete transport delivery.
    // Assert Bob's entity reaches the expected state.
}
```

Do not use this for ordinary single-window behavior.

## 11. Process-level native smoke harness

A black-box smoke test should isolate all user state:

```rust
let temp = tempfile::tempdir()?;
let mut child = std::process::Command::new(app_binary)
    .env("HOME", temp.path())
    .env("APP_CONFIG_DIR", temp.path().join("config"))
    .env("APP_DATA_DIR", temp.path().join("data"))
    .env("APP_TEST_MODE", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;
```

Use a readiness signal, accessibility query, socket, or log marker rather than sleeping for an assumed startup duration.

On failure, retain:

- application stdout and stderr
- exit status or crash signal
- screenshot
- isolated data directory
- automation step log

Always terminate the child process in cleanup, including assertion failures.

## 12. Suggested minimum test matrix

For a form or dialog flow:

| Case | Layer | Main assertion |
|---|---|---|
| opens through action | GPUI integration | dialog state is open |
| important shortcut | GPUI integration | keybinding triggers same effect |
| valid submit | GPUI integration | success state and fake call payload |
| validation failure | GPUI integration | visible error and no fake call |
| service failure | GPUI integration | error state and retry remains possible |
| duplicate submit | GPUI integration | one request or documented behavior |
| restart persistence | process E2E, only if critical | saved state survives restart |

For most features, this is more useful than a large coordinate-driven E2E script.
