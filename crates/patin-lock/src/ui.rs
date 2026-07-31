use chrono::{Local, Timelike};
use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, TextAlign};
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

#[derive(Clone, Debug, PartialEq)]
struct KeyLayout {
    hit_bounds: Rect,
    visual_bounds: Rect,
    label: String,
    key: Key,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardMode {
    Full,
    Numeric,
}

pub struct LockUi {
    pub password: Zeroizing<String>,
    pub verifying: bool,
    pub message: String,
    shift: bool,
    page: Page,
    mode: KeyboardMode,
}

impl LockUi {
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            password: Zeroizing::new(String::new()),
            verifying: false,
            message: String::new(),
            shift: false,
            page: Page::Letters,
            mode,
        }
    }

    pub fn press(&mut self, key: Key) -> bool {
        if self.verifying {
            return false;
        }
        let mut edited = false;
        match key {
            Key::Character(mut character) => {
                if self.shift {
                    character = character.to_ascii_uppercase();
                }
                if self.password.len() + character.len_utf8() <= MAX_PASSWORD_BYTES {
                    self.password.push(character);
                    edited = true;
                }
                self.shift = false;
            }
            Key::Space if self.password.len() < MAX_PASSWORD_BYTES => {
                self.password.push(' ');
                edited = true;
            }
            Key::Backspace => {
                edited = self.password.pop().is_some();
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
        keyboard(self.mode, self.page, self.shift, width, height)
            .into_iter()
            .find_map(|layout| layout.hit_bounds.contains(position).then_some(layout.key))
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
        for layout in keyboard(self.mode, self.page, self.shift, width, height) {
            let (background, foreground) = key_colors(layout.key, self.shift, self.verifying);
            commands.push(rounded_fill(
                layout.visual_bounds,
                background,
                if self.mode == KeyboardMode::Numeric {
                    18.0
                } else {
                    9.0
                },
            ));
            commands.push(text(
                layout.visual_bounds.inset(4.0),
                &layout.label,
                if self.mode == KeyboardMode::Numeric {
                    23.0
                } else {
                    18.0
                },
                foreground,
            ));
        }
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

fn keyboard(
    mode: KeyboardMode,
    page: Page,
    shift: bool,
    width: f32,
    height: f32,
) -> Vec<KeyLayout> {
    match mode {
        KeyboardMode::Full => keyboard_full(page, shift, width, height),
        KeyboardMode::Numeric => keyboard_numeric(width, height),
    }
}

fn keyboard_numeric(width: f32, height: f32) -> Vec<KeyLayout> {
    let gap = 14.0;
    let bottom_margin = keyboard_bottom_margin(height);
    let width_limited_size = (width - 60.0 - gap * 2.0) / 3.0;
    let height_limited_size = (height * 0.51 - bottom_margin - gap * 3.0) / 4.0;
    let key_size = width_limited_size
        .min(height_limited_size)
        .clamp(44.0, 72.0);
    let grid_width = key_size * 3.0 + gap * 2.0;
    let grid_height = key_size * 4.0 + gap * 3.0;
    let left = (width - grid_width) / 2.0;
    let top = height - grid_height - bottom_margin;
    let rows: [[(&str, Key); 3]; 4] = [
        [
            ("1", Key::Character('1')),
            ("2", Key::Character('2')),
            ("3", Key::Character('3')),
        ],
        [
            ("4", Key::Character('4')),
            ("5", Key::Character('5')),
            ("6", Key::Character('6')),
        ],
        [
            ("7", Key::Character('7')),
            ("8", Key::Character('8')),
            ("9", Key::Character('9')),
        ],
        [
            ("⌫", Key::Backspace),
            ("0", Key::Character('0')),
            ("✓", Key::Enter),
        ],
    ];
    let mut keys = Vec::new();
    for (row_index, row) in rows.into_iter().enumerate() {
        for (column_index, (label, key)) in row.into_iter().enumerate() {
            let visual_bounds = Rect::new(
                left + column_index as f32 * (key_size + gap),
                top + row_index as f32 * (key_size + gap),
                key_size,
                key_size,
            );
            keys.push(KeyLayout {
                hit_bounds: Rect::new(
                    visual_bounds.origin.x - gap / 2.0,
                    visual_bounds.origin.y - gap / 2.0,
                    key_size + gap,
                    key_size + gap,
                ),
                visual_bounds,
                label: label.into(),
                key,
            });
        }
    }
    keys
}

fn keyboard_full(page: Page, shift: bool, width: f32, height: f32) -> Vec<KeyLayout> {
    let rows: &[&str] = match page {
        Page::Letters => &["qwertyuiop", "asdfghjkl", "zxcvbnm"],
        Page::Symbols => &["1234567890", "@#$%&*-+=", "!?_/:;()"],
    };
    let gap = 5.0;
    let row_height = (height * 0.058).clamp(44.0, 52.0);
    let keyboard_width = (width - 12.0).clamp(0.0, 720.0);
    let keyboard_left = (width - keyboard_width) / 2.0;
    let keyboard_height = row_height * 4.0 + gap * 3.0;
    let top = (height - keyboard_height - keyboard_bottom_margin(height)).max(height * 0.49);
    let mut keys = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let chars: Vec<char> = row.chars().collect();
        let inset = if row_index == 1 {
            keyboard_width * 0.04
        } else if row_index == 2 {
            keyboard_width * 0.1
        } else {
            0.0
        };
        let row_width = keyboard_width - inset * 2.0;
        let key_width = (row_width - gap * (chars.len() - 1) as f32) / chars.len() as f32;
        for (index, character) in chars.into_iter().enumerate() {
            let shown = if shift {
                character.to_ascii_uppercase()
            } else {
                character
            };
            let hit_bounds = Rect::new(
                keyboard_left + inset + index as f32 * (key_width + gap),
                top + row_index as f32 * (row_height + gap),
                key_width,
                row_height,
            );
            keys.push(KeyLayout {
                hit_bounds,
                visual_bounds: hit_bounds.inset(1.5),
                label: shown.to_string(),
                key: Key::Character(character),
            });
        }
    }
    let y = top + (row_height + gap) * 3.0;
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
        ("✓", Key::Enter),
    ];
    let mut x = keyboard_left;
    for ((label, key), fraction) in labels.into_iter().zip(parts) {
        let part_width = keyboard_width * fraction;
        let hit_bounds = Rect::new(x + gap / 2.0, y, part_width - gap, row_height);
        keys.push(KeyLayout {
            hit_bounds,
            visual_bounds: hit_bounds.inset(1.5),
            label: label.into(),
            key,
        });
        x += part_width;
    }
    keys
}

fn keyboard_bottom_margin(height: f32) -> f32 {
    (height * 0.11).clamp(48.0, 112.0)
}

fn key_colors(key: Key, shift: bool, verifying: bool) -> (Color, Color) {
    if verifying {
        return (Color(38, 33, 49, 255), Color(126, 118, 140, 255));
    }
    match key {
        Key::Enter => (Color(119, 79, 174, 255), Color(255, 252, 255, 255)),
        Key::Backspace => (Color(72, 48, 65, 255), Color(245, 211, 224, 255)),
        Key::Shift if shift => (Color(100, 72, 139, 255), Color(255, 252, 255, 255)),
        Key::Shift | Key::Symbols | Key::Space => {
            (Color(45, 39, 58, 255), Color(218, 209, 230, 255))
        }
        Key::Character(_) => (Color(57, 48, 75, 255), Color(250, 248, 255, 255)),
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

    #[test]
    fn numeric_mode_only_exposes_digits_and_no_page_toggle() {
        let ui = LockUi::new(KeyboardMode::Numeric);
        let layouts = super::keyboard(
            KeyboardMode::Numeric,
            super::Page::Letters,
            false,
            400.0,
            800.0,
        );
        let keys: Vec<Key> = layouts.iter().map(|layout| layout.key).collect();
        assert!(!keys.contains(&Key::Shift));
        assert!(!keys.contains(&Key::Symbols));
        assert!(keys.contains(&Key::Character('5')));
        let backspace = layouts
            .iter()
            .find(|layout| layout.key == Key::Backspace)
            .unwrap()
            .hit_bounds;
        let position = ui
            .key_at(
                400.0,
                800.0,
                (
                    (backspace.origin.x + backspace.size.width / 2.0) as f64,
                    (backspace.origin.y + backspace.size.height / 2.0) as f64,
                ),
            )
            .expect("center of the numeric backspace should hit the key");
        assert_eq!(position, Key::Backspace);
    }

    #[test]
    fn keyboards_are_compact_centered_and_inside_common_outputs() {
        for (width, height) in [(320.0, 500.0), (509.0, 1020.0), (1920.0, 1080.0)] {
            for mode in [KeyboardMode::Numeric, KeyboardMode::Full] {
                let layouts = super::keyboard(mode, super::Page::Letters, false, width, height);
                let left = layouts
                    .iter()
                    .map(|layout| layout.visual_bounds.origin.x)
                    .fold(f32::INFINITY, f32::min);
                let right = layouts
                    .iter()
                    .map(|layout| layout.visual_bounds.origin.x + layout.visual_bounds.size.width)
                    .fold(f32::NEG_INFINITY, f32::max);
                let bottom = layouts
                    .iter()
                    .map(|layout| layout.visual_bounds.origin.y + layout.visual_bounds.size.height)
                    .fold(f32::NEG_INFINITY, f32::max);

                assert!(left >= 0.0);
                assert!(right <= width);
                assert!(bottom <= height - super::keyboard_bottom_margin(height));
                assert!(((left + right) / 2.0 - width / 2.0).abs() < 1.0);
                assert!(right - left <= 720.0);
                assert!(
                    layouts
                        .iter()
                        .all(|layout| layout.hit_bounds.size.height >= 44.0)
                );
            }
        }
    }
}
