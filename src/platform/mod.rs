//! The Wayland client runtime: it owns the connection, the surface, and the
//! event loop, and drives a consumer's [`Shell`] implementation.
//!
//! The pieces live in focused submodules — [`config`] for the consumer-facing
//! configuration types, [`shell`] for the trait itself, [`startup`] for
//! connecting and standing the surface up, then [`surface`], [`input`],
//! [`draw`], and [`text_input`] for the four groups of callbacks that arrive
//! once it is running. This module holds only what all of them share: the
//! `Patin` state struct, the surface-role wrapper, and the delegate glue.

mod config;
mod draw;
mod input;
mod shell;
mod startup;
mod surface;
mod text_input;

pub use config::{
    Anchors, KeyInput, KeyboardPolicy, LayerConfig, LayerLevel, LayerVisibility, TextInputPurpose,
    VirtualKey, WindowConfig,
};
pub use shell::Shell;
pub use startup::{run, run_window};

use std::time::Instant;

use smithay_client_toolkit::{
    delegate_registry,
    output::OutputState,
    reexports::{
        client::protocol::{wl_keyboard, wl_pointer, wl_seat, wl_surface, wl_touch},
        protocols::wp::{
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::WpFractionalScaleV1,
            },
            text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
            viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
        },
        protocols_misc::zwp_virtual_keyboard_v1::client::{
            zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
            zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        },
    },
    registry::{ProvidesRegistryState, RegistryState, SimpleGlobal},
    registry_handlers,
    seat::SeatState,
    shell::{WaylandSurface, wlr_layer::LayerSurface, xdg::window::Window},
    shm::{Shm, slot::SlotPool},
};

use crate::render::{CpuRenderer, Scale};

use input::ActiveTouch;
use text_input::TextInputHandle;

const BYTES_PER_PIXEL: usize = 4;

enum SurfaceConfig {
    Layer(LayerConfig),
    Window(WindowConfig),
}

enum SurfaceRole {
    Layer(LayerSurface),
    Window(Window),
}

impl SurfaceRole {
    fn wl_surface(&self) -> &wl_surface::WlSurface {
        match self {
            Self::Layer(layer) => layer.wl_surface(),
            Self::Window(window) => window.wl_surface(),
        }
    }

    fn commit(&self) {
        match self {
            Self::Layer(layer) => layer.commit(),
            Self::Window(window) => window.commit(),
        }
    }
}

struct Patin {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    role: SurfaceRole,
    _viewporter: Option<SimpleGlobal<WpViewporter, 1>>,
    viewport: Option<WpViewport>,
    _fractional_scale_manager: Option<SimpleGlobal<WpFractionalScaleManagerV1, 1>>,
    _fractional_scale: Option<WpFractionalScaleV1>,
    renderer: CpuRenderer,
    requested_size: (u32, u32),
    logical_size: Option<(u32, u32)>,
    scale: Scale,
    has_fractional_preference: bool,
    frame_pending: bool,
    redraw_requested: bool,
    shell: Box<dyn Shell>,
    pointers: Vec<(wl_seat::WlSeat, wl_pointer::WlPointer)>,
    touches: Vec<(wl_seat::WlSeat, wl_touch::WlTouch)>,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    text_input_manager: Option<ZwpTextInputManagerV3>,
    text_inputs: Vec<TextInputHandle>,
    virtual_keyboard_manager: Option<ZwpVirtualKeyboardManagerV1>,
    virtual_keyboards: Vec<(wl_seat::WlSeat, ZwpVirtualKeyboardV1)>,
    virtual_keyboard_epoch: Instant,
    active_touches: Vec<ActiveTouch>,
    trace: bool,
    exit: bool,
    hidden: bool,
    /// The real exclusive zone to restore on `set_hidden(false)` — while
    /// hidden the layer surface's exclusive zone is dropped to 0 so other
    /// windows reclaim the space, matching wvkbd's hide behavior.
    configured_exclusive_zone: i32,
    /// Set by `set_hidden` once it has committed an exclusive-zone change;
    /// cleared by the next `configure`, which is where the matching buffer
    /// transition (attach/detach) actually happens — never in the same
    /// commit as the zone change itself.
    pending_visibility_change: bool,
}

smithay_client_toolkit::reexports::client::delegate_noop!(Patin: WpViewporter);
smithay_client_toolkit::reexports::client::delegate_noop!(Patin: WpFractionalScaleManagerV1);
smithay_client_toolkit::reexports::client::delegate_noop!(Patin: ZwpVirtualKeyboardManagerV1);
smithay_client_toolkit::reexports::client::delegate_noop!(Patin: ZwpVirtualKeyboardV1);

delegate_registry!(Patin);

impl ProvidesRegistryState for Patin {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_dispatch2!(Patin);
