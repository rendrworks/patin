use patin::{
    platform::{Shell, VirtualKey},
    ui::{DrawCommand, Rect, Size},
};
use patin_keyboard::{KeyboardMode, TouchKeyboard};

pub struct OskShell {
    keyboard: TouchKeyboard,
    keymap: String,
    size: Size,
    pending_key: Option<VirtualKey>,
    damage: Vec<Rect>,
}

impl OskShell {
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            keyboard: TouchKeyboard::new(mode),
            keymap: patin_keyboard::virtual_keymap_source(),
            size: Size::default(),
            pending_key: None,
            damage: Vec::new(),
        }
    }

    fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
}

impl Shell for OskShell {
    fn resize(&mut self, size: Size) {
        if self.size != size {
            self.size = size;
            self.damage_all();
        }
    }

    fn update(&mut self) -> bool {
        false
    }

    fn activate_at(&mut self, position: (f64, f64)) -> bool {
        let Some(key) = self
            .keyboard
            .key_at(self.size.width, self.size.height, position)
        else {
            return false;
        };
        let Some((resolved, modifiers)) = self.keyboard.press_with_modifiers(key) else {
            // Shift/Symbols/Ctrl/Alt: state toggled internally, nothing to inject.
            self.damage_all();
            return true;
        };
        self.pending_key = patin_keyboard::keycode_for(resolved).map(|keycode| {
            let mut mask = 0;
            if modifiers.ctrl {
                mask |= VirtualKey::CONTROL;
            }
            if modifiers.alt {
                mask |= VirtualKey::ALT;
            }
            VirtualKey {
                keycode,
                modifiers: mask,
            }
        });
        self.damage_all();
        true
    }

    fn virtual_keyboard_keymap(&self) -> Option<&str> {
        Some(&self.keymap)
    }

    fn take_virtual_key(&mut self) -> Option<VirtualKey> {
        self.pending_key.take()
    }

    fn commands(&self) -> Vec<DrawCommand> {
        self.keyboard
            .commands(self.size.width, self.size.height, false)
    }

    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }

    fn damage_all(&mut self) {
        OskShell::damage_all(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label_center(commands: &[DrawCommand], label: &str) -> (f64, f64) {
        commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text { bounds, text, .. } if text == label => Some((
                    f64::from(bounds.origin.x + bounds.size.width / 2.0),
                    f64::from(bounds.origin.y + bounds.size.height / 2.0),
                )),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no key labeled {label:?} in the current layout"))
    }

    #[test]
    fn tapping_a_digit_queues_its_keycode_for_injection() {
        let mut shell = OskShell::new(KeyboardMode::Numeric);
        shell.resize(Size {
            width: 400.0,
            height: patin_keyboard::footprint_height(KeyboardMode::Numeric, 400.0),
        });
        let position = label_center(&shell.commands(), "1");

        assert!(shell.activate_at(position));
        assert_eq!(
            shell.take_virtual_key(),
            Some(VirtualKey {
                keycode: patin_keyboard::keycode_for(patin_keyboard::Key::Character('1')).unwrap(),
                modifiers: 0,
            })
        );
        assert_eq!(shell.take_virtual_key(), None);
    }

    #[test]
    fn tapping_shift_toggles_state_without_queuing_a_key() {
        let mut shell = OskShell::new(KeyboardMode::Full);
        shell.resize(Size {
            width: 400.0,
            height: patin_keyboard::footprint_height(KeyboardMode::Full, 400.0),
        });
        let position = label_center(&shell.commands(), "⇧");

        assert!(shell.activate_at(position));
        assert_eq!(shell.take_virtual_key(), None);
    }

    #[test]
    fn ctrl_arms_the_next_key_with_the_control_modifier() {
        let mut shell = OskShell::new(KeyboardMode::Extended);
        shell.resize(Size {
            width: 400.0,
            height: patin_keyboard::footprint_height(KeyboardMode::Extended, 400.0),
        });

        assert!(shell.activate_at(label_center(&shell.commands(), "Ctrl")));
        assert_eq!(
            shell.take_virtual_key(),
            None,
            "Ctrl only arms, doesn't inject"
        );

        assert!(shell.activate_at(label_center(&shell.commands(), "c")));
        assert_eq!(
            shell.take_virtual_key(),
            Some(VirtualKey {
                keycode: patin_keyboard::keycode_for(patin_keyboard::Key::Character('c')).unwrap(),
                modifiers: VirtualKey::CONTROL,
            })
        );

        // Ctrl released itself after the previous key.
        assert!(shell.activate_at(label_center(&shell.commands(), "c")));
        assert_eq!(
            shell.take_virtual_key(),
            Some(VirtualKey {
                keycode: patin_keyboard::keycode_for(patin_keyboard::Key::Character('c')).unwrap(),
                modifiers: 0,
            })
        );
    }

    #[test]
    fn resize_damages_the_whole_surface_once() {
        let mut shell = OskShell::new(KeyboardMode::Numeric);
        let size = Size {
            width: 400.0,
            height: patin_keyboard::footprint_height(KeyboardMode::Numeric, 400.0),
        };

        shell.resize(size);
        assert_eq!(
            shell.take_damage(),
            vec![Rect::new(0.0, 0.0, size.width, size.height)]
        );
        assert_eq!(shell.take_damage(), Vec::new());

        shell.resize(size);
        assert_eq!(
            shell.take_damage(),
            Vec::new(),
            "unchanged size stays undamaged"
        );
    }

    #[test]
    fn virtual_keyboard_keymap_matches_the_shared_keyboard_crate() {
        let shell = OskShell::new(KeyboardMode::Full);
        assert_eq!(
            shell.virtual_keyboard_keymap(),
            Some(patin_keyboard::virtual_keymap_source().as_str())
        );
    }
}
