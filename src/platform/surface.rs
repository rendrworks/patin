//! Surface lifecycle: the compositor's configure/scale/frame callbacks for
//! both surface roles (layer-shell and xdg-toplevel), plus the output and
//! shm bookkeeping they depend on. This is where compositor-driven size and
//! scale changes are turned into shell resizes and repaints.

use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    reexports::{
        client::{
            Connection, Dispatch, QueueHandle,
            protocol::{wl_output, wl_surface},
        },
        protocols::wp::{
            fractional_scale::v1::client::wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            viewporter::client::wp_viewport::{self, WpViewport},
        },
    },
    shell::{
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        xdg::window::{Window, WindowConfigure, WindowHandler},
    },
    shm::{Shm, ShmHandler},
};

use super::Patin;
use crate::render::Scale;
use crate::ui::Size;

impl LayerShellHandler for Patin {
    fn closed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _layer: &LayerSurface,
    ) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let size = (
            configure.new_size.0.max(self.requested_size.0).max(1),
            configure.new_size.1.max(self.requested_size.1).max(1),
        );
        if self.logical_size != Some(size) {
            self.logical_size = Some(size);
            self.shell.resize(Size {
                width: size.0 as f32,
                height: size.1 as f32,
            });
            self.request_redraw(queue_handle);
        }

        // The compositor's round-trip acknowledgment of a `set_hidden`
        // exclusive-zone change — only now is it safe to touch the buffer,
        // never in the same commit as the zone change that prompted this.
        if self.pending_visibility_change {
            self.pending_visibility_change = false;
            if self.hidden {
                let surface = self.role.wl_surface();
                surface.attach(None, 0, 0);
                surface.commit();
                self.redraw_requested = false;
                self.frame_pending = false;
            } else {
                self.request_redraw(queue_handle);
            }
        }
    }
}

impl WindowHandler for Patin {
    fn request_close(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _window: &Window,
    ) {
        self.disable_text_input();
        self.exit = true;
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let size = (
            configure
                .new_size
                .0
                .map(|value| value.get())
                .unwrap_or(self.requested_size.0)
                .max(1),
            configure
                .new_size
                .1
                .map(|value| value.get())
                .unwrap_or(self.requested_size.1)
                .max(1),
        );
        if self.logical_size != Some(size) {
            self.logical_size = Some(size);
            self.shell.resize(Size {
                width: size.0 as f32,
                height: size.1 as f32,
            });
            self.request_redraw(queue_handle);
        }
    }
}

impl CompositorHandler for Patin {
    fn scale_factor_changed(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if self.has_fractional_preference {
            return;
        }

        let scale = Scale::from_integer(new_factor);
        if self.scale != scale {
            self.scale = scale;
            self.shell.damage_all();
            self.request_redraw(queue_handle);
        }
    }

    fn transform_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.frame_pending = false;
        if self.redraw_requested {
            self.draw(queue_handle);
        }
    }

    fn surface_enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for Patin {
    fn event(
        state: &mut Self,
        _proxy: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _data: &(),
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            let scale = Scale::from_120ths(scale);
            state.has_fractional_preference = true;
            if state.scale != scale {
                state.scale = scale;
                state.shell.damage_all();
                state.request_redraw(queue_handle);
            }
        }
    }
}

impl Dispatch<WpViewport, ()> for Patin {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewport,
        _event: wp_viewport::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        unreachable!("wp_viewport has no events in version 1")
    }
}

impl ShmHandler for Patin {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for Patin {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}
