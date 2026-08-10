mod ui;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel};
use std::process::ExitCode;

fn main() -> ExitCode {
    let page =
        std::env::args().find_map(|argument| argument.strip_prefix("--page=").map(str::to_owned));
    let config = LayerConfig {
        namespace: "patin-network-settings".into(),
        layer: LayerLevel::Overlay,
        anchors: Anchors {
            top: true,
            bottom: true,
            left: true,
            right: true,
        },
        size: (0, 0),
        exclusive_zone: 0,
        keyboard: KeyboardPolicy::OnDemand,
    };
    match patin::platform::run(config, ui::NetworkSettings::new(page.as_deref())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-network-settings: {error}");
            ExitCode::FAILURE
        }
    }
}
