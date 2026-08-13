//! The greeter composition.
//!
//! Deliberately a sibling of `patin-lock`'s lock screen rather than a copy of
//! it: same dark, centered, touch-first shape and the same shared keypad, but
//! a cooler palette and two fields instead of one. A lock screen already
//! knows who you are and only asks for a secret; a greeter has to ask *who*
//! as well, so the username is an editable field and tapping either one moves
//! the focus.

use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, TextAlign};
use zeroize::{Zeroize, Zeroizing};

use patin_keyboard::TouchKeyboard;
pub use patin_keyboard::{Key, KeyboardMode};

const MAX_USERNAME_BYTES: usize = 64;
const MAX_PASSWORD_BYTES: usize = 256;

const FIELD_HEIGHT: f32 = 52.0;
const FIELD_GAP: f32 = 12.0;
const FIELD_RADIUS: f32 = 14.0;
const SESSION_ROW_HEIGHT: f32 = 38.0;

const BACKGROUND: Color = Color(11, 15, 24, 255);
const FIELD_FILL: Color = Color(20, 27, 38, 255);
const ACCENT: Color = Color(44, 116, 126, 255);
const ACCENT_FOCUSED: Color = Color(82, 196, 186, 255);
const TEXT_BRIGHT: Color = Color(236, 244, 248, 255);
const TEXT_MUTED: Color = Color(132, 152, 168, 255);
const TEXT_PENDING: Color = Color(190, 206, 218, 255);
const TEXT_ERROR: Color = Color(232, 150, 177, 255);

/// Which field the keypad is typing into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Username,
    Password,
}

pub struct LoginUi {
    pub username: String,
    pub password: Zeroizing<String>,
    pub focus: Field,
    pub verifying: bool,
    pub message: String,
    hostname: String,
    /// The session that will be started, and whether there is more than one
    /// to choose from — with a single session the row is not worth the space
    /// or the accidental taps.
    session: String,
    selectable: bool,
    keyboard: TouchKeyboard,
}

impl LoginUi {
    pub fn new(
        mode: KeyboardMode,
        username: String,
        hostname: String,
        session: String,
        selectable: bool,
    ) -> Self {
        // A known user starts on the password field — the common case is
        // "this is my phone, let me in", not "pick an account".
        let focus = if username.is_empty() {
            Field::Username
        } else {
            Field::Password
        };
        Self {
            username,
            password: Zeroizing::new(String::new()),
            focus,
            verifying: false,
            message: String::new(),
            hostname,
            session,
            selectable,
            keyboard: TouchKeyboard::new(mode),
        }
    }

    /// Show a different session as the selected one.
    pub fn set_session(&mut self, session: String) {
        self.session = session;
    }

    /// Whether `position` hits the session row, so a tap can cycle it.
    pub fn session_at(&self, width: f32, height: f32, position: (f64, f64)) -> bool {
        self.selectable && !self.verifying && session_row(width, height).contains(position)
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
            Key::Character(character) => edited = self.push(character),
            Key::Space => edited = self.push(' '),
            Key::Backspace => {
                edited = match self.focus {
                    Field::Username => self.username.pop().is_some(),
                    Field::Password => self.password.pop().is_some(),
                };
            }
            Key::Tab => {
                self.toggle_focus();
                return true;
            }
            Key::Shift
            | Key::Symbols
            | Key::Enter
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

    fn push(&mut self, character: char) -> bool {
        match self.focus {
            Field::Username => {
                if self.username.len() + character.len_utf8() > MAX_USERNAME_BYTES {
                    return false;
                }
                self.username.push(character);
            }
            Field::Password => {
                if self.password.len() + character.len_utf8() > MAX_PASSWORD_BYTES {
                    return false;
                }
                self.password.push(character);
            }
        }
        true
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Field::Username => Field::Password,
            Field::Password => Field::Username,
        };
    }

    /// The credentials to hand to greetd, taken out of the UI so the secret
    /// does not outlive the attempt. `None` while verifying or when either
    /// field is still empty.
    pub fn take_credentials(&mut self) -> Option<(String, Zeroizing<String>)> {
        if self.verifying || self.username.is_empty() || self.password.is_empty() {
            return None;
        }
        let password = Zeroizing::new(self.password.to_string());
        self.password.zeroize();
        self.password.clear();
        self.verifying = true;
        self.message = "Signing in…".into();
        Some((self.username.clone(), password))
    }

    pub fn failed(&mut self, message: String) {
        self.verifying = false;
        self.focus = Field::Password;
        self.message = message;
    }

    pub fn key_at(&self, width: f32, height: f32, position: (f64, f64)) -> Option<Key> {
        self.keyboard.key_at(width, height, position)
    }

