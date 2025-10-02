use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;

pub fn render_captcha(frame: &mut Frame, area: Rect, app: &mut App) {
    // let area = centered_rect(80, 35, area);
    let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).split(area);
    let block = Block::default().borders(Borders::ALL);
    let captcha_text = app.captcha_text_state.to_string();

    let styled_text = Text::styled(captcha_text, Style::default().fg(Color::default()));
    let input_text = Text::styled(&app.captcha_input, Style::default().fg(Color::default()));

    let paragraph = Paragraph::new(styled_text)
        .block(block.clone())
        .left_aligned()
        .wrap(Wrap { trim: false });
    let input = Paragraph::new(input_text)
        .block(block)
        .left_aligned()
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, layout[0]);
    frame.render_widget(input, layout[1]);
}
