mod actions;
mod ui;

use std::process::ExitCode;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel, LayerVisibility};
use patin_lua::Config;

fn main() -> ExitCode {
    let settings = match Config::load() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("patin-session: {error}");
            return ExitCode::FAILURE;
        }
    };
    settings.warn_unknown(actions::OWNED, actions::KNOWN);
    let palette = ui::Palette::from_config(&settings);

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
        visibility: LayerVisibility::Fixed,
    };
    match patin::platform::run(
        config,
        ui::SessionMenu::new(actions::configured(&settings), palette),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-session: {error}");
            ExitCode::FAILURE
        }
    }
}
