mod ui;

use std::process::ExitCode;

use patin::platform::{Anchors, KeyboardPolicy, LayerConfig, LayerLevel};
use patin_keyboard::KeyboardMode;

fn main() -> ExitCode {
    let mode = keyboard_mode_from_args();

    // Both layouts' height math is independent of width once the screen is
    // wider than a single row of keys (true for any real phone or desktop
    // display), so this placeholder only matters on implausibly narrow
    // outputs. See `patin_keyboard::footprint_height`.
    let height = patin_keyboard::footprint_height(mode, 400.0) as u32;

    let config = LayerConfig {
        namespace: "patin-osk".into(),
        layer: LayerLevel::Top,
        anchors: Anchors {
            top: false,
            bottom: true,
            left: true,
            right: true,
        },
        size: (0, height),
        exclusive_zone: height as i32,
        keyboard: KeyboardPolicy::None,
    };

    match patin::platform::run(config, ui::OskShell::new(mode)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("patin-osk: {error}");
            ExitCode::FAILURE
        }
    }
}

fn keyboard_mode_from_args() -> KeyboardMode {
    let value = std::env::args()
        .find_map(|argument| argument.strip_prefix("--keypad=").map(str::to_string))
        .or_else(|| std::env::var("PATIN_OSK_KEYPAD").ok());
    match value {
        Some(value) if value == "numeric" => KeyboardMode::Numeric,
        Some(value) if value == "full" => KeyboardMode::Full,
        Some(value) if value == "extended" => KeyboardMode::Extended,
        Some(value) => {
            eprintln!("patin-osk: unrecognized --keypad value {value:?}; using full keyboard");
            KeyboardMode::Full
        }
        None => KeyboardMode::Full,
    }
}
