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
    pub unavailable: Color,
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

fn unavailable_cross(icon: Rect, color: Color) -> Vec<DrawCommand> {
    let left = icon.origin.x + 6.0;
    let top = icon.origin.y + 6.0;
    [
        (0.0, 0.0),
        (3.0, 3.0),
        (6.0, 6.0),
        (9.0, 9.0),
        (9.0, 0.0),
        (6.0, 3.0),
        (3.0, 6.0),
        (0.0, 9.0),
    ]
    .into_iter()
    .map(|(x, y)| rounded(Rect::new(left + x, top + y, 3.0, 3.0), color, 0.75))
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

#[cfg(test)]
mod tests {
    use super::{IconPalette, WifiSignal, wifi_signal};
    use patin::ui::{Color, DrawCommand, Rect};

    fn palette() -> IconPalette {
        IconPalette {
            foreground: Color(255, 255, 255, 255),
            muted: Color(78, 70, 91, 255),
            background: Color(20, 17, 29, 255),
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
}
