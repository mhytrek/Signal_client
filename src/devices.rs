use crate::create_registered_manager;
use crate::AsyncRegisteredManager;
use std::path::Path;

use anyhow::Result;
use futures::{channel::oneshot, future};
use presage::model::identity::OnNewIdentity;
use presage::{libsignal_service::configuration::SignalServers, Manager};
use presage_store_sled::{MigrationConflictStrategy, SledStore};
use qrcode_generator::QrCodeEcc;
use tokio::fs;

use crate::paths::{self, ASSETS, QRCODE};

/// Function to link device to signal account under a given name
pub async fn link_new_device(device_name: String,to_png:bool) -> Result<()> {
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
                    if to_png{
                        qrcode_generator::to_png_to_file(url.as_ref(), QrCodeEcc::Low, 512, QRCODE).unwrap();
                    }
                    else{
                        println!("Scan the QR code to link the device!");
                        qr2term::print_qr(url.as_ref()).expect("QR generation failed");
                        println!("You can also use the URL: {}", url);
                    }

                }
                Err(err) => println!("Error while linking device: {}", err),
            }
        },
    )
    .await;
    Ok(())
}


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


// pub async fn is_registered_cli() -> Result<bool>{
//     let manager = create_registered_manager().await;

//     match manager{
//         Ok(_) => return Ok(true),
//         Err(_) => return Ok(false),
//     }

// }
