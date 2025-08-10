use crate::contacts::get_contacts_tui;
use crate::messages::receive::{self, MessageDto, list_messages_tui};
use crate::messages::send::{send_attachment_tui, send_message_tui};
use crate::paths::QRCODE;
use crate::profile::get_profile_tui;
use crate::ui::ui;
use crate::{
    AsyncContactsMap, AsyncRegisteredManager, config::Config, contacts, create_registered_manager,
    devices,
};
use anyhow::{Error, Result};
use crossterm::event::{self, Event, KeyModifiers};
use crossterm::event::{KeyCode, KeyEventKind};
use presage::libsignal_service::Profile;
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
use tokio::sync::{Mutex, RwLock};

use image::ImageFormat;
use std::thread;

#[derive(PartialEq)]
pub enum CurrentScreen {
    Main,
    Syncing,
    LinkingNewDevice,
    Writing,
    Options,
    Exiting,
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

pub struct App {
    pub contacts: Vec<(String, String, String)>, // contact_uuid, contact_name, input for this contact

    pub contact_selected: usize,
    pub message_selected: usize,

    pub current_screen: CurrentScreen,
    pub linking_status: LinkingStatus,
    pub network_status: NetworkStatus,

    pub character_index: usize,
    pub textarea: String,
    pub attachment_path: String,
    pub attachment_error: Option<String>,

    pub input_focus: InputFocus,

    pub profile: Option<Profile>,

    pub avatar_cache: Option<Vec<u8>>,
    pub picker: Option<Picker>,
    pub avatar_image: Option<StatefulProtocol>,

    pub contact_messages: HashMap<String, Vec<MessageDto>>,

    pub config: Config,
    pub config_selected: usize,

    pub manager: Option<AsyncRegisteredManager>,

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
    ContactsList(Vec<(String, String)>),
    LinkingFinished(bool),
    LinkingError(String),
    NetworkStatusChanged(NetworkStatus),

    ProfileReceived(Profile),
    AvatarReceived(Vec<u8>),

