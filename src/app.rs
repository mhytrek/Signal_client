pub enum CurrentScreen {
    Main,
    Writing,
    Options,
    Exiting,
}

pub struct App {
    pub contacts: Vec<String>,
    pub selected: usize,
    pub current_screen: CurrentScreen,
}

impl App {
    pub fn new() -> App {
        App {
            contacts: vec![
                String::from("Alice"),
                String::from("Bob"),
                String::from("Charlie"),
                String::from("Diana"),
                String::from("Eve"),
            ],
            selected: 0,
            current_screen: CurrentScreen::Main,
        }
    }
}
