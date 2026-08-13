//! Patin's session lock: a standalone `ext-session-lock-v1` client with
//! physical and on-screen keyboards and PAM authentication.
//!
//! The root holds the shared state ([`App`], [`View`]) and the supervisor
//! that restarts the locker if it ever dies while the session is locked.
//! Around it: [`app`] stands the lock up and paints it, [`surface`] and
//! [`input`] handle the compositor callbacks, and [`power`] blanks the
//! screen.

mod app;
mod auth;
mod input;
mod power;
mod surface;
mod ui;

use patin::render::{CpuRenderer, Scale};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::{
        client::{
            Connection,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_touch},
        },
        protocols_wlr::output_power_management::v1::client::{
            zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1,
            zwlr_output_power_v1::ZwlrOutputPowerV1,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::SeatState,
    session_lock::{SessionLock, SessionLockSurface},
    shm::{Shm, slot::SlotPool},
};
use std::{
    error::Error,
    process::{Command, ExitCode},
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

use auth::AuthResult;
use ui::{KeyboardMode, LockUi};

use app::run_lock;

const BYTES_PER_PIXEL: usize = 4;
const IDLE_BLANK_TIMEOUT: Duration = Duration::from_secs(1);
const IDLE_BLANK_TIMEOUT_ENTERING: Duration = Duration::from_secs(5);

static POWER_BUTTON_PRESSED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_power_button_signal(_signal: libc::c_int) {
    POWER_BUTTON_PRESSED.store(true, Ordering::SeqCst);
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

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);
