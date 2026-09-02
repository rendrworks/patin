//! Standing the lock up: binding the globals, taking the session lock, and
//! running the event loop until the password is accepted.

use std::error::Error;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use patin::render::{CpuRenderer, Scale};
use patin_status::Status;
use smithay_client_toolkit::{
    compositor::CompositorState,
    compositor::FrameCallbackData,
    output::OutputState,
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
        },
        protocols_wlr::output_power_management::v1::client::{
            zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1,
            zwlr_output_power_v1::{Mode as OutputPowerMode, ZwlrOutputPowerV1},
        },
    },
    registry::RegistryState,
    seat::SeatState,
    session_lock::SessionLockState,
    shm::{Shm, slot::SlotPool},
};

use crate::auth::{AuthResult, authenticate, effective_username};
use crate::power::{OutputPowerData, OutputPowerManagerData};
use crate::ui::{Key, LockUi};
use crate::{App, BYTES_PER_PIXEL, POWER_BUTTON_PRESSED, View, handle_power_button_signal};

pub(crate) fn run_lock() -> Result<(), Box<dyn Error>> {
    let settings = crate::settings::Settings::load();
    let username = effective_username()?;
    if !Path::new("/etc/pam.d/patin-lock").is_file() {
        return Err(
            "missing /etc/pam.d/patin-lock; install the PAM policy before starting the lock".into(),
        );
    }
    let connection = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();
    let mut event_loop = EventLoop::<App>::try_new()?;
    WaylandSource::new(connection.clone(), event_queue).insert(event_loop.handle())?;

    let compositor = CompositorState::bind(&globals, &queue_handle)?;
    let output_state = OutputState::new(&globals, &queue_handle);
    let seat_state = SeatState::new(&globals, &queue_handle);
    let shm = Shm::bind(&globals, &queue_handle)?;
    let lock_state = SessionLockState::new(&globals, &queue_handle);
    let lock = lock_state
        .lock(&queue_handle)
        .map_err(|error| format!("compositor does not support session lock: {error}"))?;
    let output_power_manager = match globals.bind::<ZwlrOutputPowerManagerV1, App, _>(
        &queue_handle,
        1..=1,
        OutputPowerManagerData,
    ) {
        Ok(manager) => Some(manager),
        Err(error) => {
            eprintln!(
                "patin-lock: compositor does not support output power management ({error}); the screen will not blank while locked"
            );
            None
        }
    };
    let (auth_tx, auth_rx) = channel();
    let mut app = App {
        connection,
        registry_state: RegistryState::new(&globals),
        compositor,
        output_state,
        seat_state,
        shm,
        lock: Some(lock),
        views: Vec::new(),
        keyboards: Vec::new(),
        pointers: Vec::new(),
        touches: Vec::new(),
        renderer: CpuRenderer::new(),
        ui: LockUi::new(settings.mode, settings.theme),
        // Icons only: the lock already draws a large clock of its own. Built
        // after the PAM policy check above, so a missing policy still fails
        // fast rather than after a D-Bus connect.
        status: Status::new(settings.theme.status_palette())
            .with_clock(false)
            .with_volume(true),
        username,
        auth_tx,
        auth_rx,
        exit: false,
        unlocked: false,
        output_power_manager,
        last_activity: Instant::now(),
        blank_after: settings.blank_after,
        blank_after_typing: settings.blank_after_typing,
        blanked: false,
        ever_woken: false,
    };
    let outputs: Vec<_> = app.output_state.outputs().collect();
    for output in outputs {
        app.add_output(output, &queue_handle)?;
    }

    unsafe {
        libc::signal(
            libc::SIGUSR1,
            handle_power_button_signal as *const () as libc::sighandler_t,
        );
    }

    while !app.exit {
        event_loop.dispatch(Some(Duration::from_millis(50)), &mut app)?;
        app.poll_auth();
        if POWER_BUTTON_PRESSED.swap(false, Ordering::SeqCst) {
            let blanked = app.blanked;
            app.set_blanked(!blanked);
        }
        app.check_idle();
        // Only while awake. `draw_pending` already skips a blanked screen, but
        // the poll behind it would still run — hitting D-Bus and spawning
        // `wpctl` every couple of seconds at a display nobody can see. Waking
        // redraws everything anyway, so nothing is missed.
        if !app.blanked && app.status.update() {
            app.redraw_all();
        }
        app.draw_pending(&queue_handle);
    }
    if app.unlocked {
        Ok(())
    } else {
        Err("session lock ended without a successful authentication".into())
    }
}

