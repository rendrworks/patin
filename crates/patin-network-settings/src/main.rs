mod ui;

use patin::platform::WindowConfig;
use std::process::ExitCode;

fn main() -> ExitCode {
    let page =
        std::env::args().find_map(|argument| argument.strip_prefix("--page=").map(str::to_owned));
    let config = WindowConfig {
        app_id: "patin-network-settings".into(),
        title: "Network Settings".into(),
        initial_size: (420, 720),
        min_size: Some((320, 480)),
    };
    match patin::platform::run_window(config, ui::NetworkSettings::new(page.as_deref())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-network-settings: {error}");
            ExitCode::FAILURE
        }
    }
}
