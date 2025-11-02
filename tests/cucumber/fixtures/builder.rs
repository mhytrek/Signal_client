use anyhow::Result;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::protocol::ServiceId;
use presage::proto::{DataMessage, GroupContextV2};
use presage::store::{ContentsStore, StateStore, Thread};
use presage_store_sqlite::SqliteStore;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MessageBuilder {
    store: SqliteStore,
    thread: Thread,
}

impl MessageBuilder {
    pub fn new(store: SqliteStore, recipient_uuid: Uuid) -> Self {
        Self {
            store,
            thread: Thread::Contact(recipient_uuid),
        }
    }

    pub fn new_group(store: SqliteStore, master_key: [u8; 32]) -> Self {
        Self {
            store,
            thread: Thread::Group(master_key),
        }
    }

    pub async fn add_received_message(
        &mut self,
        sender_uuid: Uuid,
        text: &str,
        timestamp: Option<u64>,
    ) -> Result<()> {
        let timestamp = timestamp.unwrap_or_else(current_timestamp);

        let content = create_test_content(sender_uuid, text, timestamp, false)?;

        unsafe {
            let store_ptr = &self.store as *const SqliteStore as *mut SqliteStore;
            (*store_ptr).save_message(&self.thread, content).await?;
        }

        Ok(())
    }

    pub async fn add_sent_message(&mut self, text: &str, timestamp: Option<u64>) -> Result<()> {
        let timestamp = timestamp.unwrap_or_else(current_timestamp);

        let sender_uuid = self.get_own_uuid().await?;
        let content = create_test_content(sender_uuid, text, timestamp, true)?;

        unsafe {
            let store_ptr = &self.store as *const SqliteStore as *mut SqliteStore;
            (*store_ptr).save_message(&self.thread, content).await?;
        }

        Ok(())
    }

    pub async fn add_group_message(
        &mut self,
        sender_uuid: Uuid,
        text: &str,
        master_key: &[u8; 32],
        timestamp: Option<u64>,
    ) -> Result<()> {
        let timestamp = timestamp.unwrap_or_else(current_timestamp);

        let mut content = create_test_content(sender_uuid, text, timestamp, false)?;

        if let presage::libsignal_service::content::ContentBody::DataMessage(ref mut data_msg) =
            content.body
        {
            data_msg.group_v2 = Some(GroupContextV2 {
                master_key: Some(master_key.to_vec()),
                revision: Some(0),
                ..Default::default()
            });
        }

        unsafe {
            let store_ptr = &self.store as *const SqliteStore as *mut SqliteStore;
            (*store_ptr).save_message(&self.thread, content).await?;
        }

        Ok(())
    }

    async fn get_own_uuid(&self) -> Result<Uuid> {
        let registration = self
            .store
            .load_registration_data()
            .await?
            .ok_or_else(|| anyhow::anyhow!("No registration data found"))?;
        Ok(registration.service_ids.aci)
    }
}

fn create_test_content(
    sender_uuid: Uuid,
    text: &str,
    timestamp: u64,
    is_sent: bool,
) -> Result<presage::libsignal_service::prelude::Content> {
    use presage::libsignal_service::content::{ContentBody, Metadata};
    use presage::proto::{SyncMessage, sync_message::Sent};

    let data_message = DataMessage {
        body: Some(text.to_string()),
        timestamp: Some(timestamp),
        ..Default::default()
    };

    let body = if is_sent {
        ContentBody::SynchronizeMessage(SyncMessage {
            sent: Some(Sent {
                message: Some(data_message),
                timestamp: Some(timestamp),
                ..Default::default()
            }),
            ..Default::default()
        })
    } else {
        ContentBody::DataMessage(data_message)
    };

    let sender_service_id = ServiceId::Aci(sender_uuid.into());

    Ok(presage::libsignal_service::prelude::Content {
        metadata: Metadata {
            sender: sender_service_id.clone(),
            sender_device: 1u32,
            timestamp,
            needs_receipt: false,
            unidentified_sender: false,
            was_plaintext: false,
            destination: sender_service_id,
            server_guid: None,
        },
        body,
    })
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
