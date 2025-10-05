use chrono::{DateTime, Local, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};

use crate::app::UiStatusMessage;

pub fn render_scrollbar(frame: &mut Frame, position: usize, content_length: usize, area: Rect) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);

    let mut scrollbar_state = ScrollbarState::new(content_length).position(position);

    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

/// Renders a popup with the given status message.
pub fn render_popup(frame: &mut Frame, area: Rect, status_message: &UiStatusMessage) {
    let (title, text) = match status_message {
        UiStatusMessage::Info(info_text) => ("INFO", info_text.as_str()),
        UiStatusMessage::Error(error_text) => ("ERROR", error_text.as_str()),
    };

    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title(title);

    let paragraph = Paragraph::new(Text::raw(text))
        .block(popup_block)
        .centered()
        .wrap(Wrap { trim: false });

    let popup_area = centered_rect(60, 25, area);
    frame.render_widget(paragraph, popup_area);
}


/// Renders a paragraph with given text
pub fn render_paragraph(frame: &mut Frame, area: Rect, text: &str) {
    let block = Block::default().borders(Borders::ALL);

    let styled_text = Text::styled(text, Style::default().fg(Color::default()));

    let paragraph = Paragraph::new(styled_text)
        .block(block)
        .centered()
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Creates a rectangular area centered within the given Rect
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

/// Creates a rectangular area centered within the given Rect with fixed size
pub fn centered_rect_fixed_size(width: u16, height: u16, r: Rect) -> Rect {
    let rect_width = width.min(r.width);
    let rect_height = height.min(r.height);

    let x = r.x + (r.width - rect_width) / 2;
    let y = r.y + (r.height - rect_height) / 2;

    Rect {
        x,
        y,
        width: rect_width,
        height: rect_height,
    }
}

// Takes a timestamp and transforms it to Date in a local timezone
pub fn get_local_timestamp(millis: u64) -> DateTime<Local> {
    let secs = (millis / 1000) as i64;
    let datetime_utc: DateTime<Utc> = DateTime::from_timestamp(secs, 0).expect("Invalid timestamp");

    datetime_utc.with_timezone(&Local)
}
