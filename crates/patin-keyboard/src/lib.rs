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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
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
                let character = if self.shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                };
                self.shift = false;
                Some((Key::Character(character), self.take_ctrl_alt()))
            }
            other => Some((other, self.take_ctrl_alt())),
        }
    }

    fn take_ctrl_alt(&mut self) -> Modifiers {
        let modifiers = Modifiers {
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
    // Large enough that every height-derived clamp in `keyboard_full` and
    // `keyboard_numeric` (row height, key size, bottom margin) saturates at
    // its upper bound, matching how the layout renders on real screens.
    const REFERENCE_HEIGHT: f32 = 4000.0;
    let layout = keyboard(mode, Page::Letters, false, width, REFERENCE_HEIGHT);
    let top = layout
        .iter()
        .map(|key| key.hit_bounds.origin.y)
        .fold(f32::INFINITY, f32::min);
    REFERENCE_HEIGHT - top
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
    (height * 0.11).clamp(48.0, 112.0)
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

/// The `evdev`-style wire keycode a `virtual-keyboard-v1` client should send
/// for `key`, matching the keymap returned by [`virtual_keymap_source`].
/// `None` for keys that are consumed internally by [`TouchKeyboard::press`]
/// and never emitted (`Shift`, `Symbols`).
pub fn keycode_for(key: Key) -> Option<u32> {
    key_table()
        .iter()
        .position(|(candidate, _)| *candidate == key)
        .map(|index| (index + 1) as u32)
}

/// A complete, self-contained XKB keymap (`XKB_V1` text format) covering
/// every character and control key either layout can ever emit. Levels and
/// shift state are irrelevant here: [`TouchKeyboard::press`] already
/// resolves the final literal `Key`, so each key in this keymap has exactly
/// one level. Upload once via `zwp_virtual_keyboard_v1.keymap`, then use
/// [`keycode_for`] to pick the matching wire keycode per press.
pub fn virtual_keymap_source() -> String {
    let table = key_table();
    let mut keycodes = String::new();
    let mut symbols = String::new();
    for (index, (_, keysym)) in table.iter().enumerate() {
        let wire_code = index + 1;
        let xkb_code = wire_code + 8;
        keycodes.push_str(&format!("<K{wire_code}> = {xkb_code};\n"));
        symbols.push_str(&format!("key <K{wire_code}> {{ [ {keysym} ] }};\n"));
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
         }};\n\
         xkb_compat \"(unnamed)\" {{}};\n\
         xkb_symbols \"(unnamed)\" {{\n\
         {symbols}\
         }};\n\
         }};\n"
    )
}

/// Every `Key` the two layouts can ever emit through [`TouchKeyboard::press`],
/// paired with its XKB keysym name, in the fixed order used to assign wire
/// keycodes. Derived directly from the layout functions rather than
/// duplicated by hand, so it can't drift if a row of characters changes.
fn key_table() -> Vec<(Key, String)> {
    let mut table: Vec<(Key, String)> = character_set()
        .into_iter()
        .map(|character| (Key::Character(character), keysym_name(character)))
        .collect();
    table.push((Key::Backspace, "BackSpace".to_string()));
    table.push((Key::Enter, "Return".to_string()));
    table.push((Key::Space, "space".to_string()));
    table.push((Key::Tab, "Tab".to_string()));
    table.push((Key::Escape, "Escape".to_string()));
    table.push((Key::ArrowUp, "Up".to_string()));
    table.push((Key::ArrowDown, "Down".to_string()));
    table.push((Key::ArrowLeft, "Left".to_string()));
    table.push((Key::ArrowRight, "Right".to_string()));
    table
}

fn character_set() -> std::collections::BTreeSet<char> {
    let mut set = std::collections::BTreeSet::new();
    for mode in [
        KeyboardMode::Full,
        KeyboardMode::Numeric,
        KeyboardMode::Extended,
    ] {
        for page in [Page::Letters, Page::Symbols] {
            for layout in keyboard(mode, page, false, 400.0, 4000.0) {
                if let Key::Character(character) = layout.key {
                    set.insert(character);
                    set.insert(character.to_ascii_uppercase());
                }
            }
        }
    }
    set
}

fn keysym_name(character: char) -> String {
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
            }
        }
    }

    #[test]
    fn every_emitted_key_has_a_unique_keycode_declared_in_the_keymap() {
        let mut full = TouchKeyboard::new(KeyboardMode::Full);
        let mut numeric = TouchKeyboard::new(KeyboardMode::Numeric);
        let mut emitted = Vec::new();
        for character in 'a'..='z' {
            emitted.push(full.press(Key::Character(character)).unwrap());
        }
        for row in ["1234567890", "@#$%&*-+=", "!?_/:;()"] {
            for character in row.chars() {
                emitted.push(full.press(Key::Character(character)).unwrap());
            }
        }
        emitted.push(full.press(Key::Backspace).unwrap());
        emitted.push(full.press(Key::Enter).unwrap());
        emitted.push(full.press(Key::Space).unwrap());
        // The numeric keypad emits the same logical Backspace/Enter keys,
        // so they resolve to the same keycodes already covered above.
        assert_eq!(numeric.press(Key::Backspace), Some(Key::Backspace));
        assert_eq!(numeric.press(Key::Enter), Some(Key::Enter));

        let mut extended = TouchKeyboard::new(KeyboardMode::Extended);
        for key in [
            Key::Tab,
            Key::Escape,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
        ] {
            emitted.push(extended.press(key).unwrap());
        }

        let codes: Vec<u32> = emitted
            .iter()
            .map(|key| keycode_for(*key).unwrap_or_else(|| panic!("no keycode for {key:?}")))
            .collect();
        let unique: std::collections::BTreeSet<u32> = codes.iter().copied().collect();
        assert_eq!(codes.len(), unique.len(), "keycodes must not collide");

        let keymap = virtual_keymap_source();
        for code in &codes {
            assert!(
                keymap.contains(&format!("<K{code}>")),
                "keymap is missing keycode {code}"
            );
        }

        assert_eq!(keycode_for(Key::Shift), None);
        assert_eq!(keycode_for(Key::Symbols), None);
        assert_eq!(keycode_for(Key::Ctrl), None);
        assert_eq!(keycode_for(Key::Alt), None);
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
