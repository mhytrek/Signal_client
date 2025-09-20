use crate::contacts::get_contacts_tui;
use crate::messages::receive::{self, MessageDto, contact};
use crate::messages::send::{self, send_attachment_tui};
use crate::paths::QRCODE;
use crate::profile::get_profile_tui;
use crate::ui::render_ui;
use crate::{
    AsyncContactsMap, config::Config, contacts, create_registered_manager, devices, groups,
};
use anyhow::{Error, Result};
use crossterm::event::{self, Event, KeyModifiers};
use crossterm::event::{KeyCode, KeyEventKind};
use presage::Manager;
use presage::libsignal_service::Profile;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::zkgroup::GroupMasterKeyBytes; // Fix: Use full path
use presage::manager::Registered;
use presage_store_sqlite::SqliteStore;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::HashMap;
use std::io::Stderr;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{fs, io};
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio_util::task::LocalPoolHandle;
use tracing::{error, warn}; // Removed unused 'info'

use crate::retry_manager::{OutgoingMessage, RetryManager};
use image::ImageFormat;
use std::thread;
use std::time::Duration;
use tokio::time::interval;

#[derive(PartialEq, Clone)]
pub enum RecipientId {
    Contact(Uuid),
    Group(GroupMasterKeyBytes),
}

impl Default for RecipientId {
    fn default() -> Self {
        Self::Contact(Uuid::nil())
    }
}

pub trait DisplayRecipient: Send {
    fn display_name(&self) -> &str;
    fn id(&self) -> RecipientId;
}

#[derive(Clone, PartialEq)]
pub struct DisplayContact {
    display_name: String,
    uuid: Uuid,
}

impl DisplayContact {
    fn new(display_name: String, uuid: Uuid) -> Self {
        Self { display_name, uuid }
    }
}

impl DisplayRecipient for DisplayContact {
    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn id(&self) -> RecipientId {
        RecipientId::Contact(self.uuid)
    }
}

#[derive(Clone, PartialEq)]
pub struct DisplayGroup {
    display_name: String,
    master_key: GroupMasterKeyBytes,
}

impl DisplayGroup {
    fn new(display_name: String, master_key: GroupMasterKeyBytes) -> Self {
        Self {
            display_name,
            master_key,
        }
    }
}

impl DisplayRecipient for DisplayGroup {
    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn id(&self) -> RecipientId {
        RecipientId::Group(self.master_key)
    }
}

#[derive(PartialEq)]
pub enum CurrentScreen {
    Main,
    Syncing,
    LinkingNewDevice,
    Writing,
    Options,
    Exiting,
    ContactInfo,
}

#[derive(PartialEq)]
pub enum LinkingStatus {
    Unlinked,
    InProgress,
    Linked,
    Error(String),
}

pub enum InputFocus {
    Message,
    Attachment,
}

#[derive(Clone)]
pub struct ContactInfo {
    pub uuid: String,
    pub name: String,
    pub phone_number: Option<String>,
    pub verified_state: Option<i32>,
    pub expire_timer: u32,
    pub has_avatar: bool,
}

pub struct App {
    pub recipients: Vec<(Box<dyn DisplayRecipient>, String)>, // contact_uuid, contact_name, input for this contact

    pub selected_recipient: usize,
    pub message_selected: usize,

    // New fields for contact info
    pub selected_contact_info: Option<ContactInfo>,
    pub contact_avatar_cache: Option<Vec<u8>>,
    pub contact_avatar_image: Option<StatefulProtocol>,

    pub current_screen: CurrentScreen,
    pub linking_status: LinkingStatus,
    pub network_status: NetworkStatus,

    pub character_index: usize,
    pub textarea: String,
    pub attachment_path: String,
    pub attachment_error: Option<String>,

    pub retry_manager: Arc<Mutex<RetryManager>>,
    pub message_id_map: HashMap<String, String>,

    pub input_focus: InputFocus,

    pub profile: Option<Profile>,

    pub avatar_cache: Option<Vec<u8>>,
    pub picker: Option<Picker>,
    pub avatar_image: Option<StatefulProtocol>,

    pub contact_messages: HashMap<String, Vec<MessageDto>>,
    pub group_messages: HashMap<GroupMasterKeyBytes, Vec<MessageDto>>,

    pub config: Config,
    pub config_selected: usize,

    pub manager: Option<Manager<SqliteStore, Registered>>,

    pub tx_thread: mpsc::Sender<EventApp>,
    pub rx_tui: mpsc::Receiver<EventApp>,

    pub tx_tui: mpsc::Sender<EventSend>,
    pub rx_thread: Option<mpsc::Receiver<EventSend>>,
}

#[derive(PartialEq, Clone)]
pub enum NetworkStatus {
    Connected,
    Disconnected(String),
}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<Box<dyn DisplayRecipient>>),
    LinkingFinished((bool, Option<Manager<SqliteStore, Registered>>)),
    LinkingError(String),
    NetworkStatusChanged(NetworkStatus),

    ProfileReceived(Profile),
    AvatarReceived(Vec<u8>),

    ContactInfoReceived(ContactInfo),
    ContactAvatarReceived(Vec<u8>),

    GetContactMessageHistory(String, Vec<MessageDto>),
    GetGroupMessageHistory(GroupMasterKeyBytes, Vec<MessageDto>),
    ReceiveMessage,
    QrCodeGenerated,
    Resize(u16, u16),
}
pub enum EventSend {
    SendText(RecipientId, String),
    SendAttachment(RecipientId, String, String),
    GetMessagesForContact(String),
    GetMessagesForGroup(GroupMasterKeyBytes),
    GetContactInfo(String),
}

