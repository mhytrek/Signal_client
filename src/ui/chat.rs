use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::{
    app::App,
    messages::receive::MessageDto,
    ui::{
        input::render_input_and_attachment,
        utils::{get_local_timestamp, render_scrollbar},
    },
};

#[derive(Debug)]
enum Visibility {
    Full,         // fully visible
    Partial(u16), // only last N lines visible
}

// Renders the chat window and input box in the right chunk of the screen
pub fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let messages = match app
        .contact_messages
        .get(&app.contacts[app.contact_selected].0)
    {
        Some(msgs) => msgs,
        None => &Vec::<MessageDto>::new(),
    };

    if !messages.is_empty() {
        let msg_padding = 2;
        let margin = vertical_chunks[0].width.saturating_div(4);
        let available_height = vertical_chunks[0].height;
        let max_width = vertical_chunks[0]
            .width
            .saturating_sub(msg_padding * 2 + 2 + margin) as usize;
        let min_width = 21; // hardcoded date format width

        let (heights, widths) =
            calculate_message_sizes(messages, max_width, msg_padding, min_width);
        let last_visible_start = calculate_last_visible_start(&heights, available_height);
        let start_index = app.message_selected.min(last_visible_start);
        let visible_msgs = get_visible_messages(
            &heights,
            available_height,
            start_index,
            vertical_chunks[0].y,
        );

        render_messages(
            frame,
            messages,
            &visible_msgs,
            &heights,
            &widths,
            &vertical_chunks,
            app,
        );

        let content_lenght = messages.len();
        let scrollbar_position = content_lenght - app.message_selected;

        render_scrollbar(
            frame,
            scrollbar_position,
            content_lenght,
            vertical_chunks[0],
        );
    }

    render_input_and_attachment(frame, app, &vertical_chunks);
}

// calculates the heights and widths of the messages
fn calculate_message_sizes(
    messages: &[MessageDto],
    max_width: usize,
    msg_padding: u16,
    min_width: usize,
) -> (Vec<u16>, Vec<u16>) {
    let mut heights = Vec::new();
    let mut widths = Vec::new();

    for msg in messages {
        let lines = msg.text.split('\n');
        let mut total_lines = 0;
        let mut longest_line_len = 0;

        for line in lines {
            let len = line.chars().count();
            longest_line_len = longest_line_len.max(len);
            total_lines += (len / max_width + 1) as u16;
        }

        heights.push(total_lines + 2);

        let actual_width = longest_line_len
            .min(max_width)
            .saturating_add((msg_padding * 2 + 2) as usize)
            .max(min_width) as u16;

        widths.push(actual_width);
    }

    (heights, widths)
}

// calculates which message would be the las visible
fn calculate_last_visible_start(heights: &[u16], available_height: u16) -> usize {
    let mut used_height = 0;
    let mut last_visible_start = 0;

    for (idx, &h) in heights.iter().enumerate().rev() {
        if used_height + h > available_height {
            break;
        }
        last_visible_start = idx;
        used_height += h;
    }

    last_visible_start
}

// returns list of indexes of visible messages
fn get_visible_messages(
    heights: &[u16],
    available_height: u16,
    start_index: usize,
    y_start: u16,
) -> Vec<(usize, Visibility)> {
    let mut y_cursor = y_start;
    let mut visible_msgs = Vec::new();

    for (idx, &h) in heights.iter().enumerate().skip(start_index) {
        if y_cursor >= available_height {
            break;
        } else if y_cursor + h > available_height {
            let remaining = available_height.saturating_sub(y_cursor);
            visible_msgs.push((idx, Visibility::Partial(remaining)));
            break;
        } else {
            visible_msgs.push((idx, Visibility::Full));
            y_cursor += h;
        }
    }

    visible_msgs.reverse();
    visible_msgs
}

//renders visible messages
fn render_messages(
    frame: &mut Frame,
    messages: &[MessageDto],
    visible_msgs: &[(usize, Visibility)],
    heights: &[u16],
    widths: &[u16],
    vertical_chunks: &[Rect],
    app: &App,
) {
    let mut y_pos = 0;

    for &(idx, ref visibility) in visible_msgs {
        let msg = &messages[idx];
        let datetime_local = get_local_timestamp(msg.timestamp);
        let width = widths[idx];
        let mut height = heights[idx];
        let para: Paragraph;

        match visibility {
            Visibility::Full => {
                para = Paragraph::new(msg.text.clone())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .title(datetime_local.format("%Y-%m-%d %H:%M:%S").to_string()),
                    )
                    .wrap(Wrap { trim: false });
            }
            Visibility::Partial(remaining_height) => {
                let max_text_lines = remaining_height.saturating_sub(1);
                let mut buffer = Vec::new();
                let mut used_lines = 0;

                for line in msg.text.lines().rev() {
                    let mut current_line = line;
                    while !current_line.is_empty() {
                        let take = current_line
                            .chars()
                            .take(width as usize - 4)
                            .collect::<String>();

                        if used_lines >= max_text_lines {
                            break;
                        }

                        buffer.push(take.clone());
                        used_lines += 1;
                        current_line = &current_line[take.len()..];
                    }
                    if used_lines >= max_text_lines {
                        break;
                    }
                }

                buffer.reverse();
                let visible_text = buffer.join("\n");

                height = *remaining_height;
                para = Paragraph::new(visible_text)
                    .block(
                        Block::default()
                            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                            .border_type(BorderType::Rounded),
                    )
                    .wrap(Wrap { trim: false });
            }
        }

        let mut x_pos = vertical_chunks[0].x;
        if app.contacts[app.contact_selected].0 != msg.uuid.to_string() {
            x_pos = vertical_chunks[0].x + vertical_chunks[0].width - width;
        }

        let msg_area = Rect {
            x: x_pos,
            y: y_pos,
            width,
            height,
        };
        frame.render_widget(para, msg_area);

        y_pos += height;
    }
}
