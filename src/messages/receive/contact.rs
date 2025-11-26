use std::str::FromStr;

use anyhow::Result;
use presage::{
    Manager,
    libsignal_service::prelude::{Content, Uuid},
    manager::Registered,
    store::{ContentsStore, Thread},
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};

use crate::account_management::create_registered_manager;
use crate::messages::receive::{MessageDto, get_messages_as_message_dto};

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

pub async fn list_messages_tui(
    recipient: String,
    from: String,
    manager: Manager<SqliteStore, Registered>,
) -> Result<Vec<MessageDto>> {
    let messages = list_messages(&manager, recipient, Some(from)).await?;
    get_messages_as_message_dto(messages)
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in CLI
pub async fn list_messages_cli(recipient: String, from: Option<String>) -> Result<Vec<MessageDto>> {
    let manager = create_registered_manager().await?;
    let messages = list_messages(&manager, recipient, from).await?;
    get_messages_as_message_dto(messages)
}
