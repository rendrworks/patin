//! Reusable vector icons for Patin shells and applications.
//!
//! Icons return ordinary [`patin::ui::DrawCommand`] values, so consumers can
//! compose them without depending on a particular shell composition.

use patin::ui::{Color, DrawCommand, Rect};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiSignal {
    Unavailable,
    Poor,
    Medium,
    Good,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumeLevel {
    Off,
    Low,
    Medium,
    High,
}

impl VolumeLevel {
    pub fn from_percentage(percentage: u8, muted: bool) -> Self {
        if muted || percentage == 0 {
            return Self::Off;
        }
        match percentage.min(100) {
            1..=33 => Self::Low,
            34..=66 => Self::Medium,
            _ => Self::High,
        }
    }
}

impl WifiSignal {
    pub fn from_percentage(percentage: u8) -> Self {
        match percentage.min(100) {
            0..=33 => Self::Poor,
            34..=66 => Self::Medium,
            _ => Self::Good,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IconPalette {
    pub foreground: Color,
    pub muted: Color,
    pub background: Color,
    pub accent: Color,
    pub unavailable: Color,
}

pub fn battery(
    bounds: Rect,
    percentage: u8,
    charging: bool,
    palette: IconPalette,
) -> Vec<DrawCommand> {
    let icon = centered(bounds, 24.0, 16.0);
    let body = Rect::new(icon.origin.x, icon.origin.y + 2.0, 20.0, 12.0);
    let interior = body.inset(2.0);
    let level_width = interior.size.width * f32::from(percentage.min(100)) / 100.0;
    let level_color = if percentage <= 15 {
        palette.unavailable
    } else if charging {
        palette.accent
    } else {
        palette.foreground
    };
    let mut commands = vec![
        rounded(body, palette.foreground, 2.5),
        rounded(interior, palette.background, 1.0),
        rounded(
            Rect::new(
                interior.origin.x,
                interior.origin.y,
                level_width,
                interior.size.height,
            ),
            level_color,
            1.0,
        ),
        rounded(
            Rect::new(icon.origin.x + 21.0, icon.origin.y + 6.0, 3.0, 4.0),
            palette.foreground,
            1.0,
        ),
    ];
    if charging {
        commands.push(fill(
            Rect::new(icon.origin.x + 9.0, icon.origin.y + 4.0, 2.0, 8.0),
            palette.background,
        ));
    }
    commands
}

pub fn wifi_signal(bounds: Rect, signal: WifiSignal, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 24.0, 24.0);
    if signal == WifiSignal::Unavailable {
        return unavailable_cross(icon, palette.unavailable);
    }

    let mut commands = Vec::new();
    wifi_arc(
        &mut commands,
        icon,
        3.0,
        if signal == WifiSignal::Good {
            palette.foreground
        } else {
            palette.muted
        },
        palette.background,
    );
    wifi_arc(
        &mut commands,
        icon.inset(4.0),
        3.0,
        if matches!(signal, WifiSignal::Medium | WifiSignal::Good) {
            palette.foreground
        } else {
            palette.muted
        },
        palette.background,
    );
    commands.push(rounded(
        Rect::new(icon.origin.x + 10.0, icon.origin.y + 15.0, 4.0, 4.0),
        palette.foreground,
        2.0,
    ));
    commands
}

pub fn volume(bounds: Rect, level: VolumeLevel, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 24.0, 18.0);
    let mut commands = vec![
        rounded(
            Rect::new(icon.origin.x, icon.origin.y + 6.0, 5.0, 6.0),
            palette.foreground,
            1.0,
        ),
        rounded(
            Rect::new(icon.origin.x + 4.0, icon.origin.y + 3.0, 4.0, 12.0),
            palette.foreground,
            1.5,
        ),
    ];
    if level == VolumeLevel::Off {
        commands.extend(cross(
            Rect::new(icon.origin.x + 14.0, icon.origin.y + 5.0, 8.0, 8.0),
            palette.foreground,
        ));
        return commands;
    }

    let active = match level {
        VolumeLevel::Off => 0,
        VolumeLevel::Low => 1,
        VolumeLevel::Medium => 2,
        VolumeLevel::High => 3,
    };
    for (index, height) in [4.0, 8.0, 12.0].into_iter().enumerate() {
        commands.push(rounded(
            Rect::new(
                icon.origin.x + 11.0 + index as f32 * 5.0,
                icon.origin.y + (18.0 - height) / 2.0,
                2.0,
                height,
            ),
            if index < active {
                palette.foreground
            } else {
                palette.muted
            },
            1.0,
        ));
    }
    commands
}

pub fn cellular_signal(bounds: Rect, percentage: u8, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 22.0, 18.0);
    let active = match percentage.min(100) {
        0 => 0,
        1..=25 => 1,
        26..=50 => 2,
        51..=75 => 3,
        _ => 4,
    };
    (0..4)
        .map(|index| {
            let height = 4.0 + index as f32 * 4.0;
            rounded(
                Rect::new(
                    icon.origin.x + index as f32 * 6.0,
                    icon.origin.y + 18.0 - height,
                    4.0,
                    height,
                ),
                if index < active {
                    palette.foreground
                } else {
                    palette.muted
                },
                1.5,
            )
        })
        .collect()
}

pub fn wired(bounds: Rect, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 23.0, 18.0);
    vec![
        rounded(
            Rect::new(icon.origin.x + 1.0, icon.origin.y + 3.0, 8.0, 6.0),
            palette.foreground,
            1.5,
        ),
        fill(
            Rect::new(icon.origin.x + 8.0, icon.origin.y + 5.0, 7.0, 2.0),
            palette.foreground,
        ),
        rounded(
            Rect::new(icon.origin.x + 14.0, icon.origin.y + 9.0, 8.0, 6.0),
            palette.foreground,
            1.5,
        ),
        fill(
            Rect::new(icon.origin.x + 14.0, icon.origin.y + 6.0, 2.0, 4.0),
            palette.foreground,
        ),
    ]
}

