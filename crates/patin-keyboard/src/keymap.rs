//! The wire layer: translating a resolved [`Key`] into an `evdev` keycode
//! plus real XKB modifier bits, and emitting the self-contained XKB keymap
//! that declares every keycode this crate can send.

use patin::platform::VirtualKey;

use super::{Key, Modifiers};

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
    use crate::{Key, KeyboardMode, Modifiers, TouchKeyboard, virtual_key, virtual_keymap_source};
    use patin::platform::VirtualKey;

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
