use chrono::{Local, Timelike};
use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, FontWeight, Length, Rect, Size, TextAlign, row},
};
use patin_icons::{
    IconPalette, VolumeLevel, WifiSignal, battery, cellular_signal, volume, wifi_signal, wired,
};
use patin_service_network::NetworkSnapshot;
use patin_service_upower::BatterySnapshot;
use patin_service_volume::VolumeSnapshot;
use std::process::Child;
#[cfg(not(test))]
use std::process::Command;

use super::services::SystemStatus;

#[derive(Clone, Copy, Debug)]
struct BarStyle {
    background: Color,
    accent: Color,
    text: Color,
    padding: f32,
    gap: f32,
    horizontal_inset: f32,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            background: Color(20, 17, 29, 255),
            accent: Color(124, 58, 237, 255),
            text: Color(245, 243, 255, 255),
            padding: 5.0,
            gap: 8.0,
            horizontal_inset: 12.0,
        }
    }
}

pub struct DemoBar {
    size: Size,
    style: BarStyle,
    battery_bounds: Option<Rect>,
    volume_bounds: Option<Rect>,
    wifi_bounds: Option<Rect>,
    wired_bounds: Option<Rect>,
    cellular_bounds: Option<Rect>,
    clock_bounds: Rect,
    battery: Option<BatterySnapshot>,
    volume: Option<VolumeSnapshot>,
    network: Option<NetworkSnapshot>,
    clock: String,
    damage: Vec<Rect>,
    status: SystemStatus,
    settings_child: Option<Child>,
    #[cfg(test)]
    last_launched_page: Option<&'static str>,
}

impl DemoBar {
    pub fn new() -> Self {
        let mut status = SystemStatus::new();
        let snapshot = status.poll();
        eprintln!(
            "demo_bar: status providers: battery={:?}, volume={:?}, network={:?}",
            snapshot.battery, snapshot.volume, snapshot.network
        );
        let mut bar = Self {
            size: Size::default(),
            style: BarStyle::default(),
            battery_bounds: None,
            volume_bounds: None,
            wifi_bounds: None,
            wired_bounds: None,
            cellular_bounds: None,
            clock_bounds: Rect::default(),
            battery: snapshot.battery,
            volume: snapshot.volume,
            network: snapshot.network,
            clock: current_clock(),
            damage: Vec::new(),
            status,
            settings_child: None,
            #[cfg(test)]
            last_launched_page: None,
        };
        bar.layout();
        bar
    }

    fn layout(&mut self) {
        let network = self.network.unwrap_or_default();
        let mut lengths = vec![Length::Fixed(72.0)];
        if self.volume.is_some() {
            lengths.push(Length::Fixed(40.0));
        }
        lengths.push(Length::Fill(1.0));
        if wifi_icon_visible(network) {
            lengths.push(Length::Fixed(40.0));
        }
        if network.wired {
            lengths.push(Length::Fixed(40.0));
        }
        if network.cellular_available || network.cellular.is_some() {
            lengths.push(Length::Fixed(40.0));
        }
        if self.battery.is_some() {
            lengths.push(Length::Fixed(40.0));
        }
        let content_width = (self.size.width - self.style.horizontal_inset * 2.0).max(0.0);
        let children = row(
            Rect::new(
                self.style.horizontal_inset,
                0.0,
                content_width,
                self.size.height,
            ),
            self.style.gap,
            &lengths,
        );
        self.clock_bounds = children[0];
        let mut index = 1;
        self.volume_bounds = self.volume.as_ref().map(|_| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        index += 1;
        self.wifi_bounds = wifi_icon_visible(network).then(|| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        self.wired_bounds = network.wired.then(|| {
            let bounds = children[index];
            index += 1;
            bounds
        });
        self.cellular_bounds =
            (network.cellular_available || network.cellular.is_some()).then(|| {
                let bounds = children[index];
                index += 1;
                bounds
            });
        self.battery_bounds = self.battery.as_ref().map(|_| children[children.len() - 1]);
    }

    fn launch_settings(&mut self, page: &'static str) {
        if let Some(child) = &mut self.settings_child {
            if child.try_wait().ok().flatten().is_none() {
                return;
            }
            self.settings_child = None;
        }
        #[cfg(test)]
        {
            self.last_launched_page = Some(page);
        }
        #[cfg(not(test))]
        {
            let program = std::env::var_os("PATIN_NETWORK_SETTINGS_PROGRAM")
                .unwrap_or_else(|| "patin-network-settings".into());
            match Command::new(program).arg(format!("--page={page}")).spawn() {
                Ok(child) => self.settings_child = Some(child),
                Err(error) => eprintln!("demo_bar: could not launch network settings: {error}"),
            }
        }
    }

    fn set_status(
        &mut self,
        battery: Option<BatterySnapshot>,
        volume: Option<VolumeSnapshot>,
        network: Option<NetworkSnapshot>,
    ) -> bool {
        if self.battery == battery && self.volume == volume && self.network == network {
            return false;
        }
        let layout_changed = self.battery.is_some() != battery.is_some()
            || self.volume.is_some() != volume.is_some()
            || network_membership(self.network) != network_membership(network);
        let battery_changed = self.battery != battery;
        let volume_changed = self.volume != volume;
        let network_changed = self.network != network;
        self.battery = battery;
        self.volume = volume;
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
            if network_changed {
                self.damage.extend(
                    [self.wifi_bounds, self.wired_bounds, self.cellular_bounds]
                        .into_iter()
                        .flatten(),
                );
            }
        }
        true
    }
}

fn network_membership(network: Option<NetworkSnapshot>) -> (bool, bool, bool) {
    let network = network.unwrap_or_default();
    (
        wifi_icon_visible(network),
        network.wired,
        network.cellular_available || network.cellular.is_some(),
    )
}

fn wifi_icon_visible(network: NetworkSnapshot) -> bool {
    network.wifi.is_some() || (network.wifi_available && network.wifi_enabled)
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
        if let Some(child) = &mut self.settings_child
            && child.try_wait().ok().flatten().is_some()
        {
            self.settings_child = None;
        }
        self.set_status(snapshot.battery, snapshot.volume, snapshot.network) || changed
    }

