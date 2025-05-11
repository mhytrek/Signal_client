use anyhow::Result;
use clap::Parser;

use signal_client::args::{Cli, Command};
use signal_client::messages;
use signal_client::{cli, contacts, devices, tui};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::SyncContacts => contacts::sync_contacts_cli().await?,
        Command::LinkDevice(args) => devices::link_new_device_cli(args.device_name).await?,
        Command::ListContacts => cli::print_contacts().await?,
        Command::RunApp => tui::run_tui().await?,
        Command::SendMessage(args) => {
            messages::send::send_message_cli(args.recipient, args.text_message).await?
        },
        Command::ListMessages(args) => {
            cli::print_messages(args.recipient, args.from).await?
        },
        Command::Receive => cli::print_received_message().await?
    }

    Ok(())
}
