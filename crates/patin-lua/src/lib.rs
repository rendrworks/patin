//! Lua configuration for Patin shell compositions.
//!
//! Patin's toolkit crate stays a toolkit: it knows nothing about this crate,
//! and a consumer that wants no config file never compiles a Lua VM. A
//! composition opts in by depending on `patin-lua`, calling [`Config::load`]
//! once at startup, and reading settings out of the result — every one of
//! them optional, because *a setting the config never mentions must be left
//! alone*. Missing means unset, never zero: that is what lets an explicit
//! `--keypad=` flag and a `PATIN_*` variable keep overriding the file.
//!
//! # The config file
//!
//! `$XDG_CONFIG_HOME/patin/init.lua`, else `~/.config/patin/init.lua`. It is
//! a list of statements, not a table to return, so it can branch on the
//! machine it is running on:
//!
//! ```lua
//! local patin = require("patin")
//!
//! patin.theme.accent = "#7c3aed"
//! patin.bar.pill = { width = 32, height = 10, gap = 12, radius = 5 }
//!
//! if patin.which("0xinctl") then
//!   patin.actions["logout"] = { label = "Log out", run = { "0xinctl", "exit" } }
//! end
//! ```
//!
//! Nothing is returned and nothing is merged by hand. The file assigns
//! settings and registers descriptions; Patin reads them off afterwards.

mod host;
mod value;

use std::path::{Path, PathBuf};
use std::time::Duration;

use patin::ui::Color;

pub use host::ConfigError;
pub use value::{Table, Value};

/// Where a config was found, and everything it assigned.
#[derive(Debug)]
pub struct Config {
    table: Table,
    source: Option<PathBuf>,
}

impl Config {
    /// The config for this process: `--config=`, then `PATIN_CONFIG`, then the
    /// XDG path. `PATIN_NO_CONFIG=1` skips all of it — the first question when
    /// a shell misbehaves is "is it me or my config?", and a tool with no way
    /// to start without one makes that unanswerable.
    ///
    /// A missing XDG file is not an error; a path the user named explicitly
    /// and got wrong is.
    pub fn load() -> Result<Self, ConfigError> {
        if disabled() {
            return Ok(Self::empty());
        }
        match explicit_path() {
            Some(path) => Self::load_path(&path),
            None => match default_path() {
                Some(path) if path.exists() => Self::load_path(&path),
                _ => Ok(Self::empty()),
            },
        }
    }

    /// [`Config::load`], but a broken config is reported and stepped over.
    ///
    /// For `patin-lock` and `patin-login` only. Everywhere else a bad config
    /// is fatal, because carrying on would silently apply settings the user
    /// did not ask for — but those two stand between a person and their
    /// machine, and a typo in `init.lua` must never be the reason a phone
    /// cannot be unlocked or signed into.
    pub fn load_or_report(program: &str) -> Self {
        match Self::load() {
            Ok(config) => config,
            Err(error) => {
                eprintln!("{program}: {error}");
                eprintln!("{program}: continuing with built-in defaults");
                Self::empty()
            }
        }
    }

    pub fn load_path(path: &Path) -> Result<Self, ConfigError> {
        let source = std::fs::read(path).map_err(|error| ConfigError::Read {
            path: path.to_path_buf(),
            error,
        })?;
        Ok(Self {
            table: host::evaluate(path, &source)?,
            source: Some(path.to_path_buf()),
        })
    }

    pub fn empty() -> Self {
        Self {
            table: Table::default(),
            source: None,
        }
    }

    /// The file this came from, if any. Compositions quote it in `PATIN_TRACE`
    /// output so "my config is being ignored" is answerable.
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn root(&self) -> &Table {
        &self.table
    }

    /// The first of `keys` the config actually set.
    ///
    /// Call sites pass a specific key before a shared one —
    /// `&["lock.accent", "theme.accent"]` — so one `patin.theme.accent` can
    /// colour every composition while any single one keeps the last word.
    pub fn get(&self, keys: &[&str]) -> Option<&Value> {
        keys.iter().find_map(|key| self.table.path(key))
    }

    pub fn text(&self, keys: &[&str]) -> Option<&str> {
        self.typed(keys, "a string", Value::as_str)
    }

    pub fn boolean(&self, keys: &[&str]) -> Option<bool> {
        self.typed(keys, "true or false", Value::as_bool)
    }

    pub fn number(&self, keys: &[&str]) -> Option<f32> {
        self.typed(keys, "a number", |value| value.as_f64().map(|n| n as f32))
    }

    pub fn count(&self, keys: &[&str]) -> Option<u32> {
        self.typed(keys, "a whole number", |value| {
            value
                .as_f64()
                .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                .map(|n| n as u32)
        })
    }