impl App {
    pub fn new(linking_status: LinkingStatus) -> App {
        let (tx_thread, rx_tui) = mpsc::channel();
        let (tx_tui, rx_thread) = mpsc::channel();
        let picker = Picker::from_query_stdio().ok();
        App {
            linking_status,
            recipients: vec![],
            selected_recipient: 0,
            message_selected: 0,
            character_index: 0,
            current_screen: CurrentScreen::LinkingNewDevice,
            textarea: String::new(),
            contact_messages: HashMap::new(),
            group_messages: HashMap::new(),
            network_status: NetworkStatus::Connected,
            attachment_path: String::new(),
            attachment_error: None,
            input_focus: InputFocus::Message,

            retry_manager: Arc::new(Mutex::new(RetryManager::new())),
            message_id_map: HashMap::new(),

            profile: None,
            avatar_cache: None,
            picker,
            avatar_image: None,

            selected_contact_info: None,
            contact_avatar_cache: None,
            contact_avatar_image: None,

            config: Config::load(),
            config_selected: 0,

            manager: None,

            tx_thread,
            rx_tui,
            tx_tui,
            rx_thread: Some(rx_thread),
        }
    }

    pub(crate) async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
    ) -> io::Result<bool> {
        if self.linking_status == LinkingStatus::Linked {
            if let Some(rx) = self.rx_thread.take() {
                let new_manager = match create_registered_manager().await {
                    Ok(manager) => manager,
                    Err(_) => {
                        return Err(io::Error::other("Failed to create manager"));
                    }
                };

                self.manager = Some(new_manager.clone());

                if let Err(e) = init_background_threads(
                    self.tx_thread.clone(),
                    rx,
                    new_manager,
                    Arc::clone(&self.retry_manager),
                )
                .await
                {
                    eprintln!("Failed to init threads: {e:?}");
                }
            }
            self.current_screen = CurrentScreen::Syncing;
        }

        let tx_key_events = self.tx_thread.clone();
        thread::spawn(move || {
            handle_input_events(tx_key_events);
        });

        loop {
            terminal.draw(|f| render_ui(f, self))?;

            if let Ok(event) = self.rx_tui.recv()
                && self.handle_event(event, &self.tx_tui.clone()).await?
            {
                return Ok(true);
            }
        }
    }

    pub fn load_avatar(&mut self) {
        if let (Some(avatar_data), Some(picker)) = (&self.avatar_cache, &mut self.picker) {
            match image::load_from_memory(avatar_data) {
                Ok(dynamic_image) => {
                    self.avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                }
                Err(_) => {
                    if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Png)
                    {
                        self.avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    } else if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Jpeg)
                    {
                        self.avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    }
                }
            }
        }
    }

    pub fn load_contact_avatar(&mut self) {
        if let (Some(avatar_data), Some(picker)) = (&self.contact_avatar_cache, &mut self.picker) {
            match image::load_from_memory(avatar_data) {
                Ok(dynamic_image) => {
                    self.contact_avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                }
                Err(_) => {
                    if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Png)
                    {
                        self.contact_avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    } else if let Ok(dynamic_image) =
                        image::load_from_memory_with_format(avatar_data, ImageFormat::Jpeg)
                    {
                        self.contact_avatar_image = Some(picker.new_resize_protocol(dynamic_image));
                    }
                }
            }
        }
    }

    async fn handle_event(&mut self, event: EventApp, tx: &Sender<EventSend>) -> io::Result<bool> {
        match event {
            EventApp::KeyInput(key) => {
                if key.kind == KeyEventKind::Release {
                    return Ok(false);
                }
                self.handle_key_event(key, tx)
            }
            EventApp::NetworkStatusChanged(status) => {
                self.network_status = status;
                Ok(false)
            }
            EventApp::LinkingError(error_msg) => {
                self.linking_status = LinkingStatus::Error(error_msg);
                Ok(false)
            }
            EventApp::ContactsList(recipients) => {
                if self.current_screen == CurrentScreen::Syncing {
                    self.current_screen = CurrentScreen::Main;
                }
                // This is added because contacts change order in the contact list
                // and if that happens the same contact should remain selected
                let selected_id = self
                    .recipients
                    .get(self.selected_recipient)
                    .map(|contact| contact.0.id())
                    .unwrap_or_default();

                self.recipients = recipients
                    .into_iter()
                    .map(|recipient| (recipient, String::new()))
                    .collect();

                self.selected_recipient = self
                    .recipients
                    .iter()
                    .position(|c| c.0.id() == selected_id)
                    .unwrap_or(0);
                Ok(false)
            }
            EventApp::ContactInfoReceived(contact) => {
                self.selected_contact_info = Some(contact);
                Ok(false)
            }
            EventApp::ContactAvatarReceived(avatar_data) => {
                self.contact_avatar_cache = Some(avatar_data);
                if self.config.show_images {
                    self.load_contact_avatar();
                }
                Ok(false)
            }
            EventApp::LinkingFinished((result, manager_optional)) => {
                match result {
                    true => {
                        self.linking_status = LinkingStatus::Linked;
                        if let Some(rx) = self.rx_thread.take() {
                            let new_manager = match manager_optional {
                                Some(manager) => manager,
                                None => match create_registered_manager().await {
                                    Ok(manager) => manager,
                                    Err(_) => {
                                        return Err(io::Error::other("Failed to create manager"));
                                    }
                                },
                            };

                            self.manager = Some(new_manager.clone());

                            if let Err(e) = init_background_threads(
                                self.tx_thread.clone(),
                                rx,
                                new_manager,
                                Arc::clone(&self.retry_manager),
                            )
                            .await
                            {
                                eprintln!("Failed to init threads: {e:?}");
                            }
                        }
                        self.current_screen = CurrentScreen::Syncing;
                    }
                    false => self.linking_status = LinkingStatus::Unlinked,
                }
                Ok(false)
            }
            EventApp::ProfileReceived(profile) => {
                self.profile = Some(profile);

                Ok(false)
            }
            EventApp::AvatarReceived(avatar_data) => {
                self.avatar_cache = Some(avatar_data);
                Ok(false)
            }
            EventApp::GetContactMessageHistory(uuid_str, messages) => {
                self.contact_messages.insert(uuid_str, messages);
                self.message_selected = 0;
                Ok(false)
            }
            EventApp::GetGroupMessageHistory(master_key, messages) => {
                self.group_messages.insert(master_key, messages);
                self.message_selected = 0;
                Ok(false)
            }
            EventApp::ReceiveMessage => {
                self.synchronize_messages_for_selected_recipient();
                Ok(false)
            }
            EventApp::QrCodeGenerated => Ok(false),
            EventApp::Resize(_, _) => Ok(false),
        }
    }

    fn enter_char(&mut self, new_char: char) {
        if let Some((_, input)) = self.recipients.get_mut(self.selected_recipient) {
            input.push(new_char);
            self.character_index += 1;
        }
    }

    fn delete_char(&mut self) {
        if let Some((_, input)) = self.recipients.get_mut(self.selected_recipient)
            && self.character_index > 0
        {
            input.pop();
            self.character_index -= 1;
        }
    }

    fn validate_attachment_path(&mut self) {
        if self.attachment_path.trim().is_empty() {
            self.attachment_error = None;
        } else if !Path::new(&self.attachment_path).exists() {
            self.attachment_error = Some("File does not exist".to_string());
        } else if !Path::new(&self.attachment_path).is_file() {
            self.attachment_error = Some("Path is not a file".to_string());
        } else {
            self.attachment_error = None;
        }
    }

    fn submit_message(&mut self, tx: &Sender<EventSend>) {
        let has_attachment = !self.attachment_path.trim().is_empty();
        if has_attachment {
            self.validate_attachment_path();
            if self.attachment_error.is_some() {
                return;
            }
        }
        if let Some((recipient, input)) = self.recipients.get_mut(self.selected_recipient) {
            let message_text = input.trim().to_string();
            let has_attachment = !self.attachment_path.trim().is_empty();
            let has_text = !message_text.is_empty(); // Fix: add this line

            if has_text || has_attachment {
                let outgoing = OutgoingMessage::new(
                    recipient.id(),
                    message_text.clone(),
                    if has_attachment {
                        Some(self.attachment_path.clone())
                    } else {
                        None
                    },
                );

                if let Ok(mut manager) = self.retry_manager.try_lock() {
                    let _message_id = manager.add_message(outgoing.clone());
                }

                if has_attachment {
                    tx.send(EventSend::SendAttachment(recipient.id(), message_text.clone(), self.attachment_path.clone()))
                        .unwrap();
                    self.attachment_path.clear();
                } else {
                    tx.send(EventSend::SendText(recipient.id(), message_text)).unwrap();
                }

                input.clear();
                self.character_index = 0;
            }
        }
    }

    // TODO: These unwraps must be handled gracefully
    fn synchronize_messages_for_selected_recipient(&mut self) {
        let recipient_id = self.recipients[self.selected_recipient].0.id();
        match recipient_id {
            RecipientId::Contact(uuid) => self
                .tx_tui
                .send(EventSend::GetMessagesForContact(uuid.to_string()))
                .unwrap(),
            RecipientId::Group(master_key) => self
                .tx_tui
                .send(EventSend::GetMessagesForGroup(master_key))
                .unwrap(),
        }
    }

    fn handle_key_event(
        &mut self,
        key: event::KeyEvent,
        tx: &Sender<EventSend>,
    ) -> io::Result<bool> {
        use CurrentScreen::*;
        match self.current_screen {
            Main => match key.code {
                KeyCode::Right | KeyCode::Char('d') => {
                    self.synchronize_messages_for_selected_recipient();
                    self.current_screen = Writing;
                }
                KeyCode::Char('q') | KeyCode::Esc => self.current_screen = Exiting,
                KeyCode::Char('e') => self.current_screen = Options,
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.selected_recipient < self.recipients.len() - 1 {
                        self.selected_recipient += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.selected_recipient > 0 {
                        self.selected_recipient -= 1;
                    }
                }
                KeyCode::Char('i') => {
                    let selected_recipient_id = self.recipients[self.selected_recipient].0.id();
                    let contact_uuid = match selected_recipient_id {
                        RecipientId::Contact(uuid) => uuid.to_string(),
                        // TODO: (@jbrs) Get group info
                        RecipientId::Group(_) => return Ok(false),
                    };
                    self.tx_tui
                        .send(EventSend::GetContactInfo(contact_uuid))
                        .unwrap();
                    self.current_screen = ContactInfo;

                    self.contact_avatar_cache = None;
                    self.contact_avatar_image = None;
                }
                _ => {}
            },
            Exiting => match key.code {
                KeyCode::Char('y') | KeyCode::Esc | KeyCode::Char('q') => return Ok(true),
                KeyCode::Char('n') => self.current_screen = Main,
                _ => {}
            },
            Writing => match key.code {
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.synchronize_messages_for_selected_recipient();
                }
                // KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                //     tx.send(EventSend::RetryFailedMessages).unwrap();
                // }
                KeyCode::Esc | KeyCode::Left => self.current_screen = Main,
                KeyCode::Tab => {
                    self.input_focus = match self.input_focus {
                        InputFocus::Message => InputFocus::Attachment,
                        InputFocus::Attachment => InputFocus::Message,
                    };
                }
                KeyCode::Enter => {
                    self.submit_message(tx);
                    self.synchronize_messages_for_selected_recipient();
                }
                KeyCode::Char(to_insert) => match self.input_focus {
                    InputFocus::Message => self.enter_char(to_insert),
                    InputFocus::Attachment => {
                        self.attachment_path.push(to_insert);
                        self.validate_attachment_path();
                    }
                },
                KeyCode::Backspace => match self.input_focus {
                    InputFocus::Message => self.delete_char(),
                    InputFocus::Attachment => {
                        self.attachment_path.pop();
                        self.validate_attachment_path();
                    }
                },

                KeyCode::Up => {
                    let recipient_id = self.recipients[self.selected_recipient].0.id();

                    let last_message = match recipient_id {
                        RecipientId::Contact(uuid) => self
                            .contact_messages
                            .get(&uuid.to_string())
                            .map(|msgs| msgs.len())
                            .unwrap_or(0),
                        RecipientId::Group(master_key) => self
                            .group_messages
                            .get(&master_key)
                            .map(|msgs| msgs.len())
                            .unwrap_or(0),
                    };

                    if last_message > 0 && self.message_selected < last_message - 1 {
                        self.message_selected += 1;
                    }
                }
                KeyCode::Down => {
                    if self.message_selected > 0 {
                        self.message_selected -= 1;
                    }
                }
                _ => {}
            },
            Options => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.current_screen = Main,
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.config_selected > 0 {
                        self.config_selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.config_selected < 1 {
                        self.config_selected += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => match self.config_selected {
                    0 => {
                        self.config.toggle_color_mode();
                        if let Err(e) = self.config.save() {
                            eprintln!("Failed to save config: {e:?}");
                        }
                    }
                    1 => {
                        self.config.toggle_show_images();
                        if let Err(e) = self.config.save() {
                            eprintln!("Failed to save config: {e:?}");
                        }
                        if !self.config.show_images {
                            self.avatar_image = None;
                        } else if self.avatar_cache.is_some() {
                            self.load_avatar();
                        }
                    }
                    2 => {
                        self.config.toggle_compact_messages();
                        if let Err(e) = self.config.save() {
                            eprintln!("Failed to save config: {e:?}");
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            ContactInfo => match key.code {
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('q') => {
                    self.current_screen = CurrentScreen::Main;
                    self.selected_contact_info = None;
                    self.contact_avatar_cache = None;
                    self.contact_avatar_image = None;
                }
                _ => {}
            },
            LinkingNewDevice => {
                match self.linking_status {
                    LinkingStatus::Linked => self.current_screen = Syncing,
                    LinkingStatus::Unlinked => {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    if Path::new(QRCODE).exists() {
                                        fs::remove_file(QRCODE)?;
                                    }

                                    //spawn thread to check if the qr was generated
                                    let tx_key_events = self.tx_thread.clone();
                                    thread::spawn(move || {
                                        handle_checking_qr_code(tx_key_events);
                                    });

                                    //spawn thread to link device
                                    let device_name = self.textarea.clone();
                                    let tx_link_device_event = self.tx_thread.clone();
                                    thread::Builder::new()
                                        .name(String::from("linking_device_thread"))
                                        .stack_size(1024 * 1024 * 8)
                                        .spawn(move || {
                                            let runtime = Builder::new_multi_thread()
                                                .thread_name("linking_device_runtime")
                                                .enable_all()
                                                .build()
                                                .unwrap();
                                            runtime.block_on(async move {
                                                handle_linking_device(
                                                    tx_link_device_event,
                                                    device_name,
                                                )
                                                .await;
                                            })
                                        })
                                        .unwrap();

                                    self.linking_status = LinkingStatus::InProgress
                                }
                                KeyCode::Backspace => {
                                    self.textarea.pop();
                                }

                                KeyCode::Esc => {
                                    self.current_screen = LinkingNewDevice;
                                }

                                KeyCode::Char(value) => self.textarea.push(value),

                                _ => {}
                            }
                        }
                    }
                    LinkingStatus::InProgress => {}
                    LinkingStatus::Error(_) => {
                        if key.kind == KeyEventKind::Press {
                            self.linking_status = LinkingStatus::Unlinked;
                        }
                    }
                }
            }
            Syncing => {}
        }
        Ok(false)
    }
}

