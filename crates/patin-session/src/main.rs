mod actions;
mod ui;

use std::process::ExitCode;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel};

fn main() -> ExitCode {
    let config = LayerConfig {
        namespace: "patin-session".into(),
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
    match patin::platform::run(config, ui::SessionMenu::new(actions::configured())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-session: {error}");
            ExitCode::FAILURE
        }
    }
}
