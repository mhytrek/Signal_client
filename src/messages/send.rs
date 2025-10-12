use crate::messages::receive::{MessageDto, receive_messages_cli};
use crate::{
    account_management::create_registered_manager, messages::attachments::create_attachment,
};
use anyhow::Result;
use presage::Manager;
use presage::manager::Registered;
use presage_store_sqlite::SqliteStore;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod contact;
pub mod group;

/// Send message with attachment
async fn send_attachment(
    manager: &mut Manager<SqliteStore, Registered>,
    recipient: String,
    text_message: String,
    attachment_path: String,
    quoted_message: Option<MessageDto>,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = contact::get_address(recipient, manager).await?;

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

    let mut data_message = contact::create_data_message(text_message, timestamp, quoted_message)?;
    data_message.attachments = vec![attachment_pointer];

    contact::send(manager, recipient_address, data_message, timestamp).await?;

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
) -> Result<()> {
    receive_messages_cli().await?;
    let mut manager = create_registered_manager().await?;
    send_attachment(&mut manager, recipient, text_message, attachment_path, None).await
}
