use crate::{create_registered_manager, AsyncRegisteredManager};
use anyhow::Result;
use presage::libsignal_service::Profile;
use std::sync::Arc;

pub async fn get_profile_tui(manager_mutex: Arc<AsyncRegisteredManager>) -> Result<Profile> {
    let mut manager = manager_mutex.write().await;
    manager
        .retrieve_profile()
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn get_profile_cli() -> Result<Profile> {
    let mut manager = create_registered_manager().await?;
    manager
        .retrieve_profile()
        .await
        .map_err(|e| anyhow::anyhow!(e))
}
