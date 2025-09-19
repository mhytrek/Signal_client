use std::str::FromStr;

use anyhow::{Ok, Result, anyhow};
use presage::{
    Manager,
    libsignal_service::{prelude::Content, zkgroup::GroupMasterKeyBytes},
    manager::Registered,
    store::{ContentsStore, Thread},
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};

use crate::{
    create_registered_manager,
    groups::find_master_key,
    messages::receive::{MessageDto, format_message},
};

pub async fn list_messages(
    manager: &Manager<SqliteStore, Registered>,
    group_master_key: GroupMasterKeyBytes,
    from: Option<String>,
) -> Result<Vec<Result<Content, SqliteStoreError>>> {
    let thread = Thread::Group(group_master_key);
    let from_u64: u64 = match from {
        Some(f) => u64::from_str(&f)?,
        None => 0,
    };
    // let from_u64 = u64::from_str(&from)?;

    Ok(manager
        .store()
        .messages(&thread, from_u64..)
        .await?
        .collect())
}

/// Returns iterator over stored messeges from certain time for given contact uuid, for use in CLI
pub async fn list_messages_cli(recipient: String, from: Option<String>) -> Result<Vec<MessageDto>> {
    let mut manager = create_registered_manager().await?;

    let master_key = find_master_key(recipient, &mut manager).await?;
    let master_key = match master_key {
        Some(mk) => mk,
        None => return Err(anyhow!("Group with given name does not exist.")),
    };

    let messages = list_messages(&manager, master_key, from).await?;

    let mut result = Vec::new();

    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            result.push(formatted_message);
        }
    }
    Ok(result)
}

pub async fn list_messages_tui(
    mut manager: Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    from: Option<String>,
) -> Result<Vec<MessageDto>> {
    let messages = list_messages(&mut manager, master_key, from).await?;

    let mut formatted_messages = vec![];
    for message in messages.into_iter().flatten() {
        if let Some(formatted_message) = format_message(&message) {
            formatted_messages.push(formatted_message);
        }
    }
    Ok(formatted_messages)
}
