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
}

#[derive(Args)]
pub struct LinkDeviceArgs {
    /// Name of under which linked device should be saved
    #[arg(short, long)]
    pub device_name: String,
}
