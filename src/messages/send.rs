use anyhow::Result;
use presage::proto::{DataMessage, data_message::Reaction};

pub mod contact;
pub mod group;

pub fn create_reaction_data_message(
    timestamp: u64,
    target_send_timestamp: u64,
    target_author_aci: String,
    remove: bool,
    emoji: String,
) -> Result<DataMessage> {
    let reaction = Some(Reaction {
        emoji: Some(emoji),
        remove: Some(remove),
        target_author_aci: Some(target_author_aci),
        target_sent_timestamp: Some(target_send_timestamp),
    });

    let data_msg = DataMessage {
        reaction,
        timestamp: Some(timestamp),
        ..Default::default()
    };
    Ok(data_msg)
}