    /// The field under `position`, so a tap can move the focus.
    pub fn field_at(&self, width: f32, height: f32, position: (f64, f64)) -> Option<Field> {
        let (username, password) = fields(width, height);
        if username.contains(position) {
            Some(Field::Username)
        } else if password.contains(position) {
            Some(Field::Password)
        } else {
            None
        }
    }

    pub fn commands(&self, width: f32, height: f32) -> Vec<DrawCommand> {
        let (username_bounds, password_bounds) = fields(width, height);
        let content_x = username_bounds.origin.x;
        let content_width = username_bounds.size.width;

        let mut commands = vec![
            fill(Rect::new(0.0, 0.0, width, height), BACKGROUND),
            text(
                Rect::new(0.0, height * 0.10, width, 40.0),
                &self.hostname,
                26.0,
                TEXT_BRIGHT,
            ),
            text(
                Rect::new(0.0, height * 0.10 + 38.0, width, 28.0),
                "Sign in to continue",
                15.0,
                TEXT_MUTED,
            ),
        ];

        if self.selectable {
            let row = session_row(width, height);
            commands.push(rounded_fill(row, FIELD_FILL, FIELD_RADIUS));
            commands.push(text(
                row.inset(6.0),
                &format!("Session: {}  ›", self.session),
                15.0,
                TEXT_MUTED,
            ));
        }
        commands.extend(self.field(username_bounds, Field::Username));
        commands.extend(self.field(password_bounds, Field::Password));
        commands.push(text(
            Rect::new(
                content_x,
                password_bounds.origin.y + FIELD_HEIGHT + FIELD_GAP,
                content_width,
                34.0,
            ),
            &self.message,
            16.0,
            if self.verifying {
                TEXT_PENDING
            } else {
                TEXT_ERROR
            },
        ));
        commands.extend(self.keyboard.commands(width, height, self.verifying));
        commands
    }

    fn field(&self, bounds: Rect, field: Field) -> Vec<DrawCommand> {
        let focused = self.focus == field && !self.verifying;
        let (value, filled) = match field {
            Field::Username => (self.username.clone(), !self.username.is_empty()),
            Field::Password => (self.password_dots(), !self.password.is_empty()),
        };
        let label = match field {
            Field::Username => "Username",
            Field::Password => "Password",
        };
        vec![
            rounded_fill(
                bounds,
                if focused { ACCENT_FOCUSED } else { ACCENT },
                FIELD_RADIUS,
            ),
            rounded_fill(bounds.inset(1.5), FIELD_FILL, FIELD_RADIUS - 1.5),
            text(
                bounds.inset(8.0),
                if filled { &value } else { label },
                if filled { 24.0 } else { 17.0 },
                if filled { TEXT_BRIGHT } else { TEXT_MUTED },
            ),
        ]
    }

    fn password_dots(&self) -> String {
        if self.verifying {
            String::new()
        } else {
            "•".repeat(self.password.chars().count())
        }
    }
}

/// The session row, directly above the fields.
fn session_row(width: f32, height: f32) -> Rect {
    let (username, _) = fields(width, height);
    Rect::new(
        username.origin.x,
        username.origin.y - SESSION_ROW_HEIGHT - FIELD_GAP,
        username.size.width,
        SESSION_ROW_HEIGHT,
    )
}

