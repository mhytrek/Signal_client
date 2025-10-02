use std::fmt::Display;

use ratatui::layout::Position;

#[derive(Debug)]
pub struct TextState {
    pub lines: Vec<String>,
    pub selection: Option<TextSelection>,
}

impl TextState {
    pub fn new(text: &str) -> Self {
        let lines = text.split("\n").map(|line| line.to_string()).collect();
        Self {
            lines,
            selection: None,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    pub fn lines(&self) -> usize {
        self.lines.len()
    }

    pub fn line_length(&self, line_num: usize) -> usize {
        self.lines.get(line_num).map(|line| line.len()).unwrap_or(0)
    }

    pub fn set_selection(&mut self, mut cursor: Position) {
        cursor.y = u16::min((self.lines() - 1) as u16, cursor.y);
        cursor.x = u16::min(self.lines[cursor.y as usize].len() as u16, cursor.x);
        if !self.has_selection() {
            self.selection = Some(TextSelection::new(cursor, cursor));
        } else {
            self.selection.as_mut().unwrap().change_selection(cursor);
        }
    }

    pub fn selected_text(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        match &self.selection {
            Some(selection) => {
                let selection = selection.normalized();
                let first_line = selection.anchor.y as usize;
                let last_line = selection.cursor.y as usize;

                let mut selected_lines = self
                    .lines
                    .get(first_line..(last_line + 1))
                    .unwrap()
                    .to_vec();

                let selected_lines_number = selected_lines.len();

                let first_char = selection.anchor.x as usize;
                let last_char = selection.cursor.x as usize;

                let first_byte = selected_lines
                    .first()
                    .unwrap()
                    .char_indices()
                    .nth(first_char)
                    .map(|(i, _)| i)
                    .unwrap_or(0);

                let last_byte = selected_lines
                    .last()
                    .unwrap()
                    .char_indices()
                    .nth(last_char + 1)
                    .map(|(i, _)| i)
                    .unwrap_or(selected_lines.last().unwrap().len());

                selected_lines[0] = selected_lines[0][first_byte..].to_string();
                selected_lines[selected_lines_number - 1] =
                    selected_lines[selected_lines_number - 1][..last_byte].to_string();

                selected_lines.join("\n")
            }
            None => self.to_string(),
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
}

impl Display for TextState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.lines.join("\n"))
    }
}

#[derive(Debug, Clone)]
pub struct TextSelection {
    pub anchor: Position,
    pub cursor: Position,
}

impl TextSelection {
    pub fn new(anchor: Position, cursor: Position) -> Self {
        Self { anchor, cursor }
    }

    pub fn change_selection(&mut self, cursor: Position) {
        self.cursor = cursor;
    }

    /// If anchor is greater (i.e. has larger position in text) then it flips the fields.
    pub fn normalized(&self) -> Self {
        if self.anchor > self.cursor {
            Self::new(self.cursor, self.anchor)
        } else {
            self.clone()
        }
    }
}
