mod ui;

use std::process::ExitCode;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel, LayerVisibility};
use patin_lua::Config;

fn main() -> ExitCode {
    // Fatal, unlike the lock screen and the greeter: this bar guards nothing,
    // and starting it with settings the user did not ask for is worse than
    // saying what is wrong with the file.
    let settings = match Config::load() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("patin-workspaces-bar: {error}");
            return ExitCode::FAILURE;
        }
    };
    let style = ui::Style::from_config(&settings);

    let config = LayerConfig {
        namespace: "patin-workspaces-bar".into(),
        // Bottom, not Top: 0xin's arrange_layers processes numeric layers
        // background < bottom < top < overlay in that fixed order, applying
        // each surface's exclusive zone to one shared usable box as it goes.
        // Within a layer, order instead follows surface-creation order — so
        // if the bar were Top (same as the OSK), whichever of the two was
        // created first claims the true bottom edge and the other is pushed
        // above it, meaning the bar would jump whenever the OSK is launched
        // or restarted before it. Bottom is processed before Top always,
        // regardless of creation order, so the bar deterministically claims
        // the true bottom edge first, and the keyboard — positioned
        // afterward against the now-shrunk box — stacks above the bar
        // instead of the reverse.
        layer: LayerLevel::Bottom,
        anchors: Anchors {
            top: false,
            bottom: true,
            left: true,
            right: true,
        },
        size: (0, style.height as u32),
        // A real reservation, like the OSK's own exclusive zone: windows
        // (and, per the layer ordering above, the keyboard) shrink to leave
        // room for the strip, so nothing ever overlaps it.
        exclusive_zone: style.height as i32,
        keyboard: KeyboardPolicy::None,
        visibility: LayerVisibility::Fixed,
    };

    match patin::platform::run(config, ui::WorkspacesBarShell::new(style)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-workspaces-bar: {error}");
            ExitCode::FAILURE
        }
    }
}