/// The two field rects, shared by drawing and hit-testing so a tap can never
/// disagree with what is on screen.
fn fields(width: f32, height: f32) -> (Rect, Rect) {
    let content_width = (width - 32.0).clamp(0.0, 440.0);
    let content_x = (width - content_width) / 2.0;
    let top = height * 0.26;
    (
        Rect::new(content_x, top, content_width, FIELD_HEIGHT),
        Rect::new(
            content_x,
            top + FIELD_HEIGHT + FIELD_GAP,
            content_width,
            FIELD_HEIGHT,
        ),
    )
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

#[cfg(test)]
mod tests {
    use super::{Field, Key, KeyboardMode, LoginUi, MAX_PASSWORD_BYTES, fields};
    use patin::ui::DrawCommand;

    fn ui() -> LoginUi {
        LoginUi::new(
            KeyboardMode::Full,
            "sn3rt".into(),
            "fp5".into(),
            "0xin Touch Test".into(),
            true,
        )
    }

    fn text_values(ui: &LoginUi) -> Vec<String> {
        ui.commands(500.0, 1000.0)
            .into_iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_known_user_starts_on_the_password_field() {
        assert_eq!(ui().focus, Field::Password);
        let empty = LoginUi::new(
            KeyboardMode::Full,
            String::new(),
            "fp5".into(),
            "0xin Touch Test".into(),
            true,
        );
        assert_eq!(empty.focus, Field::Username);
    }

    #[test]
    fn typing_goes_to_the_focused_field_and_tab_switches() {
        let mut ui = ui();
        ui.press(Key::Character('x'));
        assert_eq!(ui.password.as_str(), "x");
        assert_eq!(ui.username, "sn3rt");

        ui.press(Key::Tab);
        assert_eq!(ui.focus, Field::Username);
        ui.press(Key::Character('y'));
        assert_eq!(ui.username, "sn3rty");
        assert_eq!(ui.password.as_str(), "x");
    }

    #[test]
    fn password_is_masked_and_bounded() {
        let mut ui = ui();
        for _ in 0..(MAX_PASSWORD_BYTES + 20) {
            ui.press(Key::Character('a'));
        }
        assert_eq!(ui.password.len(), MAX_PASSWORD_BYTES);
        assert!(!format!("{:?}", ui.commands(500.0, 1000.0)).contains(&ui.password[..]));
        assert!(text_values(&ui).iter().any(|value| value.starts_with('•')));
    }

    #[test]
    fn submitting_moves_and_clears_the_secret() {
        let mut ui = ui();
        ui.press(Key::Character('s'));
        let (username, password) = ui.take_credentials().unwrap();
        assert_eq!(username, "sn3rt");
        assert_eq!(password.as_str(), "s");
        assert!(ui.password.is_empty());
        assert!(ui.verifying);
        assert!(ui.take_credentials().is_none(), "no resubmit while verifying");
    }

    #[test]
    fn an_empty_field_blocks_submission() {
        let mut ui = LoginUi::new(
            KeyboardMode::Full,
            String::new(),
            "fp5".into(),
            "0xin Touch Test".into(),
            true,
        );
        ui.press(Key::Character('s'));
        assert!(ui.take_credentials().is_none(), "username still empty");
    }

    #[test]
    fn failure_returns_focus_to_the_password_and_clears_on_edit() {
        let mut ui = ui();
        ui.press(Key::Character('s'));
        ui.take_credentials();
        ui.failed("Authentication failed".into());
        assert!(!ui.verifying);
        assert_eq!(ui.focus, Field::Password);
        assert!(text_values(&ui).contains(&"Authentication failed".into()));

        ui.press(Key::Character('a'));
        assert!(ui.message.is_empty());
    }

    #[test]
    fn tapping_a_field_reports_it_for_focus() {
        let ui = ui();
        let (username, password) = fields(500.0, 1000.0);
        let center = |bounds: patin::ui::Rect| {
            (
                f64::from(bounds.origin.x + bounds.size.width / 2.0),
                f64::from(bounds.origin.y + bounds.size.height / 2.0),
            )
        };
        assert_eq!(
            ui.field_at(500.0, 1000.0, center(username)),
            Some(Field::Username)
        );
        assert_eq!(
            ui.field_at(500.0, 1000.0, center(password)),
            Some(Field::Password)
        );
        assert_eq!(ui.field_at(500.0, 1000.0, (5.0, 5.0)), None);
    }

    #[test]
    fn the_session_row_is_tappable_only_when_there_is_a_choice() {
        let width = 500.0;
        let height = 1000.0;
        let row = super::session_row(width, height);
        let centre = (
            f64::from(row.origin.x + row.size.width / 2.0),
            f64::from(row.origin.y + row.size.height / 2.0),
        );

        let ui = ui();
        assert!(ui.session_at(width, height, centre));
        assert!(!ui.session_at(width, height, (5.0, 5.0)));
        assert!(
            text_values(&ui)
                .iter()
                .any(|value| value.contains("0xin Touch Test")),
            "the selected session is shown"
        );

        // A single session is not a choice, so the row is neither drawn nor
        // tappable — and it must not swallow taps meant for the fields.
        let single = LoginUi::new(
            KeyboardMode::Full,
            "sn3rt".into(),
            "fp5".into(),
            "Only".into(),
            false,
        );
        assert!(!single.session_at(width, height, centre));
        assert!(!text_values(&single).iter().any(|value| value.contains("Only")));
    }

    #[test]
    fn the_session_row_never_overlaps_the_fields() {
        let (username, _) = fields(500.0, 1000.0);
        let row = super::session_row(500.0, 1000.0);
        assert!(row.origin.y + row.size.height <= username.origin.y);
        assert!(row.origin.y > 0.0);
    }

    #[test]
    fn fields_stay_clear_of_the_keypad() {
        let (_, password) = fields(500.0, 1000.0);
        let message_bottom = password.origin.y + password.size.height + 46.0;
        let keypad_top = LoginUi::new(
            KeyboardMode::Full,
            "u".into(),
            "h".into(),
            "s".into(),
            true,
        )
            .commands(500.0, 1000.0)
            .into_iter()
            .filter_map(|command| match command {
                DrawCommand::RoundedFill { bounds, .. } => Some(bounds.origin.y),
                _ => None,
            })
            .fold(f32::INFINITY, f32::min);
        assert!(
            keypad_top > 0.0,
            "expected laid-out keys, got {keypad_top}"
        );
        assert!(message_bottom < 1000.0);
    }
}
