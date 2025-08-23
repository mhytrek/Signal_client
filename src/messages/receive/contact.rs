use std::str::FromStr;

use anyhow::Result;
use presage::{
    Manager,
    libsignal_service::prelude::{Content, Uuid},
    manager::Registered,
    store::{ContentsStore, Thread},
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};

use crate::{
    AsyncRegisteredManager, create_registered_manager,
    messages::receive::{MessageDto, format_message},
};

pub async fn list_messages(
    manager: &Manager<SqliteStore, Registered>,
    recipient: String,
    from: String,
) -> Result<Vec<Result<Content, SqliteStoreError>>> {
    let recipient_uuid = Uuid::from_str(&recipient)?;
    let thread = Thread::Contact(recipient_uuid);
    let from_u64 = u64::from_str(&from)?;

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
    manager_mutex: AsyncRegisteredManager,
) -> Result<Vec<MessageDto>> {
    let manager = manager_mutex.read().await;

    let messages = list_messages(&manager, recipient, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
    }
    Ok(result)
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in CLI
pub async fn list_messages_cli(recipient: String, from: String) -> Result<Vec<MessageDto>> {
    let manager = create_registered_manager().await?;
    let messages = list_messages(&manager, recipient, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
    }
    Ok(result)
}