fn is_connection_error(e: &Error) -> bool {
    let msg = e.to_string().to_lowercase();
    ["connection", "network", "websocket", "timeout"]
        .iter()
        .any(|keyword| msg.contains(keyword))
}

fn is_delivery_confirmation_timeout(e: &Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("websocket closing while waiting for a response")
        || (msg.contains("websocket closing") && msg.contains("waiting"))
        || (msg.contains("websocket closing") && msg.contains("sending"))
        || (msg.contains("timeout") && msg.contains("response"))
}

/// Spawn thread to sync contacts and to send messeges
pub async fn init_background_threads(
    tx_thread: mpsc::Sender<EventApp>,
    rx_thread: mpsc::Receiver<EventSend>,
    mut manager: Manager<SqliteStore, Registered>,
    retry_manager: Arc<Mutex<RetryManager>>,
) -> Result<()> {
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_tui(&mut manager).await?));

    // let local_pool = LocalPoolHandle::new(4);

    //spawn thread to sync contacts and new messages
    let tx_synchronization_events = tx_thread.clone();
    let new_manager = manager.clone();
    let new_contacts = Arc::clone(&current_contacts_mutex);
    thread::Builder::new()
        .name(String::from("synchronization_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("synchronization_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_synchronization(tx_synchronization_events, new_manager, new_contacts).await;
            })
        })
        .unwrap();

    //spawn thread to receive background events
    let new_manager = manager.clone();
    let rx_sending_thread = rx_thread;
    let new_contacts = Arc::clone(&current_contacts_mutex);
    let tx_status_clone = tx_thread.clone();
    let retry_manager_clone = Arc::clone(&retry_manager);
    thread::Builder::new()
        .name(String::from("background_events_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("background_events_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                handle_background_events(
                    rx_sending_thread,
                    new_manager,
                    new_contacts,
                    tx_status_clone,
                    retry_manager_clone,
                )
                .await;
            })
        })
        .unwrap();

    // Add profile fetching
    let mut profile_manager = manager.clone();
    let tx_profile = tx_thread.clone();
    thread::Builder::new()
        .name(String::from("profile_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("profile_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                if let Ok(profile) = get_profile_tui(&mut profile_manager).await {
                    let _ = tx_profile.send(EventApp::ProfileReceived(profile));
                }
                if let Ok(Some(avatar_data)) =
                    crate::profile::get_my_profile_avatar_tui(&mut profile_manager).await
                {
                    let _ = tx_profile.send(EventApp::AvatarReceived(avatar_data));
                }
                tokio::time::sleep(std::time::Duration::from_secs(100)).await;
            })
        })
        .unwrap();

    Ok(())
}

