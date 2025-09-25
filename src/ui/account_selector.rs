use crate::app::AccountCreationField;
use crate::{app::App, ui::utils::render_scrollbar};
use ratatui::text::Span;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
    },
};

/// Renders the account selector screen
pub fn render_account_selector(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 80, area);
    frame.render_widget(Clear, popup_area);
    let main_block = Block::default()
        .title(" 📱 Account Manager ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.config.get_accent_color()))
        .padding(Padding::uniform(1));

    let inner_area = main_block.inner(popup_area);
    frame.render_widget(main_block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(inner_area);

    let header_text = if let Some(current) = &app.current_account {
        vec![
            Line::from(vec![
                Span::raw("Current: "),
                Span::styled(
                    current,
                    Style::default()
                        .fg(app.config.get_success_color())
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Select an account or link a new one",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "⚠ No account selected",
                Style::default().fg(app.config.get_error_color()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Create or select an account to continue",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ]
    };

    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(header, chunks[0]);

    if app.available_accounts.is_empty() {
        let no_accounts_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "📭 No accounts found",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Press 'a' to create"),
            Line::from(""),
            Line::from("Signal TUI supports multiple accounts"),
            Line::from("for easy account management"),
        ];

        let no_accounts = Paragraph::new(no_accounts_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true });

        frame.render_widget(no_accounts, chunks[1]);
    } else {
        let list_items: Vec<ListItem> = app
            .available_accounts
            .iter()
            .enumerate()
            .map(|(i, account_name)| {
                let is_current = app.current_account.as_ref() == Some(account_name);
                let is_selected = i == app.account_selected;

                let mut display_parts = vec![];

                if is_current {
                    display_parts.push(Span::styled(
                        "● ",
                        Style::default().fg(app.config.get_success_color()),
                    ));
                } else {
                    display_parts.push(Span::raw("  "));
                }

                if is_selected {
                    display_parts.push(Span::styled(
                        "▶ ",
                        Style::default().fg(app.config.get_accent_color()),
                    ));
                } else {
                    display_parts.push(Span::raw("  "));
                }

                let name_style = if is_current {
                    Style::default()
                        .fg(app.config.get_success_color())
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default()
                        .fg(app.config.get_accent_color())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.config.get_primary_color())
                };

                display_parts.push(Span::styled(account_name.clone(), name_style));

                if is_current {
                    display_parts.push(Span::styled(
                        " (active)",
                        Style::default()
                            .fg(Color::Gray)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }

                ListItem::new(Line::from(display_parts))
            })
            .collect();

        let accounts_list = List::new(list_items.clone()).block(
            Block::default()
                .title(" Available Accounts ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.config.get_primary_color()))
                .padding(Padding::horizontal(1)),
        );

        let mut list_state = ListState::default();
        list_state.select(Some(app.account_selected));

        frame.render_stateful_widget(accounts_list, chunks[1], &mut list_state);

        if list_items.len() > chunks[1].height.saturating_sub(2) as usize {
            render_scrollbar(frame, app.account_selected, list_items.len(), chunks[1]);
        }
    }

    let instructions = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("↑↓/ws", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
            Span::raw(": Navigate  "),
            Span::styled("Enter", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
            Span::raw(": Select  "),
            Span::styled("a", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
            Span::raw(": New  "),
            Span::styled("d", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
            Span::raw(": Delete  "),
            Span::styled("Esc", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
            Span::raw(": Cancel"),
        ]),
        Line::from(""),
    ];

    let instructions_paragraph = Paragraph::new(instructions)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(instructions_paragraph, chunks[2]);
}

/// Renders the account creation screen
pub fn render_account_creation(frame: &mut Frame, app: &App, area: Rect) {
    let is_first_time = app.available_accounts.is_empty();
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);
    let title = if is_first_time {
        " 🚀 Welcome to Signal TUI - Link Your First Account "
    } else {
        " 🆕 Link New Account "
    };

    let main_block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.config.get_accent_color()))
        .padding(Padding::uniform(1));

    let inner_area = main_block.inner(popup_area);
    frame.render_widget(main_block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(inner_area);

    let instructions_text = if is_first_time {
        "Let's set up your first link to Signal account!\nChoose a name for this account and device."
    } else {
        "Link a new Signal account for multi-account support"
    };

    let instructions = Paragraph::new(instructions_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.config.get_primary_color()))
        .wrap(Wrap { trim: true });

    frame.render_widget(instructions, chunks[0]);

    let account_border_color = match app.account_creation_field {
        AccountCreationField::AccountName => app.config.get_accent_color(),
        _ => Color::DarkGray,
    };

    let account_input = Paragraph::new(app.textarea.as_str())
        .style(Style::default().fg(app.config.get_primary_color()))
        .block(
            Block::default()
                .title(" Account Name ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(account_border_color)),
        );

    frame.render_widget(account_input, chunks[1]);

    let device_border_color = match app.account_creation_field {
        AccountCreationField::DeviceName => app.config.get_accent_color(),
        _ => Color::DarkGray,
    };

    let device_input = Paragraph::new(app.device_name_input.as_str())
        .style(Style::default().fg(app.config.get_primary_color()))
        .block(
            Block::default()
                .title(" Device Name (optional - press Tab to edit) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(device_border_color)),
        );

    frame.render_widget(device_input, chunks[2]);

    match app.account_creation_field {
        AccountCreationField::AccountName => {
            frame.set_cursor_position((
                chunks[1].x + app.textarea.len() as u16 + 1,
                chunks[1].y + 1,
            ));
        }
        AccountCreationField::DeviceName => {
            frame.set_cursor_position((
                chunks[2].x + app.device_name_input.len() as u16 + 1,
                chunks[2].y + 1,
            ));
        }
    }

    let (validation_msg, is_valid) = app.get_account_validation_message();
    let status_color = if is_valid {
        app.config.get_success_color()
    } else {
        app.config.get_error_color()
    };

    let status_symbol = if is_valid { "✓" } else { "✗" };
    let status_text = format!("{} {}", status_symbol, validation_msg);

    let status = Paragraph::new(status_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(status_color));

    frame.render_widget(status, chunks[3]);

    let controls_text = if is_first_time {
        vec![
            Line::from(vec![
                Span::styled("Tab", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
                Span::raw(": Switch fields  "),
                Span::styled("Enter", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
                Span::raw(": Create account"),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Tab", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
                Span::raw(": Switch fields  "),
                Span::styled("Enter", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
                Span::raw(": Create  "),
                Span::styled("Esc", Style::default().fg(app.config.get_accent_color()).add_modifier(Modifier::BOLD)),
                Span::raw(": Cancel"),
            ]),
        ]
    };

    let controls = Paragraph::new(controls_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(controls, chunks[4]);
}

/// Helper function to create a centered rectangle
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
