use bir_core::db::Database;
use gpui::{Entity, EventEmitter};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    /// Fired when a different process (like bir_daemon) modifies the SQLite database.
    DatabaseChanged,
}

pub struct EventBus {}

impl EventEmitter<AppEvent> for EventBus {}

pub struct GlobalEventBus(pub Entity<EventBus>);

impl gpui::Global for GlobalEventBus {}

/// Spawns a background task that periodically checks the SQLite `data_version`
/// to detect if an external process (like `bir_daemon`) modified the database.
pub fn start_db_watcher(db: Arc<Mutex<Database>>, cx: &mut gpui::Context<crate::app::AppState>) {
    let bus = cx.global::<GlobalEventBus>().0.clone();
    cx.spawn(async move |_app_state, cx| {
        let mut last_version: Option<i32> = None;
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1000))
                .await;

            let current_version = if let Ok(db_guard) = db.lock() {
                // SQLite increments PRAGMA data_version when another connection commits to WAL.
                db_guard
                    .conn
                    .query_row("PRAGMA data_version;", [], |row| row.get(0))
                    .unwrap_or(0)
            } else {
                continue;
            };

            if let Some(lv) = last_version
                && lv != current_version {
                    tracing::info!(
                        "Database Watcher: External DB change detected (v{} -> v{})",
                        lv,
                        current_version
                    );
                    cx.update(|cx| {
                        bus.update(cx, |_, cx| {
                            cx.emit(AppEvent::DatabaseChanged);
                        })
                    });
                }
            last_version = Some(current_version);
        }
    })
    .detach();
}