pub fn handle_input_events(tx: mpsc::Sender<EventApp>) {
    loop {
        if let Ok(event) = event::read() {
            match event {
                Event::Key(key_event) => {
                    if tx.send(EventApp::KeyInput(key_event)).is_err() {
                        eprintln!("Failed to send key event");
                        break;
                    }
                }
                Event::Resize(cols, rows) => {
                    if tx.send(EventApp::Resize(cols, rows)).is_err() {
                        eprintln!("Failed to send resize event");
                        break;
                    }
                }
                _ => {}
            }
        }
    }
}

pub async fn handle_synchronization(
    tx: mpsc::Sender<EventApp>,
    mut manager: Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
) {
    match contacts::initial_sync(&mut manager).await {
        Ok(_) => {}
        Err(e) => error!("Initial contact sync failed: {e}"),
    }
    let mut previous_contacts: Vec<Box<DisplayContact>> = Vec::new();
    let mut previous_groups: Vec<Box<DisplayGroup>> = Vec::new();
    loop {
        let new_contacts_mutex = Arc::clone(&current_contacts_mutex);

        let messages = match receive::receive_messages_tui(&mut manager, new_contacts_mutex).await {
            Ok(list) => {
                let _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                list
            }
            Err(e) => {
                if is_connection_error(&e) {
                    let _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Disconnected(
                        "Cannot receive pending messages: WiFi disconnected".to_string(),
                    )));
                }
                Vec::new()
            }
        };

        let contacts_result = contacts::list_contacts_tui(&mut manager).await;
        let groups_result = groups::list_groups_tui(&mut manager).await;

        let contacts = match contacts_result {
            Ok(list) => list,
            // TODO: (@jbrs) Handle that differently so the groups can be checked
            Err(e) => {
                error!("{e}");
                continue;
            }
        };

        let groups = match groups_result {
            Ok(list) => list,
            Err(e) => {
                error!("{e}");
                continue;
            }
        };

        let contact_displays: Vec<Box<DisplayContact>> = contacts
            .into_iter()
            .filter_map(|contact_res| {
                let contact = contact_res.ok()?;

                let uuid_str = contact.uuid.to_string();

                let display_name = if !contact.name.is_empty() {
                    contact.name
                } else if let Some(phone) = contact.phone_number {
                    phone.to_string()
                } else {
                    uuid_str.clone()
                };

                let display_contact = Box::new(DisplayContact::new(display_name, contact.uuid));

                Some(display_contact)
            })
            .collect();

        let group_displays: Vec<Box<DisplayGroup>> = groups
            .into_iter()
            .filter_map(|groups_res| {
                let (group_master_key, group) = groups_res.ok()?;

                let display_name = group.title;
                let display_group = Box::new(DisplayGroup::new(display_name, group_master_key));

                Some(display_group)
            })
            .collect();

        let contacts_differ = contact_displays != previous_contacts;
        let groups_differ = group_displays != previous_groups;

        let display_recipients: Vec<Box<dyn DisplayRecipient>> = contact_displays
            .iter()
            .cloned()
            .map(|c| c as Box<dyn DisplayRecipient>)
            .chain(
                group_displays
                    .iter()
                    .cloned()
                    .map(|g| g as Box<dyn DisplayRecipient>),
            )
            .collect();

        if contacts_differ || groups_differ {
            if tx.send(EventApp::ContactsList(display_recipients)).is_err() {
                break;
            }
            if contacts_differ {
                previous_contacts = contact_displays;
            }
            if groups_differ {
                previous_groups = group_displays;
            }
        }

        if !messages.is_empty() && tx.send(EventApp::ReceiveMessage).is_err() {}
    }
}

