use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, CurrentScreen, InputFocus};
// renders input and attachment boxes
pub fn render_input_and_attachment(frame: &mut Frame, app: &App, vertical_chunks: &[Rect]) {
    let input_area_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)])
        .split(vertical_chunks[1]);

    let input_title = match app.quoted_message.is_some() {
        true => "Input - Replying",
        false => "Input",
    };

    let input_window = Paragraph::new(app.recipients[app.selected_recipient].1.clone())
        .block(Block::default().title(input_title).borders(Borders::ALL));

    let attachment_title = match &app.attachment_error {
        Some(error) => format!("Attachment Path - ERROR: {error}"),
        None => "Attachment Path".to_string(),
    };

    let attachment_style = match &app.attachment_error {
        Some(_) => Style::default().fg(Color::Red),
        None => Style::default(),
    };

    let attachment_border_style = match &app.attachment_error {
        Some(_) => Style::default().fg(Color::Red),
        None => Style::default(),
    };

    let attachment_window =
        Paragraph::new(Text::styled(app.attachment_path.clone(), attachment_style)).block(
            Block::default()
                .title(attachment_title)
                .borders(Borders::ALL)
                .border_style(attachment_border_style),
        );
    frame.render_widget(input_window, input_area_chunks[0]);
    frame.render_widget(attachment_window, input_area_chunks[1]);

    if let CurrentScreen::Writing = app.current_screen {
        match app.input_focus {
            InputFocus::Message => {
                frame.set_cursor_position((
                    input_area_chunks[0].x + app.character_index as u16 + 1,
                    input_area_chunks[0].y + 1,
                ));
            }
            InputFocus::Attachment => {
                frame.set_cursor_position((
                    input_area_chunks[1].x + app.attachment_path.len() as u16 + 1,
                    input_area_chunks[1].y + 1,
                ));
            }
        }
    }
}
