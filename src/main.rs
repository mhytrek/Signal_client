use anyhow::Result;
use clap::Parser;

use signal_client::args::{Cli, Command};
use signal_client::{cli, contacts, devices, tui};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::LinkDevice(args) => devices::link_new_device(args.device_name).await?,
        Command::SyncContacts => contacts::sync_contacts().await?,
        Command::ListContacts => cli::print_contacts().await?,
        Command::RunApp => tui::run_tui()?,
    }

    Ok(())
}
