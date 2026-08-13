//! How each key looks: the fill/label colors for every key kind, and the
//! text metrics that keep multi-letter labels inside a key sized for one.

use patin::ui::Color;

use super::{Key, KeyboardMode};

pub(crate) fn key_colors(key: Key, armed: bool, disabled: bool) -> (Color, Color) {
    if disabled {
        return (Color(38, 33, 49, 255), Color(126, 118, 140, 255));
    }
    match key {
        Key::Enter => (Color(119, 79, 174, 255), Color(255, 252, 255, 255)),
        Key::Backspace => (Color(72, 48, 65, 255), Color(245, 211, 224, 255)),
        Key::Shift | Key::Ctrl | Key::Alt if armed => {
            (Color(100, 72, 139, 255), Color(255, 252, 255, 255))
        }
        Key::Shift
        | Key::Symbols
        | Key::Space
        | Key::Ctrl
        | Key::Alt
        | Key::Tab
        | Key::Escape
        | Key::ArrowUp
        | Key::ArrowDown
        | Key::ArrowLeft
        | Key::ArrowRight => (Color(45, 39, 58, 255), Color(218, 209, 230, 255)),
        Key::Character(_) => (Color(57, 48, 75, 255), Color(250, 248, 255, 255)),
    }
}

pub(crate) fn label_metrics(mode: KeyboardMode, key: Key) -> (f32, f32) {
    if mode == KeyboardMode::Numeric {
        return (23.0, 30.0);
    }
    match key {
        // These keys carry multi-letter labels ("Ctrl", "Esc") in a row
        // sized for single characters, so they need smaller text to fit.
        Key::Tab | Key::Escape | Key::Ctrl | Key::Alt => (12.0, 16.0),
        _ => (18.0, 24.0),
    }
}
