//! Reusable touch keyboard geometry, state, hit-testing, and drawing.

use patin::platform::VirtualKey;
use patin::ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, TextAlign};

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
        KeyboardMode::Extended => keyboard_extended(page, shift, width, height),
    }
}

/// The height a keyboard of the given mode occupies at the given width,
/// independent of any surrounding canvas. Lets a standalone surface (one
/// that *is* the keyboard, rather than embedding it above other content)
/// size itself exactly, with no dead space and no clipped keys.
pub fn footprint_height(mode: KeyboardMode, width: f32) -> f32 {
    // Row/key sizing and the bottom safe-area margin are themselves clamped
    // functions of height (so the keyboard stays usable on very short or
    // very tall screens), so "the height that exactly fits this keyboard"
    // is a fixed point, not something derivable from one arbitrary
    // reference height — the clamps can land on different values there
    // than they do at the real (much smaller) docked height, leaving dead
    // space above the keys and an undersized margin below them.
    //
    // `height - top` at a given height *is* this mode's natural footprint
    // if that height were the container (top = height - rows - margin, so
    // height - top = rows + margin) — as long as `height` is large enough
    // that `top` doesn't hit its own `.max(0.0)` safety clamp. Starting
    // from a height that's comfortably large and iterating downward keeps
    // `top` on its natural (positive) branch throughout, so this converges
    // to the fixed point in a handful of steps.
    // Measured against `visual_bounds`, not `hit_bounds`: the numeric
    // keypad's hit targets are padded outward by half a gap for touch
    // generosity, so using `hit_bounds` here would converge with the
    // *touch target* flush at y=0 while the actually-drawn key sits
    // several pixels lower — invisible slack that looks like dead space.
    let mut height = 4000.0_f32;
    for _ in 0..8 {
        let layout = keyboard(mode, Page::Letters, false, width, height);
        let top = layout
            .iter()
            .map(|key| key.visual_bounds.origin.y)
            .fold(f32::INFINITY, f32::min);
        let next = height - top;
        if (next - height).abs() < 0.01 {
            return next;
        }
        height = next;
    }
    height
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

const QWERTY_GAP: f32 = 5.0;

fn qwerty_row_height(height: f32) -> f32 {
    (height * 0.058).clamp(44.0, 52.0)
}

fn qwerty_bounds(width: f32) -> (f32, f32) {
    let keyboard_width = (width - 12.0).clamp(0.0, 720.0);
    let keyboard_left = (width - keyboard_width) / 2.0;
    (keyboard_width, keyboard_left)
}

fn keyboard_full(page: Page, shift: bool, width: f32, height: f32) -> Vec<KeyLayout> {
    let row_height = qwerty_row_height(height);
    let top = (height - (row_height * 4.0 + QWERTY_GAP * 3.0) - bottom_margin(height)).max(0.0);
    qwerty_and_bottom_rows(page, shift, width, top, row_height)
}

/// [`KeyboardMode::Full`]'s letter/symbol rows plus its bottom function row,
/// starting at `top`. Shared with [`keyboard_extended`], which stacks an
/// extra row of keys above this block instead of duplicating it.
fn qwerty_and_bottom_rows(
    page: Page,
    shift: bool,
    width: f32,
    top: f32,
    row_height: f32,
) -> Vec<KeyLayout> {
    let rows: &[&str] = match page {
        Page::Letters => &["qwertyuiop", "asdfghjkl", "zxcvbnm"],
        Page::Symbols => &["1234567890", "@#$%&*-+=", "!?_/:;()"],
    };
    let gap = QWERTY_GAP;
    let (keyboard_width, keyboard_left) = qwerty_bounds(width);
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

fn keyboard_extended(page: Page, shift: bool, width: f32, height: f32) -> Vec<KeyLayout> {
    let row_height = qwerty_row_height(height);
    let top = (height - (row_height * 5.0 + QWERTY_GAP * 4.0) - bottom_margin(height)).max(0.0);
    let mut keys = extra_keys_row(width, top, row_height);
    keys.extend(qwerty_and_bottom_rows(
        page,
        shift,
        width,
        top + row_height + QWERTY_GAP,
        row_height,
    ));
    keys
}

fn extra_keys_row(width: f32, top: f32, row_height: f32) -> Vec<KeyLayout> {
    let (keyboard_width, keyboard_left) = qwerty_bounds(width);
    let labels = [
        ("Esc", Key::Escape),
        ("Tab", Key::Tab),
        ("Ctrl", Key::Ctrl),
        ("Alt", Key::Alt),
        ("←", Key::ArrowLeft),
        ("↓", Key::ArrowDown),
        ("↑", Key::ArrowUp),
        ("→", Key::ArrowRight),
    ];
    let key_width = (keyboard_width - QWERTY_GAP * (labels.len() - 1) as f32) / labels.len() as f32;
    let mut keys = Vec::new();
    let mut x = keyboard_left;
    for (label, key) in labels {
        let hit_bounds = Rect::new(x, top, key_width, row_height);
        keys.push(KeyLayout {
            hit_bounds,
            visual_bounds: hit_bounds.inset(1.5),
            label: label.into(),
            key,
        });
        x += key_width + QWERTY_GAP;
    }
    keys
}

fn bottom_margin(height: f32) -> f32 {
    // The lower bound only ever binds for a compact, already-docked-at-the-
    // bottom-edge surface (a full screen's height*0.11 always lands well
    // above it) — there's no other content below to keep clear of there,
    // just a small touch-safety gap, not a full gesture-nav reservation.
    (height * 0.11).clamp(8.0, 112.0)
}

fn key_colors(key: Key, armed: bool, disabled: bool) -> (Color, Color) {
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

fn label_metrics(mode: KeyboardMode, key: Key) -> (f32, f32) {
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

/// The real, `evdev`-standard wire keycode and modifiers for a resolved
/// `Key`, combining any sticky Ctrl/Alt captured by
/// [`TouchKeyboard::press_with_modifiers`] with whichever real Shift bit
/// the character itself needs. `None` for keys consumed internally and
/// never emitted (`Shift`, `Symbols`).
///
/// This deliberately uses the same keycodes and (unshifted, shifted) key
/// pairs as a real "us" physical keyboard, rather than a made-up numbering:
/// a receiver tells "Ctrl+Shift+C" apart from plain "Ctrl+C" by checking
/// which modifiers were needed to select a key's *level* (XKB's "consumed
/// modifiers"), which only works if Shift is a real, held modifier on a key
/// that genuinely has two levels — exactly like physical hardware, and
/// exactly what this reproduces. It also means this keeps working even
/// against a compositor that substitutes its own default keymap instead of
/// honoring the one [`virtual_keymap_source`] uploads (some do).
pub fn virtual_key(key: Key, modifiers: Modifiers) -> Option<VirtualKey> {
    let (evdev, needs_shift) = physical_keys()
        .into_iter()
        .find_map(|(evdev, base, shifted)| {
            if base == key {
                Some((evdev, false))
            } else if shifted == Some(key) {
                Some((evdev, true))
            } else {
                None
            }
        })?;
    let mut mask = 0;
    if needs_shift {
        mask |= VirtualKey::SHIFT;
    }
    if modifiers.ctrl {
        mask |= VirtualKey::CONTROL;
    }
    if modifiers.alt {
        mask |= VirtualKey::ALT;
    }
    Some(VirtualKey {
        keycode: evdev,
        modifiers: mask,
    })
}

/// A complete, self-contained XKB keymap (`XKB_V1` text format), covering
/// every physical key [`virtual_key`] can address with the same keycodes
/// and shift levels a real "us" layout would use for them.
pub fn virtual_keymap_source() -> String {
    let mut keycodes = String::new();
    let mut symbols = String::new();
    for (evdev, base, shifted) in physical_keys() {
        let xkb_code = evdev + 8;
        keycodes.push_str(&format!("<E{evdev}> = {xkb_code};\n"));
        let base_sym = keysym_name(base);
        match shifted {
            Some(shifted_key) => {
                let shifted_sym = keysym_name(shifted_key);
                // The explicit `type=` is load-bearing: with both ONE_LEVEL
                // and TWO_LEVEL types declared, relying on the implicit
                // type-from-symbol-count inference silently collapses this
                // to one level, dropping the shifted symbol.
                symbols.push_str(&format!(
                    "key <E{evdev}> {{ type=\"TWO_LEVEL\", [ {base_sym}, {shifted_sym} ] }};\n"
                ));
            }
            None => symbols.push_str(&format!(
                "key <E{evdev}> {{ type=\"ONE_LEVEL\", [ {base_sym} ] }};\n"
            )),
        }
    }
    format!(
        "xkb_keymap {{\n\
         xkb_keycodes \"(unnamed)\" {{\n\
         minimum = 8;\n\
         maximum = 255;\n\
         {keycodes}\
         }};\n\
         xkb_types \"(unnamed)\" {{\n\
         type \"ONE_LEVEL\" {{\n\
         modifiers = none;\n\
         level_name[Level1] = \"Any\";\n\
         }};\n\
         type \"TWO_LEVEL\" {{\n\
         modifiers = Shift;\n\
         map[Shift] = Level2;\n\
         level_name[Level1] = \"Base\";\n\
         level_name[Level2] = \"Shift\";\n\
         }};\n\
         }};\n\
         xkb_compat \"(unnamed)\" {{}};\n\
         xkb_symbols \"(unnamed)\" {{\n\
         {symbols}\
         }};\n\
         }};\n"
    )
}

/// Every physical key [`virtual_key`]/[`virtual_keymap_source`] address, as
/// `(evdev keycode, unshifted Key, shifted Key)` — matching a real "us"
/// keyboard's evdev keycode assignments and shift pairing exactly, which is
/// the whole point (see [`virtual_key`]'s doc comment).
fn physical_keys() -> Vec<(u32, Key, Option<Key>)> {
    let digit_row: [(u32, char, Option<char>); 12] = [
        (2, '1', Some('!')),
        (3, '2', Some('@')),
        (4, '3', Some('#')),
        (5, '4', Some('$')),
        (6, '5', Some('%')),
        (7, '6', None), // shift-6 (^) isn't used by any layout.
        (8, '7', Some('&')),
        (9, '8', Some('*')),
        (10, '9', Some('(')),
        (11, '0', Some(')')),
        (12, '-', Some('_')),
        (13, '=', Some('+')),
    ];
    let letters: [(u32, char); 26] = [
        (16, 'q'),
        (17, 'w'),
        (18, 'e'),
        (19, 'r'),
        (20, 't'),
        (21, 'y'),
        (22, 'u'),
        (23, 'i'),
        (24, 'o'),
        (25, 'p'),
        (30, 'a'),
        (31, 's'),
        (32, 'd'),
        (33, 'f'),
        (34, 'g'),
        (35, 'h'),
        (36, 'j'),
        (37, 'k'),
        (38, 'l'),
        (44, 'z'),
        (45, 'x'),
        (46, 'c'),
        (47, 'v'),
        (48, 'b'),
        (49, 'n'),
        (50, 'm'),
    ];

    let mut keys: Vec<(u32, Key, Option<Key>)> = digit_row
        .into_iter()
        .map(|(evdev, base, shifted)| (evdev, Key::Character(base), shifted.map(Key::Character)))
        .collect();
    keys.push((39, Key::Character(';'), Some(Key::Character(':'))));
    keys.push((53, Key::Character('/'), Some(Key::Character('?'))));
    keys.extend(letters.into_iter().map(|(evdev, letter)| {
        (
            evdev,
            Key::Character(letter),
            Some(Key::Character(letter.to_ascii_uppercase())),
        )
    }));
    keys.push((1, Key::Escape, None));
    keys.push((14, Key::Backspace, None));
    keys.push((15, Key::Tab, None));
    keys.push((28, Key::Enter, None));
    keys.push((57, Key::Space, None));
    keys.push((103, Key::ArrowUp, None));
    keys.push((105, Key::ArrowLeft, None));
    keys.push((106, Key::ArrowRight, None));
    keys.push((108, Key::ArrowDown, None));
    keys
}

fn keysym_name(key: Key) -> String {
    match key {
        Key::Character(character) => character_keysym_name(character),
        Key::Backspace => "BackSpace".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::Enter => "Return".to_string(),
        Key::Space => "space".to_string(),
        Key::Escape => "Escape".to_string(),
        Key::ArrowUp => "Up".to_string(),
        Key::ArrowDown => "Down".to_string(),
        Key::ArrowLeft => "Left".to_string(),
        Key::ArrowRight => "Right".to_string(),
        Key::Shift | Key::Symbols | Key::Ctrl | Key::Alt => {
            unreachable!("{key:?} is a modifier, never placed in the physical key table")
        }
    }
}

fn character_keysym_name(character: char) -> String {
    match character {
        'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_string(),
        '!' => "exclam".to_string(),
        '#' => "numbersign".to_string(),
        '$' => "dollar".to_string(),
        '%' => "percent".to_string(),
        '&' => "ampersand".to_string(),
        '(' => "parenleft".to_string(),
        ')' => "parenright".to_string(),
        '*' => "asterisk".to_string(),
        '+' => "plus".to_string(),
        '-' => "minus".to_string(),
        '/' => "slash".to_string(),
        ':' => "colon".to_string(),
        ';' => "semicolon".to_string(),
        '=' => "equal".to_string(),
        '?' => "question".to_string(),
        '@' => "at".to_string(),
        '_' => "underscore".to_string(),
        other => unreachable!("no XKB keysym mapping for character {other:?}"),
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
        for (width, height) in [
            (320.0, 500.0),
            (509.0, 1020.0),
            (1920.0, 1080.0),
            (400.0, 360.0),
        ] {
            for mode in [
                KeyboardMode::Numeric,
                KeyboardMode::Full,
                KeyboardMode::Extended,
            ] {
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

    #[test]
    fn footprint_height_fits_a_standalone_surface_exactly() {
        for width in [320.0, 400.0, 509.0, 1080.0] {
            for mode in [
                KeyboardMode::Numeric,
                KeyboardMode::Full,
                KeyboardMode::Extended,
            ] {
                let height = footprint_height(mode, width);
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

                // No dead space above the first row: the clamps that decide
                // row/key height and the bottom margin must be evaluated at
                // this *real* footprint height, not at some other height,
                // or the keys land well below y=0 with room to spare.
                let top = commands
                    .iter()
                    .filter_map(|command| match command {
                        DrawCommand::RoundedFill { bounds, .. } => Some(bounds.origin.y),
                        _ => None,
                    })
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    top < 5.0,
                    "{mode:?} at width {width} has {top}px of dead space above its keys \
                     (footprint_height returned {height})"
                );
            }
        }
    }

    #[test]
    fn every_emitted_key_resolves_to_a_keycode_declared_in_the_keymap() {
        let mut full = TouchKeyboard::new(KeyboardMode::Full);
        let mut numeric = TouchKeyboard::new(KeyboardMode::Numeric);
        let mut extended = TouchKeyboard::new(KeyboardMode::Extended);
        let mut emitted = Vec::new();
        for character in 'a'..='z' {
            emitted.push(
                full.press_with_modifiers(Key::Character(character))
                    .unwrap(),
            );
        }
        for row in ["1234567890", "@#$%&*-+=", "!?_/:;()"] {
            for character in row.chars() {
                emitted.push(
                    full.press_with_modifiers(Key::Character(character))
                        .unwrap(),
                );
            }
        }
        emitted.push(full.press_with_modifiers(Key::Backspace).unwrap());
        emitted.push(full.press_with_modifiers(Key::Enter).unwrap());
        emitted.push(full.press_with_modifiers(Key::Space).unwrap());
        // The numeric keypad emits the same logical Backspace/Enter keys,
        // so they resolve to the same keycodes already covered above.
        assert_eq!(numeric.press(Key::Backspace), Some(Key::Backspace));
        assert_eq!(numeric.press(Key::Enter), Some(Key::Enter));
        for key in [
            Key::Tab,
            Key::Escape,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
        ] {
            emitted.push(extended.press_with_modifiers(key).unwrap());
        }

        let keymap = virtual_keymap_source();
        for (key, modifiers) in &emitted {
            let wire = virtual_key(*key, *modifiers)
                .unwrap_or_else(|| panic!("no virtual key for {key:?}"));
            assert!(
                keymap.contains(&format!("<E{}>", wire.keycode)),
                "keymap is missing evdev keycode {}",
                wire.keycode
            );
        }

        assert_eq!(virtual_key(Key::Shift, Modifiers::default()), None);
        assert_eq!(virtual_key(Key::Symbols, Modifiers::default()), None);
        assert_eq!(virtual_key(Key::Ctrl, Modifiers::default()), None);
        assert_eq!(virtual_key(Key::Alt, Modifiers::default()), None);
    }

    #[test]
    fn upper_and_lower_case_share_a_keycode_and_differ_only_by_shift() {
        let lower = virtual_key(Key::Character('c'), Modifiers::default()).unwrap();
        let upper = virtual_key(Key::Character('C'), Modifiers::default()).unwrap();
        assert_eq!(lower.keycode, upper.keycode);
        assert_eq!(lower.modifiers, 0);
        assert_eq!(upper.modifiers, VirtualKey::SHIFT);

        // A symbol reached directly (e.g. tapping "!" on the symbols page,
        // not via a held Shift) still needs the real Shift bit: consumed-
        // modifier keybinding matching in receivers depends on it.
        let exclaim = virtual_key(Key::Character('!'), Modifiers::default()).unwrap();
        let one = virtual_key(Key::Character('1'), Modifiers::default()).unwrap();
        assert_eq!(exclaim.keycode, one.keycode);
        assert_eq!(exclaim.modifiers, VirtualKey::SHIFT);
        assert_eq!(one.modifiers, 0);
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

    #[test]
    fn ctrl_shift_c_produces_a_distinct_wire_key_from_plain_ctrl_c() {
        // TouchKeyboard's job is just resolving the tapped key and which
        // modifiers were held (baking Shift into the character's case, like
        // it always has); telling Ctrl+Shift+C apart from Ctrl+C on the
        // wire is `virtual_key`'s job, since it depends on which physical
        // key actually needs Shift — see its doc comment.
        let mut keyboard = TouchKeyboard::new(KeyboardMode::Extended);

        keyboard.press_with_modifiers(Key::Shift);
        keyboard.press_with_modifiers(Key::Ctrl);
        let (key, modifiers) = keyboard.press_with_modifiers(Key::Character('c')).unwrap();
        assert_eq!(key, Key::Character('C'));
        assert!(modifiers.ctrl);
        let ctrl_shift_c = virtual_key(key, modifiers).unwrap();

        keyboard.press_with_modifiers(Key::Ctrl);
        let (key, modifiers) = keyboard.press_with_modifiers(Key::Character('c')).unwrap();
        assert_eq!(key, Key::Character('c'));
        assert!(modifiers.ctrl);
        let ctrl_c = virtual_key(key, modifiers).unwrap();

        assert_eq!(ctrl_shift_c.keycode, ctrl_c.keycode, "same physical key");
        assert_eq!(
            ctrl_shift_c.modifiers,
            VirtualKey::CONTROL | VirtualKey::SHIFT
        );
        assert_eq!(ctrl_c.modifiers, VirtualKey::CONTROL);
        assert_ne!(ctrl_shift_c.modifiers, ctrl_c.modifiers);
    }
}
