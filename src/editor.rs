use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Default)]
pub struct Editor {
    pub text: String,
    /// A byte offset that always falls on a UTF-8 boundary.
    pub cursor: usize,
}

impl Editor {
    pub fn set(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
    }

    pub fn insert(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let text: String = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .collect();
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
    }

    pub fn position(&self) -> (usize, usize) {
        let before = &self.text[..self.cursor];
        (
            before.bytes().filter(|b| *b == b'\n').count(),
            before.rsplit('\n').next().unwrap_or("").chars().count(),
        )
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => self.cursor = self.line_start(),
                KeyCode::Char('e') => self.cursor = self.line_end(),
                KeyCode::Char('u') => {
                    self.text.clear();
                    self.cursor = 0;
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::ALT) => {
                self.insert(&c.to_string())
            }
            KeyCode::Enter => self.insert("\n"),
            KeyCode::Left => self.cursor = self.previous(),
            KeyCode::Right => self.cursor = self.next(),
            KeyCode::Home => self.cursor = self.line_start(),
            KeyCode::End => self.cursor = self.line_end(),
            KeyCode::Up => self.vertical(-1),
            KeyCode::Down => self.vertical(1),
            KeyCode::Backspace if self.cursor > 0 => {
                let previous = self.previous();
                self.text.drain(previous..self.cursor);
                self.cursor = previous;
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                self.text.drain(self.cursor..self.next());
            }
            _ => {}
        }
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn next(&self) -> usize {
        self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)
    }
    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
    }
    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i)
    }

    fn vertical(&mut self, delta: isize) {
        let (row, col) = self.position();
        let target = row.saturating_add_signed(delta);
        if let Some(line) = self.text.split('\n').nth(target) {
            let offset: usize = self
                .text
                .split('\n')
                .take(target)
                .map(|l| l.len() + 1)
                .sum();
            self.cursor = offset + line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_multibyte_text_and_moves_between_lines() {
        let mut e = Editor::default();
        e.insert("SELECT '你好'\nFROM customers");
        e.key(KeyCode::Home.into());
        e.key(KeyCode::Up.into());
        assert_eq!(e.cursor, 0);
        e.key(KeyCode::End.into());
        e.key(KeyCode::Left.into());
        e.key(KeyCode::Backspace.into());
        assert_eq!(e.text, "SELECT '你'\nFROM customers");
        e.key(KeyCode::Delete.into());
        assert_eq!(e.text, "SELECT '你\nFROM customers");
    }
    #[test]
    fn normalizes_paste_and_removes_control_sequences() {
        let mut e = Editor::default();
        e.insert("a\r\nb\rc\u{1b}\0");
        assert_eq!(e.text, "a\nb\nc");
        assert_eq!(e.cursor, e.text.len());
    }
}
