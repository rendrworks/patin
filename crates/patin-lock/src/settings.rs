//! What a config may change about the lock screen, and what it may not.
//!
//! A broken `init.lua` is *reported and stepped over* here, unlike everywhere
//! else in Patin. The lock screen stands between a person and their own
//! machine: refusing to start because of a typo in a colour would mean a
//! phone that cannot be unlocked, which is a far worse failure than running
//! with the built-in palette and a line on stderr.
//!
//! Nothing here can weaken authentication. PAM's service name, the policy
//! path, and the password limit are not settings, and the supervisor still
//! restarts a worker that dies whatever the config says.

use std::time::Duration;

use patin_lua::Config;

use crate::ui::{KeyboardMode, Theme};

/// Blanking is deliberately fast on an untouched screen and slow once someone
/// has started typing: a phone in a pocket should go dark almost at once, but
/// blanking mid-password would be maddening.
const BLANK_AFTER: Duration = Duration::from_secs(1);
const BLANK_AFTER_TYPING: Duration = Duration::from_secs(5);

const OWNED: &[&str] = &["lock"];
const KNOWN: &[&str] = &[
    "lock.keypad",
    "lock.blank_after",
    "lock.blank_after_typing",
    "lock.background",
    "lock.accent",
    "lock.foreground",
    "lock.muted",
    "lock.error",
    "lock.field_fill",
    "lock.field_border",
];

pub(crate) struct Settings {
    pub theme: Theme,
    pub mode: KeyboardMode,
    pub blank_after: Duration,
    pub blank_after_typing: Duration,
}

impl Settings {
    pub fn load() -> Self {
        let config = Config::load_or_report("patin-lock");
        config.warn_unknown(OWNED, KNOWN);
        Self {
            theme: Theme::from_config(&config),
            mode: keyboard_mode(&config),
            blank_after: config.seconds(&["lock.blank_after"]).unwrap_or(BLANK_AFTER),
            blank_after_typing: config
                .seconds(&["lock.blank_after_typing"])
                .unwrap_or(BLANK_AFTER_TYPING),
        }
    }
}

/// `--keypad=` beats `PATIN_LOCK_KEYPAD`, which beats `lock.keypad`, which
/// beats the built-in default. An explicit argument has always had the last
/// word in Patin, and a config file arriving does not change that.
fn keyboard_mode(config: &Config) -> KeyboardMode {
    let value = std::env::args()
        .find_map(|argument| argument.strip_prefix("--keypad=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_LOCK_KEYPAD").ok())
        .or_else(|| config.text(&["lock.keypad"]).map(str::to_string));
    match value {
        Some(value) if value == "numeric" => KeyboardMode::Numeric,
        Some(value) if value == "full" => KeyboardMode::Full,
        Some(value) => {
            eprintln!("patin-lock: unrecognized keypad value {value:?}; using full keyboard");
            KeyboardMode::Full
        }
        None => KeyboardMode::Full,
    }
}
