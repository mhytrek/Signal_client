use clap::{Parser, Subcommand, Args};

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Link this device to your signal account
    LinkDevice(LinkDeviceArgs),
}

#[derive(Args)]
pub struct LinkDeviceArgs {
    /// Name of under which linked device should be saved
    #[arg(short, long)]
    pub device_name: String,
}