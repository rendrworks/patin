mod auth;
mod keyboard;
mod ui;

use auth::{AuthResult, authenticate, effective_username};
use patin::render::{CpuRenderer, Scale};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    dispatch2::Dispatch2,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, Proxy, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_touch},
        },
        protocols_wlr::output_power_management::v1::client::{
            zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1,
            zwlr_output_power_v1::{
                Event as OutputPowerEvent, Mode as OutputPowerMode, ZwlrOutputPowerV1,
            },
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler},
        touch::TouchHandler,
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::{
    error::Error,
    path::Path,
    process::{Command, ExitCode},
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant},
};
use ui::{Key, KeyboardMode, LockUi};

const BYTES_PER_PIXEL: usize = 4;
const IDLE_BLANK_TIMEOUT: Duration = Duration::from_secs(1);
const IDLE_BLANK_TIMEOUT_ENTERING: Duration = Duration::from_secs(5);

static POWER_BUTTON_PRESSED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_power_button_signal(_signal: libc::c_int) {
    POWER_BUTTON_PRESSED.store(true, Ordering::SeqCst);
}

struct OutputPowerManagerData;

impl Dispatch2<ZwlrOutputPowerManagerV1, App> for OutputPowerManagerData {
    fn event(
        &self,
        _state: &mut App,
        _proxy: &ZwlrOutputPowerManagerV1,
        _event: <ZwlrOutputPowerManagerV1 as Proxy>::Event,
        _conn: &Connection,
        _qh: &QueueHandle<App>,
    ) {
    }
}

struct OutputPowerData;

impl Dispatch2<ZwlrOutputPowerV1, App> for OutputPowerData {
    fn event(
        &self,
        state: &mut App,
        proxy: &ZwlrOutputPowerV1,
        event: OutputPowerEvent,
        _conn: &Connection,
        _qh: &QueueHandle<App>,
    ) {
        match event {
            OutputPowerEvent::Mode { mode } => {
                eprintln!("patin-lock: output power mode changed to {mode:?}");
            }
            OutputPowerEvent::Failed => {
                if let Some(index) = state.view_for_power(proxy) {
                    eprintln!(
                        "patin-lock: output power control for output {index} is no longer valid"
                    );
                    state.views[index].power = None;
                }
            }
            _ => {}
        }
    }
}

struct View {
    output: wl_output::WlOutput,
    lock_surface: SessionLockSurface,
    pool: SlotPool,
    size: Option<(u32, u32)>,
    scale: Scale,
    frame_pending: bool,
    redraw: bool,
    power: Option<ZwlrOutputPowerV1>,
}

struct App {
    connection: Connection,
    registry_state: RegistryState,
    compositor: CompositorState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    lock: Option<SessionLock>,
    views: Vec<View>,
    keyboards: Vec<(wl_seat::WlSeat, wl_keyboard::WlKeyboard)>,
    pointers: Vec<(wl_seat::WlSeat, wl_pointer::WlPointer)>,
    touches: Vec<(wl_seat::WlSeat, wl_touch::WlTouch)>,
    renderer: CpuRenderer,
    ui: LockUi,
    username: String,
    auth_tx: Sender<AuthResult>,
    auth_rx: Receiver<AuthResult>,
    exit: bool,
    unlocked: bool,
    output_power_manager: Option<ZwlrOutputPowerManagerV1>,
    last_activity: Instant,
    blanked: bool,
    ever_woken: bool,
}

fn main() -> ExitCode {
    if std::env::args().any(|argument| argument == "--worker") {
        return match run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("patin-lock: {error}");
                ExitCode::from(2)
            }
        };
    }

    match supervise() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-lock: {error}");
            ExitCode::from(2)
        }
    }
}

fn supervise() -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let keypad_arg = std::env::args().find(|argument| argument.starts_with("--keypad="));
    loop {
        let mut command = Command::new(&executable);
        command.arg("--worker");
        if let Some(keypad_arg) = &keypad_arg {
            command.arg(keypad_arg);
        }
        let status = command.status()?;
        match status.code() {
            Some(0) => return Ok(()),
            Some(2) => return Err("lock worker stopped because of a terminal error".into()),
            code => {
                eprintln!(
                    "patin-lock: lock worker exited unexpectedly ({code:?}); restarting in one second"
                );
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    if std::env::args()
        .filter(|argument| argument == "--worker")
        .count()
        != 1
    {
        return Err("the lock worker must be started by the patin-lock supervisor".into());
    }
    if let Err(error) = run_lock() {
        eprintln!("patin-lock: {error}");
        return Err(error);
    }
    Ok(())
}

fn keyboard_mode_from_args() -> KeyboardMode {
    let value = std::env::args()
        .find_map(|argument| argument.strip_prefix("--keypad=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_LOCK_KEYPAD").ok());
    match value {
        Some(value) if value == "numeric" => KeyboardMode::Numeric,
        Some(value) if value == "full" => KeyboardMode::Full,
        Some(value) => {
            eprintln!("patin-lock: unrecognized --keypad value {value:?}; using full keyboard");
            KeyboardMode::Full
        }
        None => KeyboardMode::Full,
    }
}

fn run_lock() -> Result<(), Box<dyn Error>> {
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
        ui: LockUi::new(keyboard_mode_from_args()),
        username,
        auth_tx,
        auth_rx,
        exit: false,
        unlocked: false,
        output_power_manager,
        last_activity: Instant::now(),
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
        app.draw_pending(&queue_handle);
    }
    if app.unlocked {
        Ok(())
    } else {
        Err("session lock ended without a successful authentication".into())
    }
}

impl App {
    fn add_output(
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

    fn view_for_surface(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.views
            .iter()
            .position(|view| view.lock_surface.wl_surface() == surface)
    }

    fn view_for_power(&self, power: &ZwlrOutputPowerV1) -> Option<usize> {
        self.views
            .iter()
            .position(|view| view.power.as_ref() == Some(power))
    }

    fn redraw_all(&mut self) {
        for view in &mut self.views {
            view.redraw = true;
        }
    }

    fn set_blanked(&mut self, blanked: bool) {
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
            IDLE_BLANK_TIMEOUT_ENTERING
        } else {
            IDLE_BLANK_TIMEOUT
        };
        if self.last_activity.elapsed() >= timeout {
            self.set_blanked(true);
        }
    }

    fn press(&mut self, key: Key) {
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

    fn draw_pending(&mut self, queue_handle: &QueueHandle<Self>) {
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
        let commands = self
            .ui
            .commands(width as f32, height as f32, &self.username);
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

impl SessionLockHandler for App {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        eprintln!("patin-lock: session locked");
    }

    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        eprintln!("patin-lock: compositor refused or finished the lock");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        queue_handle: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _: u32,
    ) {
        if let Some(view) = self
            .views
            .iter_mut()
            .find(|view| view.lock_surface.wl_surface() == surface.wl_surface())
        {
            view.size = Some((configure.new_size.0.max(1), configure.new_size.1.max(1)));
            view.redraw = true;
        }
        self.draw_pending(queue_handle);
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if let Some(index) = self.view_for_surface(surface) {
            self.views[index].scale = Scale::from_integer(factor);
            self.views[index].redraw = true;
        }
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if let Some(index) = self.view_for_surface(surface) {
            self.views[index].frame_pending = false;
        }
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Err(error) = self.add_output(output, qh) {
            eprintln!("patin-lock: could not cover new output: {error}");
        }
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.views.retain(|view| view.output != output);
    }
}

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

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);