pub async fn handle_background_events(
    rx: Receiver<EventSend>,
    manager: Manager<SqliteStore, Registered>,
    current_contacts_mutex: AsyncContactsMap,
    tx_status: mpsc::Sender<EventApp>,
    retry_manager: Arc<Mutex<RetryManager>>,
) {
    let local_pool = LocalPoolHandle::new(4); // Add this line

    let mut retry_interval = interval(Duration::from_secs(30));
    let mut cleanup_interval = interval(Duration::from_secs(3600));

    loop {
        tokio::select! {
            _ = retry_interval.tick() => {
                handle_retry_tick(
                    &manager,
                    &tx_status,
                    &retry_manager
                ).await;
            }

            _ = cleanup_interval.tick() => {
                handle_cleanup_tick(&retry_manager).await;
            }

            event = async {
                rx.recv().ok()
            } => {
                if let Some(event) = event {
                    handle_incoming_event(
                        event,
                        &manager,
                        &current_contacts_mutex,
                        &tx_status,
                        &retry_manager,
                        &local_pool
                    ).await;
                } else {
                    break;
                }
            }
        }
    }
}


async fn handle_retry_tick(
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
) {
    let mut retry_mgr = retry_manager.lock().await;
    let messages_to_retry = retry_mgr.messages_to_retry();
    drop(retry_mgr);

    for msg in messages_to_retry {
        let result = if let Some(attachment_path) = &msg.attachment_path {
            // Fix: Handle RecipientId properly
            match &msg.recipient {
                RecipientId::Contact(uuid) => {
                    send_attachment_tui(
                        uuid.to_string(),
                        msg.text.clone(),
                        attachment_path.clone(),
                        manager.clone(),
                    ).await
                }
                RecipientId::Group(_) => {
                    // TODO: Implement group attachment sending
                    Err(anyhow::anyhow!("Group attachment sending not yet implemented"))
                }
            }
        } else {
            // Fix: Handle RecipientId properly for text messages
            match &msg.recipient {
                RecipientId::Contact(uuid) => {
                    send::contact::send_message_tui(
                        uuid.to_string(),
                        msg.text.clone(),
                        manager.clone(),
                    ).await
                }
                RecipientId::Group(master_key) => {
                    send::group::send_message_tui(
                        *master_key,
                        msg.text.clone(),
                        manager.clone(),
                    ).await
                }
            }
        };

        let mut retry_mgr = retry_manager.lock().await;
        match result {
            Ok(_) => {
                retry_mgr.mark_sent(&msg.id);
                let _ = tx_status.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
            }
            Err(e) => {
                let error_msg = e.to_string();
                retry_mgr.mark_failed(&msg.id, error_msg.clone());

                if is_connection_error(&e) {
                    let _ = tx_status.send(EventApp::NetworkStatusChanged(
                        NetworkStatus::Disconnected("Cannot send: WiFi disconnected".to_string())
                    ));
                }
            }
        }
        drop(retry_mgr);
    }
}

