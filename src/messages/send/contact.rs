use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::account_management::create_registered_manager;
use crate::messages::attachments::create_attachment;
use crate::messages::format_message;
use crate::messages::receive::MessageDto;
use crate::messages::receive::receive_messages_cli;
use crate::messages::send::create_reaction_data_message;
use anyhow::{Result, bail};
use presage::libsignal_service::protocol::ServiceId;
use presage::proto::DataMessage;
use presage::proto::data_message::{Delete, Quote, Reaction};
use presage::store::{ContentsStore, Thread};
use presage::{
    Manager, libsignal_service::prelude::Uuid, manager::Registered, model::contacts::Contact,
};
use presage_store_sqlite::{SqliteStore, SqliteStoreError};
use tracing::error;

/// finds contact uuid from string that can be contact_name or contact phone_number
pub async fn find_uuid(
    recipient_info: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<Uuid> {
    let contacts: Vec<Result<Contact, SqliteStoreError>> =
        manager.store().contacts().await?.collect();
    let uuid = contacts
        .into_iter()
        .filter_map(|c| c.ok())
        .find(|c| {
            // Compare first by name, then by phone number
            // and finally by UUID
            (c.name == recipient_info)
                || (c.phone_number.is_some()
                    && c.phone_number.clone().unwrap().to_string() == recipient_info)
                || (c.uuid.to_string() == recipient_info)
        })
        .map(|c| c.uuid);

    uuid.ok_or_else(|| anyhow::anyhow!("Recipient '{}' not found", recipient_info))
}

pub async fn get_address(
    recipient: String,
    manager: &mut Manager<SqliteStore, Registered>,
) -> Result<ServiceId> {
    let recipient_uuid = find_uuid(recipient, manager).await?;
    // let recipient_uuid = Uuid::from_str(&recipient)?;
    Ok(ServiceId::Aci(recipient_uuid.into()))
}

pub fn create_data_message(
    text_message: String,
    timestamp: u64,
    quote_message: Option<MessageDto>,
) -> Result<DataMessage> {
    let quote = match quote_message {
        Some(mes) => Some(Quote {
            id: Some(mes.timestamp),
            text: Some(mes.text),
            author_aci: Some(mes.uuid.to_string()),
            ..Default::default()
        }),
        None => None,
    };
    let data_msg = DataMessage {
        body: Some(
            text_message
                .parse()
                .map_err(|_| anyhow::anyhow!("Failed to parse text message!"))?,
        ),
        timestamp: Some(timestamp),
        quote,
        ..Default::default()
    };
    Ok(data_msg)
}

pub fn create_delete_data_message(
    timestamp: u64,
    target_send_timestamp: u64,
) -> Result<DataMessage> {
    let del_mes = Delete {
        target_sent_timestamp: Some(target_send_timestamp),
    };
    let data_msg = DataMessage {
        timestamp: Some(timestamp),
        delete: Some(del_mes),
        ..Default::default()
    };
    Ok(data_msg)
}

pub async fn send(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient_addr: ServiceId,
    data_message: DataMessage,
    timestamp: u64,
) -> Result<()> {
    manager
        .send_message(recipient_addr, data_message, timestamp)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
    Ok(())
}

async fn send_message(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: String,
    text_message: String,
    quoted_message: Option<MessageDto>,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = get_address(recipient, manager).await?;
    let data_message = create_data_message(text_message, timestamp, quoted_message)?;

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

async fn send_delete_message(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: String,
    target_send_timestamp: u64,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = get_address(recipient, manager).await?;
    let data_message = create_delete_data_message(timestamp, target_send_timestamp)?;

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

async fn send_reaction_message(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: String,
    target_send_timestamp: u64,
    target_author_aci: String,
    remove:bool,
    emoji: String
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = get_address(recipient, manager).await?;
    let data_message = create_reaction_data_message(timestamp, target_send_timestamp,target_author_aci,remove,emoji)?;

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

pub async fn send_reaction_message_tui(
    mut manager: Manager<SqliteStore, Registered>,
    recipient: String,
    target_send_timestamp: u64,
    target_author_aci: String,
    remove:bool,
    emoji: String
) -> Result<()> {
    send_reaction_message(&mut manager, recipient, target_send_timestamp, target_author_aci, remove,emoji).await
}

/// sends text message to recipient ( phone number or name ), for usage with TUI
pub async fn send_message_tui(
    recipient: String,
    text_message: String,
    quoted_message: Option<MessageDto>,
    mut manager: Manager<SqliteStore, Registered>,
) -> Result<()> {
    // let mut manager = create_registered_manager().await?;
    send_message(&mut manager, recipient, text_message, quoted_message).await
}

pub async fn send_delete_message_tui(
    mut manager: Manager<SqliteStore, Registered>,
    recipient: String,
    target_send_timestamp: u64,
) -> Result<()> {
    send_delete_message(&mut manager, recipient, target_send_timestamp).await
}

/// sends text message to recipient ( phone number or name ), for usage with CLI
pub async fn send_message_cli(
    recipient: String,
    text_message: String,
    quoted_message: Option<u64>,
) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let quoted_message_dto = match quoted_message {
        Some(quote_ts) => {
            let recipient_uuid = Uuid::from_str(&recipient)?;
            let thread = Thread::Contact(recipient_uuid);
            let quoted_data_message = manager.store().message(&thread, quote_ts).await?;
            if let Some(content) = quoted_data_message {
                format_message(&content)
            } else {
                None
            }
        }
        None => None,
    };
    send_message(&mut manager, recipient, text_message, quoted_message_dto).await
}

async fn send_attachment(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: String,
    text_message: String,
    attachment_path: String,
    quoted_message: Option<MessageDto>,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = get_address(recipient, manager).await?;

    let attachment_spec = create_attachment(attachment_path).await?;

    let attachment_specs = vec![attachment_spec];

    let attachments: Result<Vec<_>, _> = manager
        .upload_attachments(attachment_specs)
        .await?
        .into_iter()
        .collect();
    let attachments = attachments?;

    let attachment_pointer = attachments
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to get attachment pointer"))?;

    let mut data_message = create_data_message(text_message, timestamp, quoted_message)?;
    data_message.attachments = vec![attachment_pointer];

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

/// sends attachment to recipient ( phone number or name ), for usage with TUI
pub async fn send_attachment_tui(
    recipient: String,
    text_message: String,
    attachment_path: String,
    quoted_message: Option<MessageDto>,
    mut manager: Manager<SqliteStore, Registered>,
) -> Result<()> {
    send_attachment(
        &mut manager,
        recipient,
        text_message,
        attachment_path,
        quoted_message,
    )
    .await
}

/// sends attachment to recipient ( phone number or name ), for usage with CLI
pub async fn send_attachment_cli(
    recipient: String,
    text_message: String,
    attachment_path: String,
    quoted_message: Option<u64>,
) -> Result<()> {
    receive_messages_cli().await?;
    let mut manager = create_registered_manager().await?;
    let quoted_message_dto = match quoted_message {
        Some(quote_ts) => {
            let recipient_uuid = Uuid::from_str(&recipient)?;
            let thread = Thread::Contact(recipient_uuid);
            let quoted_data_message = manager.store().message(&thread, quote_ts).await?;
            if let Some(content) = quoted_data_message {
                format_message(&content)
            } else {
                None
            }
        }
        None => None,
    };
    send_attachment(
        &mut manager,
        recipient,
        text_message,
        attachment_path,
        quoted_message_dto,
    )
    .await
}
pub async fn send_delete_message_cli(recipient: String, target_send_timestamp: u64) -> Result<()> {
    let mut manager: Manager<SqliteStore, Registered> = create_registered_manager().await?;
    let uuid = Uuid::from_str(&recipient)?;
    let thread = Thread::Contact(uuid);

    let sender = match manager
        .store()
        .message(&thread, target_send_timestamp)
        .await?
    {
        Some(con) => con.metadata.sender,
        None => bail!("Message with given timestamp not found."),
    };

    let user = manager.whoami().await?;

    match sender.raw_uuid() == user.aci {
        true => send_delete_message(&mut manager, recipient, target_send_timestamp).await,
        false => {
            error!("Cannot delete message not send by this user");
            Ok(())
        }
    }
}
