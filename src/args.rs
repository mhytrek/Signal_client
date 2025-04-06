use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Link this device to your signal account
    LinkDevice(LinkDeviceArgs),
    /// Synchronizes contacts with primary device
    SyncContacts,
    /// Prints locally stored contacts
    ListContacts,
    /// Displays a prototype of a layout with example data
    RunApp,
    /// Send text message
    SendMessage(SendMessageArgs),
}

#[derive(Args)]
pub struct LinkDeviceArgs {
    /// Name of under which linked device should be saved
    #[arg(short, long)]
    pub device_name: String,
}

#[derive(Args)]
pub struct SendMessageArgs {
    /// Name of under which linked device should be saved
    #[arg(short, long)]
    pub recipient: String,
    pub text_message: String,
}
