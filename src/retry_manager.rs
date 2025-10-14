use crate::app::RecipientId;
use crate::messages::receive::MessageDto;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Pending,
    Sending,
    Sent,
    Failed(String),
}

#[derive(Clone)]
pub struct OutgoingMessage {
    pub id: String,
    pub recipient: RecipientId,
    pub text: String,
    pub quoted_message: Option<MessageDto>,
    pub attachment_path: Option<String>,
    pub status: DeliveryStatus,
    pub retry_count: u32,
    pub created_at: u64,
    pub last_attempt_at: Option<u64>,
}

impl OutgoingMessage {
    pub fn new(
        recipient: RecipientId,
        text: String,
        attachment_path: Option<String>,
        quoted_message: Option<MessageDto>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            recipient,
            text,
            attachment_path,
            quoted_message,
            status: DeliveryStatus::Pending,
            retry_count: 0,
            created_at: Self::current_timestamp(),
            last_attempt_at: None,
        }
    }

    pub fn should_retry(&self, max_retries: u32, retry_delay_seconds: u64) -> bool {
        if !matches!(self.status, DeliveryStatus::Failed(_)) {
            return false;
        }

        if let DeliveryStatus::Failed(ref reason) = self.status
            && reason
                .to_lowercase()
                .contains("websocket closing while waiting")
        {
            return false;
        }

        if self.retry_count >= max_retries {
            return false;
        }

        if let Some(last_attempt) = self.last_attempt_at {
            let now = Self::current_timestamp();
            let delay_ms = retry_delay_seconds * 1000;
            (now - last_attempt) >= delay_ms
        } else {
            true
        }
    }

    pub fn mark_sending(&mut self) {
        self.status = DeliveryStatus::Sending;
        self.last_attempt_at = Some(Self::current_timestamp());
        self.retry_count += 1;
    }

    pub fn mark_sent(&mut self) {
        self.status = DeliveryStatus::Sent;
    }

    pub fn mark_failed(&mut self, reason: String) {
        self.status = DeliveryStatus::Failed(reason);
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

pub struct RetryManager {
    outgoing_messages: HashMap<String, OutgoingMessage>,
    max_retries: u32,
    retry_delay_seconds: u64,
}

impl Default for RetryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryManager {
    pub fn new() -> Self {
        Self {
            outgoing_messages: HashMap::new(),
            max_retries: 3,
            retry_delay_seconds: 30,
        }
    }

    pub fn add_message(&mut self, message: OutgoingMessage) -> String {
        let id = message.id.clone();
        self.outgoing_messages.insert(id.clone(), message);
        id
    }

    pub fn mark_sent(&mut self, message_id: &str) {
        if let Some(msg) = self.outgoing_messages.get_mut(message_id) {
            msg.mark_sent();
        }
    }

    pub fn mark_sending(&mut self, message_id: &str) {
        if let Some(msg) = self.outgoing_messages.get_mut(message_id) {
            msg.mark_sending();
        }
    }

    pub fn mark_failed(&mut self, message_id: &str, reason: String) {
        if let Some(msg) = self.outgoing_messages.get_mut(message_id) {
            msg.mark_failed(reason);
        }
    }

    pub fn messages_to_retry(&mut self) -> Vec<OutgoingMessage> {
        let mut retry_messages = Vec::new();

        for msg in self.outgoing_messages.values_mut() {
            if msg.should_retry(self.max_retries, self.retry_delay_seconds) {
                retry_messages.push(msg.clone());
            }
        }
        retry_messages
    }
    pub fn cleanup_old_messages(&mut self) {
        let cutoff = OutgoingMessage::current_timestamp() - (24 * 60 * 60 * 1000); // 24 hours

        self.outgoing_messages.retain(|_, msg| {
            msg.created_at > cutoff
                || (!matches!(msg.status, DeliveryStatus::Sent)
                    && msg.retry_count < self.max_retries)
        });
    }

    pub fn failed_count(&self) -> usize {
        self.outgoing_messages
            .values()
            .filter(|msg| matches!(msg.status, DeliveryStatus::Failed(_)))
            .count()
    }

    pub fn message_status(&self, message_id: &str) -> Option<&DeliveryStatus> {
        self.outgoing_messages
            .get(message_id)
            .map(|msg| &msg.status)
    }
}
