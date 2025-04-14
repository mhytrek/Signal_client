use crate::sending_text::send_message_tui;
use crate::{contacts, create_registered_manager, devices, AsyncRegisteredManager};
use crate::paths::QRCODE;
use crate::ui::ui;
use crossterm::event;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::runtime::Builder;
use tokio::sync::RwLock;
use std::{fs, io};
use std::io::Stderr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::path::Path;
use anyhow::Result;


use std::thread;
pub enum CurrentScreen {
    Main,
    LinkingNewDevice,
    QrCode,
    Writing,
    Options,
    Exiting,
}

pub enum LinkingStatus {
    Unlinked,
    InProgress,
    Linked,
}

pub struct App {
    pub contacts: Vec<(String, String)>, // contact_name, input for this contact
    pub selected: usize,
    pub current_screen: CurrentScreen,
    pub linking_status:LinkingStatus,
    pub character_index: usize,
    pub textarea: String,

    pub tx_thread: mpsc::Sender<EventApp>,
    pub rx_tui: mpsc::Receiver<EventApp>,

    pub tx_tui: mpsc::Sender<EventSend>,
    pub rx_thread: mpsc::Receiver<EventSend>,



}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<String>),
    LinkingFinished(bool),
}
pub enum EventSend {
    SendText(String, String),

}

impl App {
    pub fn new(linking_status: LinkingStatus) -> App {
        let (tx_thread, rx_tui) = mpsc::channel();
        let (tx_tui, rx_thread) = mpsc::channel();
        App {
            linking_status,
            contacts: vec![],
            selected: 0,
            character_index: 0,
            current_screen: CurrentScreen::LinkingNewDevice,
            textarea: String::new(),

            tx_thread,
            rx_tui,
            tx_tui,
            rx_thread,
            
        }
    }

    pub async fn init(&self) -> Result<()>{
        let manager: AsyncRegisteredManager = Arc::new(RwLock::new(create_registered_manager().await?));

        let tx_contacts_events = self.tx_thread.clone();
        let new_manager = Arc::clone(&manager);
        thread::Builder::new()
            .name(String::from("contacts_thread"))
            .stack_size(1024 * 1024 * 8)
            .spawn(move || {
                let runtime = Builder::new_multi_thread()
                    .thread_name("contacts_runtime")
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    handle_contacts(tx_contacts_events, new_manager).await;
                })
            })
            .unwrap();

        // let new_manager = Arc::clone(&manager);
        // thread::Builder::new()
        //     .name(String::from("sending_thread"))
        //     .stack_size(1024 * 1024 * 8)
        //     .spawn(move || {
        //         let runtime = Builder::new_multi_thread()
        //             .thread_name("sending_runtime")
        //             .enable_all()
        //             .build()
        //             .unwrap();
        //         runtime.block_on(async move {
        //             handle_sending_messages(self.rx_thread, new_manager).await;
        //         });
        //     })
        //     .unwrap();

        Ok(())
    }



    pub(crate) async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>
    ) -> io::Result<bool> {

        let tx_key_events = self.tx_thread.clone();
        thread::spawn(move || {
            handle_key_input_events(tx_key_events);
        });

        match self.linking_status {
            LinkingStatus::Linked => {
                let _ = self.init().await;
                self.current_screen = CurrentScreen::Main;
            },
            _ => {},
        };
        


        loop {
            terminal.draw(|f| ui(f, self))?;

            if let Ok(event) = self.rx_tui.recv() {
                if self.handle_event(event, &self.tx_tui.clone()).await? {
                    return Ok(true);
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
            EventApp::ContactsList(contacts) => {
                self.contacts = contacts
                    .into_iter()
                    .map(|name| (name, String::new()))
                    .collect();
                Ok(false)
                    }
            EventApp::LinkingFinished(result) => {
                match result{
                true => {
                    self.linking_status = LinkingStatus::Linked;
                    let _ = self.init().await;
                },
                false => self.linking_status = LinkingStatus::Unlinked, 
            }
            Ok(false)
        },
        }
    }

    fn enter_char(&mut self, new_char: char) {
        if let Some((_, input)) = self.contacts.get_mut(self.selected) {
            input.push(new_char);
            self.character_index += 1;
        }
    }

    fn delete_char(&mut self) {
        if let Some((_, input)) = self.contacts.get_mut(self.selected) {
            input.pop();
            self.character_index -= 1;
        }
    }
    fn submit_message(&mut self, tx: &Sender<EventSend>) {
        if let Some((name, input)) = self.contacts.get_mut(self.selected) {
            if !input.trim().is_empty() {
                let message = input.clone();
                tx.send(EventSend::SendText(name.clone(), message)).unwrap();
                input.clear();
                self.character_index = 0;
            }
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
                KeyCode::Right | KeyCode::Char('d') => self.current_screen = Writing,
                KeyCode::Char('q') | KeyCode::Esc => self.current_screen = Exiting,
                KeyCode::Char('e') => self.current_screen = Options,
                KeyCode::Down | KeyCode::Char('s') => {
                    if self.selected < self.contacts.len() - 1 {
                        self.selected += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('w') => {
                    if self.selected > 0 {
                        self.selected -= 1;
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
                KeyCode::Esc | KeyCode::Left => self.current_screen = Main,
                KeyCode::Enter => self.submit_message(tx),
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Backspace => self.delete_char(),
                _ => {}
            },
            Options => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.current_screen = Main,
                _ => {}
            },
            LinkingNewDevice => {
                match self.linking_status{
                    LinkingStatus::Linked => self.current_screen = Main,
                    LinkingStatus::Unlinked =>{
                        print!("aaa");
                        if key.kind == KeyEventKind::Press {

                            match key.code {
                                KeyCode::Enter => {       
                                    if Path::new(QRCODE).exists(){
                                        fs::remove_file(QRCODE)?;
                                    }                     
                                        self.current_screen = QrCode;
                                    }
                                KeyCode::Backspace => {
                                            self.textarea.pop();
                                        }
                    
                                KeyCode::Esc => {
                                    self.current_screen = LinkingNewDevice;
                                }

                                KeyCode::Char(value) => {

                                    self.textarea.push(value)
                                            }
                            
                        
                                _ => {}
                            }
                        }
                    }
                    LinkingStatus::InProgress => {},
                }
                },

            QrCode => {
                match self.linking_status{
                    LinkingStatus::Linked => self.current_screen = Main,
                    LinkingStatus::Unlinked =>{
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
                                    handle_linking_device(tx_link_device_event, device_name).await;
                                })
                            })
                            .unwrap();
                    }
                    LinkingStatus::InProgress => {}
                }


            },
        }
        Ok(false)
    }
}

