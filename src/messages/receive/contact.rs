use std::str::FromStr;

use anyhow::Result;
use presage::{
    Manager,
    libsignal_service::prelude::{Content, Uuid},
    manager::Registered,
    store::{ContentsStore, Thread},
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};

use crate::messages::receive::{MessageDto, format_message};
use crate::{account_management::create_registered_manager, messages::receive::format_attachments};

pub async fn list_messages(
    manager: &Manager<SqliteStore, Registered>,
    recipient: String,
    from: Option<String>,
) -> Result<Vec<Result<Content, SqliteStoreError>>> {
    let recipient_uuid = Uuid::from_str(&recipient)?;
    let thread = Thread::Contact(recipient_uuid);
    let from_u64: u64 = match from {
        Some(f) => u64::from_str(&f)?,
        None => 0,
    };

    Ok(manager
        .store()
        .messages(&thread, from_u64..)
        .await?
        .collect())
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in TUI
pub async fn list_messages_tui(
    recipient: String,
    from: String,
    manager: Manager<SqliteStore, Registered>,
) -> Result<Vec<MessageDto>> {
    let messages = list_messages(&manager, recipient, Some(from)).await?;

    let mut formatted_messages = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            formatted_messages.push(formatted_message);
        }
        let attachment_msgs = format_attachments(&message);
        formatted_messages.extend(attachment_msgs);
    }
    Ok(formatted_messages)
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in CLI
pub async fn list_messages_cli(recipient: String, from: Option<String>) -> Result<Vec<MessageDto>> {
    let manager = create_registered_manager().await?;
    let messages = list_messages(&manager, recipient, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
        let attachment_msgs = format_attachments(&message);
        result.extend(attachment_msgs);
    }
    Ok(result)
}
