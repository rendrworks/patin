use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
};

use patin_lua::Config;

/// What an action does, independent of how it is labelled. The logout row's
/// text is configurable, so the icon is chosen from this rather than by
/// matching on words that a shell is free to change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionKind {
    LogOut,
    Reboot,
    ShutDown,
}

impl ActionKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "logout" => Some(ActionKind::LogOut),
            "reboot" | "restart" => Some(ActionKind::Reboot),
            "power" | "shutdown" | "poweroff" => Some(ActionKind::ShutDown),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub label: String,
    pub kind: ActionKind,
    program: PathBuf,
    arguments: Vec<OsString>,
}

impl Action {
    fn new(label: &str, kind: ActionKind, program: impl Into<PathBuf>, arguments: &[&str]) -> Self {
        Self {
            label: label.into(),
            kind,
            program: program.into(),
            arguments: arguments.iter().map(OsString::from).collect(),
        }
    }

    pub fn launch(&self) -> Result<(), String> {
        Command::new(&self.program)
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not run {}: {error}", self.label))
    }

    #[cfg(test)]
    pub fn fixture(label: &str) -> Self {
        Self::new(label, ActionKind::LogOut, "/bin/true", &[])
    }
}

/// A row before it is ordered. Rows are keyed by name so a config that names
/// `reboot` twice replaces it rather than showing two Reboot buttons, and so
/// it can replace or delete one of Patin's own.
#[derive(Clone, Debug)]
struct Row {
    action: Action,
    order: i64,
}

const DEFAULT_ORDER: i64 = 100;

pub(crate) const OWNED: &[&str] = &["session", "actions"];
pub(crate) const KNOWN: &[&str] = &[
    "actions.*",
    "session.panel",
    "session.label",
    "session.error",
];

/// The menu's rows: Patin's own, then whatever `patin.actions` says about them.
///
/// ```lua
/// patin.actions["suspend"] = {
///   label = "Suspend", icon = "power", run = { "systemctl", "suspend" }, order = 15,
/// }
/// patin.actions["shutdown"] = false   -- take Patin's row away
/// ```
///
/// `false` removes a row, because a config that simply never mentions one must
/// leave it alone — that is the difference between "unset" and "off", and
/// `nil` cannot express it: a key assigned `nil` is a key that was never
/// there.
pub fn configured(config: &Config) -> Vec<Action> {
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();

    // A compositor exports these to add its own logout command. They still win
    // over the config file: an integration that ships a working logout row
    // must not stop working because someone wrote an `init.lua`.
    if let Some(program) = env::var_os("PATIN_SESSION_LOGOUT_PROGRAM") {
        let argument = env::var_os("PATIN_SESSION_LOGOUT_ARGUMENT");
        rows.insert(
            "logout".into(),
            Row {
                action: Action {
                    label: env::var("PATIN_SESSION_LOGOUT_LABEL")
                        .unwrap_or_else(|_| "Log out".into()),
                    kind: ActionKind::LogOut,
                    program: PathBuf::from(program),
                    arguments: argument.into_iter().collect(),
                },
                order: 10,
            },
        );
    }
    rows.insert(
        "reboot".into(),
        Row {
            action: Action::new("Reboot", ActionKind::Reboot, "systemctl", &["reboot"]),
            order: 20,
        },
    );
    rows.insert(
        "shutdown".into(),
        Row {
            action: Action::new(
                "Shut down",
                ActionKind::ShutDown,
                "systemctl",
                &["poweroff"],
            ),
            order: 30,
        },
    );

    apply(config, &mut rows);

    let mut ordered: Vec<(i64, String, Action)> = rows
        .into_iter()
        .map(|(name, row)| (row.order, name, row.action))
        .collect();
    // Sorted by the order a config gave, then by name: the menu must not
    // reshuffle itself between launches because a table iterated differently.
    ordered.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    ordered.into_iter().map(|(_, _, action)| action).collect()
}

fn apply(config: &Config, rows: &mut BTreeMap<String, Row>) {
    let Some(table) = config.table(&["actions"]) else {
        return;
    };
    for (name, value) in &table.map {
        if value.as_bool() == Some(false) {
            rows.remove(name);
            continue;
        }
        if name == "logout" && env::var_os("PATIN_SESSION_LOGOUT_PROGRAM").is_some() {
            continue;
        }
        let Some(entry) = value.as_table() else {
            eprintln!(
                "patin-session: actions.{name} expects a table or false, found {} — ignoring it",
                value.type_name()
            );
            continue;
        };
        let run_key = format!("actions.{name}.run");
        let Some(run) = config.strings(&[run_key.as_str()]) else {
            eprintln!(
                "patin-session: actions.{name} needs run = {{ \"program\", ... }} — ignoring it"
            );
            continue;
        };
        let label = entry
            .get("label")
            .and_then(|value| value.as_str())
            .unwrap_or(name)
            .to_string();
        let icon = entry.get("icon").and_then(|value| value.as_str());
        let kind = icon
            .and_then(ActionKind::from_name)
            .or_else(|| ActionKind::from_name(name))
            .unwrap_or_else(|| {
                eprintln!(
                    "patin-session: actions.{name} has no icon; use icon = \"logout\", \
                     \"reboot\", or \"power\""
                );
                ActionKind::ShutDown
            });
        let order = entry
            .get("order")
            .and_then(|value| value.as_f64())
            .map(|order| order as i64)
            .or_else(|| rows.get(name).map(|row| row.order))
            .unwrap_or(DEFAULT_ORDER);
        rows.insert(
            name.clone(),
            Row {
                action: Action {
                    label,
                    kind,
                    program: PathBuf::from(&run[0]),
                    arguments: run[1..].iter().map(OsString::from).collect(),
                },
                order,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, ActionKind, configured};
    use patin_lua::Config;

    #[test]
    fn action_keeps_program_and_arguments_separate() {
        let action = Action::new("Restart", ActionKind::Reboot, "systemctl", &["reboot"]);
        assert_eq!(action.program.to_string_lossy(), "systemctl");
        assert_eq!(action.arguments, ["reboot"]);
    }

    #[test]
    fn without_a_config_the_menu_is_the_two_rows_it_always_had() {
        let actions = configured(&Config::empty());
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, vec!["Reboot", "Shut down"]);
    }

    #[test]
    fn a_config_adds_replaces_removes_and_orders_rows() {
        let config = Config::from_source(
            "init.lua",
            r##"
            patin.actions["logout"] = {
              label = "Log out", icon = "logout", run = { "0xinctl", "exit" }, order = 10,
            }
            patin.actions["reboot"] = { label = "Restart", run = { "loginctl", "reboot" } }
            patin.actions["shutdown"] = false
            patin.actions["suspend"] = {
              label = "Suspend", icon = "power", run = { "systemctl", "suspend" }, order = 15,
            }
            "##,
        )
        .unwrap();
        let actions = configured(&config);
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, vec!["Log out", "Suspend", "Restart"]);
        assert_eq!(actions[0].program.to_string_lossy(), "0xinctl");
        assert_eq!(actions[0].arguments, ["exit"]);
        // Replacing a built-in keeps the position it already had.
        assert_eq!(actions[2].kind, ActionKind::Reboot);
    }

    #[test]
    fn an_entry_without_a_run_list_is_reported_and_skipped() {
        let config = Config::from_source(
            "init.lua",
            r##"patin.actions["broken"] = { label = "Broken" }"##,
        )
        .unwrap();
        let actions = configured(&config);
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, vec!["Reboot", "Shut down"]);
    }
}
