use chrono::{Local, Timelike};
use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, TextAlign};
use zeroize::{Zeroize, Zeroizing};

use patin_icons::IconPalette;
use patin_keyboard::TouchKeyboard;
pub use patin_keyboard::{Key, KeyboardMode};
use patin_lua::Config;

const MAX_PASSWORD_BYTES: usize = 256;

const BACKGROUND: Color = Color(15, 13, 24, 255);
const FIELD_BORDER: Color = Color(75, 62, 98, 255);
const FIELD_FILL: Color = Color(35, 30, 47, 255);
const ACCENT: Color = Color(124, 58, 237, 255);
const TEXT_BRIGHT: Color = Color(250, 248, 255, 255);
const TEXT_MUTED: Color = Color(166, 155, 184, 255);
const TEXT_DIM: Color = Color(220, 214, 232, 255);
const TEXT_PENDING: Color = Color(200, 190, 218, 255);
const TEXT_ERROR: Color = Color(232, 150, 177, 255);

/// The lock screen's palette. Defaults are the purple it shipped with; a
/// config names one colour at a time, and anything it leaves out stays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub background: Color,
    pub field_border: Color,
    pub field_fill: Color,
    pub accent: Color,
    pub bright: Color,
    pub muted: Color,
    pub dim: Color,
    pub pending: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: BACKGROUND,
            field_border: FIELD_BORDER,
            field_fill: FIELD_FILL,
            accent: ACCENT,
            bright: TEXT_BRIGHT,
            muted: TEXT_MUTED,
            dim: TEXT_DIM,
            pending: TEXT_PENDING,
            error: TEXT_ERROR,
        }
    }
}

impl Theme {
    pub fn from_config(config: &Config) -> Self {
        let mut theme = Theme::default();
        if let Some(color) = config.color(&["lock.background", "theme.background"]) {
            theme.background = color;
        }
        if let Some(color) = config.color(&["lock.accent", "theme.accent"]) {
            theme.accent = color;
        }
        if let Some(color) = config.color(&["lock.foreground", "theme.foreground"]) {
            theme.bright = color;
        }
        if let Some(color) = config.color(&["lock.muted", "theme.muted"]) {
            theme.muted = color;
        }
        if let Some(color) = config.color(&["lock.error", "theme.error"]) {
            theme.error = color;
        }
        // The field is the one part with no shared equivalent: a password box
        // is the lock's own furniture, so it is named here or not at all.
        if let Some(color) = config.color(&["lock.field_fill"]) {
            theme.field_fill = color;
        }
        if let Some(color) = config.color(&["lock.field_border"]) {
            theme.field_border = color;
        }
        theme
    }

    /// The lock's colours for the shared status strip. `background` has to be
    /// the fill the strip is drawn over, because several glyphs punch holes
    /// in it.
    pub(crate) fn status_palette(&self) -> IconPalette {
        IconPalette {
            foreground: self.bright,
            muted: self.muted,
            background: self.background,
            accent: self.accent,
            unavailable: self.error,
        }
    }
}

pub struct LockUi {
    pub password: Zeroizing<String>,
    pub verifying: bool,
    pub message: String,
    theme: Theme,
    keyboard: TouchKeyboard,
}

impl LockUi {
    pub fn new(mode: KeyboardMode, theme: Theme) -> Self {
        Self {
            password: Zeroizing::new(String::new()),
            verifying: false,
            message: String::new(),
            theme,
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
        let theme = self.theme;
        let field_color = if self.password.is_empty() {
            theme.muted
        } else {
            theme.bright
        };
        let mut commands = vec![
            fill(Rect::new(0.0, 0.0, width, height), theme.background),
            text(
                Rect::new(0.0, clock_top(height), width, 100.0),
                &current_time(),
                64.0,
                theme.bright,
            ),
            text(
                Rect::new(content_x, height * 0.25, content_width, 48.0),
                username,
                22.0,
                theme.dim,
            ),
            rounded_fill(field_bounds, theme.field_border, 18.0),
            rounded_fill(field_bounds.inset(1.0), theme.field_fill, 17.0),
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
                    theme.pending
                } else {
                    theme.error
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

/// The big clock, kept clear of the status strip on short outputs.
pub(crate) fn clock_top(height: f32) -> f32 {
    (height * 0.09).max(patin_status::STRIP_BOTTOM + 10.0)
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyboardMode, LockUi, MAX_PASSWORD_BYTES, Theme};
    use patin::ui::DrawCommand;

    #[test]
    fn the_clock_never_collides_with_the_status_strip() {
        // The strip is drawn by the shared crate at a fixed offset, so it is
        // the clock that has to move down on a short output.
        for height in [360.0, 780.0, 1000.0, 2340.0] {
            assert!(
                super::clock_top(height) >= patin_status::STRIP_BOTTOM,
                "the clock overlaps the strip at height {height}"
            );
        }
    }

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
        let mut ui = LockUi::new(KeyboardMode::Full, Theme::default());
        for _ in 0..(MAX_PASSWORD_BYTES + 20) {
            ui.press(Key::Character('a'));
        }
        assert_eq!(ui.password.len(), MAX_PASSWORD_BYTES);
        assert!(!format!("{:?}", ui.commands(500.0, 1000.0, "user")).contains(&ui.password[..]));
    }

    #[test]
    fn submitting_moves_and_clears_the_ui_secret() {
        let mut ui = LockUi::new(KeyboardMode::Full, Theme::default());
        ui.press(Key::Character('s'));
        let password = ui.take_password().unwrap();
        assert_eq!(password.as_str(), "s");
        assert!(ui.password.is_empty());
        assert!(ui.verifying);
    }

    #[test]
    fn password_hint_tracks_editing_and_verification() {
        let mut ui = LockUi::new(KeyboardMode::Full, Theme::default());
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
        let mut ui = LockUi::new(KeyboardMode::Full, Theme::default());
        ui.failed("Authentication failed".into());
        assert_eq!(ui.message, "Authentication failed");

        ui.press(Key::Shift);
        assert_eq!(ui.message, "Authentication failed");
        ui.press(Key::Character('a'));
        assert!(ui.message.is_empty());
    }

    #[test]
    fn an_empty_config_reproduces_the_palette_this_crate_shipped_with() {
        assert_eq!(
            Theme::from_config(&patin_lua::Config::empty()),
            Theme::default()
        );
    }

    #[test]
    fn a_lock_colour_beats_the_shared_one_and_the_rest_stay_put() {
        let config = patin_lua::Config::from_source(
            "init.lua",
            r##"
            patin.theme.accent = "#112233"
            patin.lock.accent = "#445566"
            patin.lock.field_fill = "#0a0a0a"
            "##,
        )
        .unwrap();
        let theme = Theme::from_config(&config);
        assert_eq!(theme.accent, super::Color(0x44, 0x55, 0x66, 255));
        assert_eq!(theme.field_fill, super::Color(0x0a, 0x0a, 0x0a, 255));
        assert_eq!(theme.background, Theme::default().background);
    }
}
