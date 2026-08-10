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
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch},
        },
        protocols::wp::{
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            },
            text_input::zv3::client::{
                zwp_text_input_manager_v3::ZwpTextInputManagerV3,
                zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3},
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
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler},
        touch::TouchHandler,
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
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

pub struct WindowConfig {
    pub app_id: String,
    pub title: String,
    pub initial_size: (u32, u32),
    pub min_size: Option<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputPurpose {
    Normal,
    Password,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyInput {
    Text(String),
    Backspace,
    Enter,
    Escape,
}

pub trait Shell {
    fn resize(&mut self, size: Size);
    fn update(&mut self) -> bool;
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
    fn close_requested(&self) -> bool {
        false
    }
    fn commands(&self) -> Vec<DrawCommand>;
    fn take_damage(&mut self) -> Vec<Rect>;
    fn damage_all(&mut self);
}

pub fn run(config: LayerConfig, shell: impl Shell + 'static) -> Result<(), Box<dyn Error>> {
    run_surface(SurfaceConfig::Layer(config), shell)
}

pub fn run_window(config: WindowConfig, shell: impl Shell + 'static) -> Result<(), Box<dyn Error>> {
    run_surface(SurfaceConfig::Window(config), shell)
}

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

struct TextInputHandle {
    seat: wl_seat::WlSeat,
    proxy: ZwpTextInputV3,
    entered: bool,
    applied: Option<TextInputPurpose>,
    pending_commit: Option<String>,
}

fn run_surface(config: SurfaceConfig, shell: impl Shell + 'static) -> Result<(), Box<dyn Error>> {
    let connection = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();

    let mut event_loop = EventLoop::<Patin>::try_new()?;
    WaylandSource::new(connection, event_queue).insert(event_loop.handle())?;

    let compositor = CompositorState::bind(&globals, &queue_handle)
        .map_err(|error| format!("compositor does not provide wl_compositor: {error}"))?;
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

    let (role, requested_size) = match config {
        SurfaceConfig::Layer(config) => {
            let layer_shell = LayerShell::bind(&globals, &queue_handle)
                .map_err(|error| format!("compositor does not support layer-shell: {error}"))?;
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
            (SurfaceRole::Layer(layer), config.size)
        }
        SurfaceConfig::Window(config) => {
            let xdg_shell = XdgShell::bind(&globals, &queue_handle)
                .map_err(|error| format!("compositor does not support xdg-shell: {error}"))?;
            let window =
                xdg_shell.create_window(surface, WindowDecorations::RequestServer, &queue_handle);
            window.set_app_id(config.app_id);
            window.set_title(config.title);
            window.set_min_size(config.min_size);
            window.commit();
            (SurfaceRole::Window(window), config.initial_size)
        }
    };

    let initial_pool_size =
        requested_size.0.max(1) as usize * requested_size.1.max(1) as usize * BYTES_PER_PIXEL;
    let pool = SlotPool::new(initial_pool_size, &shm)?;
    let mut patin = Patin {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &queue_handle),
        output_state: OutputState::new(&globals, &queue_handle),
        shm,
        pool,
        role,
        _viewporter: viewporter,
        viewport,
        _fractional_scale_manager: fractional_scale_manager,
        _fractional_scale: fractional_scale,
        renderer: CpuRenderer::new(),
        requested_size,
        logical_size: None,
        scale: Scale::ONE,
        has_fractional_preference: false,
        frame_pending: false,
        redraw_requested: false,
        shell: Box::new(shell),
        pointers: Vec::new(),
        touches: Vec::new(),
        keyboards: Vec::new(),
        text_input_manager: globals
            .bind::<ZwpTextInputManagerV3, _, _>(&queue_handle, 1..=1, ())
            .ok(),
        text_inputs: Vec::new(),
        active_touches: Vec::new(),
        trace: std::env::var_os("PATIN_TRACE").is_some(),
        exit: false,
    };

    event_loop.handle().insert_source(
        Timer::from_duration(Duration::from_secs(1)),
        |_, _, patin| {
            if patin.shell.update() {
                patin.redraw_requested = true;
            }
            patin.sync_text_input();
            if patin.shell.close_requested() {
                patin.exit = true;
            }
            TimeoutAction::ToDuration(Duration::from_secs(1))
        },
    )?;
    eprintln!("patin: connected; waiting for the compositor to configure the surface");

    while !patin.exit {
        event_loop.dispatch(None, &mut patin)?;

        if patin.redraw_requested && !patin.frame_pending {
            patin.draw(&queue_handle);
        }
    }

    patin.disable_text_input();

    Ok(())
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
    active_touches: Vec<ActiveTouch>,
    trace: bool,
    exit: bool,
}

struct ActiveTouch {
    touch: wl_touch::WlTouch,
    id: i32,
    start: (f64, f64),
    last: (f64, f64),
    moved: bool,
}

impl Patin {
    fn ensure_text_input(&mut self, seat: &wl_seat::WlSeat, queue_handle: &QueueHandle<Self>) {
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

    fn sync_text_input(&mut self) {
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

    fn disable_text_input(&mut self) {
        for handle in &mut self.text_inputs {
            if handle.applied.take().is_some() {
                handle.proxy.disable();
                handle.proxy.commit();
            }
        }
    }

    fn request_redraw(&mut self, queue_handle: &QueueHandle<Self>) {
        self.redraw_requested = true;
        if !self.frame_pending {
            self.draw(queue_handle);
        }
    }

    fn scroll_by(&mut self, queue_handle: &QueueHandle<Self>, delta_y: f64) {
        if self.shell.scroll_by(delta_y) {
            self.request_redraw(queue_handle);
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

    fn activate_at(&mut self, queue_handle: &QueueHandle<Self>, position: (f64, f64)) {
        let redraw = self.shell.activate_at(position);
        self.sync_text_input();
        if self.shell.close_requested() {
            self.exit = true;
        } else if redraw {
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
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
    ) {
        self.ensure_text_input(&seat, queue_handle);
    }

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        self.ensure_text_input(&seat, queue_handle);
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
