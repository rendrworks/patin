use chrono::{Local, Timelike};
use patin::ui::{Color, DrawCommand, FontFamily, Rect, TextAlign};
use zeroize::{Zeroize, Zeroizing};

const MAX_PASSWORD_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Character(char),
    Backspace,
    Enter,
    Shift,
    Symbols,
    Space,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Letters,
    Symbols,
}

pub struct LockUi {
    pub password: Zeroizing<String>,
    pub verifying: bool,
    pub message: String,
    shift: bool,
    page: Page,
}

impl LockUi {
    pub fn new() -> Self {
        Self {
            password: Zeroizing::new(String::new()),
            verifying: false,
            message: "Enter password".into(),
            shift: false,
            page: Page::Letters,
        }
    }

    pub fn press(&mut self, key: Key) -> bool {
        if self.verifying {
            return false;
        }
        match key {
            Key::Character(mut character) => {
                if self.shift {
                    character = character.to_ascii_uppercase();
                }
                if self.password.len() + character.len_utf8() <= MAX_PASSWORD_BYTES {
                    self.password.push(character);
                }
                self.shift = false;
            }
            Key::Space if self.password.len() < MAX_PASSWORD_BYTES => self.password.push(' '),
            Key::Backspace => {
                self.password.pop();
            }
            Key::Shift => self.shift = !self.shift,
            Key::Symbols => {
                self.page = if self.page == Page::Letters {
                    Page::Symbols
                } else {
                    Page::Letters
                };
                self.shift = false;
            }
            Key::Enter | Key::Space => {}
        }
        true
    }

    pub fn take_password(&mut self) -> Option<Zeroizing<String>> {
        if self.verifying || self.password.is_empty() {
            return None;
        }
        let password = Zeroizing::new(self.password.to_string());
        self.password.zeroize();
        self.password.clear();
        self.verifying = true;
        self.message = "Verifying…".into();
        Some(password)
    }

    pub fn failed(&mut self, message: String) {
        self.verifying = false;
        self.message = message;
    }

    pub fn key_at(&self, width: f32, height: f32, position: (f64, f64)) -> Option<Key> {
        keyboard(self.page, self.shift, width, height)
            .into_iter()
            .find_map(|(bounds, _, key)| bounds.contains(position).then_some(key))
    }

    pub fn commands(&self, width: f32, height: f32, username: &str) -> Vec<DrawCommand> {
        let keyboard_top = (height * 0.54).max(360.0).min(height - 260.0);
        let mut commands = vec![
            fill(Rect::new(0.0, 0.0, width, height), Color(15, 13, 24, 255)),
            text(
                Rect::new(0.0, height * 0.09, width, 100.0),
                &current_time(),
                64.0,
                Color(250, 248, 255, 255),
            ),
            text(
                Rect::new(width * 0.1, height * 0.27, width * 0.8, 48.0),
                username,
                24.0,
                Color(220, 214, 232, 255),
            ),
            fill(
                Rect::new(width * 0.1, height * 0.34, width * 0.8, 64.0),
                Color(42, 36, 57, 255),
            ),
            text(
                Rect::new(width * 0.12, height * 0.34, width * 0.76, 64.0),
                &"•".repeat(self.password.chars().count()),
                30.0,
                Color(250, 248, 255, 255),
            ),
            text(
                Rect::new(width * 0.1, height * 0.41, width * 0.8, 42.0),
                &self.message,
                18.0,
                Color(200, 190, 218, 255),
            ),
        ];
        for (bounds, label, _) in keyboard_at(self.page, self.shift, width, height, keyboard_top) {
            commands.push(fill(bounds.inset(3.0), Color(55, 47, 72, 255)));
            commands.push(text(
                bounds.inset(5.0),
                &label,
                20.0,
                Color(250, 248, 255, 255),
            ));
        }
        commands
    }
}

fn keyboard(page: Page, shift: bool, width: f32, height: f32) -> Vec<(Rect, String, Key)> {
    let top = (height * 0.54).max(360.0).min(height - 260.0);
    keyboard_at(page, shift, width, height, top)
}

fn keyboard_at(
    page: Page,
    shift: bool,
    width: f32,
    height: f32,
    top: f32,
) -> Vec<(Rect, String, Key)> {
    let rows: &[&str] = match page {
        Page::Letters => &["qwertyuiop", "asdfghjkl", "zxcvbnm"],
        Page::Symbols => &["1234567890", "@#$%&*-+=", "!?_/:;()"],
    };
    let gap = 3.0;
    let row_height = ((height - top - 12.0) / 4.0).max(44.0);
    let mut keys = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let chars: Vec<char> = row.chars().collect();
        let inset = if row_index == 1 {
            width * 0.04
        } else if row_index == 2 {
            width * 0.1
        } else {
            0.0
        };
        let key_width = (width - inset * 2.0) / chars.len() as f32;
        for (index, character) in chars.into_iter().enumerate() {
            let shown = if shift {
                character.to_ascii_uppercase()
            } else {
                character
            };
            keys.push((
                Rect::new(
                    inset + index as f32 * key_width,
                    top + row_index as f32 * row_height,
                    key_width,
                    row_height,
                ),
                shown.to_string(),
                Key::Character(character),
            ));
        }
    }
    let y = top + row_height * 3.0;
    let parts = [0.18, 0.16, 0.32, 0.16, 0.18];
    let labels = [
        (
            if page == Page::Letters { "⇧" } else { "ABC" },
            if page == Page::Letters {
                Key::Shift
            } else {
                Key::Symbols
            },
        ),
        ("?123", Key::Symbols),
        ("space", Key::Space),
        ("⌫", Key::Backspace),
        ("enter", Key::Enter),
    ];
    let mut x = 0.0;
    for ((label, key), fraction) in labels.into_iter().zip(parts) {
        let key_width = width * fraction;
        keys.push((
            Rect::new(x + gap, y, key_width - gap * 2.0, row_height),
            label.into(),
            key,
        ));
        x += key_width;
    }
    keys
}

fn fill(bounds: Rect, color: Color) -> DrawCommand {
    DrawCommand::Fill { bounds, color }
}

fn text(bounds: Rect, value: &str, font_size: f32, color: Color) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color,
        font_size,
        line_height: font_size * 1.3,
        family: FontFamily::SansSerif,
        align: TextAlign::Center,
    }
}

fn current_time() -> String {
    let now = Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
}

#[cfg(test)]
mod tests {
    use super::{Key, LockUi, MAX_PASSWORD_BYTES};

    #[test]
    fn password_is_masked_and_bounded() {
        let mut ui = LockUi::new();
        for _ in 0..(MAX_PASSWORD_BYTES + 20) {
            ui.press(Key::Character('a'));
        }
        assert_eq!(ui.password.len(), MAX_PASSWORD_BYTES);
        assert!(!format!("{:?}", ui.commands(500.0, 1000.0, "user")).contains(&ui.password[..]));
    }

    #[test]
    fn submitting_moves_and_clears_the_ui_secret() {
        let mut ui = LockUi::new();
        ui.press(Key::Character('s'));
        let password = ui.take_password().unwrap();
        assert_eq!(password.as_str(), "s");
        assert!(ui.password.is_empty());
        assert!(ui.verifying);
    }
}
