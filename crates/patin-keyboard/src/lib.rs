//! Reusable touch keyboard geometry, state, hit-testing, and drawing.
//!
//! This root holds the vocabulary ([`Key`], [`KeyboardMode`],
//! [`Modifiers`]) and the [`TouchKeyboard`] state machine that ties it
//! together. The details live alongside it: [`layout`] decides where keys
//! go, [`style`] how they look, and [`keymap`] how a press becomes a
//! keycode on the wire.

mod keymap;
mod layout;
mod style;

pub use keymap::{virtual_key, virtual_keymap_source};
pub use layout::footprint_height;

use patin::ui::{DrawCommand, FontFamily, FontWeight, Rect, TextAlign};

use layout::keyboard;
use style::{key_colors, label_metrics};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Character(char),
    Backspace,
    Enter,
    Shift,
    Symbols,
    Space,
    Tab,
    Escape,
    Ctrl,
    Alt,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardMode {
    Full,
    Numeric,
    /// [`Full`](Self::Full) plus a row of Esc/Tab/Ctrl/Alt/arrow keys, for
    /// typing into a terminal or editor rather than a plain text field.
    Extended,
}

/// Which held modifiers apply to a [`Key`] resolved by
/// [`TouchKeyboard::press_with_modifiers`]. `Ctrl`/`Alt` are sticky like
/// `Shift`: tapped once, they arm for exactly the next key, then release.
///
/// `shift` reports whether Shift applied to this key too (e.g. Ctrl+Shift+C).
/// It's already folded into `Key::Character`'s case by the time you see it
/// here, so a caller only needs it to distinguish combinations like
/// Ctrl+Shift+C from Ctrl+C on the wire — not to re-derive the character.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
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
    ctrl: bool,
    alt: bool,
}

impl TouchKeyboard {
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            mode,
            page: Page::Letters,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    pub fn key_at(&self, width: f32, height: f32, position: (f64, f64)) -> Option<Key> {
        keyboard(self.mode, self.page, self.shift, width, height)
            .into_iter()
            .find_map(|layout| layout.hit_bounds.contains(position).then_some(layout.key))
    }

    /// Resolves a tap, discarding which modifiers (if any) apply to it. Use
    /// [`press_with_modifiers`](Self::press_with_modifiers) when `Ctrl`/`Alt`
    /// need to reach the injected key event, e.g. to send a real Ctrl+C.
    pub fn press(&mut self, key: Key) -> Option<Key> {
        self.press_with_modifiers(key).map(|(key, _)| key)
    }

    pub fn press_with_modifiers(&mut self, key: Key) -> Option<(Key, Modifiers)> {
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
            Key::Ctrl => {
                self.ctrl = !self.ctrl;
                None
            }
            Key::Alt => {
                self.alt = !self.alt;
                None
            }
            Key::Character(character) => {
                let shift = self.shift;
                let character = if shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                };
                self.shift = false;
                Some((Key::Character(character), self.take_modifiers(shift)))
            }
            other => Some((other, self.take_modifiers(false))),
        }
    }

    fn take_modifiers(&mut self, shift: bool) -> Modifiers {
        let modifiers = Modifiers {
            shift,
            ctrl: self.ctrl,
            alt: self.alt,
        };
        self.ctrl = false;
        self.alt = false;
        modifiers
    }

    pub fn commands(&self, width: f32, height: f32, disabled: bool) -> Vec<DrawCommand> {
        keyboard(self.mode, self.page, self.shift, width, height)
            .into_iter()
            .flat_map(|layout| {
                let armed = match layout.key {
                    Key::Shift => self.shift,
                    Key::Ctrl => self.ctrl,
                    Key::Alt => self.alt,
                    _ => false,
                };
                let (background, foreground) = key_colors(layout.key, armed, disabled);
                let (font_size, line_height) = label_metrics(self.mode, layout.key);
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
                        font_size,
                        line_height,
                        family: FontFamily::SansSerif,
                        weight: FontWeight::Semibold,
                        align: TextAlign::Center,
                    },
                ]
            })
            .collect()
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
    fn ctrl_and_alt_arm_for_exactly_one_key_each() {
        let mut keyboard = TouchKeyboard::new(KeyboardMode::Extended);

        assert_eq!(keyboard.press_with_modifiers(Key::Ctrl), None);
        assert_eq!(
            keyboard.press_with_modifiers(Key::Character('c')),
            Some((
                Key::Character('c'),
                Modifiers {
                    shift: false,
                    ctrl: true,
                    alt: false
                }
            ))
        );
        // Ctrl released itself after the previous key.
        assert_eq!(
            keyboard.press_with_modifiers(Key::Character('c')),
            Some((Key::Character('c'), Modifiers::default()))
        );

        assert_eq!(keyboard.press_with_modifiers(Key::Alt), None);
        assert_eq!(
            keyboard.press_with_modifiers(Key::ArrowLeft),
            Some((
                Key::ArrowLeft,
                Modifiers {
                    shift: false,
                    ctrl: false,
                    alt: true
                }
            ))
        );

        // The plain `press` wrapper still reports just the key, ignoring
        // whichever modifiers happened to apply.
        keyboard.press_with_modifiers(Key::Ctrl);
        assert_eq!(keyboard.press(Key::Tab), Some(Key::Tab));
    }
}
