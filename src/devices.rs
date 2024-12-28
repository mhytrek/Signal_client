use futures::{channel::oneshot, future};
use presage::{libsignal_service::configuration::SignalServers, Manager};
use presage::model::identity::OnNewIdentity;
use presage_store_sled::{SledStore, MigrationConflictStrategy};
use anyhow::Result;

use crate::paths;

/// Function to link device to signal account under a given name
pub async fn link_new_device(device_name: String) -> Result<()> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    ).await?;

    let (tx, rx) = oneshot::channel();
    let (_manager, _err) = future::join(
        Manager::link_secondary_device(
            store,
            SignalServers::Production,
            device_name,
            tx,
        ),
        async move {
            match rx.await {
                Ok(url) => {
                    println!("Scan the QR code to link the device!");
                    qr2term::print_qr(&url.to_string()).expect("QR generation failed");
                    println!("You can also use the URL: {}", url);
                },
                Err(err) => println!("Error while linking device: {}", err),
            }
        },
    )
    .await;
    Ok(())
}