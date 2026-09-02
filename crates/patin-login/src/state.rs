//! Remembering the last successful choice.
//!
//! A greeter that forgets which session you picked makes you re-pick it every
//! single boot, so the selection is written to a small state file and read
//! back on the next start. The username is remembered alongside it, which is
//! what makes a multi-account machine come up on the right one.
//!
//! Persistence is strictly best-effort. A greeter must start even when the
//! state directory is read-only, missing, or owned by someone else, so every
//! failure here is silently ignored — the cost is only that the previous
//! choice is not restored.

use std::fs;
use std::path::PathBuf;

const STATE_FILE: &str = "patin-login/last-session";

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Remembered {
    pub username: Option<String>,
    pub session: Option<String>,
}

pub fn load() -> Remembered {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|text| parse(&text))
        .unwrap_or_default()
}

/// Record the choice just made. Called when credentials are submitted rather
/// than when greetd confirms them: greetd tears the greeter down the moment a
/// session starts, so waiting for success risks losing the write to the
/// teardown.
pub fn save(username: &str, session: &str) {
    let Some(path) = path() else {
        return;
    };
    if let Some(directory) = path.parent()
        && fs::create_dir_all(directory).is_err()
    {
        return;
    }
    let contents = format!("user={username}\nsession={session}\n");
    fs::write(&path, contents).ok();
}

fn path() -> Option<PathBuf> {
    // An explicit override first, so a session script can place the file
    // somewhere the greeter user is known to be able to write.
    if let Some(explicit) = std::env::var_os("PATIN_LOGIN_STATE")
        && !explicit.is_empty()
    {
        return Some(PathBuf::from(explicit));
    }
    if let Some(state_home) = std::env::var_os("XDG_STATE_HOME")
        && !state_home.is_empty()
    {
        return Some(PathBuf::from(state_home).join(STATE_FILE));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local/state").join(STATE_FILE))
}

fn parse(text: &str) -> Remembered {
    let mut remembered = Remembered::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "user" => remembered.username = Some(value.to_string()),
            // Session names carry spaces ("0xin Touch Test"), so the value is
            // the rest of the line verbatim rather than a first word.
            "session" => remembered.session = Some(value.to_string()),
            _ => {}
        }
    }
    remembered
}

#[cfg(test)]
mod tests {
    use super::{Remembered, parse};

    #[test]
    fn reads_back_a_saved_choice() {
        assert_eq!(
            parse("user=sn3rt\nsession=0xin Touch Test\n"),
            Remembered {
                username: Some("sn3rt".into()),
                session: Some("0xin Touch Test".into()),
            }
        );
    }

    #[test]
    fn a_missing_or_damaged_file_is_simply_no_memory() {
        assert_eq!(parse(""), Remembered::default());
        assert_eq!(parse("nonsense\n"), Remembered::default());
        assert_eq!(parse("user=\nsession=\n"), Remembered::default());
    }

    #[test]
    fn a_partial_file_still_yields_what_it_has() {
        assert_eq!(
            parse("session=Phosh\n"),
            Remembered {
                username: None,
                session: Some("Phosh".into()),
            }
        );
    }

    #[test]
    fn saving_then_loading_round_trips_through_a_real_file() {
        let directory =
            std::env::temp_dir().join(format!("patin-login-state-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("last-session");
        // SAFETY: single-threaded test, and the variable is restored below.
        unsafe { std::env::set_var("PATIN_LOGIN_STATE", &file) };

        super::save("sn3rt", "0xin Touch Test");
        let remembered = super::load();

        unsafe { std::env::remove_var("PATIN_LOGIN_STATE") };
        std::fs::remove_dir_all(&directory).ok();

        assert_eq!(remembered.username.as_deref(), Some("sn3rt"));
        assert_eq!(remembered.session.as_deref(), Some("0xin Touch Test"));
    }
}
