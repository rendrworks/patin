use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, Dispatch, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch},
        },
        protocols::wp::{
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            },
            viewporter::client::{
                wp_viewport::{self, WpViewport},
                wp_viewporter::WpViewporter,
            },
        },
    },
    registry::{ProvidesRegistryState, RegistryState, SimpleGlobal},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler},
        touch::TouchHandler,
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::{error::Error, time::Duration};

use crate::{
    render::{CpuRenderer, Scale},
    ui::{DrawCommand, Rect, Size},
};

const BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerLevel {
    Background,
    Bottom,
    Top,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardPolicy {
    None,
    Exclusive,
    OnDemand,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Anchors {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

pub struct LayerConfig {
    pub namespace: String,
    pub layer: LayerLevel,
    pub anchors: Anchors,
    pub size: (u32, u32),
    pub exclusive_zone: i32,
    pub keyboard: KeyboardPolicy,
}

pub trait Shell {
    fn resize(&mut self, size: Size);
    fn update(&mut self) -> bool;
    fn activate_at(&mut self, position: (f64, f64)) -> bool;
    fn commands(&self) -> Vec<DrawCommand>;
    fn take_damage(&mut self) -> Vec<Rect>;
    fn damage_all(&mut self);
}

pub fn run(config: LayerConfig, shell: impl Shell + 'static) -> Result<(), Box<dyn Error>> {
    let connection = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();

    let mut event_loop = EventLoop::<Patin>::try_new()?;
    WaylandSource::new(connection, event_queue).insert(event_loop.handle())?;

    let compositor = CompositorState::bind(&globals, &queue_handle)
        .map_err(|error| format!("compositor does not provide wl_compositor: {error}"))?;
    let layer_shell = LayerShell::bind(&globals, &queue_handle)
        .map_err(|error| format!("compositor does not support layer-shell: {error}"))?;
    let shm = Shm::bind(&globals, &queue_handle)
        .map_err(|error| format!("compositor does not provide wl_shm: {error}"))?;

    let viewporter = SimpleGlobal::<WpViewporter, 1>::bind(&globals, &queue_handle).ok();
    let fractional_scale_manager =
        SimpleGlobal::<WpFractionalScaleManagerV1, 1>::bind(&globals, &queue_handle).ok();

    let surface = compositor.create_surface(&queue_handle);
    let viewport = viewporter
        .as_ref()
        .and_then(|manager| manager.get().ok())
        .map(|manager| manager.get_viewport(&surface, &queue_handle, ()));
    let fractional_scale = viewport.as_ref().and_then(|_| {
        fractional_scale_manager
            .as_ref()
            .and_then(|manager| manager.get().ok())
            .map(|manager| manager.get_fractional_scale(&surface, &queue_handle, ()))
    });

    let layer = layer_shell.create_layer_surface(
        &queue_handle,
        surface,
        match config.layer {
            LayerLevel::Background => Layer::Background,
            LayerLevel::Bottom => Layer::Bottom,
            LayerLevel::Top => Layer::Top,
            LayerLevel::Overlay => Layer::Overlay,
        },
        Some(config.namespace),
        None,
    );
    let mut anchor = Anchor::empty();
    anchor.set(Anchor::TOP, config.anchors.top);
    anchor.set(Anchor::BOTTOM, config.anchors.bottom);
    anchor.set(Anchor::LEFT, config.anchors.left);
    anchor.set(Anchor::RIGHT, config.anchors.right);
    layer.set_anchor(anchor);
    layer.set_size(config.size.0, config.size.1);
    layer.set_exclusive_zone(config.exclusive_zone);
    layer.set_keyboard_interactivity(match config.keyboard {
        KeyboardPolicy::None => KeyboardInteractivity::None,
        KeyboardPolicy::Exclusive => KeyboardInteractivity::Exclusive,
        KeyboardPolicy::OnDemand => KeyboardInteractivity::OnDemand,
    });
    layer.commit();

    let initial_pool_size =
        config.size.0.max(1) as usize * config.size.1.max(1) as usize * BYTES_PER_PIXEL;
    let pool = SlotPool::new(initial_pool_size, &shm)?;
    let mut patin = Patin {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &queue_handle),
        output_state: OutputState::new(&globals, &queue_handle),
        shm,
        pool,
        layer,
        _viewporter: viewporter,
        viewport,
        _fractional_scale_manager: fractional_scale_manager,
        _fractional_scale: fractional_scale,
        renderer: CpuRenderer::new(),
        requested_size: config.size,
        logical_size: None,
        scale: Scale::ONE,
        has_fractional_preference: false,
        frame_pending: false,
        redraw_requested: false,
        shell: Box::new(shell),
        pointers: Vec::new(),
        touches: Vec::new(),
        active_touches: Vec::new(),
        exit: false,
    };

    event_loop.handle().insert_source(
        Timer::from_duration(Duration::from_secs(1)),
        |_, _, patin| {
            if patin.shell.update() {
                patin.redraw_requested = true;
            }
            TimeoutAction::ToDuration(Duration::from_secs(1))
        },
    )?;
    eprintln!("patin: connected; waiting for the compositor to configure the bar");

    while !patin.exit {
        event_loop.dispatch(None, &mut patin)?;

        if patin.redraw_requested && !patin.frame_pending {
            patin.draw(&queue_handle);
        }
    }

    Ok(())
}

struct Patin {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
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
    active_touches: Vec<(wl_touch::WlTouch, i32)>,
    exit: bool,
}

impl Patin {
    fn request_redraw(&mut self, queue_handle: &QueueHandle<Self>) {
        self.redraw_requested = true;
        if !self.frame_pending {
            self.draw(queue_handle);
        }
    }

    fn draw(&mut self, queue_handle: &QueueHandle<Self>) {
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

        let surface = self.layer.wl_surface();
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
        self.layer.commit();

        self.frame_pending = true;
        self.redraw_requested = false;

        eprintln!(
            "patin: rendered {physical_width}x{physical_height} buffer for \
             {logical_width}x{logical_height} logical bar ({} damaged region{})",
            damage.len(),
            if damage.len() == 1 { "" } else { "s" }
        );
    }

    fn activate_at(&mut self, queue_handle: &QueueHandle<Self>, position: (f64, f64)) {
        if self.shell.activate_at(position) {
            self.request_redraw(queue_handle);
        }
    }
}

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

impl SeatHandler for Patin {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
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
                        .retain(|(active_touch, _)| active_touch != touch);
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
        self.remove_capability(connection, queue_handle, seat, Capability::Touch);
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
            if &event.surface != self.layer.wl_surface() {
                continue;
            }

            if matches!(
                event.kind,
                PointerEventKind::Press {
                    button: BTN_LEFT,
                    ..
                }
            ) {
                self.activate_at(queue_handle, event.position);
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
            .any(|(known_touch, known_id)| known_touch == touch && *known_id == id)
        {
            self.active_touches.push((touch.clone(), id));
        }
        eprintln!(
            "patin: touch contact {id} down; active contacts: {}",
            self.active_touches.len()
        );

        if surface == *self.layer.wl_surface() {
            self.activate_at(queue_handle, position);
        }
    }

    fn up(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        touch: &wl_touch::WlTouch,
        _serial: u32,
        _time: u32,
        id: i32,
    ) {
        self.active_touches
            .retain(|(known_touch, known_id)| known_touch != touch || *known_id != id);
        eprintln!(
            "patin: touch contact {id} up; active contacts: {}",
            self.active_touches.len()
        );
    }

    fn motion(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _time: u32,
        _id: i32,
        _position: (f64, f64),
    ) {
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
            .retain(|(known_touch, _)| known_touch != touch);
        eprintln!("patin: touch sequence cancelled");
    }
}

smithay_client_toolkit::reexports::client::delegate_noop!(Patin: WpViewporter);
smithay_client_toolkit::reexports::client::delegate_noop!(Patin: WpFractionalScaleManagerV1);

delegate_registry!(Patin);

impl ProvidesRegistryState for Patin {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_dispatch2!(Patin);