async fn handle_cleanup_tick(retry_manager: &Arc<Mutex<RetryManager>>) {
    let mut retry_mgr = retry_manager.lock().await;
    retry_mgr.cleanup_old_messages();
    drop(retry_mgr);
}

async fn handle_incoming_event(
    event: EventSend,
    manager: &Manager<SqliteStore, Registered>,
    current_contacts_mutex: &AsyncContactsMap,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
    local_pool: &LocalPoolHandle,
) {
    match event {
        EventSend::SendText(recipient, text) => {
            handle_send_text_event(recipient, text, manager, tx_status, retry_manager, local_pool).await;
        }

        EventSend::SendAttachment(recipient, text, attachment_path) => {
            handle_send_attachment_event(
                recipient,
                text,
                attachment_path,
                manager,
                current_contacts_mutex,
                tx_status,
                retry_manager,
                local_pool
            ).await;
        }

        EventSend::GetMessagesForContact(uuid_str) => {
            handle_get_contact_messages_event(uuid_str, manager, tx_status, local_pool).await;
        }

        EventSend::GetMessagesForGroup(master_key) => {
            handle_get_group_messages_event(master_key, manager, tx_status, local_pool).await;
        }

        EventSend::GetContactInfo(uuid_str) => {
            handle_get_contact_info_event(uuid_str, current_contacts_mutex, tx_status).await;
        }
    }
}

