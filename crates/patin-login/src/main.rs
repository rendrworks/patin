//! Patin's greeter: a touch-friendly greetd greeter.
//!
//! Where `patin-lock` is a privileged `ext-session-lock-v1` client that talks
//! to PAM itself, this is an ordinary layer-shell client that talks to
//! greetd. greetd owns the authentication and the privileges; the greeter
//! only collects a username and password, relays them, and asks for the
//! session to start. That makes it a plain Patin consumer — it runs on
//! [`patin::platform::run`] like the bar and the on-screen keyboard, instead
//! of driving the Wayland queue by hand.

mod greetd;
mod sessions;
mod state;
mod ui;

use std::process::ExitCode;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use patin::{
    platform::{
        Anchors, KeyInput, KeyboardPolicy, LayerConfig, LayerLevel, LayerVisibility, Shell,
    },
    ui::{DrawCommand, Rect, Size},
};

use greetd::{Backend, LoginResult};
use sessions::Session;
use ui::{Key, KeyboardMode, LoginUi};

/// The session greetd starts once the credentials are accepted.
const DEFAULT_SESSION: &str = "0xin";

fn main() -> ExitCode {
    let backend = Backend::detect();
    if backend.is_preview() {
        eprintln!(
            "patin-login: GREETD_SOCK is not set; running in preview mode \
             (the UI renders, but sign-in is unavailable)"
        );
    }

    let config = LayerConfig {
        namespace: "patin-login".into(),
        // Overlay with an exclusive keyboard grab: a greeter is the only
        // thing on screen, and nothing behind it may receive the password.
        layer: LayerLevel::Overlay,
        anchors: Anchors {
            top: true,
            bottom: true,
            left: true,
            right: true,
        },
        size: (0, 0),
        exclusive_zone: -1,
        keyboard: KeyboardPolicy::Exclusive,
        visibility: LayerVisibility::Fixed,
    };

    match patin::platform::run(config, Greeter::new(backend)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-login: {error}");
            ExitCode::FAILURE
        }
    }
}

struct Greeter {
    ui: LoginUi,
    backend: Backend,
    sessions: Vec<Session>,
    selected: usize,
    results: Receiver<LoginResult>,
    result_sender: Sender<LoginResult>,
    size: Size,
    damage: Vec<Rect>,
    finished: bool,
}

impl Greeter {
    fn new(backend: Backend) -> Self {
        let (result_sender, results) = channel();
        let sessions = sessions::discover(&session_command());
        let remembered = state::load();
        // Whatever was used last, if it is still on offer; otherwise the
        // first, so a removed session degrades to a sensible default rather
        // than an empty selection.
        let selected = remembered
            .session
            .as_deref()
            .and_then(|name| sessions.iter().position(|session| session.name == name))
            .unwrap_or(0);
        let name = sessions
            .get(selected)
            .map(|session| session.name.clone())
            .unwrap_or_default();
        // An explicit --user= or PATIN_LOGIN_USER still wins over memory.
        let username = requested_username()
            .or(remembered.username)
            .unwrap_or_else(default_username);
        Self {
            ui: LoginUi::new(
                keyboard_mode_from_args(),
                username,
                hostname(),
                name,
                sessions.len() > 1,
            ),
            backend,
            sessions,
            selected,
            results,
            result_sender,
            size: Size::default(),
            damage: Vec::new(),
            finished: false,
        }
    }

    fn press(&mut self, key: Key) -> bool {
        if key == Key::Enter {
            return self.submit();
        }
        let handled = self.ui.press(key);
        if handled {
            self.damage_all();
        }
        handled
    }

    fn submit(&mut self) -> bool {
        let Some((username, password)) = self.ui.take_credentials() else {
            return false;
        };
        let chosen = self.sessions.get(self.selected);
        let command = chosen
            .map(|session| session.command.clone())
            .unwrap_or_else(session_command);
        if let Some(session) = chosen {
            state::save(&username, &session.name);
        }
        greetd::sign_in(
            &self.backend,
            username,
            password,
            command,
            self.result_sender.clone(),
        );
        self.damage_all();
        true
    }

    /// Move to the next advertised session. Cycling suits a handful of
    /// entries and needs no second surface to pick from.
    fn cycle_session(&mut self) {
        if self.sessions.len() < 2 {
            return;
        }
        self.selected = (self.selected + 1) % self.sessions.len();
        self.ui.set_session(self.sessions[self.selected].name.clone());
        self.damage_all();
    }

    fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
}

impl Shell for Greeter {
    fn resize(&mut self, size: Size) {
        if self.size != size {
            self.size = size;
            self.damage_all();
        }
    }

    fn poll_interval(&self) -> Duration {
        // Only drains a channel; short enough that the result of an attempt
        // lands as soon as greetd's PAM conversation returns.
        Duration::from_millis(200)
    }

