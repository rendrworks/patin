use chrono::{Local, Timelike};
use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, FontWeight, Length, Rect, Size, TextAlign, row},
};
use patin_service_brightness::BrightnessSnapshot;
use patin_service_network::NetworkSnapshot;
use patin_service_upower::BatterySnapshot;
use patin_service_volume::VolumeSnapshot;

use super::services::SystemStatus;

#[derive(Clone, Copy, Debug)]
struct BarStyle {
    background: Color,
    accent: Color,
    text: Color,
    padding: f32,
    gap: f32,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            background: Color(20, 17, 29, 255),
            accent: Color(124, 58, 237, 255),
            text: Color(245, 243, 255, 255),
            padding: 5.0,
            gap: 8.0,
        }
    }
}

pub struct DemoBar {
    size: Size,
    style: BarStyle,
    battery_bounds: Option<Rect>,
    volume_bounds: Option<Rect>,
    brightness_bounds: Option<Rect>,
    network_bounds: Option<Rect>,
    clock_bounds: Rect,
    battery: Option<BatterySnapshot>,
    volume: Option<VolumeSnapshot>,
    brightness: Option<BrightnessSnapshot>,
    network: Option<NetworkSnapshot>,
    clock: String,
    damage: Vec<Rect>,
    status: SystemStatus,
}

impl DemoBar {
    pub fn new() -> Self {
        let mut status = SystemStatus::new();
        let snapshot = status.poll();
        eprintln!(
            "demo_bar: status providers: battery={:?}, volume={:?}, brightness={:?}, network={:?}",
            snapshot.battery, snapshot.volume, snapshot.brightness, snapshot.network
        );
        let mut bar = Self {
            size: Size::default(),
            style: BarStyle::default(),
            battery_bounds: None,
            volume_bounds: None,
            brightness_bounds: None,
            network_bounds: None,
            clock_bounds: Rect::default(),
            battery: snapshot.battery,
            volume: snapshot.volume,
            brightness: snapshot.brightness,
            network: snapshot.network,
            clock: current_clock(),
            damage: Vec::new(),
            status,
        };
        bar.layout();
        bar
    }

