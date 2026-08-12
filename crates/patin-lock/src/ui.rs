use chrono::{Local, Timelike};
use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, TextAlign};
use zeroize::{Zeroize, Zeroizing};

use patin_keyboard::TouchKeyboard;
pub use patin_keyboard::{Key, KeyboardMode};

const MAX_PASSWORD_BYTES: usize = 256;

pub struct LockUi {
    pub password: Zeroizing<String>,
    pub verifying: bool,
    pub message: String,
    keyboard: TouchKeyboard,
}

impl LockUi {
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            password: Zeroizing::new(String::new()),
            verifying: false,
            message: String::new(),
            keyboard: TouchKeyboard::new(mode),
        }
    }

    pub fn press(&mut self, key: Key) -> bool {
        if self.verifying {
            return false;
        }
        let Some(key) = self.keyboard.press(key) else {
            return true;
        };
        let mut edited = false;
        match key {
            Key::Character(character) => {
                if self.password.len() + character.len_utf8() <= MAX_PASSWORD_BYTES {
                    self.password.push(character);
                    edited = true;
                }
            }
            Key::Space if self.password.len() < MAX_PASSWORD_BYTES => {
                self.password.push(' ');
                edited = true;
            }
            Key::Backspace => {
                edited = self.password.pop().is_some();
            }
            Key::Shift
            | Key::Symbols
            | Key::Enter
            | Key::Space
            | Key::Tab
            | Key::Escape
            | Key::Ctrl
            | Key::Alt
            | Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight => {}
        }
        if edited {
            self.message.clear();
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
        self.keyboard.key_at(width, height, position)
    }

    pub fn commands(&self, width: f32, height: f32, username: &str) -> Vec<DrawCommand> {
        let content_width = (width - 32.0).clamp(0.0, 440.0);
        let content_x = (width - content_width) / 2.0;
        let field_bounds = Rect::new(content_x, height * 0.33, content_width, 58.0);
        let field_value = self.password_field_text();
        let field_color = if self.password.is_empty() {
            Color(166, 155, 184, 255)
        } else {
            Color(250, 248, 255, 255)
        };
        let mut commands = vec![
            fill(Rect::new(0.0, 0.0, width, height), Color(15, 13, 24, 255)),
            text(
                Rect::new(0.0, height * 0.09, width, 100.0),
                &current_time(),
                64.0,
                Color(250, 248, 255, 255),
            ),
            text(
                Rect::new(content_x, height * 0.25, content_width, 48.0),
                username,
                22.0,
                Color(220, 214, 232, 255),
            ),
            rounded_fill(field_bounds, Color(75, 62, 98, 255), 18.0),
            rounded_fill(field_bounds.inset(1.0), Color(35, 30, 47, 255), 17.0),
            text(
                field_bounds.inset(8.0),
                &field_value,
                if self.password.is_empty() { 18.0 } else { 28.0 },
                field_color,
            ),
            text(
                Rect::new(content_x, height * 0.405, content_width, 42.0),
                &self.message,
                17.0,
                if self.verifying {
                    Color(200, 190, 218, 255)
                } else {
                    Color(232, 150, 177, 255)
                },
            ),
        ];
        commands.extend(self.keyboard.commands(width, height, self.verifying));
        commands
    }

    fn password_field_text(&self) -> String {
        if self.verifying {
            String::new()
        } else if self.password.is_empty() {
            "Enter password".into()
        } else {
            "•".repeat(self.password.chars().count())
        }
    }
}

fn fill(bounds: Rect, color: Color) -> DrawCommand {
    DrawCommand::Fill { bounds, color }
}

fn rounded_fill(bounds: Rect, color: Color, radius: f32) -> DrawCommand {
    DrawCommand::RoundedFill {
        bounds,
        color,
        radius,
    }
}

fn text(bounds: Rect, value: &str, font_size: f32, color: Color) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color,
        font_size,
        line_height: font_size * 1.3,
        family: FontFamily::SansSerif,
        weight: FontWeight::Semibold,
        align: TextAlign::Center,
    }
}

fn current_time() -> String {
    let now = Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyboardMode, LockUi, MAX_PASSWORD_BYTES};
    use patin::ui::DrawCommand;

    fn text_values(ui: &LockUi) -> Vec<String> {
        ui.commands(500.0, 1000.0, "user")
            .into_iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn password_is_masked_and_bounded() {
        let mut ui = LockUi::new(KeyboardMode::Full);
        for _ in 0..(MAX_PASSWORD_BYTES + 20) {
            ui.press(Key::Character('a'));
        }
        assert_eq!(ui.password.len(), MAX_PASSWORD_BYTES);
        assert!(!format!("{:?}", ui.commands(500.0, 1000.0, "user")).contains(&ui.password[..]));
    }

    #[test]
    fn submitting_moves_and_clears_the_ui_secret() {
        let mut ui = LockUi::new(KeyboardMode::Full);
        ui.press(Key::Character('s'));
        let password = ui.take_password().unwrap();
        assert_eq!(password.as_str(), "s");
        assert!(ui.password.is_empty());
        assert!(ui.verifying);
    }

    #[test]
    fn password_hint_tracks_editing_and_verification() {
        let mut ui = LockUi::new(KeyboardMode::Full);
        assert!(text_values(&ui).contains(&"Enter password".into()));

        ui.press(Key::Character('s'));
        let values = text_values(&ui);
        assert!(!values.contains(&"Enter password".into()));
        assert!(values.contains(&"•".into()));

        ui.take_password();
        let values = text_values(&ui);
        assert!(!values.contains(&"Enter password".into()));
        assert!(values.contains(&"Verifying…".into()));
    }

    #[test]
    fn authentication_error_clears_when_editing_resumes() {
        let mut ui = LockUi::new(KeyboardMode::Full);
        ui.failed("Authentication failed".into());
        assert_eq!(ui.message, "Authentication failed");

        ui.press(Key::Shift);
        assert_eq!(ui.message, "Authentication failed");
        ui.press(Key::Character('a'));
        assert!(ui.message.is_empty());
    }
}
