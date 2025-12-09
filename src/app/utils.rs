use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::join_all;
use presage::libsignal_service::content::ContentBody;
use presage::store::ContentsStore;
use presage::{Manager, manager::Registered, store::Thread};
use presage_store_sqlite::SqliteStore;
use tracing::error;

use crate::app::{
    DisplayRecipient, DisplayRecipientType, contact_to_display_contact, group_to_display_group,
};
use crate::{contacts, groups};

pub(super) async fn timestamp_recipient_sort(
    manager: &mut Manager<SqliteStore, Registered>,
) -> Vec<DisplayRecipient> {
    let contacts = contacts::list_contacts_tui(manager)
        .await
        .expect("Failed to retrieve contacts.")
        .into_iter()
        .flatten();
    let groups = groups::list_groups_tui(manager)
        .await
        .expect("Failed to retrieve groups.")
        .into_iter()
        .flatten();

    let display_contacts = contacts.map(|contact| async {
        let thread = Thread::Contact(contact.uuid);
        let timestamp = get_messages_backoff(manager, &thread).await;
        let display_contact = contact_to_display_contact(contact, manager.clone()).await;
        display_contact.map(|dc| DisplayRecipient {
            recipient_type: DisplayRecipientType::Contact(dc),
            latest_message_timestamp: timestamp,
        })
    });
    let display_contacts = join_all(display_contacts)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let display_groups = groups.map(|(master_key, group)| {
        let inner_manager = manager.clone();
        async move {
            let thread = Thread::Group(master_key);
            let timestamp = get_messages_backoff(&inner_manager, &thread).await;
            let display_group = group_to_display_group(group, master_key);
            display_group.map(|dg| DisplayRecipient {
                recipient_type: DisplayRecipientType::Group(dg),
                latest_message_timestamp: timestamp,
            })
        }
    });
    let display_groups = join_all(display_groups)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let mut display_recipients = display_contacts
        .into_iter()
        .chain(display_groups.into_iter())
        .collect::<Vec<_>>();
    display_recipients.sort_by_key(|dr| dr.latest_message_timestamp.unwrap_or(0));
    display_recipients.reverse();

    display_recipients
}

async fn get_messages_backoff(
    manager: &Manager<SqliteStore, Registered>,
    thread: &Thread,
) -> Option<u64> {
    const HOURS: [u64; 5] = [24, 72, 168, 336, 720]; // 1 day, 3 days, 7 days, 14 days, 30 days
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get current system time");
    error!(?now);

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
                .next(),
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
