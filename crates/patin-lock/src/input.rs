//! Seat handling: the keyboard that types the password and the pointer and
//! touch input that drive the on-screen keypad.

use smithay_client_toolkit::{
    reexports::client::{
        Connection, QueueHandle,
        protocol::{wl_keyboard, wl_pointer, wl_seat, wl_surface, wl_touch},
    },
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler},
        touch::TouchHandler,
    },
};

use crate::ui::Key;
use crate::App;

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard if !self.keyboards.iter().any(|(known, _)| known == &seat) => {
                if let Ok(device) = self.seat_state.get_keyboard(qh, &seat, None) {
                    self.keyboards.push((seat, device));
                }
            }
            Capability::Pointer if !self.pointers.iter().any(|(known, _)| known == &seat) => {
                if let Ok(device) = self.seat_state.get_pointer(qh, &seat) {
                    self.pointers.push((seat, device));
                }
            }
            Capability::Touch if !self.touches.iter().any(|(known, _)| known == &seat) => {
                if let Ok(device) = self.seat_state.get_touch(qh, &seat) {
                    self.touches.push((seat, device));
                }
            }
            _ => {}
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard => self.keyboards.retain(|(known, device)| {
                if known == &seat {
                    device.release();
                    false
                } else {
                    true
                }
            }),
            Capability::Pointer => self.pointers.retain(|(known, device)| {
                if known == &seat {
                    device.release();
                    false
                } else {
                    true
                }
            }),
            Capability::Touch => self.touches.retain(|(known, device)| {
                if known == &seat {
                    device.release();
                    false
                } else {
                    true
                }
            }),
            _ => {}
        }
    }
    fn remove_seat(&mut self, conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.remove_capability(conn, qh, seat.clone(), Capability::Keyboard);
        self.remove_capability(conn, qh, seat.clone(), Capability::Pointer);
        self.remove_capability(conn, qh, seat, Capability::Touch);
    }
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        if event.keysym == Keysym::XF86_PowerOff {
            // The compositor forwards every key straight to the locked
            // client instead of running its own keybinds while locked (a
            // deliberate anti-bypass choice), so the power button has to be
            // handled here rather than via an external signal while locked.
            let blanked = self.blanked;
            self.set_blanked(!blanked);
        } else if event.keysym == Keysym::BackSpace {
            self.press(Key::Backspace);
        } else if event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter {
            self.press(Key::Enter);
        } else if let Some(text) = event.utf8 {
            for character in text.chars().filter(|character| !character.is_control()) {
                self.press(Key::Character(character));
            }
        }
    }
    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, keyboard, serial, event);
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if matches!(
                event.kind,
                PointerEventKind::Press {
                    button: BTN_LEFT,
                    ..
                }
            ) && let Some(index) = self.view_for_surface(&event.surface)
                && let Some((width, height)) = self.views[index].size
                && let Some(key) = self.ui.key_at(width as f32, height as f32, event.position)
            {
                self.press(key);
            }
        }
    }
}

impl TouchHandler for App {
    fn down(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        _: u32,
        surface: wl_surface::WlSurface,
        _: i32,
        position: (f64, f64),
    ) {
        if let Some(index) = self.view_for_surface(&surface)
            && let Some((width, height)) = self.views[index].size
            && let Some(key) = self.ui.key_at(width as f32, height as f32, position)
        {
            self.press(key);
        }
    }
    fn up(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        _: u32,
        _: i32,
    ) {
    }
    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        _: i32,
        _: (f64, f64),
    ) {
    }
    fn shape(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: i32,
        _: f64,
        _: f64,
    ) {
    }
    fn orientation(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: i32,
        _: f64,
    ) {
    }
    fn cancel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_touch::WlTouch) {}
}
