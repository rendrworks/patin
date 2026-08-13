//! The interface a consumer implements to become a Patin surface: it
//! receives lifecycle, input, and sizing callbacks, and returns logical
//! draw commands plus the regions that changed.

use std::time::Duration;

use crate::ui::{DrawCommand, Rect, Size};

use super::config::{KeyInput, TextInputPurpose, VirtualKey};

pub trait Shell {
    fn resize(&mut self, size: Size);
    fn update(&mut self) -> bool;
    /// How often `update` is polled. Defaults to 1s; shells backed by a
    /// fast-changing external source (e.g. a control-socket poll) can
    /// override this for snappier feedback.
    fn poll_interval(&self) -> Duration {
        Duration::from_secs(1)
    }
    fn activate_at(&mut self, position: (f64, f64)) -> bool;
    fn scroll_by(&mut self, _delta_y: f64) -> bool {
        false
    }
    fn key_input(&mut self, _input: KeyInput) -> bool {
        false
    }
    fn text_input(&self) -> Option<TextInputPurpose> {
        None
    }
    /// A complete `XKB_V1` keymap to upload to any bound `virtual-keyboard-v1`
    /// object, covering every key this shell will ever inject. Returning
    /// `None` (the default) leaves virtual-keyboard support inactive.
    fn virtual_keyboard_keymap(&self) -> Option<&str> {
        None
    }
    /// A synthetic press-then-release to inject, matching the keymap
    /// returned by `virtual_keyboard_keymap`. Polled once after each
    /// `activate_at`.
    fn take_virtual_key(&mut self) -> Option<VirtualKey> {
        None
    }
    fn close_requested(&self) -> bool {
        false
    }
    fn commands(&self) -> Vec<DrawCommand>;
    fn take_damage(&mut self) -> Vec<Rect>;
    fn damage_all(&mut self);
}