fn unavailable_cross(icon: Rect, color: Color) -> Vec<DrawCommand> {
    cross(
        Rect::new(icon.origin.x + 6.0, icon.origin.y + 6.0, 12.0, 12.0),
        color,
    )
}

fn cross(bounds: Rect, color: Color) -> Vec<DrawCommand> {
    let step = bounds.size.width / 4.0;
    let dot = step;
    [
        (0.0, 0.0),
        (1.0, 1.0),
        (2.0, 2.0),
        (3.0, 3.0),
        (3.0, 0.0),
        (2.0, 1.0),
        (1.0, 2.0),
        (0.0, 3.0),
    ]
    .into_iter()
    .map(|(x, y)| {
        rounded(
            Rect::new(
                bounds.origin.x + x * step,
                bounds.origin.y + y * step,
                dot,
                dot,
            ),
            color,
            dot / 4.0,
        )
    })
    .collect()
}

fn wifi_arc(
    commands: &mut Vec<DrawCommand>,
    bounds: Rect,
    thickness: f32,
    color: Color,
    cutout: Color,
) {
    commands.push(rounded(bounds, color, bounds.size.width / 2.0));
    commands.push(rounded(
        bounds.inset(thickness),
        cutout,
        (bounds.size.width - thickness * 2.0) / 2.0,
    ));
    commands.push(DrawCommand::Fill {
        bounds: Rect::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height / 2.0,
            bounds.size.width,
            bounds.size.height / 2.0,
        ),
        color: cutout,
    });
}

fn centered(bounds: Rect, width: f32, height: f32) -> Rect {
    Rect::new(
        bounds.origin.x + (bounds.size.width - width) / 2.0,
        bounds.origin.y + (bounds.size.height - height) / 2.0,
        width,
        height,
    )
}

fn rounded(bounds: Rect, color: Color, radius: f32) -> DrawCommand {
    DrawCommand::RoundedFill {
        bounds,
        color,
        radius,
    }
}

fn fill(bounds: Rect, color: Color) -> DrawCommand {
    DrawCommand::Fill { bounds, color }
}

#[cfg(test)]
mod tests {
    use super::{
        IconPalette, VolumeLevel, WifiSignal, battery, cellular_signal, volume, wifi_signal, wired,
    };
    use patin::ui::{Color, DrawCommand, Rect};

    fn palette() -> IconPalette {
        IconPalette {
            foreground: Color(255, 255, 255, 255),
            muted: Color(78, 70, 91, 255),
            background: Color(20, 17, 29, 255),
            accent: Color(124, 58, 237, 255),
            unavailable: Color(239, 96, 119, 255),
        }
    }

    #[test]
    fn percentages_map_to_three_available_states() {
        assert_eq!(WifiSignal::from_percentage(0), WifiSignal::Poor);
        assert_eq!(WifiSignal::from_percentage(33), WifiSignal::Poor);
        assert_eq!(WifiSignal::from_percentage(34), WifiSignal::Medium);
        assert_eq!(WifiSignal::from_percentage(67), WifiSignal::Good);
    }

    #[test]
    fn all_four_states_are_distinct_vector_commands() {
        let bounds = Rect::new(0.0, 0.0, 32.0, 32.0);
        let states = [
            WifiSignal::Unavailable,
            WifiSignal::Poor,
            WifiSignal::Medium,
            WifiSignal::Good,
        ];
        let commands = states.map(|state| wifi_signal(bounds, state, palette()));

        assert!(
            commands
                .iter()
                .flatten()
                .all(|command| !matches!(command, DrawCommand::Text { .. }))
        );
        assert!(commands.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn zero_and_muted_volume_use_the_distinct_off_state() {
        assert_eq!(VolumeLevel::from_percentage(0, false), VolumeLevel::Off);
        assert_eq!(VolumeLevel::from_percentage(80, true), VolumeLevel::Off);
        assert_eq!(VolumeLevel::from_percentage(1, false), VolumeLevel::Low);

        let bounds = Rect::new(0.0, 0.0, 32.0, 32.0);
        let off = volume(bounds, VolumeLevel::Off, palette());
        let low = volume(bounds, VolumeLevel::Low, palette());
        let mut differently_colored_warning = palette();
        differently_colored_warning.unavailable = Color(1, 2, 3, 255);
        assert_ne!(off, low);
        assert!(off.len() > low.len());
        assert_eq!(
            off,
            volume(bounds, VolumeLevel::Off, differently_colored_warning)
        );
    }

    #[test]
    fn remaining_status_icons_are_vector_and_stateful() {
        let bounds = Rect::new(0.0, 0.0, 32.0, 32.0);
        let icons = [
            battery(bounds, 75, false, palette()),
            cellular_signal(bounds, 55, palette()),
            wired(bounds, palette()),
        ];
        assert!(
            icons
                .iter()
                .flatten()
                .all(|command| !matches!(command, DrawCommand::Text { .. }))
        );
        assert_ne!(
            battery(bounds, 10, false, palette()),
            battery(bounds, 90, true, palette())
        );
        assert_ne!(
            cellular_signal(bounds, 20, palette()),
            cellular_signal(bounds, 80, palette())
        );
    }
}