    fn activate_at(&mut self, position: (f64, f64)) -> bool {
        if self
            .wifi_bounds
            .is_some_and(|bounds| bounds.contains(position))
        {
            self.launch_settings("wifi");
        } else if self
            .cellular_bounds
            .is_some_and(|bounds| bounds.contains(position))
        {
            self.launch_settings("cellular");
        }
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
            commands.extend(battery(
                bounds,
                status.percentage,
                status.charging,
                icon_palette(self.style),
            ));
        }
        if let (Some(bounds), Some(status)) = (self.volume_bounds, self.volume) {
            commands.extend(volume(
                bounds,
                VolumeLevel::from_percentage(status.percentage, status.muted),
                icon_palette(self.style),
            ));
        }
        if let (Some(bounds), Some(network)) = (self.wifi_bounds, self.network) {
            let signal = network
                .wifi
                .map(WifiSignal::from_percentage)
                .unwrap_or(WifiSignal::Unavailable);
            commands.extend(wifi_signal(bounds, signal, icon_palette(self.style)));
        }
        if let (Some(bounds), Some(network)) = (self.wired_bounds, self.network)
            && network.wired
        {
            commands.extend(wired(bounds, icon_palette(self.style)));
        }
        if let (Some(bounds), Some(percentage)) = (
            self.cellular_bounds,
            self.network.map(|network| network.cellular.unwrap_or(0)),
        ) {
            commands.extend(cellular_signal(
                bounds,
                percentage,
                icon_palette(self.style),
            ));
        }
        commands.push(DrawCommand::Text {
            bounds: self.clock_bounds.inset(self.style.padding),
            text: self.clock.clone(),
            color: self.style.text,
            font_size: 15.0,
            line_height: 20.0,
            family: FontFamily::Monospace,
            weight: FontWeight::Semibold,
            align: TextAlign::Start,
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

fn icon_palette(style: BarStyle) -> IconPalette {
    IconPalette {
        foreground: style.text,
        muted: Color(78, 70, 91, 255),
        background: style.background,
        accent: style.accent,
        unavailable: Color(239, 96, 119, 255),
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
    use super::{BarStyle, DemoBar, format_clock, icon_palette};
    use patin::{
        platform::Shell,
        ui::{DrawCommand, Rect, Size},
    };
    use patin_icons::{
        VolumeLevel, WifiSignal, battery, cellular_signal, volume, wifi_signal, wired,
    };
    use patin_service_network::NetworkSnapshot;
    use patin_service_upower::BatterySnapshot;
    use patin_service_volume::VolumeSnapshot;

    #[test]
    fn formats_clock_with_leading_zeroes() {
        assert_eq!(format_clock(7, 5), "07:05");
        assert_eq!(format_clock(23, 59), "23:59");
    }

    #[test]
    fn tapping_outside_network_icons_has_no_effect() {
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
    fn network_icons_launch_the_matching_page() {
        let mut bar = DemoBar::new();
        bar.set_status(
            None,
            None,
            Some(NetworkSnapshot {
                wifi_available: true,
                wifi_enabled: true,
                cellular_available: true,
                ..Default::default()
            }),
        );
        bar.resize(Size {
            width: 500.0,
            height: 32.0,
        });
        let wifi = bar.wifi_bounds.unwrap();
        bar.activate_at((f64::from(wifi.origin.x + 2.0), 16.0));
        assert_eq!(bar.last_launched_page, Some("wifi"));
        let cellular = bar.cellular_bounds.unwrap();
        bar.activate_at((f64::from(cellular.origin.x + 2.0), 16.0));
        assert_eq!(bar.last_launched_page, Some("cellular"));
    }

    #[test]
    fn status_clusters_leave_the_output_center_clear() {
        let mut bar = DemoBar::new();
        bar.set_status(
            Some(BatterySnapshot {
                percentage: 75,
                charging: false,
            }),
            Some(VolumeSnapshot {
                percentage: 50,
                muted: false,
            }),
            Some(NetworkSnapshot {
                wifi: Some(75),
                cellular: Some(55),
                wired: false,
                ..Default::default()
            }),
        );
        bar.resize(Size {
            width: 509.0,
            height: 32.0,
        });

        let battery = bar.battery_bounds.expect("battery should have a slot");
        let output_center = bar.size.width / 2.0;
        assert_eq!(bar.clock_bounds.origin.x, bar.style.horizontal_inset);
        assert!(bar.clock_bounds.origin.x < bar.volume_bounds.unwrap().origin.x);
        assert!(bar.volume_bounds.unwrap().origin.x < output_center - 32.0);
        assert!(bar.wifi_bounds.unwrap().origin.x > output_center + 32.0);
        assert!(bar.wifi_bounds.unwrap().origin.x < bar.cellular_bounds.unwrap().origin.x);
        assert!(bar.cellular_bounds.unwrap().origin.x < battery.origin.x);
        let battery_right = battery.origin.x + battery.size.width;
        let inset_right = bar.size.width - bar.style.horizontal_inset;
        assert!((battery_right - inset_right).abs() < 0.001);
    }

    #[test]
    fn wifi_icon_has_no_slot_while_radio_is_off() {
        let mut bar = DemoBar::new();
        bar.set_status(
            None,
            None,
            Some(NetworkSnapshot {
                wifi_available: true,
                wifi_enabled: false,
                ..Default::default()
            }),
        );
        bar.resize(Size {
            width: 320.0,
            height: 32.0,
        });

        assert_eq!(bar.wifi_bounds, None);
    }

    #[test]
    fn status_icons_use_shapes_and_change_with_state() {
        let bounds = Rect::new(0.0, 0.0, 64.0, 32.0);
        let style = BarStyle::default();
        let low_battery = battery(bounds, 10, false, icon_palette(style));
        let charged_battery = battery(bounds, 90, true, icon_palette(style));
        let icons = [
            low_battery.clone(),
            volume(bounds, VolumeLevel::Medium, icon_palette(style)),
            wifi_signal(bounds, WifiSignal::Good, icon_palette(style)),
            cellular_signal(bounds, 55, icon_palette(style)),
            wired(bounds, icon_palette(style)),
        ];

        assert!(
            icons
                .iter()
                .flatten()
                .all(|command| !matches!(command, DrawCommand::Text { .. }))
        );
        assert_ne!(low_battery, charged_battery);
        assert_ne!(
            wifi_signal(bounds, WifiSignal::Poor, icon_palette(style)),
            wifi_signal(bounds, WifiSignal::Good, icon_palette(style))
        );
        assert_ne!(
            cellular_signal(bounds, 20, icon_palette(style)),
            cellular_signal(bounds, 80, icon_palette(style))
        );
        assert_ne!(
            volume(bounds, VolumeLevel::Medium, icon_palette(style)),
            volume(bounds, VolumeLevel::Off, icon_palette(style))
        );
    }
}