async fn handle_send_text_event(
    recipient: RecipientId,
    text: String,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
    local_pool: &LocalPoolHandle,
) {
    // Create retry tracking entry
    let outgoing_msg = OutgoingMessage::new(recipient.clone(), text.clone(), None);
    let message_id = {
        let mut retry_mgr = retry_manager.lock().await;
        retry_mgr.add_message(outgoing_msg)
    };

    // Clone variables for the spawned task
    let recipient_clone = recipient.clone();
    let text_clone = text.clone();
    let tx_status_clone = tx_status.clone();
    let retry_manager_clone = Arc::clone(retry_manager);
    let manager_clone = manager.clone();

    // Spawn the send operation
    local_pool.spawn_pinned(move || async move {
        // Attempt to send based on recipient type
        let send_result = match recipient_clone {
            RecipientId::Contact(uuid) => {
                send::contact::send_message_tui(
                    uuid.to_string(),
                    text_clone,
                    manager_clone,
                )
                    .await
            }
            RecipientId::Group(master_key) => {
                send::group::send_message_tui(
                    master_key,
                    text_clone,
                    manager_clone,
                )
                    .await
            }
        };

        // Handle the result and update retry manager
        match send_result {
            Ok(_) => {
                // Mark as sent in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;
                retry_mgr.mark_sent(&message_id);
                drop(retry_mgr);

                // Send success status
                let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                let _ = tx_status_clone.send(EventApp::ReceiveMessage);
            }
            Err(e) => {
                // Mark as failed in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;

                if is_delivery_confirmation_timeout(&e) {
                    retry_mgr.mark_sent(&message_id);
                    warn!("Message likely delivered despite confirmation timeout");
                } else {
                    retry_mgr.mark_failed(&message_id, e.to_string());

                    // Handle connection errors
                    if is_connection_error(&e) {
                        let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                            NetworkStatus::Disconnected("Cannot send: WiFi disconnected".to_string())
                        ));
                    } else {
                        error!("Error sending message: {e:?}");
                    }
                }
                drop(retry_mgr);
            }
        }
    });
}

async fn handle_send_attachment_event(
    recipient: RecipientId,
    text: String,
    attachment_path: String,
    manager: &Manager<SqliteStore, Registered>,
    current_contacts_mutex: &AsyncContactsMap,
    tx_status: &mpsc::Sender<EventApp>,
    retry_manager: &Arc<Mutex<RetryManager>>,
    local_pool: &LocalPoolHandle,
) {
    // Create retry tracking entry
    let outgoing_msg = OutgoingMessage::new(
        recipient.clone(),
        text.clone(),
        Some(attachment_path.clone())
    );
    let message_id = {
        let mut retry_mgr = retry_manager.lock().await;
        retry_mgr.add_message(outgoing_msg)
    };

    // Clone variables for the spawned task
    let recipient_clone = recipient.clone();
    let text_clone = text.clone();
    let attachment_path_clone = attachment_path.clone();
    let tx_status_clone = tx_status.clone();
    let retry_manager_clone = Arc::clone(retry_manager);
    let manager_clone = manager.clone();

    // Spawn the send operation
    local_pool.spawn_pinned(move || async move {
        // Handle recipient type - for now, only Contact is implemented
        let send_result = match recipient_clone {
            RecipientId::Contact(uuid) => {
                send_attachment_tui(
                    uuid.to_string(),
                    text_clone,
                    attachment_path_clone,
                    manager_clone,
                )
                    .await
            }
            RecipientId::Group(_) => {
                // TODO: Implement group attachment sending
                Err(anyhow::anyhow!("Group attachment sending not yet implemented"))
            }
        };

        // Handle the result and update retry manager
        match send_result {
            Ok(_) => {
                // Mark as sent in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;
                retry_mgr.mark_sent(&message_id);
                drop(retry_mgr);

                // Send success status
                let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                let _ = tx_status_clone.send(EventApp::ReceiveMessage);
            }
            Err(e) => {
                // Mark as failed in retry manager
                let mut retry_mgr = retry_manager_clone.lock().await;

                if is_delivery_confirmation_timeout(&e) {
                    retry_mgr.mark_sent(&message_id);
                    warn!("Message likely delivered despite confirmation timeout");
                } else {
                    retry_mgr.mark_failed(&message_id, e.to_string());

                    // Handle connection errors
                    if is_connection_error(&e) {
                        let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                            NetworkStatus::Disconnected("Cannot send: WiFi disconnected".to_string())
                        ));
                    } else {
                        error!("Error sending attachment: {e:?}");
                    }
                }
                drop(retry_mgr);
            }
        }
    });

    // Sync contacts after attachment operation
    if let Err(e) = contacts::sync_contacts_tui(
        manager.clone(),
        Arc::clone(current_contacts_mutex),
    ).await {
        error!("Failed to sync contacts: {}", e);
    }
}

