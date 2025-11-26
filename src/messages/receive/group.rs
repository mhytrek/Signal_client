use std::{cmp::Reverse, collections::HashMap, str::FromStr};

use anyhow::{Ok, Result, bail};
use presage::{
    Manager, libsignal_service::{prelude::Content, zkgroup::GroupMasterKeyBytes}, manager::Registered, proto::data_message::Reaction, store::{ContentsStore, Thread}
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};
use uuid::Uuid;

use crate::{account_management::create_registered_manager, messages::receive::{extract_reaction, get_messages_as_message_dto}};
use crate::{
    groups::find_master_key,
    messages::receive::{MessageDto, format_attachments, format_message},
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
        None => bail!("Group with given name does not exist."),
    };

    let messages = list_messages(&manager, master_key, from).await?;

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

pub async fn list_messages_tui(
    manager: Manager<SqliteStore, Registered>,
    master_key: GroupMasterKeyBytes,
    from: Option<String>,
) -> Result<Vec<MessageDto>> {
    let messages = list_messages(&manager, master_key, from).await?;
    get_messages_as_message_dto(messages)
}