    fn layout(&mut self) {
        let status_count = [
            self.battery.is_some(),
            self.volume.is_some(),
            self.brightness.is_some(),
            self.network.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        let mut lengths = vec![Length::Fill(1.0); status_count.max(1)];
        lengths.push(Length::Fixed(72.0));
        let children = row(
            Rect::new(0.0, 0.0, self.size.width, self.size.height),
            self.style.gap,
            &lengths,
        );
        let mut index = 0;
        self.battery_bounds = self.battery.as_ref().map(|_| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        self.volume_bounds = self.volume.as_ref().map(|_| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        self.brightness_bounds = self.brightness.as_ref().map(|_| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        self.network_bounds = self.network.as_ref().map(|_| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        self.clock_bounds = children[children.len() - 1];
    }

    fn set_status(
        &mut self,
        battery: Option<BatterySnapshot>,
        volume: Option<VolumeSnapshot>,
        brightness: Option<BrightnessSnapshot>,
        network: Option<NetworkSnapshot>,
    ) -> bool {
        if self.battery == battery
            && self.volume == volume
            && self.brightness == brightness
            && self.network == network
        {
            return false;
        }
        let layout_changed = self.battery.is_some() != battery.is_some()
            || self.volume.is_some() != volume.is_some()
            || self.brightness.is_some() != brightness.is_some()
            || self.network.is_some() != network.is_some();
        let battery_changed = self.battery != battery;
        let volume_changed = self.volume != volume;
        let brightness_changed = self.brightness != brightness;
        let network_changed = self.network != network;
        self.battery = battery;
        self.volume = volume;
        self.brightness = brightness;
        self.network = network;
        if layout_changed {
            self.layout();
            self.damage_all();
        } else {
            if battery_changed && let Some(bounds) = self.battery_bounds {
                self.damage.push(bounds);
            }
            if volume_changed && let Some(bounds) = self.volume_bounds {
                self.damage.push(bounds);
            }
            if brightness_changed && let Some(bounds) = self.brightness_bounds {
                self.damage.push(bounds);
            }
            if network_changed && let Some(bounds) = self.network_bounds {
                self.damage.push(bounds);
            }
        }
        true
    }
}

impl Shell for DemoBar {
    fn resize(&mut self, size: Size) {
        if self.size != size {
            self.size = size;
            self.layout();
            self.damage_all();
        }
    }

    fn update(&mut self) -> bool {
        let mut changed = false;
        let clock = current_clock();
        if self.clock != clock {
            self.clock = clock;
            self.damage.push(self.clock_bounds);
            changed = true;
        }
        let snapshot = self.status.poll();
        self.set_status(
            snapshot.battery,
            snapshot.volume,
            snapshot.brightness,
            snapshot.network,
        ) || changed
    }

    fn activate_at(&mut self, _position: (f64, f64)) -> bool {
        false
    }

    fn commands(&self) -> Vec<DrawCommand> {
        let full = Rect::new(0.0, 0.0, self.size.width, self.size.height);
        let accent = Rect::new(0.0, (self.size.height - 2.0).max(0.0), self.size.width, 2.0);
        let mut commands = vec![
            DrawCommand::Fill {
                bounds: full,
                color: self.style.background,
            },
            DrawCommand::Fill {
                bounds: accent,
                color: self.style.accent,
            },
        ];
        if let (Some(bounds), Some(status)) = (self.battery_bounds, self.battery) {
            commands.extend(battery_icon(bounds, status, self.style));
        }
        if let (Some(bounds), Some(status)) = (self.volume_bounds, self.volume) {
            commands.extend(volume_icon(bounds, status, self.style));
        }
        if let (Some(bounds), Some(status)) = (self.brightness_bounds, self.brightness) {
            commands.extend(brightness_icon(bounds, status, self.style));
        }
        if let (Some(bounds), Some(status)) = (self.network_bounds, self.network) {
            commands.extend(network_icon(bounds, status, self.style));
        }
        commands.push(DrawCommand::Text {
            bounds: self.clock_bounds.inset(self.style.padding),
            text: self.clock.clone(),
            color: self.style.text,
            font_size: 15.0,
            line_height: 20.0,
            family: FontFamily::Monospace,
            weight: FontWeight::Semibold,
            align: TextAlign::End,
        });
        commands
    }

    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }

    fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
}

fn battery_icon(bounds: Rect, status: BatterySnapshot, style: BarStyle) -> Vec<DrawCommand> {
    let icon = centered_icon(bounds, 24.0, 16.0);
    let body = Rect::new(icon.origin.x, icon.origin.y + 2.0, 20.0, 12.0);
    let interior = body.inset(2.0);
    let level_width = interior.size.width * f32::from(status.percentage.min(100)) / 100.0;
    let level_color = if status.percentage <= 15 {
        Color(239, 96, 119, 255)
    } else if status.charging {
        style.accent
    } else {
        style.text
    };
    let mut commands = vec![
        rounded(body, style.text, 2.5),
        rounded(interior, style.background, 1.0),
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
            style.text,
            1.0,
        ),
    ];
    if status.charging {
        commands.push(fill(
            Rect::new(icon.origin.x + 9.0, icon.origin.y + 4.0, 2.0, 8.0),
            style.background,
        ));
    }
    commands
}

fn volume_icon(bounds: Rect, status: VolumeSnapshot, style: BarStyle) -> Vec<DrawCommand> {
    let icon = centered_icon(bounds, 24.0, 18.0);
    let mut commands = vec![
        rounded(
            Rect::new(icon.origin.x, icon.origin.y + 6.0, 5.0, 6.0),
            style.text,
            1.0,
        ),
        rounded(
            Rect::new(icon.origin.x + 4.0, icon.origin.y + 3.0, 4.0, 12.0),
            style.text,
            1.5,
        ),
    ];
    if status.muted {
        commands.push(rounded(
            Rect::new(icon.origin.x + 14.0, icon.origin.y + 3.0, 2.0, 12.0),
            Color(239, 96, 119, 255),
            1.0,
        ));
        return commands;
    }
    let active = match status.percentage {
        0 => 0,
        1..=33 => 1,
        34..=66 => 2,
        _ => 3,
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
                style.text
            } else {
                Color(78, 70, 91, 255)
            },
            1.0,
        ));
    }
    commands
}

fn brightness_icon(bounds: Rect, status: BrightnessSnapshot, style: BarStyle) -> Vec<DrawCommand> {
    let icon = centered_icon(bounds, 20.0, 20.0);
    let center_size = 6.0 + f32::from(status.percentage.min(100)) * 0.04;
    let center = Rect::new(
        icon.origin.x + (20.0 - center_size) / 2.0,
        icon.origin.y + (20.0 - center_size) / 2.0,
        center_size,
        center_size,
    );
    vec![
        rounded(center, style.text, center_size / 2.0),
        rounded(
            Rect::new(icon.origin.x + 9.0, icon.origin.y, 2.0, 4.0),
            style.text,
            1.0,
        ),
        rounded(
            Rect::new(icon.origin.x + 9.0, icon.origin.y + 16.0, 2.0, 4.0),
            style.text,
            1.0,
        ),
        rounded(
            Rect::new(icon.origin.x, icon.origin.y + 9.0, 4.0, 2.0),
            style.text,
            1.0,
        ),
        rounded(
            Rect::new(icon.origin.x + 16.0, icon.origin.y + 9.0, 4.0, 2.0),
            style.text,
            1.0,
        ),
    ]
}