async fn handle_get_contact_messages_event(
    uuid_str: String,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    local_pool: &LocalPoolHandle,
) {
    let manager_clone = manager.clone();
    let tx_status_clone = tx_status.clone();

    local_pool.spawn_pinned(move || async move {
        let result = contact::list_messages_tui(
            uuid_str.clone(),
            "0".to_string(),
            manager_clone,
        )
            .await;

        let messages = match result {
            Ok(list) => {
                let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                list
            }
            Err(e) => {
                error!("Failed to list messages: {}", e);
                if is_connection_error(&e) {
                    let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                        NetworkStatus::Disconnected(
                            "Cannot get messages from store: WiFi disconnected".to_string(),
                        ),
                    ));
                }
                Vec::new()
            }
        };

        if !messages.is_empty() {
            if let Err(e) = tx_status_clone.send(EventApp::GetContactMessageHistory(uuid_str, messages)) {
                error!("Failed to send contact message history event: {}", e);
            }
        }
    });
}

async fn handle_get_group_messages_event(
    master_key: GroupMasterKeyBytes,
    manager: &Manager<SqliteStore, Registered>,
    tx_status: &mpsc::Sender<EventApp>,
    local_pool: &LocalPoolHandle,
) {
    let manager_clone = manager.clone();
    let tx_status_clone = tx_status.clone();

    local_pool.spawn_pinned(move || async move {
        let result = receive::group::list_messages_tui(manager_clone, master_key, None).await;

        let messages = match result {
            Ok(list) => {
                let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                list
            }
            Err(e) => {
                error!("Failed to list group messages: {}", e);
                if is_connection_error(&e) {
                    let _ = tx_status_clone.send(EventApp::NetworkStatusChanged(
                        NetworkStatus::Disconnected(
                            "Cannot get messages from store: WiFi disconnected".to_string(),
                        ),
                    ));
                }
                Vec::new()
            }
        };

        if !messages.is_empty() {
            if let Err(e) = tx_status_clone.send(EventApp::GetGroupMessageHistory(master_key, messages)) {
                error!("Failed to send group message history event: {}", e);
            }
        }
    });
}

async fn handle_get_contact_info_event(
    uuid_str: String,
    current_contacts_mutex: &AsyncContactsMap,
    tx_status: &mpsc::Sender<EventApp>,
) {
    let contacts_mutex = Arc::clone(current_contacts_mutex);
    let contacts = contacts_mutex.lock().await;

    if let Ok(uuid) = uuid_str.parse()
        && let Some(contact) = contacts.get(&uuid) {
        let contact_info = ContactInfo {
            uuid: contact.uuid.to_string(),
            name: contact.name.clone(),
            phone_number: contact.phone_number.as_ref().map(|p| p.to_string()),
            verified_state: contact.verified.state,
            expire_timer: contact.expire_timer,
            has_avatar: contact.avatar.is_some(),
        };
        if let Err(e) = tx_status.send(EventApp::ContactInfoReceived(contact_info)) {
            error!("Failed to send ContactInfoReceived event: {}", e);
        }

        if let Some(ref avatar_attachment) = contact.avatar {
            let avatar_bytes: Vec<u8> = avatar_attachment.reader.to_vec();
            if let Err(e) = tx_status.send(EventApp::ContactAvatarReceived(avatar_bytes)) {
                error!("Failed to send ContactAvatarReceived event: {}", e);
            }
        }
    }
}

pub async fn handle_linking_device(tx: mpsc::Sender<EventApp>, device_name: String) {
    let result = devices::link_new_device_tui(device_name).await;

    match result {
        Ok(manager) => {
            if tx
                .send(EventApp::LinkingFinished((true, Some(manager))))
                .is_err()
            {
                eprintln!("Failed to send linking status");
            }
        }
        Err(e) => {
            let error_msg = if e.to_string().contains("connection")
                || e.to_string().contains("network")
                || e.to_string().contains("unreachable")
                || e.to_string().contains("timeout")
            {
                "Network error: Please check your WiFi connection".to_string()
            } else {
                e.to_string()
            };

            if tx.send(EventApp::LinkingError(error_msg)).is_err() {
                eprintln!("Failed to send linking error");
            }
        }
    }
}

pub fn handle_checking_qr_code(tx: mpsc::Sender<EventApp>) {
    loop {
        if Path::new(QRCODE).exists() {
            if tx.send(EventApp::QrCodeGenerated).is_err() {
                eprintln!("Failed to send QrCodeGenerated event");
            }
            break;
        }
    }
}
