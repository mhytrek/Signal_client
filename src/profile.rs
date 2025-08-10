use crate::{AsyncRegisteredManager, create_registered_manager};
use anyhow::Result;
use presage::libsignal_service::Profile;
use std::sync::Arc;

pub async fn get_profile_tui(manager_mutex: Arc<AsyncRegisteredManager>) -> Result<Profile> {
    let mut manager = manager_mutex.write().await;
    manager.retrieve_profile().await.map_err(anyhow::Error::new)
}

pub async fn get_profile_cli() -> Result<Profile> {
    let mut manager = create_registered_manager().await?;
    manager.retrieve_profile().await.map_err(anyhow::Error::new)
}

pub async fn get_my_profile_avatar_cli() -> Result<Option<Vec<u8>>> {
    let mut manager = create_registered_manager().await?;

    let registration_data = manager.registration_data();
    let profile_key = registration_data.profile_key();

    let whoami = manager.whoami().await?;
    let uuid = whoami.aci;

    match manager
        .retrieve_profile_avatar_by_uuid(uuid, profile_key)
        .await
    {
        Ok(Some(avatar_bytes)) => Ok(Some(avatar_bytes.to_vec())),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::Error::new(e)),
    }
}

pub async fn get_my_profile_avatar_tui(
    manager_mutex: Arc<AsyncRegisteredManager>,
) -> Result<Option<Vec<u8>>> {
    let mut manager = manager_mutex.write().await;

    let registration_data = manager.registration_data();
    let profile_key = registration_data.profile_key();

    let whoami = manager.whoami().await?;
    let uuid = whoami.aci;

    match manager
        .retrieve_profile_avatar_by_uuid(uuid, profile_key)
        .await
    {
        Ok(Some(avatar_bytes)) => Ok(Some(avatar_bytes.to_vec())),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::Error::new(e)),
    }
}
