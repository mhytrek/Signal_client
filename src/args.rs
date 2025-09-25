use clap::{ArgGroup, Args, Parser, Subcommand};

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

    /// Prints locally stored groups
    ListGroups,

    /// Displays a prototype of a layout with example data
    RunApp,

    /// Send text message
    SendMessage(SendMessageArgs),

    /// Send text message to group
    SendToGroup(SendToGroupArgs),

    /// Send attachment
    SendAttachment(SendAttachmentArgs),

    /// Prints messages from given point in time
    ListMessages(ListMessagesArgs),

    /// Prints the messages received from the last synchronization
    Receive,

    /// Prints profile info
    GetProfile,

    /// Create a new account
    CreateAccount(CreateAccountArgs),

    /// List all accounts
    ListAccounts,

    /// Switch to an account
    SwitchAccount(SwitchAccountArgs),

    /// Get current active account
    GetCurrentAccount,
}

#[derive(Args)]
pub struct CreateAccountArgs {
    /// Name of the new account
    #[arg(short, long)]
    pub account_name: String,

    /// Device name for Signal registration
    #[arg(short, long)]
    pub device_name: String,
}

#[derive(Args)]
pub struct SwitchAccountArgs {
    /// Name of the account to switch to
    #[arg(short, long)]
    pub account_name: String,
}

#[derive(Args)]
pub struct LinkDeviceArgs {
    /// Name of under which linked device should be saved
    #[arg(short, long)]
    pub device_name: String,
}

#[derive(Args)]
pub struct SendMessageArgs {
    /// Name, phone number or UUID of the contact that the message should be send to
    #[arg(short, long)]
    pub recipient: String,

    /// Content of the message
    pub text_message: String,
}

#[derive(Args)]
pub struct SendToGroupArgs {
    /// Name of the destination group
    #[arg(short, long)]
    pub group: String,

    /// Content of the message
    pub text_message: String,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("recipient")
        .required(true)
))]
pub struct ListMessagesArgs {
    /// Uuid of the contact that the message history should be shown
    #[arg(short, long, group = "recipient")]
    pub contact: Option<String>,

    /// Name of the group that the message history should be shown
    #[arg(short, long, group = "recipient")]
    pub group: Option<String>,

    /// The timestamp from which messages start being displayed
    pub from: Option<String>,
}

#[derive(Args)]
pub struct SendAttachmentArgs {
    /// Uuid of the contact that the message should be send to
    #[arg(short, long)]
    pub recipient: String,

    /// Content of the message
    #[arg(short, long, default_value_t = String::from(""))]
    pub text_message: String,

    /// Full path to attachment
    pub attachment_path: String,
}
