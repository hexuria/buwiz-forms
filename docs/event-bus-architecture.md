# Event Bus Architecture — Developer Guide

> How the desktop app detects database changes from background tasks and propagates live UI updates across views.

---

## Table of Contents

1. [How It Works](#how-it-works)
2. [Subscribing a View](#subscribing-a-view)
3. [Adding a New Event](#adding-a-new-event)
4. [Current Subscribers](#current-subscribers)
5. [Rules & Gotchas](#rules--gotchas)

---

## How It Works

Background tasks (form submission retries, email polling, generic cron jobs) run **in-process** on a dedicated Tokio thread spawned in `main.rs`. After each cron tick or job completion, the cron engine calls `bir_core::ipc::post_db_changed()` to signal that the database was modified.

### macOS (Dual-Mode — Event-Driven + Polling Fallback)

```
┌────────────────────────────────────────────────────────────────────┐
│  bir-desktop process                                               │
│                                                                    │
│  ┌───────────────────┐       writes        ┌──────────┐           │
│  │  Cron Engine      │ ──────────────────▶ │  SQLite  │           │
│  │  (Tokio thread)   │                     │  (WAL)   │           │
│  └───────┬───────────┘                     └────┬─────┘           │
│          │                                      │                 │
│          │ ipc::post_db_changed()                │ PRAGMA          │
│          ▼                                      │ data_version    │
│  ┌───────────────────┐                          │ (5s fallback)   │
│  │ CFNotification    │                          │                 │
│  │ Center (local)    │                          │                 │
│  └───────┬───────────┘                          │                 │
│          │ instant (<100ms)                     │                 │
│          ▼                                      ▼                 │
│  ┌──────────────────┐    ┌──────────────────┐                     │
│  │ macOS Listener   │    │  DB Watcher      │                     │
│  │ (AtomicBool flag)│    │  (5s polling)    │                     │
│  └────────┬─────────┘    └────────┬─────────┘                     │
│           │                       │                               │
│           └─────────┬─────────────┘                               │
│                     ▼                                             │
│  ┌──────────────────────┐                                         │
│  │  GlobalEventBus      │  cx.emit(AppEvent::DatabaseChanged)     │
│  └──────────┬───────────┘                                         │
│             │ broadcast to all subscribers                        │
│             ├──▶ GlobalDashboardView::reload_actionable_forms()   │
│             ├──▶ DashboardView::reload_filing_progress()          │
│             ├──▶ Form2551QView::reload draft if status changed    │
│             └──▶ CronTasksView::load_settings()                  │
└────────────────────────────────────────────────────────────────────┘
```

### Linux / Windows (Polling Only)

```
┌──────────────────────────────────────────────────────────────────┐
│  bir-desktop process                                              │
│                                                                   │
│  ┌───────────────────┐       writes        ┌──────────┐          │
│  │  Cron Engine      │ ──────────────────▶ │  SQLite  │          │
│  │  (Tokio thread)   │                     │  (WAL)   │          │
│  └───────────────────┘                     └────┬─────┘          │
│                                                  │               │
│                       PRAGMA data_version        │ (1s polling)  │
│                                                  │               │
│  ┌──────────────────┐                            │               │
│  │  DB Watcher      │  polls every 1 second      ▼               │
│  │  (events.rs)     │  compares PRAGMA data_version              │
│  └────────┬─────────┘                                            │
│           ▼                                                      │
│  ┌──────────────────────┐                                        │
│  │  GlobalEventBus      │  cx.emit(AppEvent::DatabaseChanged)    │
│  └──────────┬───────────┘                                        │
│             │ broadcast to all subscribers                       │
│             ├──▶ (same subscriber list as macOS)                 │
└─────────────┴────────────────────────────────────────────────────┘
```

**Key insight:** The cron engine runs on a dedicated Tokio thread with its own `Arc<Mutex<Database>>`. Because this Tokio thread spawns async tasks that open separate SQLite connections (via `tokio::spawn`), `PRAGMA data_version` increments when those tasks commit. On macOS, `post_db_changed()` also posts a `CFNotification` that the desktop app receives within ~100ms via an `AtomicBool` flag. On Linux/Windows, `post_db_changed()` is a no-op and 1s PRAGMA polling is the primary mechanism.

---

## Subscribing a View

To make any view react to database changes, add this to its constructor (`fn new(...)`):

```rust
// 1. Get the global event bus
let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();

// 2. Subscribe to it
cx.subscribe(
    &bus,
    |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| match event {
        crate::events::AppEvent::DatabaseChanged => {
            // 3. Your refresh logic here
            this.reload_data(cx);
            cx.notify(); // trigger re-render
        }
    },
)
.detach();
```

### Requirements

- The `GlobalEventBus` **must** already be set as a global before your view is created. It's initialized in `app.rs` at startup (line ~171), so any view created after that point is safe.
- Always call `cx.notify()` after updating state, otherwise the UI won't re-render.

### Subscription Lifetime

You have two options for managing the subscription lifetime:

| Method | When to Use | Example |
|--------|-------------|---------|
| `.detach()` | View lives for the entire app session | `GlobalDashboardView`, `DashboardView` |
| Store in `Vec<Subscription>` | View is created/destroyed dynamically | `Form2551QView` (stores in `_subscriptions`) |

Using `.detach()` means the subscription lives as long as the view entity. If the view entity is dropped, GPUI automatically cleans up the subscription. For views that get recreated frequently, storing subscriptions gives you explicit control.

---

## Adding a New Event

### Step 1 — Define the Event Variant

Edit `crates/bir-desktop/src/events.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    /// Fired when the background cron engine modifies the SQLite database.
    DatabaseChanged,

    // ── Add your new event here ──
    /// Fired when a specific form's status changes.
    FormStatusChanged { tin: String, form_code: String },
}
```

### Step 2 — Emit the Event

From anywhere that has access to `cx`:

```rust
// Option A: From within a view that has access to cx
let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
bus.update(cx, |_, cx| {
    cx.emit(crate::events::AppEvent::FormStatusChanged {
        tin: "123-456-789".into(),
        form_code: "2551Q".into(),
    });
});
```

```rust
// Option B: From within the DB watcher (events.rs)
// Add detection logic in the polling loop and emit alongside DatabaseChanged
cx.update(|cx| {
    bus.update(cx, |_, cx| {
        cx.emit(AppEvent::FormStatusChanged { tin, form_code });
    })
});
```

### Step 3 — Handle the Event in Subscribers

Update existing subscribers to handle the new variant:

```rust
cx.subscribe(
    &bus,
    |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| match event {
        crate::events::AppEvent::DatabaseChanged => {
            this.reload_all(cx);
        }
        crate::events::AppEvent::FormStatusChanged { tin, form_code } => {
            // Only refresh if this view cares about this form
            if this.draft.tin == *tin {
                this.reload_draft(cx);
            }
        }
    },
)
.detach();
```

---

## Current Subscribers

| View | File | Event | Handler | Notes |
|------|------|-------|---------|-------|
| `GlobalDashboardView` | `views/global_dashboard.rs` | `DatabaseChanged` | `reload_actionable_forms()` | Refreshes the "Action Required" table |
| `DashboardView` | `views/dashboard.rs` | `DatabaseChanged` | `reload_filing_progress()` | Refreshes per-profile quarter cards. Guards on `active_profile.is_some()` |
| `Form2551QView` | `views/form_2551q_view.rs` | `DatabaseChanged` | Reloads draft from DB | **Smart diff**: only replaces draft if `status` actually changed. Emits `Form2551QEvent::Confirmed` when status transitions to Confirmed |
| `CronTasksView` | `views/cron_tasks.rs` | `DatabaseChanged` | `load_settings()` | Refreshes the full job list + settings |

### Views That Do NOT Subscribe (by design)

| View | Reason |
|------|--------|
| `ProfileManagerView` | Profile data rarely changes from background tasks; user triggers reload manually |
| `SettingsView` | Settings are user-driven, not background-driven |
| `LockScreenView` | No database-backed state that changes externally |
| `SidebarView` | Receives updates via parent `AppState` events, not the DB bus |

---

## Rules & Gotchas

### 1. Self-Trigger Safety

The DB watcher uses `PRAGMA data_version`, which only increments when a **different SQLite connection** commits. The cron engine runs on a separate Tokio thread and its spawned tasks open separate connections, so their writes **DO** trigger `DatabaseChanged`. The desktop app's own writes (via the shared `Arc<Mutex<Database>>`) do **NOT** trigger it.

> **⚠️ Warning:** If you ever introduce additional `Connection` instances in the GPUI main thread (e.g., for async DB access on a background executor), writes through those connections WILL trigger `DatabaseChanged` and cause unexpected reloads. Stick to the single shared `Arc<Mutex<Database>>` for desktop-initiated writes.

### 2. macOS Distributed Notifications

On macOS, the cron engine calls `ipc::post_db_changed()` which posts a `CFNotification` named `dev.goldcoders.bir.DatabaseChanged` via `CoreFoundation`. The desktop app observes this via an `AtomicBool` flag that a C callback sets, checked every 100ms by a lightweight GPUI task.

This gives us **<100ms latency** on macOS vs ~1000ms on Linux/Windows.

If you add a new background module that writes to the DB from a separate thread, call `bir_core::ipc::post_db_changed()` after your writes.

### 3. Handler Performance

The `DatabaseChanged` event fires at most once per second on Linux/Windows (the polling interval), or instantly on macOS. Your handler runs on the main thread, so keep it fast:
- ✅ SQLite queries on a small dataset — fine
- ✅ Replacing a `Vec<JobViewModel>` — fine
- ❌ Heavy computation or network calls — use `cx.spawn()` to move to background

### 4. Init Order

The `GlobalEventBus` is set as a GPUI global in `app.rs` **before** any views are created:

```
Line 181: let bus = cx.new(|_| crate::events::EventBus {});
Line 182: cx.set_global(crate::events::GlobalEventBus(bus));
// macOS notification listener + DB watcher started here
// ... all views created after this point ...
```

If you create a view before line 182, calling `cx.global::<GlobalEventBus>()` will panic.

### 5. Detach vs Store

- Use `.detach()` for views that live the entire app lifetime.
- Store `Subscription` objects in a `Vec<Subscription>` field if the view is created/destroyed dynamically, so the subscription is dropped with the view.

### 6. Testing

To manually trigger a `DatabaseChanged` event in development:

**On any platform (via PRAGMA polling):**
```bash
sqlite3 /path/to/ebirforms.db "UPDATE settings SET value = value WHERE key = 'background_cron_enabled';"
```
This commits through a different connection, incrementing `data_version` and firing the event.

**On macOS (via distributed notification — instant):**
```bash
# From a Rust scratch file or test:
bir_core::ipc::post_db_changed();
```
Or trigger it from Swift/ObjC for testing:
```swift
DistributedNotificationCenter.default().postNotificationName(
    NSNotification.Name("dev.goldcoders.bir.DatabaseChanged"),
    object: nil
)
```
