//! Text-input and virtual-keyboard plumbing: tracking a `text-input-v3`
//! object per seat, mirroring the shell's requested purpose onto it, and
//! injecting synthetic key events through `virtual-keyboard-v1`.

use std::io::Write;
use std::os::fd::AsFd;

use smithay_client_toolkit::reexports::{
    client::{Connection, Dispatch, QueueHandle, protocol::wl_seat},
    protocols::wp::text_input::zv3::client::{
        zwp_text_input_manager_v3::ZwpTextInputManagerV3,
        zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3},
    },
    protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use super::Patin;
use super::config::{KeyInput, TextInputPurpose, VirtualKey};

pub(super) struct TextInputHandle {
    pub(super) seat: wl_seat::WlSeat,
    pub(super) proxy: ZwpTextInputV3,
    entered: bool,
    applied: Option<TextInputPurpose>,
    pending_commit: Option<String>,
}

impl Patin {
    pub(super) fn ensure_text_input(
        &mut self,
        seat: &wl_seat::WlSeat,
        queue_handle: &QueueHandle<Self>,
    ) {
        if let Some(manager) = &self.text_input_manager
            && !self.text_inputs.iter().any(|known| known.seat == *seat)
        {
            self.text_inputs.push(TextInputHandle {
                proxy: manager.get_text_input(seat, queue_handle, ()),
                seat: seat.clone(),
                entered: false,
                applied: None,
                pending_commit: None,
            });
        }
    }

    pub(super) fn sync_text_input(&mut self) {
        let desired = self.shell.text_input();
        for handle in &mut self.text_inputs {
            let desired = handle.entered.then_some(desired).flatten();
            if handle.applied == desired {
                continue;
            }
            if handle.applied.is_some() {
                handle.proxy.disable();
                handle.proxy.commit();
            }
            if let Some(purpose) = desired {
                let (hint, protocol_purpose) = match purpose {
                    TextInputPurpose::Normal => (ContentHint::None, ContentPurpose::Normal),
                    TextInputPurpose::Password => {
                        (ContentHint::SensitiveData, ContentPurpose::Password)
                    }
                };
                handle.proxy.enable();
                handle.proxy.set_content_type(hint, protocol_purpose);
                handle.proxy.commit();
            }
            handle.applied = desired;
        }
    }

    pub(super) fn ensure_virtual_keyboard(
        &mut self,
        seat: &wl_seat::WlSeat,
        queue_handle: &QueueHandle<Self>,
    ) {
        if let Some(manager) = &self.virtual_keyboard_manager
            && !self
                .virtual_keyboards
                .iter()
                .any(|(known, _)| known == seat)
        {
            let virtual_keyboard = manager.create_virtual_keyboard(seat, queue_handle, ());
            if let Some(keymap) = self.shell.virtual_keyboard_keymap() {
                upload_virtual_keymap(&virtual_keyboard, keymap);
            }
            self.virtual_keyboards
                .push((seat.clone(), virtual_keyboard));
        }
    }

    pub(super) fn send_pending_virtual_key(&mut self) {
        let Some(VirtualKey { keycode, modifiers }) = self.shell.take_virtual_key() else {
            return;
        };
        if self.virtual_keyboards.is_empty() {
            return;
        }
        let time = self.virtual_keyboard_epoch.elapsed().as_millis() as u32;
        const PRESSED: u32 = 1; // wl_keyboard::KeyState::Pressed
        const RELEASED: u32 = 0; // wl_keyboard::KeyState::Released
        for (_, virtual_keyboard) in &self.virtual_keyboards {
            if modifiers != 0 {
                virtual_keyboard.modifiers(modifiers, 0, 0, 0);
            }
            virtual_keyboard.key(time, keycode, PRESSED);
            virtual_keyboard.key(time, keycode, RELEASED);
            if modifiers != 0 {
                virtual_keyboard.modifiers(0, 0, 0, 0);
            }
        }
    }

    pub(super) fn disable_text_input(&mut self) {
        for handle in &mut self.text_inputs {
            if handle.applied.take().is_some() {
                handle.proxy.disable();
                handle.proxy.commit();
            }
        }
    }
}

fn upload_virtual_keymap(virtual_keyboard: &ZwpVirtualKeyboardV1, keymap: &str) {
    const XKB_V1_FORMAT: u32 = 1; // wl_keyboard::KeymapFormat::XkbV1

    // The mapped region must be null-terminated per the wl_keyboard.keymap
    // convention that this request reuses.
    let mut contents = keymap.as_bytes().to_vec();
    contents.push(0);
    let size = contents.len() as u32;

    let mut file = match rustix::fs::memfd_create(
        "patin-virtual-keyboard-keymap",
        rustix::fs::MemfdFlags::CLOEXEC,
    ) {
        Ok(fd) => std::fs::File::from(fd),
        Err(error) => {
            eprintln!("patin: could not create keymap memfd: {error}");
            return;
        }
    };
    if let Err(error) = file.write_all(&contents) {
        eprintln!("patin: could not write keymap: {error}");
        return;
    }
    virtual_keyboard.keymap(XKB_V1_FORMAT, file.as_fd(), size);
}

impl Dispatch<ZwpTextInputManagerV3, ()> for Patin {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        unreachable!("zwp_text_input_manager_v3 has no events")
    }
}

impl Dispatch<ZwpTextInputV3, ()> for Patin {
    fn event(
        state: &mut Self,
        proxy: &ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _data: &(),
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        let Some(index) = state
            .text_inputs
            .iter()
            .position(|handle| handle.proxy == *proxy)
        else {
            return;
        };
        match event {
            zwp_text_input_v3::Event::Enter { surface } => {
                state.text_inputs[index].entered = surface == *state.role.wl_surface();
                state.sync_text_input();
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                let handle = &mut state.text_inputs[index];
                if handle.applied.is_some() {
                    handle.proxy.disable();
                    handle.proxy.commit();
                }
                handle.entered = false;
                handle.applied = None;
                handle.pending_commit = None;
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                state.text_inputs[index].pending_commit = text;
            }
            zwp_text_input_v3::Event::Done { .. } => {
                if let Some(text) = state.text_inputs[index].pending_commit.take()
                    && state.shell.key_input(KeyInput::Text(text))
                {
                    state.request_redraw(queue_handle);
                }
                state.sync_text_input();
            }
            _ => {}
        }
    }
}
