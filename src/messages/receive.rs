use futures::Stream;
use futures::{pin_mut, StreamExt};
use presage::model::messages::Received;

pub async fn receiving_loop(messages: impl Stream<Item = Received>) {
    pin_mut!(messages);
    while let Some(content) = messages.next().await {
        match content {
            Received::QueueEmpty => break,
            Received::Contacts => {}
            Received::Content(_) => continue,
        }
    }
}
