use std::{
    env,
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
};

/// What an action does, independent of how it is labelled. The logout row's
/// text is configurable, so the icon is chosen from this rather than by
/// matching on words that a shell is free to change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionKind {
    LogOut,
    Reboot,
    ShutDown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub label: String,
    pub kind: ActionKind,
    program: PathBuf,
    arguments: Vec<OsString>,
}

impl Action {
    fn new(
        label: &str,
        kind: ActionKind,
        program: impl Into<PathBuf>,
        arguments: &[&str],
    ) -> Self {
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

pub fn configured() -> Vec<Action> {
    let mut actions = Vec::new();
    if let Some(program) = env::var_os("PATIN_SESSION_LOGOUT_PROGRAM") {
        let argument = env::var_os("PATIN_SESSION_LOGOUT_ARGUMENT");
        actions.push(Action {
            label: env::var("PATIN_SESSION_LOGOUT_LABEL").unwrap_or_else(|_| "Log out".into()),
            kind: ActionKind::LogOut,
            program: PathBuf::from(program),
            arguments: argument.into_iter().collect(),
        });
    }
    actions.push(Action::new(
        "Reboot",
        ActionKind::Reboot,
        "systemctl",
        &["reboot"],
    ));
    actions.push(Action::new(
        "Shut down",
        ActionKind::ShutDown,
        "systemctl",
        &["poweroff"],
    ));
    actions
}

#[cfg(test)]
mod tests {
    use super::{Action, ActionKind};

    #[test]
    fn action_keeps_program_and_arguments_separate() {
        let action = Action::new("Restart", ActionKind::Reboot, "systemctl", &["reboot"]);
        assert_eq!(action.program.to_string_lossy(), "systemctl");
        assert_eq!(action.arguments, ["reboot"]);
    }
}