fn network_icon(bounds: Rect, status: NetworkSnapshot, style: BarStyle) -> Vec<DrawCommand> {
    let icon = centered_icon(bounds, 23.0, 18.0);
    match status {
        NetworkSnapshot::Wifi { percentage } => {
            let active = match percentage {
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
                            style.text
                        } else {
                            Color(78, 70, 91, 255)
                        },
                        1.5,
                    )
                })
                .collect()
        }
        NetworkSnapshot::Disconnected => (0..4)
            .map(|index| {
                let height = 4.0 + index as f32 * 4.0;
                rounded(
                    Rect::new(
                        icon.origin.x + index as f32 * 6.0,
                        icon.origin.y + 18.0 - height,
                        4.0,
                        height,
                    ),
                    Color(91, 64, 78, 255),
                    1.5,
                )
            })
            .collect(),
        NetworkSnapshot::Wired => vec![
            rounded(
                Rect::new(icon.origin.x + 1.0, icon.origin.y + 3.0, 8.0, 6.0),
                style.text,
                1.5,
            ),
            fill(
                Rect::new(icon.origin.x + 8.0, icon.origin.y + 5.0, 7.0, 2.0),
                style.text,
            ),
            rounded(
                Rect::new(icon.origin.x + 14.0, icon.origin.y + 9.0, 8.0, 6.0),
                style.text,
                1.5,
            ),
            fill(
                Rect::new(icon.origin.x + 14.0, icon.origin.y + 6.0, 2.0, 4.0),
                style.text,
            ),
        ],
        NetworkSnapshot::Other => vec![
            rounded(
                Rect::new(icon.origin.x + 4.0, icon.origin.y + 1.0, 15.0, 15.0),
                style.text,
                7.5,
            ),
            rounded(
                Rect::new(icon.origin.x + 7.0, icon.origin.y + 4.0, 9.0, 9.0),
                style.background,
                4.5,
            ),
            rounded(
                Rect::new(icon.origin.x + 10.0, icon.origin.y + 7.0, 3.0, 3.0),
                style.accent,
                1.5,
            ),
        ],
    }
}

fn centered_icon(bounds: Rect, width: f32, height: f32) -> Rect {
    Rect::new(
        bounds.origin.x + (bounds.size.width - width) / 2.0,
        bounds.origin.y + (bounds.size.height - height) / 2.0,
        width,
        height,
    )
}

fn fill(bounds: Rect, color: Color) -> DrawCommand {
    DrawCommand::Fill { bounds, color }
}

fn rounded(bounds: Rect, color: Color, radius: f32) -> DrawCommand {
    DrawCommand::RoundedFill {
        bounds,
        color,
        radius,
    }
}

fn current_clock() -> String {
    let now = Local::now();
    format_clock(now.hour(), now.minute())
}

fn format_clock(hour: u32, minute: u32) -> String {
    format!("{hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::{
        BarStyle, DemoBar, battery_icon, brightness_icon, format_clock, network_icon, volume_icon,
    };
    use patin::{
        platform::Shell,
        ui::{DrawCommand, Rect, Size},
    };
    use patin_service_brightness::BrightnessSnapshot;
    use patin_service_network::NetworkSnapshot;
    use patin_service_upower::BatterySnapshot;
    use patin_service_volume::VolumeSnapshot;

    #[test]
    fn formats_clock_with_leading_zeroes() {
        assert_eq!(format_clock(7, 5), "07:05");
        assert_eq!(format_clock(23, 59), "23:59");
    }

    #[test]
    fn activate_at_has_no_effect_without_interactive_elements() {
        let mut bar = DemoBar::new();
        bar.resize(Size {
            width: 500.0,
            height: 32.0,
        });
        bar.take_damage();
        assert!(!bar.activate_at((20.0, 16.0)));
        assert!(bar.take_damage().is_empty());
    }

    #[test]
    fn status_icons_use_shapes_and_change_with_state() {
        let bounds = Rect::new(0.0, 0.0, 64.0, 32.0);
        let style = BarStyle::default();
        let low_battery = battery_icon(
            bounds,
            BatterySnapshot {
                percentage: 10,
                charging: false,
            },
            style,
        );
        let charged_battery = battery_icon(
            bounds,
            BatterySnapshot {
                percentage: 90,
                charging: true,
            },
            style,
        );
        let icons = [
            low_battery.clone(),
            volume_icon(
                bounds,
                VolumeSnapshot {
                    percentage: 55,
                    muted: false,
                },
                style,
            ),
            brightness_icon(bounds, BrightnessSnapshot { percentage: 60 }, style),
            network_icon(bounds, NetworkSnapshot::Wifi { percentage: 75 }, style),
        ];

        assert!(
            icons
                .iter()
                .flatten()
                .all(|command| !matches!(command, DrawCommand::Text { .. }))
        );
        assert_ne!(low_battery, charged_battery);
        assert_ne!(
            volume_icon(
                bounds,
                VolumeSnapshot {
                    percentage: 55,
                    muted: false,
                },
                style,
            ),
            volume_icon(
                bounds,
                VolumeSnapshot {
                    percentage: 55,
                    muted: true,
                },
                style,
            )
        );
    }
}
