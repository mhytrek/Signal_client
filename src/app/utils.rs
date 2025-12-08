use std::time::{Duration, SystemTime, UNIX_EPOCH};

use presage::libsignal_service::content::ContentBody;
use presage::store::ContentsStore;
use presage::{Manager, manager::Registered, store::Thread};
use presage_store_sqlite::SqliteStore;
use tracing::error;

use crate::app::DisplayRecipient;

pub(super) async fn initial_contact_sort(
    manager: &mut Manager<SqliteStore, Registered>,
) -> Vec<DisplayRecipient> {
    todo!()
}

async fn get_messages_backoff(
    manager: &Manager<SqliteStore, Registered>,
    thread: &Thread,
) -> Option<u64> {
    const HOURS: [u64; 5] = [24, 72, 168, 336, 720]; // 1 day, 3 days, 7 days, 14 days, 30 days
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get current system time");

    for hours_backoff in HOURS {
        let start = now.saturating_sub(Duration::from_hours(hours_backoff));
        let messages_result = manager
            .store()
            .messages(
                thread,
                (start.as_millis() as u64)..=(now.as_millis() as u64),
            )
            .await;
        let latest_msg = match messages_result {
            Ok(m) => m
                .flatten()
                .filter_map(|c| match c.body {
                    ContentBody::DataMessage(dmsg) => Some(dmsg),
                    _ => None,
                })
                .last(),
            Err(error) => {
                error!(?error, "Failed to get messages from the store.");
                return None;
            }
        };

        if let Some(msg) = latest_msg
            && let Some(timestamp) = msg.timestamp
        {
            return Some(timestamp);
        }
    }
    None
}