    /// Durations are written in seconds, fractions allowed: `blank_after = 1.5`.
    pub fn seconds(&self, keys: &[&str]) -> Option<Duration> {
        self.typed(keys, "a number of seconds", |value| {
            value
                .as_f64()
                .filter(|n| *n >= 0.0 && n.is_finite())
                .map(Duration::from_secs_f64)
        })
    }

    pub fn color(&self, keys: &[&str]) -> Option<Color> {
        self.typed(keys, "a colour like \"#7c3aed\"", Value::as_color)
    }

    pub fn table(&self, keys: &[&str]) -> Option<&Table> {
        self.typed(keys, "a table", Value::as_table)
    }

    /// An array of strings — a program and its arguments, most often.
    pub fn strings(&self, keys: &[&str]) -> Option<Vec<String>> {
        self.typed(keys, "a list of strings", |value| {
            let table = value.as_table()?;
            table
                .array
                .iter()
                .map(|entry| entry.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .filter(|list| !list.is_empty())
        })
    }

    /// A value of the wrong type is reported rather than quietly ignored.
    ///
    /// This is the cost of assigning settings straight onto the module table:
    /// the host cannot tell a typo from the config stashing a helper of its
    /// own, so it must say what it saw and carry on with the default.
    fn typed<'a, T>(
        &'a self,
        keys: &[&str],
        expected: &str,
        extract: impl Fn(&'a Value) -> Option<T>,
    ) -> Option<T> {
        for key in keys {
            let Some(value) = self.table.path(key) else {
                continue;
            };
            match extract(value) {
                Some(value) => return Some(value),
                None => {
                    self.warn(format!(
                        "{key} expects {expected}, found {} — using the default",
                        value.type_name()
                    ));
                    return None;
                }
            }
        }
        None
    }

    /// Report settings inside namespaces this composition owns that it does
    /// not recognise. A `known` entry ending in `.*` accepts anything below
    /// it, which is how user-named entries such as `actions.reboot.label`
    /// avoid being called typos.
    pub fn warn_unknown(&self, owned: &[&str], known: &[&str]) {
        let mut leaves = Vec::new();
        self.table.leaf_paths("", &mut leaves);
        for leaf in leaves {
            let mine = owned
                .iter()
                .any(|namespace| leaf == *namespace || leaf.starts_with(&format!("{namespace}.")));
            if !mine {
                continue;
            }
            let recognised = known.iter().any(|key| match key.strip_suffix(".*") {
                Some(prefix) => leaf.starts_with(&format!("{prefix}.")),
                None => leaf == *key,
            });
            if !recognised {
                self.warn(format!("unknown setting {leaf}"));
            }
        }
    }

    fn warn(&self, message: String) {
        match &self.source {
            Some(path) => eprintln!("patin config: {}: {message}", path.display()),
            None => eprintln!("patin config: {message}"),
        }
    }

    /// Evaluate config source directly, naming the chunk for error messages.
    ///
    /// Compositions use this to test their own settings without writing a
    /// file, which is what keeps `cargo test` free of a home directory.
    pub fn from_source(name: &str, source: &str) -> Result<Self, ConfigError> {
        Ok(Self {
            table: host::evaluate(Path::new(name), source.as_bytes())?,
            source: None,
        })
    }
}

fn disabled() -> bool {
    matches!(std::env::var("PATIN_NO_CONFIG"), Ok(value) if !value.is_empty() && value != "0")
}

/// `--config=` first, so a flag always beats the environment — the rule the
/// rest of Patin's arguments already follow.
fn explicit_path() -> Option<PathBuf> {
    std::env::args()
        .find_map(|argument| argument.strip_prefix("--config=").map(PathBuf::from))
        .or_else(|| std::env::var_os("PATIN_CONFIG").map(PathBuf::from))
}

fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("patin").join("init.lua"))
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};
    use patin::ui::Color;

    #[test]
    fn settings_are_assigned_onto_the_parked_namespaces() {
        let config = Config::from_source(
            "init.lua",
            r##"
            local patin = require("patin")
            patin.theme.accent = "#7c3aed"
            patin.bar.height = 14
            patin.bar.pill = { width = 32, gap = 12.5 }
            patin.lock.keypad = "full"
            patin.lock.blank_after = 1.5
            "##,
        )
        .unwrap();

        assert_eq!(
            config.color(&["theme.accent"]),
            Some(Color(124, 58, 237, 255))
        );
        assert_eq!(config.number(&["bar.height"]), Some(14.0));
        assert_eq!(config.number(&["bar.pill.gap"]), Some(12.5));
        assert_eq!(config.text(&["lock.keypad"]), Some("full"));
        assert_eq!(
            config.seconds(&["lock.blank_after"]),
            Some(std::time::Duration::from_millis(1500))
        );
    }

    #[test]
    fn an_unmentioned_setting_stays_unset_rather_than_becoming_zero() {
        let config = Config::from_source("init.lua", "patin.bar.height = 14").unwrap();
        assert_eq!(config.number(&["bar.height"]), Some(14.0));
        assert_eq!(config.number(&["bar.pill.width"]), None);
        assert_eq!(config.text(&["login.session"]), None);
    }

    #[test]
    fn a_specific_key_wins_over_the_shared_one_it_falls_back_to() {
        let config = Config::from_source(
            "init.lua",
            r##"
            patin.theme.accent = "#7c3aed"
            patin.login.accent = "#52c4ba"
            "##,
        )
        .unwrap();
        assert_eq!(
            config.color(&["login.accent", "theme.accent"]),
            Some(Color(82, 196, 186, 255))
        );
        assert_eq!(
            config.color(&["lock.accent", "theme.accent"]),
            Some(Color(124, 58, 237, 255))
        );
    }

    #[test]
    fn the_config_can_branch_on_the_machine_it_is_running_on() {
        // SAFETY: single-threaded test process, and the variable is read back
        // only by the Lua chunk below.
        unsafe { std::env::set_var("PATIN_TEST_PROBE", "yes") };
        let config = Config::from_source(
            "init.lua",
            r##"
            if patin.env("PATIN_TEST_PROBE") == "yes" then
              patin.bar.height = 20
            else
              patin.bar.height = 10
            end
            patin.bar.shell_found = patin.exists("/bin/sh")
            patin.bar.missing = patin.env("PATIN_TEST_ABSENT")
            "##,
        )
        .unwrap();
        assert_eq!(config.number(&["bar.height"]), Some(20.0));
        assert_eq!(config.boolean(&["bar.shell_found"]), Some(true));
        assert_eq!(config.get(&["bar.missing"]), None);
        unsafe { std::env::remove_var("PATIN_TEST_PROBE") };
    }

    #[test]
    fn keyed_registration_is_idempotent_so_re_registering_replaces() {
        let config = Config::from_source(
            "init.lua",
            r##"
            patin.actions["reboot"] = { label = "Reboot", run = { "systemctl", "reboot" } }
            patin.actions["reboot"] = { label = "Restart", run = { "loginctl", "reboot" } }
            for _, name in ipairs({ "a", "b" }) do
              patin.actions[name] = { label = name:upper(), run = { "true" } }
            end
            "##,
        )
        .unwrap();
        assert_eq!(config.text(&["actions.reboot.label"]), Some("Restart"));
        assert_eq!(
            config.strings(&["actions.reboot.run"]),
            Some(vec!["loginctl".into(), "reboot".into()])
        );
        assert_eq!(config.text(&["actions.a.label"]), Some("A"));
        assert_eq!(config.text(&["actions.b.label"]), Some("B"));
    }

    #[test]
    fn a_parse_error_names_the_file_and_the_line() {
        let error = Config::from_source("init.lua", "patin.bar.height =\n").unwrap_err();
        let message = error.to_string();
        assert!(matches!(error, ConfigError::Parse { .. }), "{message}");
        assert!(message.starts_with("init.lua:"), "{message}");
        assert!(message.contains("line 2"), "{message}");
    }

    #[test]
    fn a_runtime_error_names_the_file_and_the_line() {
        let error = Config::from_source("init.lua", "local t = nil\nt.x = 1\n").unwrap_err();
        let message = error.to_string();
        assert!(matches!(error, ConfigError::Run { .. }), "{message}");
        assert!(message.contains("init.lua:2"), "{message}");
    }

    #[test]
    fn a_config_that_never_finishes_is_stopped_rather_than_hanging_the_shell() {
        let error = Config::from_source("init.lua", "while true do end").unwrap_err();
        assert!(matches!(error, ConfigError::Budget { .. }));
    }

    #[test]
    fn requiring_anything_but_patin_says_so_instead_of_failing_later() {
        let error = Config::from_source("init.lua", "local x = require('socket')").unwrap_err();
        assert!(error.to_string().contains("no module 'socket'"), "{error}");
    }

    #[test]
    fn the_sandbox_withholds_io_and_os() {
        let error = Config::from_source("init.lua", "os.execute('true')").unwrap_err();
        assert!(matches!(error, ConfigError::Run { .. }), "{error}");
        let error = Config::from_source("init.lua", "io.open('/etc/passwd')").unwrap_err();
        assert!(matches!(error, ConfigError::Run { .. }), "{error}");
    }

    #[test]
    fn a_self_referencing_table_is_bounded_rather_than_recursing_forever() {
        let config = Config::from_source("init.lua", "patin.theme.self = patin").unwrap();
        assert!(config.get(&["theme.self"]).is_some());
    }
}
