use crate::sending_text::send_message_tui;
use crate::ui::ui;
use crate::{contacts, AsyncRegisteredManager};
use crossterm::event;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::io::Stderr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

pub enum CurrentScreen {
    Main,
    Writing,
    Options,
    Exiting,
}

pub struct App {
    pub contacts: Vec<(String, String)>, // contact_name, input for this contact
    pub selected: usize,
    pub current_screen: CurrentScreen,
    pub character_index: usize,
}

pub enum EventApp {
    KeyInput(event::KeyEvent),
    ContactsList(Vec<String>),
}

pub enum EventSend {
    SendText(String, String),
}

impl App {
    pub fn new() -> App {
        App {
            contacts: vec![],
            selected: 0,
            current_screen: CurrentScreen::Main,
            character_index: 0,
        }
    }

    pub(crate) async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stderr>>,
        rx: Receiver<EventApp>,
        tx: Sender<EventSend>,
    ) -> io::Result<bool> {
        loop {
            terminal.draw(|f| ui(f, self))?;

            if let Ok(event) = rx.recv() {
                if self.handle_event(event, &tx)? {
                    return Ok(true);
                }
            }
        }
    }

    fn handle_event(&mut self, event: EventApp, tx: &Sender<EventSend>) -> io::Result<bool> {
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
        }
        Ok(false)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

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
