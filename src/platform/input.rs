//! Seat handling and the three input paths that reach a shell: pointer
//! clicks and axis scrolls, touch taps and drags (with the move threshold
//! that tells one from the other), and physical key presses.

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

use super::config::KeyInput;
use super::Patin;

pub(super) struct ActiveTouch {
    touch: wl_touch::WlTouch,
    id: i32,
    start: (f64, f64),
    last: (f64, f64),
    moved: bool,
}

impl Patin {
    pub(super) fn scroll_by(&mut self, queue_handle: &QueueHandle<Self>, delta_y: f64) {
        if self.shell.scroll_by(delta_y) {
            self.request_redraw(queue_handle);
        }
    }


    pub(super) fn activate_at(&mut self, queue_handle: &QueueHandle<Self>, position: (f64, f64)) {
        let redraw = self.shell.activate_at(position);
        self.sync_text_input();
        self.send_pending_virtual_key();
        if self.shell.close_requested() {
            self.exit = true;
        } else if redraw {
            self.request_redraw(queue_handle);
        }
    }
}

impl SeatHandler for Patin {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
    ) {
        self.ensure_text_input(&seat, queue_handle);
        self.ensure_virtual_keyboard(&seat, queue_handle);
    }

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        self.ensure_text_input(&seat, queue_handle);
        self.ensure_virtual_keyboard(&seat, queue_handle);
        match capability {
            Capability::Pointer if !self.pointers.iter().any(|(known, _)| known == &seat) => {
                match self.seat_state.get_pointer(queue_handle, &seat) {
                    Ok(pointer) => self.pointers.push((seat, pointer)),
                    Err(error) => eprintln!("patin: could not create pointer: {error}"),
                }
            }
            Capability::Touch if !self.touches.iter().any(|(known, _)| known == &seat) => {
                match self.seat_state.get_touch(queue_handle, &seat) {
                    Ok(touch) => self.touches.push((seat, touch)),
                    Err(error) => eprintln!("patin: could not create touch input: {error}"),
                }
            }
            Capability::Keyboard if !self.keyboards.iter().any(|(known, _)| known == &seat) => {
                match self.seat_state.get_keyboard(queue_handle, &seat, None) {
                    Ok(keyboard) => self.keyboards.push((seat, keyboard)),
                    Err(error) => eprintln!("patin: could not create keyboard: {error}"),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer => {
                self.pointers.retain(|(known, pointer)| {
                    if known == &seat {
                        pointer.release();
                        false
                    } else {
                        true
                    }
                });
            }
            Capability::Touch => {
                for (_, touch) in self.touches.iter().filter(|(known, _)| known == &seat) {
                    self.active_touches
                        .retain(|contact| &contact.touch != touch);
                }
                self.touches.retain(|(known, touch)| {
                    if known == &seat {
                        touch.release();
                        false
                    } else {
                        true
                    }
                });
            }
            Capability::Keyboard => {
                self.keyboards.retain(|(known, keyboard)| {
                    if known == &seat {
                        keyboard.release();
                        false
                    } else {
                        true
                    }
                });
            }
            _ => {}
        }
    }

    fn remove_seat(
        &mut self,
        connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
    ) {
        self.remove_capability(connection, queue_handle, seat.clone(), Capability::Pointer);
        self.remove_capability(connection, queue_handle, seat.clone(), Capability::Touch);
        self.remove_capability(connection, queue_handle, seat.clone(), Capability::Keyboard);
        self.text_inputs.retain(|handle| {
            if handle.seat == seat {
                handle.proxy.destroy();
                false
            } else {
                true
            }
        });
        self.virtual_keyboards.retain(|(known, virtual_keyboard)| {
            if *known == seat {
                virtual_keyboard.destroy();
                false
            } else {
                true
            }
        });
    }
}

impl KeyboardHandler for Patin {
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
        queue_handle: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let input = if event.keysym == Keysym::BackSpace {
            Some(KeyInput::Backspace)
        } else if event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter {
            Some(KeyInput::Enter)
        } else if event.keysym == Keysym::Escape {
            Some(KeyInput::Escape)
        } else {
            event
                .utf8
                .filter(|value| !value.chars().all(char::is_control))
                .map(KeyInput::Text)
        };
        if let Some(input) = input
            && self.shell.key_input(input)
        {
            self.request_redraw(queue_handle);
        }
        self.sync_text_input();
        if self.shell.close_requested() {
            self.exit = true;
        }
    }

    fn repeat_key(
        &mut self,
        connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        keyboard: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(connection, queue_handle, keyboard, serial, event);
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

impl PointerHandler for Patin {
    fn pointer_frame(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.role.wl_surface() {
                continue;
            }

            match &event.kind {
                PointerEventKind::Press {
                    button: BTN_LEFT, ..
                } => self.activate_at(queue_handle, event.position),
                PointerEventKind::Axis { vertical, .. } => {
                    let delta = if vertical.value120 != 0 {
                        f64::from(vertical.value120) * 48.0 / 120.0
                    } else if vertical.discrete != 0 {
                        f64::from(vertical.discrete) * 48.0
                    } else {
                        vertical.absolute
                    };
                    if delta != 0.0 {
                        self.scroll_by(queue_handle, delta);
                    }
                }
                _ => {}
            }
        }
    }
}

impl TouchHandler for Patin {
    fn down(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        touch: &wl_touch::WlTouch,
        _serial: u32,
        _time: u32,
        surface: wl_surface::WlSurface,
        id: i32,
        position: (f64, f64),
    ) {
        if !self
            .active_touches
            .iter()
            .any(|contact| contact.touch == *touch && contact.id == id)
        {
            self.active_touches.push(ActiveTouch {
                touch: touch.clone(),
                id,
                start: position,
                last: position,
                moved: false,
            });
        }
        if self.trace {
            eprintln!(
                "patin: touch contact {id} down; active contacts: {}",
                self.active_touches.len()
            );
        }

        let _ = (queue_handle, surface);
    }

    fn up(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        touch: &wl_touch::WlTouch,
        _serial: u32,
        _time: u32,
        id: i32,
    ) {
        let contact = self
            .active_touches
            .iter()
            .position(|contact| contact.touch == *touch && contact.id == id)
            .map(|index| self.active_touches.remove(index));
        if let Some(contact) = contact
            && !contact.moved
        {
            self.activate_at(queue_handle, contact.start);
        }
        if self.trace {
            eprintln!(
                "patin: touch contact {id} up; active contacts: {}",
                self.active_touches.len()
            );
        }
    }

    fn motion(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        touch: &wl_touch::WlTouch,
        _time: u32,
        id: i32,
        position: (f64, f64),
    ) {
        let mut delta = None;
        if let Some(contact) = self
            .active_touches
            .iter_mut()
            .find(|contact| contact.touch == *touch && contact.id == id)
        {
            if (position.0 - contact.start.0).hypot(position.1 - contact.start.1) >= 8.0 {
                contact.moved = true;
            }
            if contact.moved {
                delta = Some(contact.last.1 - position.1);
            }
            contact.last = position;
        }
        if let Some(delta) = delta {
            self.scroll_by(queue_handle, delta);
        }
    }

    fn shape(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _id: i32,
        _major: f64,
        _minor: f64,
    ) {
    }

    fn orientation(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _id: i32,
        _orientation: f64,
    ) {
    }

    fn cancel(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        touch: &wl_touch::WlTouch,
    ) {
        self.active_touches
            .retain(|contact| contact.touch != *touch);
        if self.trace {
            eprintln!("patin: touch sequence cancelled");
        }
    }
}
