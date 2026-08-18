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

/// The power symbol: a broken ring with an upright stem through the gap.
///
/// Session actions are drawn here rather than looked up in an icon theme so
/// they cannot go missing, and so they take the same [`IconPalette`] as the
/// status icons. Every shape is a rect or a rounded rect, which is all
/// `DrawCommand` offers — the ring is a disc with the background punched out
/// of its middle, and the gap is a second background cut across the top.
pub fn power(bounds: Rect, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 18.0, 18.0);
    let mut commands = ring(icon, 16.0, 2.2, palette.foreground, palette.background);
    let centre_x = icon.origin.x + icon.size.width / 2.0;
    // Punch the gap, then lay the stem through it.
    commands.push(fill(
        Rect::new(centre_x - 2.4, icon.origin.y - 1.0, 4.8, 6.0),
        palette.background,
    ));
    commands.push(rounded(
        Rect::new(centre_x - 1.1, icon.origin.y + 1.0, 2.2, 7.5),
        palette.foreground,
        1.1,
    ));
    commands
}

/// A ring broken at the top with an arrowhead flaring off it: reboot.
///
/// The head points up, across the stroke rather than along it — capping a
/// horizontal stroke with a perpendicular head is what makes it read as an
/// arrow instead of a thicker line.
pub fn reboot(bounds: Rect, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 18.0, 18.0);
    let mut commands = ring(icon, 15.0, 2.2, palette.foreground, palette.background);
    let centre_x = icon.origin.x + icon.size.width / 2.0;
    // Break the ring just past twelve o'clock, then stand the head on the
    // end of the remaining stroke.
    commands.push(fill(
        Rect::new(centre_x + 2.2, icon.origin.y + 0.4, 6.5, 4.2),
        palette.background,
    ));
    // Narrow enough that its base is close to the stroke it grows out of; a
    // much wider base just reads as a lump on the ring.
    commands.extend(arrowhead_up(
        centre_x + 2.6,
        icon.origin.y - 0.8,
        2.7,
        5.2,
        palette.foreground,
    ));
    commands
}

/// A door standing open with an arrow leaving through it: log out.
pub fn logout(bounds: Rect, palette: IconPalette) -> Vec<DrawCommand> {
    let icon = centered(bounds, 18.0, 18.0);
    let (x, y) = (icon.origin.x, icon.origin.y);
    let door_width = 8.5;
    let thickness = 2.0;
    let middle = y + 9.0;
    let mut commands = vec![
        // Three sides only: the open side is where the arrow leaves.
        fill(Rect::new(x, y + 1.0, thickness, 16.0), palette.foreground),
        fill(Rect::new(x, y + 1.0, door_width, thickness), palette.foreground),
        fill(
            Rect::new(x, y + 15.0, door_width, thickness),
            palette.foreground,
        ),
        // The shaft, level with the middle of the door.
        fill(
            Rect::new(x + 5.5, middle - 1.0, 7.5, thickness),
            palette.foreground,
        ),
    ];
    commands.extend(arrowhead_right(x + 17.5, middle, 3.6, 5.0, palette.foreground));
    commands
}

/// A triangle pointing up, stacked out of rects for the same reason
/// [`arrowhead_right`] is.
fn arrowhead_up(
    centre_x: f32,
    tip_y: f32,
    half_width: f32,
    height: f32,
    color: Color,
) -> Vec<DrawCommand> {
    const STEPS: usize = 7;
    let step_height = height / STEPS as f32;
    (0..STEPS)
        .map(|index| {
            let depth = index as f32 + 1.0;
            let half = half_width * depth / STEPS as f32;
            fill(
                Rect::new(
                    centre_x - half,
                    tip_y + (depth - 1.0) * step_height,
                    half * 2.0,
                    step_height + SEAM,
                ),
                color,
            )
        })
        .collect()
}

/// An outlined circle: a disc with the background punched out of it.
fn ring(icon: Rect, diameter: f32, thickness: f32, color: Color, background: Color) -> Vec<DrawCommand> {
    let outer = centered(icon, diameter, diameter);
    let inner = outer.inset(thickness);
    vec![
        rounded(outer, color, outer.size.width / 2.0),
        rounded(inner, background, inner.size.width / 2.0),
    ]
}

/// How far each step of a stacked triangle overhangs the next. Without it the
/// anti-aliased edges of neighbouring rects leave visible seams down the
/// arrowhead; a fraction of a logical pixel is enough to close them.
const SEAM: f32 = 0.35;

/// A triangle pointing right, stacked out of rects the way [`cross`] stacks
/// its diagonal — `DrawCommand` has no polygon, and at icon sizes the steps
/// read as a solid arrowhead.
fn arrowhead_right(
    tip_x: f32,
    centre_y: f32,
    half_height: f32,
    width: f32,
    color: Color,
) -> Vec<DrawCommand> {
    const STEPS: usize = 7;
    let step_width = width / STEPS as f32;
    (0..STEPS)
        .map(|index| {
            let depth = index as f32 + 1.0;
            let half = half_height * depth / STEPS as f32;
            fill(
                Rect::new(
                    tip_x - depth * step_width,
                    centre_y - half,
                    step_width + SEAM,
                    half * 2.0,
                ),
                color,
            )
        })
        .collect()
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
