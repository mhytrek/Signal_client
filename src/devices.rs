use std::path::Path;

use anyhow::Result;
use futures::{channel::oneshot, future};
use presage::model::identity::OnNewIdentity;
use presage::{libsignal_service::configuration::SignalServers, Manager};
use presage_store_sled::{MigrationConflictStrategy, SledStore};
use qrcode_generator::QrCodeEcc;
use tokio::fs;

use crate::paths::{self, ASSETS, QRCODE};

/// Links a new device to the Signal account using the given name.
/// Generates a QR code as a PNG file and waits for the user to scan it to complete the linking process.
pub async fn link_new_device_tui(device_name: String) -> Result<()> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    )
    .await?;

    if !Path::new(ASSETS).exists(){
        fs::create_dir(ASSETS).await?;
    }  

    let (tx, rx) = oneshot::channel();
    let (_manager, _err) = future::join(
        Manager::link_secondary_device(store, SignalServers::Production, device_name, tx),
        async move {
            match rx.await {
                Ok(url) => {
                    qrcode_generator::to_png_to_file(url.as_ref(), QrCodeEcc::Low, 600, QRCODE).unwrap();
                }
                Err(err) => println!("Error while linking device: {}", err),
            }
        },
    )
    .await;

    if Path::new(ASSETS).exists(){
        fs::remove_dir_all(ASSETS).await?;
    }  

    Ok(())
}

/// Links a new device to the Signal account using the given name.
/// Generates a QR code and prints it in the terminal, then waits for the user to scan it to complete the linking process.
pub async fn link_new_device_cli(device_name: String) -> Result<()> {
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    )
    .await?;

    let (tx, rx) = oneshot::channel();
    let (_manager, _err) = future::join(
        Manager::link_secondary_device(store, SignalServers::Production, device_name, tx),
        async move {
            match rx.await {
                Ok(url) => {
                    println!("Scan the QR code to link the device!");
                    qr2term::print_qr(url.as_ref()).expect("QR generation failed");
                    println!("You can also use the URL: {}", url);
                }
                Err(err) => println!("Error while linking device: {}", err),
            }
        },
    )
    .await;
    Ok(())
}


/// return true if the device is registered and false otherwise
pub async fn is_registered() -> Result<bool>{
    let store = SledStore::open(
        paths::STORE,
        MigrationConflictStrategy::Drop,
        OnNewIdentity::Trust,
    )
    .await?;


    match Manager::load_registered(store).await{
        Ok(_) => return Ok(true),
        Err(_) => return Ok(false),
    }
}

