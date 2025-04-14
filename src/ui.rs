

use ratatui::{
    layout::{ Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{picker::Picker, Image, Resize};

use crate::{app::{App, CurrentScreen}, paths::QRCODE};

// Main UI rendering function.
pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)])
        .split(chunks[0]);

    match app.current_screen {
        CurrentScreen::Main => {
                render_contact_list(frame, app, main_chunks[0]);
                render_footer(frame, app, chunks[1]);
            }
        CurrentScreen::Writing => {
                render_contact_list(frame, app, main_chunks[0]);
                render_chat_and_contact(frame, app, main_chunks[1]);
                render_footer(frame, app, chunks[1]);
            }
        CurrentScreen::Options => {
                render_options(frame);
                render_footer(frame, app, chunks[1]);
            }
        CurrentScreen::Exiting => {
                render_popup(frame,frame.area(),"Would you like to quit? \n (y/n)");
            }
        CurrentScreen::LinkingNewDevice => {
            render_textarea(frame, app,frame.area());

        },
        CurrentScreen::QrCode =>{
            render_qrcode(frame, frame.area());

        },
    }
}

// Renders the contact list in the left chunk of the screen
fn render_contact_list(frame: &mut Frame, app: &App, area: Rect) {
    let list_items: Vec<ListItem> = app
        .contacts
        .iter()
        .enumerate()
        .map(|(i, (name, _id))| {
            let mut style = Style::default().fg(Color::White);
            if i == app.selected {
                style = style
                    .bg(Color::White)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
            }
            ListItem::new(name.clone()).style(style)
        })
        .collect();

    let chat_list_widget = List::new(list_items).block(
        Block::default()
            .padding(Padding::new(1, 1, 1, 1))
            .title("Chats")
            .borders(Borders::ALL),
    );

    frame.render_widget(chat_list_widget, area);
}

// Renders the chat window and input box in the right chunk of the screen
fn render_chat_and_contact(frame: &mut Frame, app: &App, area: Rect) {
    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let chat_window = Paragraph::new("Tu będzie chat :p").block(
        Block::default()
            .title(app.contacts[app.selected].0.clone())
            .borders(Borders::ALL),
    );

    let input_window = Paragraph::new(app.contacts[app.selected].1.clone())
        .block(Block::default().title("Input").borders(Borders::ALL));

    frame.render_widget(chat_window, chat_chunks[0]);
    frame.render_widget(input_window, chat_chunks[1]);

    if let CurrentScreen::Writing = app.current_screen {
        frame.set_cursor_position((
            chat_chunks[1].x + app.character_index as u16 + 1,
            chat_chunks[1].y + 1,
        ));
    }
}

// Renders a popup  with given text
fn render_popup(frame: &mut Frame, area: Rect, text: &str) {
    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let styled_text = Text::styled(
        text,
        Style::default().fg(Color::White),
    );

    let paragraph = Paragraph::new(styled_text)
        .block(popup_block)
        .centered()
        .wrap(Wrap { trim: false });

    let popup_area = centered_rect(60, 25, area);
    frame.render_widget(paragraph, popup_area);
}


// Renders the footer section at the bottom of the screen.
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let current_keys_hint = {
        match app.current_screen {
            CurrentScreen::Main => Span::styled(
                        "(q) to quit | (↑ ↓) to navigate | (→) to select chat | (e) to show more options",
                        Style::default(),
                    ),
            CurrentScreen::Writing => Span::styled(
                        "(q) to exit | (ENTER) to send",
                        Style::default(),
                    ),
            CurrentScreen::Options => Span::styled(
                        "(q) to exit | (e) to select",
                        Style::default(),
                    ),

            _ => Span::default(),
        }
    };

    let key_notes_footer =
        Paragraph::new(Line::from(current_keys_hint)).block(Block::default().borders(Borders::ALL));

    frame.render_widget(key_notes_footer, area);
}

//Renders a textarea for device name
fn render_textarea(frame: &mut Frame, app: &App,area:Rect){
    let input_area = Block::default().title("Input device name").borders(Borders::ALL);
    let input_text = Paragraph::new(app.textarea.clone()).block(input_area);
    let area = centered_rect(60, 30, area);

    frame.render_widget(input_text,area);
}

//Renders 70x70 QRCode if it exists in the QRCODE path
fn render_qrcode(frame: &mut Frame,area:Rect){

    if let Ok(image_reader) = image::ImageReader::open(QRCODE) {
        match image_reader.decode() {
            Ok(image_source) => {

                let picker = Picker::from_query_stdio().unwrap();
                let mut image_static = picker
                .new_protocol(image_source.clone(), Rect::new(0,0,70,70),Resize::Scale(None))
                .unwrap();

            let image = Image::new(&mut image_static);
            if area.width>=50 && area.height>=50{
            frame.render_widget(image, area);

            }
            else{

                let text = format!("Terminal too small to show QRcode.\nMinimum window size 50x50 \n Curren window size {}x{}", area.width, area.height);
                render_popup(frame, area, &text);
            }
            }
            Err(_e) => {
                return;
            }
        }
    } else {
        let text = "Generating QR Code...";
        render_popup(frame, area, text);
    }

}


// Renders the options screen
fn render_options(frame: &mut Frame) {
    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double);

    let exit_paragraph = Paragraph::new("lista opcji....")
        .block(popup_block)
        .centered()
        .wrap(Wrap { trim: false });

    let area = centered_rect(60, 80, frame.area());

    frame.render_widget(exit_paragraph, area);
}

// Creates a rectangular area centered within the given Rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
