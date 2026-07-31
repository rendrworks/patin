mod apps;
mod ui;

use std::process::ExitCode;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel};

fn main() -> ExitCode {
    let applications = apps::discover();
    eprintln!(
        "patin-launcher: discovered {} launchable applications",
        applications.len()
    );
    let config = LayerConfig {
        namespace: "patin-launcher".into(),
        layer: LayerLevel::Overlay,
        anchors: Anchors {
            top: true,
            bottom: true,
            left: true,
            right: true,
        },
        size: (0, 0),
        exclusive_zone: 0,
        keyboard: KeyboardPolicy::None,
    };
    match patin::platform::run(config, ui::Launcher::new(applications)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-launcher: {error}");
            ExitCode::FAILURE
        }
    }
}
