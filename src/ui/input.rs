use crate::app::{App, CurrentScreen, InputFocus};
use ratatui::widgets::Wrap;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};
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

    let input_text = &app.recipients[app.selected_recipient].1;
    let available_width = input_area_chunks[0].width.saturating_sub(2) as usize;
    let available_height = input_area_chunks[0].height.saturating_sub(2) as usize;

    let total_lines = if available_width > 0 {
        input_text.chars().count().div_ceil(available_width)
    } else {
        1
    };

    let visible_text = if total_lines > available_height {
        let lines_to_skip = total_lines - available_height;
        let chars_to_skip = lines_to_skip * available_width;
        input_text.chars().skip(chars_to_skip).collect::<String>()
    } else {
        input_text.clone()
    };

    let input_window = Paragraph::new(visible_text)
        .block(Block::default().title(input_title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    let attachment_title = match &app.attachment_error {
        Some(error) => format!("Attachment Path - ERROR: {error}"),
        None => "Attachment Path".to_string(),
    };

    let attachment_style = match &app.attachment_error {
        Some(_) => Style::default().fg(Color::Rgb(255, 77, 0)),
        None => Style::default(),
    };

    let attachment_border_style = match &app.attachment_error {
        Some(_) => Style::default().fg(Color::Rgb(255, 77, 0)),
        None => Style::default(),
    };

    let attachment_available_width = input_area_chunks[1].width.saturating_sub(2) as usize;
    let attachment_available_height = input_area_chunks[1].height.saturating_sub(2) as usize;

    let attachment_total_lines = if attachment_available_width > 0 {
        app.attachment_path.chars().count().div_ceil(attachment_available_width)
    } else {
        1
    };

    let visible_attachment = if attachment_total_lines > attachment_available_height {
        let lines_to_skip = attachment_total_lines - attachment_available_height;
        let chars_to_skip = lines_to_skip * attachment_available_width;
        app.attachment_path
            .chars()
            .skip(chars_to_skip)
            .collect::<String>()
    } else {
        app.attachment_path.clone()
    };

    let attachment_window = Paragraph::new(Text::styled(visible_attachment, attachment_style))
        .block(
            Block::default()
                .title(attachment_title)
                .borders(Borders::ALL)
                .border_style(attachment_border_style),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(input_window, input_area_chunks[0]);
    frame.render_widget(attachment_window, input_area_chunks[1]);

    if let CurrentScreen::Writing = app.current_screen {
        match app.input_focus {
            InputFocus::Message => {
                let line_in_view = total_lines.saturating_sub(available_height);
                let current_line =
                    (input_text.chars().count() / available_width).saturating_sub(line_in_view);
                let col = input_text.chars().count() % available_width;

                frame.set_cursor_position((
                    input_area_chunks[0].x + col as u16 + 1,
                    input_area_chunks[0].y + current_line as u16 + 1,
                ));
            }
            InputFocus::Attachment => {
                let line_in_view = attachment_total_lines.saturating_sub(attachment_available_height);
                let current_line = (app.attachment_path.chars().count()
                    / attachment_available_width)
                    .saturating_sub(line_in_view);
                let col = app.attachment_path.chars().count() % attachment_available_width;

                frame.set_cursor_position((
                    input_area_chunks[1].x + col as u16 + 1,
                    input_area_chunks[1].y + current_line as u16 + 1,
                ));
            }
        }
    }
}