impl App {
    pub(crate) fn add_output(
        &mut self,
        output: wl_output::WlOutput,
        queue_handle: &QueueHandle<Self>,
    ) -> Result<(), Box<dyn Error>> {
        if self.views.iter().any(|view| view.output == output) {
            return Ok(());
        }
        let surface = self.compositor.create_surface(queue_handle);
        let lock_surface = self
            .lock
            .as_ref()
            .ok_or("lock disappeared")?
            .create_lock_surface(surface, &output, queue_handle);
        let power = self
            .output_power_manager
            .as_ref()
            .map(|manager| manager.get_output_power(&output, queue_handle, OutputPowerData));
        self.views.push(View {
            output,
            lock_surface,
            pool: SlotPool::new(4, &self.shm)?,
            size: None,
            scale: Scale::ONE,
            frame_pending: false,
            redraw: true,
            power,
        });
        Ok(())
    }

    pub(crate) fn view_for_surface(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.views
            .iter()
            .position(|view| view.lock_surface.wl_surface() == surface)
    }

    pub(crate) fn view_for_power(&self, power: &ZwlrOutputPowerV1) -> Option<usize> {
        self.views
            .iter()
            .position(|view| view.power.as_ref() == Some(power))
    }

    fn redraw_all(&mut self) {
        for view in &mut self.views {
            view.redraw = true;
        }
    }

    pub(crate) fn set_blanked(&mut self, blanked: bool) {
        if self.blanked == blanked {
            return;
        }
        self.blanked = blanked;
        for view in &self.views {
            if let Some(power) = &view.power {
                power.set_mode(if blanked {
                    OutputPowerMode::Off
                } else {
                    OutputPowerMode::On
                });
            }
        }
        if blanked {
            eprintln!("patin-lock: blanking the display");
        } else {
            eprintln!("patin-lock: waking the display");
            self.ever_woken = true;
            self.last_activity = Instant::now();
            self.redraw_all();
        }
    }

    fn check_idle(&mut self) {
        if self.blanked {
            return;
        }
        let timeout = if self.ever_woken {
            self.blank_after_typing
        } else {
            self.blank_after
        };
        if self.last_activity.elapsed() >= timeout {
            self.set_blanked(true);
        }
    }

    pub(crate) fn press(&mut self, key: Key) {
        if self.blanked {
            return;
        }
        self.last_activity = Instant::now();
        if key == Key::Enter {
            if let Some(password) = self.ui.take_password() {
                authenticate(self.username.clone(), password, self.auth_tx.clone());
                self.redraw_all();
            }
        } else if self.ui.press(key) {
            self.redraw_all();
        }
    }

    fn poll_auth(&mut self) {
        let Ok(result) = self.auth_rx.try_recv() else {
            return;
        };
        match result {
            AuthResult::Success => {
                if let Some(lock) = self.lock.take() {
                    lock.unlock();
                    let _ = self.connection.roundtrip();
                }
                self.unlocked = true;
                self.exit = true;
            }
            AuthResult::Failure(message) => {
                self.ui.failed(message);
                self.redraw_all();
            }
        }
    }

    pub(crate) fn draw_pending(&mut self, queue_handle: &QueueHandle<Self>) {
        if self.blanked {
            return;
        }
        for index in 0..self.views.len() {
            if self.views[index].redraw && !self.views[index].frame_pending {
                self.draw(index, queue_handle);
            }
        }
    }

    fn draw(&mut self, index: usize, queue_handle: &QueueHandle<Self>) {
        let Some((width, height)) = self.views[index].size else {
            return;
        };
        let scale = self.views[index].scale;
        let physical_width = scale.physical(width);
        let physical_height = scale.physical(height);
        let stride = physical_width as i32 * BYTES_PER_PIXEL as i32;
        // The strip draws over the lock's own background fill, so it has to
        // come second.
        let mut commands = self
            .ui
            .commands(width as f32, height as f32, &self.username);
        commands.extend(self.status.commands(width as f32));
        let view = &mut self.views[index];
        let Ok((buffer, canvas)) = view.pool.create_buffer(
            physical_width as i32,
            physical_height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) else {
            eprintln!("patin-lock: could not allocate render buffer");
            return;
        };
        if self
            .renderer
            .render_bar(canvas, physical_width, physical_height, scale, &commands)
            .is_err()
        {
            eprintln!("patin-lock: rendering failed");
            return;
        }
        let surface = view.lock_surface.wl_surface();
        surface.set_buffer_scale((scale.factor().round() as i32).max(1));
        surface.damage_buffer(0, 0, physical_width as i32, physical_height as i32);
        surface.frame(queue_handle, FrameCallbackData(surface.clone()));
        if buffer.attach_to(surface).is_err() {
            eprintln!("patin-lock: buffer attachment failed");
            return;
        }
        surface.commit();
        view.frame_pending = true;
        view.redraw = false;
    }
}