    fn update(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.results.try_recv() {
            match result {
                // greetd is starting the session and will tear this greeter
                // down; exiting first keeps a dead surface off the screen.
                LoginResult::Success => self.finished = true,
                LoginResult::Failure(message) => self.ui.failed(message),
            }
            changed = true;
        }
        if changed {
            self.damage_all();
        }
        changed
    }

    fn activate_at(&mut self, position: (f64, f64)) -> bool {
        if self.ui.session_at(self.size.width, self.size.height, position) {
            self.cycle_session();
            return true;
        }
        if let Some(field) = self.ui.field_at(self.size.width, self.size.height, position) {
            if self.ui.focus != field {
                self.ui.focus = field;
                self.damage_all();
            }
            return true;
        }
        let Some(key) = self.ui.key_at(self.size.width, self.size.height, position) else {
            return false;
        };
        self.press(key)
    }

    fn key_input(&mut self, input: KeyInput) -> bool {
        match input {
            KeyInput::Text(text) => {
                let mut handled = false;
                for character in text.chars() {
                    handled |= self.press(Key::Character(character));
                }
                handled
            }
            KeyInput::Backspace => self.press(Key::Backspace),
            KeyInput::Enter => self.submit(),
            // A greeter has nowhere to escape to; the key only moves between
            // the two fields, which is what a physical keyboard's Tab does.
            KeyInput::Escape => {
                self.ui.toggle_focus();
                self.damage_all();
                true
            }
        }
    }

    fn close_requested(&self) -> bool {
        self.finished
    }

    fn commands(&self) -> Vec<DrawCommand> {
        self.ui.commands(self.size.width, self.size.height)
    }

    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }

    fn damage_all(&mut self) {
        Greeter::damage_all(self);
    }
}

fn keyboard_mode_from_args() -> KeyboardMode {
    let value = std::env::args()
        .find_map(|argument| argument.strip_prefix("--keypad=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_LOGIN_KEYPAD").ok());
    match value.as_deref() {
        Some("numeric") => KeyboardMode::Numeric,
        Some("extended") => KeyboardMode::Extended,
        Some("full") | None => KeyboardMode::Full,
        Some(other) => {
            eprintln!("patin-login: unrecognized --keypad value {other:?}; using full keyboard");
            KeyboardMode::Full
        }
    }
}

/// The session command greetd should exec. Whitespace-separated so a profile
/// can pass arguments (`--session="0xin --debug"`).
fn session_command() -> Vec<String> {
    let value = std::env::args()
        .find_map(|argument| argument.strip_prefix("--session=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_LOGIN_SESSION").ok())
        .unwrap_or_else(|| DEFAULT_SESSION.to_string());
    let command: Vec<String> = value.split_whitespace().map(str::to_string).collect();
    if command.is_empty() {
        vec![DEFAULT_SESSION.to_string()]
    } else {
        command
    }
}

/// An operator-specified account, which overrides both memory and detection.
fn requested_username() -> Option<String> {
    std::env::args()
        .find_map(|argument| argument.strip_prefix("--user=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_LOGIN_USER").ok())
        .filter(|username| !username.is_empty())
}

fn default_username() -> String {
    first_login_account(&std::fs::read_to_string("/etc/passwd").unwrap_or_default())
        .unwrap_or_default()
}

/// The first ordinary login account in a `passwd` file: a real UID (not a
/// system or `nobody` account) with a shell that can actually start a
/// session. Phones have exactly one, which is the user this greeter offers.
fn first_login_account(passwd: &str) -> Option<String> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let uid: u32 = fields.nth(1)?.parse().ok()?;
        let shell = fields.nth(3)?;
        let usable = (1000..65534).contains(&uid)
            && !shell.ends_with("nologin")
            && !shell.ends_with("false");
        usable.then(|| name.to_string())
    })
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Patin".into())
}

#[cfg(test)]
mod tests {
    use super::{first_login_account, session_command};

    #[test]
    fn picks_the_first_real_login_account() {
        let passwd = "\
root:x:0:0:root:/root:/bin/ash
nobody:x:65534:65534:nobody:/:/sbin/nologin
messagebus:x:101:101:messagebus:/:/sbin/nologin
sn3rt:x:10000:10000:,,,:/home/sn3rt:/bin/ash
other:x:10001:10001:,,,:/home/other:/bin/ash
";
        assert_eq!(first_login_account(passwd), Some("sn3rt".into()));
    }

    #[test]
    fn skips_system_and_shell_less_accounts() {
        let passwd = "\
root:x:0:0:root:/root:/bin/ash
greetd:x:114:120:greetd:/var/lib/greetd:/sbin/nologin
locked:x:1001:1001:,,,:/home/locked:/bin/false
";
        assert_eq!(first_login_account(passwd), None);
    }

    #[test]
    fn the_session_command_defaults_and_splits_arguments() {
        // No --session argument and no environment override in the test
        // process, so this exercises the default.
        assert_eq!(session_command(), vec!["0xin".to_string()]);
    }
}
