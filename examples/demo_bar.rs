#[path = "demo_bar/scene.rs"]
mod scene;
#[path = "demo_bar/services.rs"]
mod services;

use std::process::ExitCode;

fn main() -> ExitCode {
    let config = patin::platform::LayerConfig {
        namespace: "patin-demo-bar".into(),
        layer: patin::platform::LayerLevel::Top,
        anchors: patin::platform::Anchors {
            top: true,
            left: true,
            right: true,
            ..Default::default()
        },
        size: (0, 32),
        exclusive_zone: 32,
        keyboard: patin::platform::KeyboardPolicy::None,
    };
    match patin::platform::run(config, scene::DemoBar::new()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("demo_bar: {error}");
            ExitCode::FAILURE
        }
    }
}
