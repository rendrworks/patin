//! Reusable touch keyboard geometry, state, hit-testing, and drawing.

use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, TextAlign};

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
pub enum KeyboardMode {
    Full,
    Numeric,
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

pub struct TouchKeyboard {
    mode: KeyboardMode,
    page: Page,
    shift: bool,
}

impl TouchKeyboard {
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            mode,
            page: Page::Letters,
            shift: false,
        }
    }

    pub fn key_at(&self, width: f32, height: f32, position: (f64, f64)) -> Option<Key> {
        keyboard(self.mode, self.page, self.shift, width, height)
            .into_iter()
            .find_map(|layout| layout.hit_bounds.contains(position).then_some(layout.key))
    }

    pub fn press(&mut self, key: Key) -> Option<Key> {
        match key {
            Key::Shift => {
                self.shift = !self.shift;
                None
            }
            Key::Symbols => {
                self.page = if self.page == Page::Letters {
                    Page::Symbols
                } else {
                    Page::Letters
                };
                self.shift = false;
                None
            }
            Key::Character(character) => {
                let character = if self.shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                };
                self.shift = false;
                Some(Key::Character(character))
            }
            other => Some(other),
        }
    }

    pub fn commands(&self, width: f32, height: f32, disabled: bool) -> Vec<DrawCommand> {
        keyboard(self.mode, self.page, self.shift, width, height)
            .into_iter()
            .flat_map(|layout| {
                let (background, foreground) = key_colors(layout.key, self.shift, disabled);
                [
                    DrawCommand::RoundedFill {
                        bounds: layout.visual_bounds,
                        color: background,
                        radius: if self.mode == KeyboardMode::Numeric {
                            18.0
                        } else {
                            9.0
                        },
                    },
                    DrawCommand::Text {
                        bounds: layout.visual_bounds.inset(4.0),
                        text: layout.label,
                        color: foreground,
                        font_size: if self.mode == KeyboardMode::Numeric {
                            23.0
                        } else {
                            18.0
                        },
                        line_height: if self.mode == KeyboardMode::Numeric {
                            30.0
                        } else {
                            24.0
                        },
                        family: FontFamily::SansSerif,
                        weight: FontWeight::Semibold,
                        align: TextAlign::Center,
                    },
                ]
            })
            .collect()
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
    let bottom_margin = bottom_margin(height);
    let key_size = ((width - 60.0 - gap * 2.0) / 3.0)
        .min((height * 0.51 - bottom_margin - gap * 3.0) / 4.0)
        .clamp(44.0, 72.0);
    let left = (width - (key_size * 3.0 + gap * 2.0)) / 2.0;
    let top = height - (key_size * 4.0 + gap * 3.0) - bottom_margin;
    let rows = [
        [
            ('1', Key::Character('1')),
            ('2', Key::Character('2')),
            ('3', Key::Character('3')),
        ],
        [
            ('4', Key::Character('4')),
            ('5', Key::Character('5')),
            ('6', Key::Character('6')),
        ],
        [
            ('7', Key::Character('7')),
            ('8', Key::Character('8')),
            ('9', Key::Character('9')),
        ],
        [
            ('⌫', Key::Backspace),
            ('0', Key::Character('0')),
            ('✓', Key::Enter),
        ],
    ];
    let mut keys = Vec::new();
    for (row, values) in rows.into_iter().enumerate() {
        for (column, (label, key)) in values.into_iter().enumerate() {
            let visual_bounds = Rect::new(
                left + column as f32 * (key_size + gap),
                top + row as f32 * (key_size + gap),
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
                label: label.to_string(),
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
    let top = (height - (row_height * 4.0 + gap * 3.0) - bottom_margin(height)).max(height * 0.49);
    let mut keys = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let characters: Vec<char> = row.chars().collect();
        let inset = match row_index {
            1 => keyboard_width * 0.04,
            2 => keyboard_width * 0.1,
            _ => 0.0,
        };
        let row_width = keyboard_width - inset * 2.0;
        let key_width = (row_width - gap * (characters.len() - 1) as f32) / characters.len() as f32;
        for (index, character) in characters.into_iter().enumerate() {
            let hit_bounds = Rect::new(
                keyboard_left + inset + index as f32 * (key_width + gap),
                top + row_index as f32 * (row_height + gap),
                key_width,
                row_height,
            );
            keys.push(KeyLayout {
                hit_bounds,
                visual_bounds: hit_bounds.inset(1.5),
                label: if shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                }
                .to_string(),
                key: Key::Character(character),
            });
        }
    }
    let y = top + (row_height + gap) * 3.0;
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
    for ((label, key), fraction) in labels.into_iter().zip([0.18, 0.16, 0.32, 0.16, 0.18]) {
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

fn bottom_margin(height: f32) -> f32 {
    (height * 0.11).clamp(48.0, 112.0)
}

fn key_colors(key: Key, shift: bool, disabled: bool) -> (Color, Color) {
    if disabled {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_applies_once_and_symbol_page_toggles() {
        let mut keyboard = TouchKeyboard::new(KeyboardMode::Full);
        assert_eq!(keyboard.press(Key::Shift), None);
        assert_eq!(
            keyboard.press(Key::Character('a')),
            Some(Key::Character('A'))
        );
        assert_eq!(
            keyboard.press(Key::Character('a')),
            Some(Key::Character('a'))
        );
        assert_eq!(keyboard.press(Key::Symbols), None);
        assert!(!keyboard.commands(400.0, 800.0, false).is_empty());
    }

    #[test]
    fn layouts_stay_within_common_outputs() {
        for (width, height) in [(320.0, 500.0), (509.0, 1020.0), (1920.0, 1080.0)] {
            for mode in [KeyboardMode::Numeric, KeyboardMode::Full] {
                let commands = TouchKeyboard::new(mode).commands(width, height, false);
                assert!(!commands.is_empty());
                assert!(commands.iter().all(|command| match command {
                    DrawCommand::RoundedFill { bounds, .. } | DrawCommand::Text { bounds, .. } =>
                        bounds.origin.x >= 0.0
                            && bounds.origin.y >= 0.0
                            && bounds.origin.x + bounds.size.width <= width
                            && bounds.origin.y + bounds.size.height <= height,
                    _ => true,
                }));
            }
        }
    }
}
