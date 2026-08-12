mod apps;
mod ui;

use std::process::ExitCode;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel, LayerVisibility};

fn main() -> ExitCode {
    let applications = apps::discover();
    let icon_count = applications
        .iter()
        .filter(|application| application.icon.is_some())
        .count();
    eprintln!(
        "patin-launcher: discovered {} launchable applications ({icon_count} resolved icons)",
        applications.len(),
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
        visibility: LayerVisibility::Fixed,
    };
    match patin::platform::run(config, ui::Launcher::new(applications)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-launcher: {error}");
            ExitCode::FAILURE
        }
    }
}
