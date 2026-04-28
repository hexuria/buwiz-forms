# Event Bus Architecture — Developer Guide

> How the desktop app detects external database changes and propagates live UI updates across views.

---

## Table of Contents

1. [How It Works](#how-it-works)
2. [Subscribing a View](#subscribing-a-view)
3. [Adding a New Event](#adding-a-new-event)
4. [Current Subscribers](#current-subscribers)
5. [Rules & Gotchas](#rules--gotchas)

---

## How It Works

```
┌───────────────┐       writes        ┌──────────┐
│  bir-daemon   │ ──────────────────▶ │  SQLite  │
│  (background) │                     │  (WAL)   │
└───────────────┘                     └────┬─────┘
                                           │
                      PRAGMA data_version  │ changes
                                           │
┌──────────────────────────────────────────┼──────────────────────┐
│  bir-desktop (UI)                        ▼                     │
│                                                                │
│  ┌──────────────────┐                                          │
│  │  DB Watcher      │  polls every 1 second                    │
│  │  (events.rs)     │  compares PRAGMA data_version            │
│  └────────┬─────────┘                                          │
│           │ version changed?                                   │
│           ▼                                                    │
│  ┌──────────────────┐                                          │
│  │  GlobalEventBus  │  cx.emit(AppEvent::DatabaseChanged)      │
│  │  (GPUI Global)   │                                          │
│  └────────┬─────────┘                                          │
│           │ broadcast to all subscribers                       │
│           ├──▶ GlobalDashboardView::reload_actionable_forms()  │
│           ├──▶ DashboardView::reload_filing_progress()         │
│           ├──▶ Form2551QView::reload draft if status changed   │
│           └──▶ CronTasksView::load_settings()                 │
└────────────────────────────────────────────────────────────────┘
```

**Key insight:** SQLite's `PRAGMA data_version` increments when a *different connection* commits. Since `bir-daemon` is a separate process with its own connection, any write it makes is detected by the desktop app's watcher within 1 second.

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
    /// Fired when an external process modifies the SQLite database.
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
| `ProfileManagerView` | Profile data rarely changes from daemon; user triggers reload manually |
| `SettingsView` | Settings are user-driven, not daemon-driven |
| `LockScreenView` | No database-backed state that changes externally |
| `SidebarView` | Receives updates via parent `AppState` events, not the DB bus |

---

## Rules & Gotchas

### 1. Self-Trigger Safety

The DB watcher uses `PRAGMA data_version`, which only increments when a **different SQLite connection** commits. Since the desktop app uses a single `Connection` (wrapped in `Arc<Mutex<Database>>`), writes from the desktop app itself do **NOT** trigger `DatabaseChanged`.

> **⚠️ Warning:** If you ever introduce a second `Connection` in the desktop process (e.g., for async DB access on a background thread), writes through that connection WILL trigger `DatabaseChanged` and cause unexpected reloads. Stick to the single shared `Arc<Mutex<Database>>`.

### 2. Handler Performance

The `DatabaseChanged` event fires at most once per second (the polling interval). Your handler runs on the main thread, so keep it fast:
- ✅ SQLite queries on a small dataset — fine
- ✅ Replacing a `Vec<JobViewModel>` — fine
- ❌ Heavy computation or network calls — use `cx.spawn()` to move to background

### 3. Init Order

The `GlobalEventBus` is set as a GPUI global in `app.rs` **before** any views are created:

```
Line 170: let bus = cx.new(|_| crate::events::EventBus {});
Line 171: cx.set_global(crate::events::GlobalEventBus(bus));
Line 172: crate::events::start_db_watcher(Arc::clone(&db), cx);
// ... all views created after this point ...
```

If you create a view before line 171, calling `cx.global::<GlobalEventBus>()` will panic.

### 4. Detach vs Store

- Use `.detach()` for views that live the entire app lifetime.
- Store `Subscription` objects in a `Vec<Subscription>` field if the view is created/destroyed dynamically, so the subscription is dropped with the view.

### 5. Testing

To manually trigger a `DatabaseChanged` event in development, open a second terminal and run:

```bash
sqlite3 /path/to/ebirforms.db "UPDATE settings SET value = value WHERE key = 'background_cron_enabled';"
```

This commits through a different connection, incrementing `data_version` and firing the event.
