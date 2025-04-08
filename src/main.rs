use anyhow::Result;
use clap::Parser;

use signal_client::args::{Cli, Command};
use signal_client::sending_text;
use signal_client::{cli, contacts, devices, tui};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::LinkDevice(args) => devices::link_new_device(args.device_name).await?,
        Command::SyncContacts => contacts::sync_contacts_cli().await?,
        Command::ListContacts => cli::print_contacts().await?,
        Command::RunApp => tui::run_tui().await?,
        Command::SendMessage(args) => {
            sending_text::send_message_cli(args.recipient, args.text_message).await?
        }

    }

    Ok(())
}
