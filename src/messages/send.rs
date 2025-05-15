use crate::contacts::get_contacts_cli;
use crate::messages::receive::receiving_loop;
use crate::{create_registered_manager, AsyncContactsMap, AsyncRegisteredManager};
use anyhow::Result;
use mime_guess::mime::APPLICATION_OCTET_STREAM;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::protocol::ServiceId;
use presage::libsignal_service::sender::AttachmentSpec;
use presage::manager::Registered;
use presage::model::contacts::Contact;
use presage::proto::DataMessage;
use presage::store::ContentsStore;
use presage::Manager;
use presage_store_sled::{SledStore, SledStoreError};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// finds contact uuid from string that can be contact_name or contact phone_number
pub async fn find_uuid(
    recipient_info: String,
    manager: &mut Manager<SledStore, Registered>,
) -> Result<Uuid> {
    let contacts: Vec<Result<Contact, SledStoreError>> =
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

async fn get_address(
    recipient: String,
    manager: &mut Manager<SledStore, Registered>,
) -> Result<ServiceId> {
    let recipient_uuid = find_uuid(recipient, manager).await?;
    // let recipient_uuid = Uuid::from_str(&recipient)?;
    Ok(ServiceId::Aci(recipient_uuid.into()))
}

fn create_data_message(text_message: String, timestamp: u64) -> Result<DataMessage> {
    let data_msg = DataMessage {
        body: Some(
            text_message
                .parse()
                .map_err(|_| anyhow::anyhow!("Failed to parse text message!"))?,
        ),
        timestamp: Some(timestamp),
        ..Default::default()
    };
    Ok(data_msg)
}

async fn send(
    manager: &mut Manager<SledStore, Registered>,
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
    manager: &mut Manager<SledStore, Registered>,
    recipient: String,
    text_message: String,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let recipient_address = get_address(recipient, manager).await?;
    let data_message = create_data_message(text_message, timestamp)?;

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, current_contacts_mutex).await?;

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

/// sends text message to recipient ( phone number or name ), for usage with TUI
pub async fn send_message_tui(
    recipient: String,
    text_message: String,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    // let mut manager = create_registered_manager().await?;
    let mut manager = manager_mutex.write().await;
    send_message(
        &mut manager,
        recipient,
        text_message,
        current_contacts_mutex,
    )
    .await
}

/// sends text message to recipient ( phone number or name ), for usage with CLI
pub async fn send_message_cli(recipient: String, text_message: String) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    send_message(
        &mut manager,
        recipient,
        text_message,
        current_contacts_mutex,
    )
    .await
}

/// Create attachment spec from file path
async fn create_attachment(attachment_path: String) -> Result<(AttachmentSpec, Vec<u8>)> {
    // Resolve absolute path
    let path: PathBuf = fs::canonicalize(&attachment_path)
        .map_err(|_| anyhow::anyhow!("Failed to resolve path: {}", attachment_path))?;

    if !path.exists() {
        return Err(anyhow::anyhow!(
            "Attachment file not found: {}",
            path.display()
        ));
    }

    if !path.is_file() {
        return Err(anyhow::anyhow!(
            "Attachment path is not a file: {}",
            path.display()
        ));
    }

    let file_data = fs::read(&path)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file name for path: {}", path.display()))?
        .to_string_lossy()
        .to_string();

    let attachment_spec = AttachmentSpec {
        content_type: mime_guess::from_path(&path)
            .first()
            .unwrap_or(APPLICATION_OCTET_STREAM)
            .to_string(),
        length: file_data.len(),
        file_name: Some(file_name),
        preview: None,
        voice_note: None,
        borderless: None,
        width: None,
        height: None,
        caption: None,
        blur_hash: None,
    };

    Ok((attachment_spec, file_data))
}

/// Send message with attachment
async fn send_attachment(
    manager: &mut Manager<SledStore, Registered>,
    recipient: String,
    text_message: String,
    attachment_path: String,
    current_contacts_mutex: AsyncContactsMap,
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

    let mut data_message = create_data_message(text_message, timestamp)?;
    data_message.attachments = vec![attachment_pointer];

    let messages = manager.receive_messages().await?;
    receiving_loop(messages, manager, None, current_contacts_mutex).await?;

    send(manager, recipient_address, data_message, timestamp).await?;

    Ok(())
}

/// sends attachment to recipient ( phone number or name ), for usage with TUI
pub async fn send_attachment_tui(
    recipient: String,
    text_message: String,
    attachment_path: String,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) -> Result<()> {
    let mut manager = manager_mutex.write().await;
    send_attachment(
        &mut manager,
        recipient,
        text_message,
        attachment_path,
        current_contacts_mutex,
    )
    .await
}

/// sends attachment to recipient ( phone number or name ), for usage with CLI
pub async fn send_attachment_cli(
    recipient: String,
    text_message: String,
    attachment_path: String,
) -> Result<()> {
    let mut manager = create_registered_manager().await?;
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_cli(&manager).await?));
    send_attachment(
        &mut manager,
        recipient,
        text_message,
        attachment_path,
        current_contacts_mutex,
    )
    .await
}
