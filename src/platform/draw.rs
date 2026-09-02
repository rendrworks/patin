//! Frame scheduling and painting: deciding when a new frame is owed,
//! allocating the `wl_shm` buffer for it, handing the shell's draw
//! commands to the CPU renderer, and attaching the result. Also the
//! hide/show transition, which rides the same buffer lifecycle.

use smithay_client_toolkit::{
    compositor::FrameCallbackData,
    reexports::client::{QueueHandle, protocol::wl_shm},
    shell::WaylandSurface,
};

use super::{BYTES_PER_PIXEL, Patin, SurfaceRole};

impl Patin {
    pub(super) fn request_redraw(&mut self, queue_handle: &QueueHandle<Self>) {
        self.redraw_requested = true;
        if !self.frame_pending {
            self.draw(queue_handle);
        }
    }

    /// Unmaps the surface and drops its exclusive zone to 0 (`hidden`), or
    /// restores both (`!hidden`). Only meaningful for a layer surface built
    /// with `LayerVisibility::ToggleBySignal`; a no-op otherwise since
    /// `hidden` never becomes true for any other surface.
    ///
    /// Only commits the exclusive-zone change here — changing it is a
    /// layout-affecting request, so per the compositor round-trip every
    /// other layer-shell request implicitly relies on (attach a new buffer
    /// only after that state has been configure-acked), the actual buffer
    /// transition (attach/detach) happens later, from `configure`, once the
    /// compositor's resulting reconfigure has actually arrived. Bundling
    /// both into one commit is what a fresh surface's mandatory "no buffer
    /// on the first commit" rule prevents by construction; reusing an
    /// already-mapped surface has no such guard rail, so it has to be done
    /// by hand here.
    pub(super) fn set_hidden(&mut self, hidden: bool) {
        if self.hidden == hidden {
            return;
        }
        self.hidden = hidden;
        if let SurfaceRole::Layer(layer) = &self.role {
            layer.set_exclusive_zone(if hidden {
                0
            } else {
                self.configured_exclusive_zone
            });
            layer.commit();
        }
        self.pending_visibility_change = true;
    }

    pub(super) fn draw(&mut self, queue_handle: &QueueHandle<Self>) {
        if self.hidden {
            self.redraw_requested = false;
            return;
        }
        let Some((logical_width, logical_height)) = self.logical_size else {
            return;
        };

        let physical_width = self.scale.physical(logical_width);
        let physical_height = self.scale.physical(logical_height);
        let stride = physical_width as i32 * BYTES_PER_PIXEL as i32;
        let (buffer, canvas) = self
            .pool
            .create_buffer(
                physical_width as i32,
                physical_height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("shared-memory buffer creation failed");

        let commands = self.shell.commands();
        self.renderer
            .render_bar(
                canvas,
                physical_width,
                physical_height,
                self.scale,
                &commands,
            )
            .expect("CPU rendering failed");

        let surface = self.role.wl_surface();
        if let Some(viewport) = &self.viewport {
            surface.set_buffer_scale(1);
            viewport.set_destination(logical_width as i32, logical_height as i32);
        } else {
            let integer_scale = i32::try_from(self.scale.physical(1)).unwrap_or(i32::MAX);
            surface.set_buffer_scale(integer_scale);
        }

        let damage = self.shell.take_damage();
        for rect in &damage {
            let factor = self.scale.factor();
            let x = (rect.origin.x * factor).floor() as i32;
            let y = (rect.origin.y * factor).floor() as i32;
            let right = ((rect.origin.x + rect.size.width) * factor).ceil() as i32;
            let bottom = ((rect.origin.y + rect.size.height) * factor).ceil() as i32;
            surface.damage_buffer(x, y, (right - x).max(1), (bottom - y).max(1));
        }
        surface.frame(queue_handle, FrameCallbackData(surface.clone()));
        buffer
            .attach_to(surface)
            .expect("shared-memory buffer attachment failed");
        self.role.commit();

        self.frame_pending = true;
        self.redraw_requested = false;

        if self.trace {
            eprintln!(
                "patin: rendered {physical_width}x{physical_height} buffer for \
                 {logical_width}x{logical_height} logical surface ({} damaged region{})",
                damage.len(),
                if damage.len() == 1 { "" } else { "s" }
            );
        }
    }
}
