use std::{
    fs::{self},
    path::Path,
};

use chrono::{DateTime, Local, Utc};
use qrcode::QrCode;
use ratatui::layout::Alignment;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};
use ratatui_image::{Resize, StatefulImage};
use tui_qrcode::{Colors, QrCodeWidget};

use crate::{
    app::{App, CurrentScreen, InputFocus, LinkingStatus, NetworkStatus},
    messages::receive::MessageDto,
    paths::QRCODE, ui::utils::{centered_rect_fixed_size, render_popup},
};

/// Renders 50x50 QRCode if it exists in the QRCODE path
pub fn render_qrcode(frame: &mut Frame, area: Rect) {
    if Path::new(QRCODE).exists() {
        if area.width >= 50 && area.height >= 25 {
            let url = fs::read_to_string(QRCODE).expect("failed to read from file");
            let qr_code = QrCode::new(url).expect("Failed to generate QRcode");
            let widget = QrCodeWidget::new(qr_code).colors(Colors::Inverted);
            let qr_area = centered_rect_fixed_size(50, 25, area);
            frame.render_widget(widget, qr_area);
        } else {
            let text = format!(
                "Terminal too small to show QRcode.\nMinimum window size 50x25 \n Current window size {}x{}",
                area.width, area.height
            );
            render_popup(frame, area, &text);
        }
    } else {
        let text = "Generating QR Code...";
        render_popup(frame, area, text);
    }
}

/// Renders a textarea for device name
pub fn render_textarea(frame: &mut Frame, app: &App, area: Rect) {
    let input_area = Block::default()
        .title("Input device name")
        .borders(Borders::ALL);
    let input_text = Paragraph::new(app.textarea.clone()).block(input_area);
    let area = centered_rect_fixed_size(50, 5, area);

    frame.render_widget(input_text, area);
}