    GetMessageHistory(String, Vec<MessageDto>),
    ReceiveMessage,
    QrCodeGenerated,
    Resize(u16, u16),
}
pub enum EventSend {
    SendText(String, String),
    SendAttachment(String, String, String),
    GetMessagesForContact(String),
}

impl App {
    pub fn new(linking_status: LinkingStatus) -> App {
        let (tx_thread, rx_tui) = mpsc::channel();
        let (tx_tui, rx_thread) = mpsc::channel();
        let picker = Picker::from_query_stdio().ok();
        App {
            linking_status,
            contacts: vec![],
            contact_selected: 0,
            message_selected: 0,
            character_index: 0,
            current_screen: CurrentScreen::LinkingNewDevice,
            textarea: String::new(),
            contact_messages: HashMap::new(),
            network_status: NetworkStatus::Connected,
            attachment_path: String::new(),
            attachment_error: None,
            input_focus: InputFocus::Message,

            profile: None,
            avatar_cache: None,
            picker,
            avatar_image: None,

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
                let new_manager: AsyncRegisteredManager = match create_registered_manager().await {
                    Ok(manager) => Arc::new(RwLock::new(manager)),
                    Err(_) => {
                        return Err(io::Error::other("Failed to create manager"));
                    }
                };
                let new_manager_mutex = Arc::clone(&new_manager);

                self.manager = Some(new_manager);

                if let Err(e) =
                    init_background_threads(self.tx_thread.clone(), rx, new_manager_mutex).await
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
            terminal.draw(|f| ui(f, self))?;

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
            EventApp::ContactsList(contacts) => {
                if self.current_screen == CurrentScreen::Syncing {
                    self.current_screen = CurrentScreen::Main;
                }
                // This is added because contacts change order in the contact list
                // and if that happens the same contact should remain selected
                let selected_uuid = self
                    .contacts
                    .get(self.contact_selected)
                    .map(|contact| contact.0.clone())
                    .unwrap_or_default();

                self.contacts = contacts
                    .into_iter()
                    .map(|(uuid, name)| (uuid, name, String::new()))
                    .collect();

                self.contact_selected = self
                    .contacts
                    .iter()
                    .position(|c| c.0 == selected_uuid)
                    .unwrap_or(0);
                Ok(false)
            }
            EventApp::LinkingFinished(result) => {
                match result {
                    true => {
                        self.linking_status = LinkingStatus::Linked;
                        if let Some(rx) = self.rx_thread.take() {
                            let new_manager: AsyncRegisteredManager =
                                match create_registered_manager().await {
                                    Ok(manager) => Arc::new(RwLock::new(manager)),
                                    Err(_) => {
                                        return Err(io::Error::other("Failed to create manager"));
                                    }
                                };
                            let new_manager_mutex = Arc::clone(&new_manager);

                            self.manager = Some(new_manager);

                            if let Err(e) = init_background_threads(
                                self.tx_thread.clone(),
                                rx,
                                new_manager_mutex,
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
            EventApp::GetMessageHistory(uuid_str, messages) => {
                self.contact_messages.insert(uuid_str, messages);
                self.message_selected = match self
                    .contact_messages
                    .get(&self.contacts[self.contact_selected].0)
                {
                    Some(msgs) => msgs.len().max(0),
                    None => 0,
                };
                Ok(false)
            }
            EventApp::ReceiveMessage => {
                self.synchronize_messages_for_selected_contact();
                Ok(false)
            }
            EventApp::QrCodeGenerated => Ok(false),
            EventApp::Resize(_, _) => Ok(false),
        }
    }

    fn enter_char(&mut self, new_char: char) {
        if let Some((_, _, input)) = self.contacts.get_mut(self.contact_selected) {
            input.push(new_char);
            self.character_index += 1;
        }
    }

    fn delete_char(&mut self) {
        if let Some((_, _, input)) = self.contacts.get_mut(self.contact_selected)
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
        if let Some((_uuid, name, input)) = self.contacts.get_mut(self.contact_selected) {
            let message_text = input.trim().to_string();
            let has_text = !message_text.is_empty();

            if has_text || has_attachment {
                if has_attachment {
                    tx.send(EventSend::SendAttachment(
                        name.clone(),
                        message_text.clone(),
                        self.attachment_path.clone(),
                    ))
                    .unwrap();
                    self.attachment_path.clear();
                    self.attachment_error = None;
                } else {
                    tx.send(EventSend::SendText(name.clone(), message_text.clone()))
                        .unwrap();
                }

                self.character_index = 0;

                input.clear();
            }
        }
    }

    fn synchronize_messages_for_selected_contact(&mut self) {
        self.tx_tui
            .send(EventSend::GetMessagesForContact(
                self.contacts[self.contact_selected].0.clone(),
            ))
            .unwrap();
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
                    self.synchronize_messages_for_selected_contact();
                    self.current_screen = Writing;
                }
                KeyCode::Char('q') | KeyCode::Esc => self.current_screen = Exiting,
                KeyCode::Char('e') => self.current_screen = Options,
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.contact_selected < self.contacts.len() - 1 {
                        self.contact_selected += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.contact_selected > 0 {
                        self.contact_selected -= 1;
                    }
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
                    self.synchronize_messages_for_selected_contact();
                }
                KeyCode::Esc | KeyCode::Left => self.current_screen = Main,
                KeyCode::Tab => {
                    self.input_focus = match self.input_focus {
                        InputFocus::Message => InputFocus::Attachment,
                        InputFocus::Attachment => InputFocus::Message,
                    };
                }
                KeyCode::Enter => {
                    self.submit_message(tx);
                    self.synchronize_messages_for_selected_contact();
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

                KeyCode::Down => {
                    let last_message = match self
                        .contact_messages
                        .get(&self.contacts[self.contact_selected].0)
                    {
                        Some(msgs) => msgs.len(),
                        None => 0,
                    };

                    if last_message > 0 && self.message_selected < last_message - 1 {
                        self.message_selected += 1;
                    }
                }
                KeyCode::Up => {
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
                    _ => {}
                },
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

/// spawn thread to sync contacts and to send messeges
pub async fn init_background_threads(
    tx_thread: mpsc::Sender<EventApp>,
    rx_thread: mpsc::Receiver<EventSend>,
    manager: AsyncRegisteredManager,
) -> Result<()> {
    let new_manager_mutex = Arc::clone(&manager);
    let current_contacts_mutex: AsyncContactsMap =
        Arc::new(Mutex::new(get_contacts_tui(new_manager_mutex).await?));

    //spawn thread to sync contacts and new messages
    let tx_synchronization_events = tx_thread.clone();
    let new_manager = Arc::clone(&manager);
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
    let new_manager = Arc::clone(&manager);
    let rx_sending_thread = rx_thread;
    let new_contacts = Arc::clone(&current_contacts_mutex);
    let tx_status_clone = tx_thread.clone();
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
                )
                .await;
            })
        })
        .unwrap();

    // Add profile fetching
    let profile_manager_1 = Arc::clone(&manager);
    let profile_manager_2 = Arc::clone(&manager);
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
                if let Ok(profile) = get_profile_tui(Arc::from(profile_manager_1)).await {
                    let _ = tx_profile.send(EventApp::ProfileReceived(profile));
                }
                if let Ok(Some(avatar_data)) =
                    crate::profile::get_my_profile_avatar_tui(Arc::from(profile_manager_2)).await
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
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
) {
    let mut previous_contacts: Vec<(String, String)> = Vec::new();
    loop {
        let new_mutex = Arc::clone(&manager_mutex);
        let new_contacts_mutex = Arc::clone(&current_contacts_mutex);

        let messages = match receive::receive_messages_tui(new_mutex, new_contacts_mutex).await {
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

        let new_mutex = Arc::clone(&manager_mutex);
        let result = contacts::list_contacts_tui(new_mutex).await;

        let contacts = match result {
            Ok(list) => list,
            Err(_) => continue,
        };

        let contact_names: Vec<(String, String)> = contacts
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

                Some((uuid_str, display_name))
            })
            .collect();

        if contact_names != previous_contacts {
            if tx
                .send(EventApp::ContactsList(contact_names.clone()))
                .is_err()
            {
                break;
            }

            previous_contacts = contact_names;
        }

        if !messages.is_empty() && tx.send(EventApp::ReceiveMessage).is_err() {}
    }
}

// pub async fn handle_contacts(
//     tx: mpsc::Sender<EventApp>,
//     manager_mutex: AsyncRegisteredManager,
//     current_contacts_mutex: AsyncContactsMap,
// ) {
//     let mut previous_contacts: Vec<(String, String)> = Vec::new();

//     loop {
//         let new_mutex = Arc::clone(&manager_mutex);
//         let new_contacts_mutex = Arc::clone(&current_contacts_mutex);
//         match contacts::sync_contacts_tui(new_mutex, new_contacts_mutex).await {
//             Ok(_) => {
//                 let _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
//             }
//             Err(e) => {
//                 if e.to_string().contains("connection")
//                     || e.to_string().contains("network")
//                     || e.to_string().contains("Websocket")
//                     || e.to_string().contains("timeout")
//                 {
//                     let _ = tx.send(EventApp::NetworkStatusChanged(NetworkStatus::Disconnected(
//                         "WiFi connection lost".to_string(),
//                     )));
//                 }
//             }
//         };

//         let new_mutex = Arc::clone(&manager_mutex);
//         let result = contacts::list_contacts_tui(new_mutex).await;

//         let contacts = match result {
//             Ok(list) => list,
//             Err(_) => continue,
//         };

//         let contact_names: Vec<(String, String)> = contacts
//             .into_iter()
//             .filter_map(|contact_res| {
//                 let contact = contact_res.ok()?;

//                 let uuid_str = contact.uuid.to_string();

//                 let display_name = if !contact.name.is_empty() {
//                     contact.name
//                 } else if let Some(phone) = contact.phone_number {
//                     phone.to_string()
//                 } else {
//                     uuid_str.clone()
//                 };

//                 Some((uuid_str, display_name))
//             })
//             .collect();

//         if contact_names != previous_contacts {
//             if tx
//                 .send(EventApp::ContactsList(contact_names.clone()))
//                 .is_err()
//             {
//                 break;
//             }

//             previous_contacts = contact_names;
//         }

//         tokio::time::sleep(std::time::Duration::from_secs(200)).await;
//     }
// }

pub async fn handle_background_events(
    rx: Receiver<EventSend>,
    manager_mutex: AsyncRegisteredManager,
    current_contacts_mutex: AsyncContactsMap,
    tx_status: mpsc::Sender<EventApp>,
) {
    loop {
        if let Ok(event) = rx.recv() {
            match event {
                EventSend::SendText(recipient, text) => {
                    match send_message_tui(
                        recipient.clone(),
                        text.clone(),
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await
                    {
                        Ok(_) => {
                            let _ = tx_status
                                .send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                        }
                        Err(e) => {
                            if is_connection_error(&e) {
                                let _ = tx_status.send(EventApp::NetworkStatusChanged(
                                    NetworkStatus::Disconnected(
                                        "Cannot send: WiFi disconnected".to_string(),
                                    ),
                                ));
                            } else {
                                println!("Error sending message: {e:?}");
                            }
                        }
                    }

                    let _ = contacts::sync_contacts_tui(
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await;
                }
                EventSend::SendAttachment(recipient, text, attachment_path) => {
                    match send_attachment_tui(
                        recipient.clone(),
                        text.clone(),
                        attachment_path,
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await
                    {
                        Ok(_) => {
                            let _ = tx_status
                                .send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                        }
                        Err(e) => {
                            if is_connection_error(&e) {
                                let _ = tx_status.send(EventApp::NetworkStatusChanged(
                                    NetworkStatus::Disconnected(
                                        "Cannot send: WiFi disconnected".to_string(),
                                    ),
                                ));
                            } else {
                                println!("Error sending attachment: {e:?}");
                            }
                        }
                    }

                    let _ = contacts::sync_contacts_tui(
                        Arc::clone(&manager_mutex),
                        Arc::clone(&current_contacts_mutex),
                    )
                    .await;
                }
                EventSend::GetMessagesForContact(uuid_str) => {
                    let new_mutex = Arc::clone(&manager_mutex);
                    let result =
                        list_messages_tui(uuid_str.clone(), "0".to_string(), new_mutex).await;
                    let messages = match result {
                        Ok(list) => {
                            let _ = tx_status
                                .send(EventApp::NetworkStatusChanged(NetworkStatus::Connected));
                            list
                        }
                        Err(e) => {
                            if is_connection_error(&e) {
                                let _ = tx_status.send(EventApp::NetworkStatusChanged(
                                    NetworkStatus::Disconnected(
                                        "Cannot get messages from store: WiFi disconnected"
                                            .to_string(),
                                    ),
                                ));
                            }
                            Vec::new()
                        }
                    };

                    if !messages.is_empty()
                        && tx_status
                            .send(EventApp::GetMessageHistory(uuid_str.clone(), messages))
                            .is_err()
                    {}
                }
            }
        }
    }
}

pub async fn handle_linking_device(tx: mpsc::Sender<EventApp>, device_name: String) {
    let result = devices::link_new_device_tui(device_name).await;

    match result {
        Ok(_) => {
            if tx.send(EventApp::LinkingFinished(true)).is_err() {
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
