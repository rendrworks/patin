//! Connecting to the compositor and standing a surface up: binding the
//! globals a shell needs, creating its layer-shell or xdg-toplevel role,
//! wiring the calloop sources (Wayland queue, the shell's poll timer, and
//! the optional visibility signals), then running the loop to completion.

use std::error::Error;
use std::time::Instant;

use calloop::signals::{Signal, Signals};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        calloop_wayland_source::WaylandSource,
        client::{Connection, globals::registry_queue_init},
        protocols::wp::{
            fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
            text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
            viewporter::client::wp_viewporter::WpViewporter,
        },
        protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    },
    registry::{RegistryState, SimpleGlobal},
    seat::SeatState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
        xdg::{XdgShell, window::WindowDecorations},
    },
    shm::{Shm, slot::SlotPool},
};

use crate::render::{CpuRenderer, Scale};

use super::config::{KeyboardPolicy, LayerConfig, LayerLevel, LayerVisibility, WindowConfig};
use super::shell::Shell;
use super::{BYTES_PER_PIXEL, Patin, SurfaceConfig, SurfaceRole};

pub fn run(config: LayerConfig, shell: impl Shell + 'static) -> Result<(), Box<dyn Error>> {
    run_surface(SurfaceConfig::Layer(config), shell)
}

pub fn run_window(config: WindowConfig, shell: impl Shell + 'static) -> Result<(), Box<dyn Error>> {
    run_surface(SurfaceConfig::Window(config), shell)
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

    let (initial_hidden, configured_exclusive_zone, signal_toggle_enabled) = match &config {
        SurfaceConfig::Layer(layer_config) => match layer_config.visibility {
            LayerVisibility::Fixed => (false, layer_config.exclusive_zone, false),
            LayerVisibility::ToggleBySignal { start_visible } => {
                (!start_visible, layer_config.exclusive_zone, true)
            }
        },
        SurfaceConfig::Window(_) => (false, 0, false),
    };

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
            layer.set_exclusive_zone(if initial_hidden {
                0
            } else {
                config.exclusive_zone
            });
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
    let poll_interval = shell.poll_interval();
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
        virtual_keyboard_manager: globals
            .bind::<ZwpVirtualKeyboardManagerV1, _, _>(&queue_handle, 1..=1, ())
            .ok(),
        virtual_keyboards: Vec::new(),
        virtual_keyboard_epoch: Instant::now(),
        active_touches: Vec::new(),
        trace: std::env::var_os("PATIN_TRACE").is_some(),
        exit: false,
        hidden: initial_hidden,
        configured_exclusive_zone,
        pending_visibility_change: false,
    };

    event_loop
        .handle()
        .insert_source(Timer::from_duration(poll_interval), |_, _, patin| {
            if patin.shell.update() {
                patin.redraw_requested = true;
            }
            patin.sync_text_input();
            if patin.shell.close_requested() {
                patin.exit = true;
            }
            TimeoutAction::ToDuration(patin.shell.poll_interval())
        })?;

    if signal_toggle_enabled {
        let signals = Signals::new(&[Signal::SIGUSR1, Signal::SIGUSR2])?;
        event_loop
            .handle()
            .insert_source(signals, |event, _, patin| match event.signal() {
                Signal::SIGUSR1 => patin.set_hidden(true),
                Signal::SIGUSR2 => patin.set_hidden(false),
                _ => {}
            })?;
    }
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
