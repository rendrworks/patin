//! What a config may change about the greeter.
//!
//! Like the lock screen and unlike everything else, a broken `init.lua` is
//! reported and stepped over rather than fatal: a greeter that refuses to
//! start is a machine nobody can sign into, and no colour is worth that.
//!
//! greetd still owns authentication. The socket, the session it is asked to
//! exec, and the account it authenticates are greetd's business; what a
//! config sets here is the *default* offered on screen, which the person
//! standing at the greeter can still change.

use patin_lua::Config;

use crate::ui::{KeyboardMode, Theme};

/// A compositor to fall back on when nothing else names one.
const DEFAULT_SESSION: &str = "0xin";

const OWNED: &[&str] = &["login"];
const KNOWN: &[&str] = &[
    "login.keypad",
    "login.session",
    "login.user",
    "login.greeting",
    "login.background",
    "login.accent",
    "login.foreground",
    "login.muted",
    "login.error",
    "login.field_fill",
];

pub(crate) struct Settings {
    config: Config,
    pub theme: Theme,
    pub mode: KeyboardMode,
    pub greeting: Option<String>,
}

impl Settings {
    pub fn load() -> Self {
        let config = Config::load_or_report("patin-login");
        config.warn_unknown(OWNED, KNOWN);
        Self {
            theme: Theme::from_config(&config),
            mode: keyboard_mode(&config),
            greeting: config.text(&["login.greeting"]).map(str::to_string),
            config,
        }
    }

    /// The session command greetd should exec. Whitespace-separated so a
    /// profile can pass arguments (`--session="0xin --debug"`).
    pub fn session_command(&self) -> Vec<String> {
        let value = std::env::args()
            .find_map(|argument| argument.strip_prefix("--session=").map(str::to_string))
            .or_else(|| std::env::var("PATIN_LOGIN_SESSION").ok())
            .or_else(|| self.config.text(&["login.session"]).map(str::to_string))
            .unwrap_or_else(|| DEFAULT_SESSION.to_string());
        let command: Vec<String> = value.split_whitespace().map(str::to_string).collect();
        if command.is_empty() {
            vec![DEFAULT_SESSION.to_string()]
        } else {
            command
        }
    }

    /// An operator-specified account, which overrides both memory and
    /// detection.
    pub fn requested_username(&self) -> Option<String> {
        std::env::args()
            .find_map(|argument| argument.strip_prefix("--user=").map(str::to_string))
            .or_else(|| std::env::var("PATIN_LOGIN_USER").ok())
            .or_else(|| self.config.text(&["login.user"]).map(str::to_string))
            .filter(|username| !username.is_empty())
    }
}

/// `--keypad=` beats `PATIN_LOGIN_KEYPAD`, which beats `login.keypad`. An
/// explicit argument has always had the last word in Patin.
fn keyboard_mode(config: &Config) -> KeyboardMode {
    let value = std::env::args()
        .find_map(|argument| argument.strip_prefix("--keypad=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_LOGIN_KEYPAD").ok())
        .or_else(|| config.text(&["login.keypad"]).map(str::to_string));
    match value.as_deref() {
        Some("numeric") => KeyboardMode::Numeric,
        Some("extended") => KeyboardMode::Extended,
        Some("full") | None => KeyboardMode::Full,
        Some(other) => {
            eprintln!("patin-login: unrecognized keypad value {other:?}; using full keyboard");
            KeyboardMode::Full
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_SESSION, Settings};
    use patin_lua::Config;

    fn settings(source: &str) -> Settings {
        let config = Config::from_source("init.lua", source).unwrap();
        Settings {
            theme: crate::ui::Theme::from_config(&config),
            mode: super::keyboard_mode(&config),
            greeting: config.text(&["login.greeting"]).map(str::to_string),
            config,
        }
    }

    #[test]
    fn without_a_config_the_greeter_offers_the_session_it_always_did() {
        // No --session argument and no environment override in the test
        // process, so this exercises the default.
        assert_eq!(
            settings("").session_command(),
            vec![DEFAULT_SESSION.to_string()]
        );
    }

    #[test]
    fn a_configured_session_is_split_into_a_command_and_its_arguments() {
        let settings = settings(r##"patin.login.session = "0xin --debug""##);
        assert_eq!(
            settings.session_command(),
            vec!["0xin".to_string(), "--debug".to_string()]
        );
    }

    #[test]
    fn a_configured_greeting_and_user_are_carried_through() {
        let settings = settings(
            r##"
            patin.login.greeting = "Staff only"
            patin.login.user = "sn3rt"
            "##,
        );
        assert_eq!(settings.greeting.as_deref(), Some("Staff only"));
        assert_eq!(settings.requested_username().as_deref(), Some("sn3rt"));
    }
}