// impl Default for App {
//     fn default() -> Self {
//         let (tx, rx) = mpsc::channel::<EventApp>();

//         Self::new(CurrentScreen::LinkingNewDevice,tx)
//     }
// }


pub fn handle_key_input_events(tx: mpsc::Sender<EventApp>) {
    loop {
        if let Ok(event::Event::Key(key_event)) = crossterm::event::read() {
            if tx.send(EventApp::KeyInput(key_event)).is_err() {
                eprintln!("Failed to send key event");
                break;
            }
        }
    }
}

pub async fn handle_contacts(tx: mpsc::Sender<EventApp>, manager_mutex: AsyncRegisteredManager) {
    let mut previous_contacts: Vec<String> = Vec::new();

    loop {
        let new_mutex = Arc::clone(&manager_mutex);
        contacts::sync_contacts_tui(new_mutex).await.unwrap();

        let new_mutex = Arc::clone(&manager_mutex);
        let result = contacts::list_contacts_tui(new_mutex).await;

        let contacts = match result {
            Ok(list) => list,
            Err(_) => continue,
        };

        let contact_names: Vec<String> = contacts
            .into_iter()
            .filter_map(|contact| {
                // let name = contact.ok()?.name.trim().to_string();
                let name = contact.ok()?.uuid.to_string().trim().to_string();
                if name.is_empty() {
                    None
                } else {
                    Some(name)
                }
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

        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    }
}

pub async fn handle_sending_messages(
    rx: Receiver<EventSend>,
    manager_mutex: AsyncRegisteredManager,
) {
    loop {
        if let Ok(event) = rx.recv() {
            match event {
                EventSend::SendText(recipient, text) => {
                    if let Result::Err(err_mess) =
                        send_message_tui(recipient, text, Arc::clone(&manager_mutex)).await
                    {
                        println!("{:?}", err_mess)
                    }
                    contacts::sync_contacts_tui(Arc::clone(&manager_mutex))
                        .await
                        .unwrap();
                    // Need to add error handling
                }
            }
        }
    }
}

pub async fn handle_linking_device(tx: mpsc::Sender<EventApp>, device_name: String) {
    let result = devices::link_new_device(device_name,true).await;

    let success = result.is_ok();

    if tx.send(EventApp::LinkingFinished(success)).is_err() {
        eprintln!("Failed to send linking status");
    }
}

