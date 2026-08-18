//! The status strip along the top of the greeter.
//!
//! Someone standing at a login screen wants the same things the shell's bar
//! shows them: the time, how much battery is left, and whether the device is
//! actually on a network — a phone that silently dropped its SIM or Wi-Fi is
//! worth knowing about *before* signing in. It reuses `patin-icons`, and keeps
//! the bar's sides: clock on the left, radios and battery on the right.
//!
//! Every reading is optional. The greeter runs as an unprivileged user that
//! may not be able to reach UPower or NetworkManager at all, so a provider
//! that returns nothing simply drops its icon rather than showing a wrong or
//! placeholder value.

use chrono::{Local, Timelike};
use patin::service::Provider;
use patin::ui::{DrawCommand, FontFamily, FontWeight, Rect, TextAlign};
use patin_icons::{IconPalette, WifiSignal, battery, cellular_signal, wifi_signal, wired};
use patin_service_network::{NetworkProvider, NetworkSnapshot};
use patin_service_upower::{BatteryProvider, BatterySnapshot};

use crate::ui::{ACCENT_FOCUSED, BACKGROUND, TEXT_BRIGHT, TEXT_ERROR, TEXT_MUTED};

/// Distance from the screen edges, matching the bar's own inset.
const INSET: f32 = 16.0;
const STRIP_TOP: f32 = 14.0;
const STRIP_HEIGHT: f32 = 28.0;
/// Where the greeter's own header must start below, so a short screen cannot
/// stack the hostname on top of the strip.
pub(crate) const STRIP_BOTTOM: f32 = STRIP_TOP + STRIP_HEIGHT;
const ICON_WIDTH: f32 = 30.0;
const ICON_GAP: f32 = 6.0;
const CLOCK_WIDTH: f32 = 96.0;

/// Ticks between provider refreshes. The greeter polls fast so a sign-in
/// result lands promptly; battery and signal move far slower than that, and
/// each read is a D-Bus round trip, so they are sampled every few seconds
/// instead of every tick.
const REFRESH_TICKS: u8 = 10;

pub struct Status {
    battery_provider: BatteryProvider,
    network_provider: NetworkProvider,
    battery: Option<BatterySnapshot>,
    network: Option<NetworkSnapshot>,
    clock: String,
    ticks: u8,
    /// Providers are not read during construction: the first D-Bus round trip
    /// would delay the greeter's first frame, and on a machine with no system
    /// bus reachable it would delay it for nothing.
    initial_refresh_pending: bool,
}

impl Status {
    pub fn new() -> Self {
        Self {
            battery_provider: BatteryProvider::new(),
            network_provider: NetworkProvider::new(),
            battery: None,
            network: None,
            clock: current_clock(),
            ticks: 0,
            initial_refresh_pending: true,
        }
    }

    /// Advance one greeter tick, returning whether anything visible changed.
    pub fn update(&mut self) -> bool {
        let mut changed = false;

        let clock = current_clock();
        if self.clock != clock {
            self.clock = clock;
            changed = true;
        }

        if self.initial_refresh_pending {
            self.initial_refresh_pending = false;
        } else {
            self.ticks = self.ticks.saturating_add(1);
            if self.ticks < REFRESH_TICKS {
                return changed;
            }
        }
        self.ticks = 0;

        let battery = self.battery_provider.poll();
        if self.battery != battery {
            self.battery = battery;
            changed = true;
        }
        let network = self.network_provider.poll();
        if self.network != network {
            self.network = network;
            changed = true;
        }
        changed
    }

    pub fn commands(&self, width: f32) -> Vec<DrawCommand> {
        let mut commands = vec![DrawCommand::Text {
            bounds: Rect::new(INSET, STRIP_TOP, CLOCK_WIDTH, STRIP_HEIGHT),
            text: self.clock.clone(),
            color: TEXT_BRIGHT,
            font_size: 16.0,
            line_height: 21.0,
            family: FontFamily::Monospace,
            weight: FontWeight::Semibold,
            align: TextAlign::Start,
        }];

        // Laid out from the right edge inwards, so the battery keeps the
        // corner however many radios happen to be present.
        let icons = self.icons();
        let mut right = width - INSET;
        for icon in icons.iter().rev() {
            let bounds = Rect::new(right - ICON_WIDTH, STRIP_TOP, ICON_WIDTH, STRIP_HEIGHT);
            commands.extend(self.icon(*icon, bounds));
            right -= ICON_WIDTH + ICON_GAP;
        }
        commands
    }

    /// Which icons have something to say, in the bar's left-to-right order.
    fn icons(&self) -> Vec<Icon> {
        let network = self.network.unwrap_or_default();
        let mut icons = Vec::new();
        if self.network.is_some() {
            if wifi_visible(&network) {
                icons.push(Icon::Wifi);
            }
            if network.wired {
                icons.push(Icon::Wired);
            }
            if cellular_visible(&network) {
                icons.push(Icon::Cellular);
            }
        }
        if self.battery.is_some() {
            icons.push(Icon::Battery);
        }
        icons
    }

    fn icon(&self, icon: Icon, bounds: Rect) -> Vec<DrawCommand> {
        let network = self.network.unwrap_or_default();
        match icon {
            Icon::Wifi => {
                let signal = network
                    .wifi
                    .map(WifiSignal::from_percentage)
                    .unwrap_or(WifiSignal::Unavailable);
                wifi_signal(bounds, signal, palette())
            }
            Icon::Wired => wired(bounds, palette()),
            Icon::Cellular => cellular_signal(bounds, network.cellular.unwrap_or(0), palette()),
            Icon::Battery => match self.battery {
                Some(status) => battery(bounds, status.percentage, status.charging, palette()),
                None => Vec::new(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Icon {
    Wifi,
    Wired,
    Cellular,
    Battery,
}

/// Wi-Fi is worth a slot when it is connected, or when the radio is on and
/// simply has nothing yet — its absence then means "off", which is itself
/// information.
fn wifi_visible(network: &NetworkSnapshot) -> bool {
    network.wifi.is_some() || (network.wifi_available && network.wifi_enabled)
}

fn cellular_visible(network: &NetworkSnapshot) -> bool {
    network.cellular_available || network.cellular.is_some()
}

fn palette() -> IconPalette {
    IconPalette {
        foreground: TEXT_BRIGHT,
        muted: TEXT_MUTED,
        background: BACKGROUND,
        accent: ACCENT_FOCUSED,
        unavailable: TEXT_ERROR,
    }
}

fn current_clock() -> String {
    let now = Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
}

#[cfg(test)]
mod tests {
    use super::{CLOCK_WIDTH, INSET, Icon, Status, cellular_visible, wifi_visible};
    use patin::ui::{DrawCommand, Rect};
    use patin_service_network::NetworkSnapshot;
    use patin_service_upower::BatterySnapshot;

    fn status(network: Option<NetworkSnapshot>, battery: Option<BatterySnapshot>) -> Status {
        let mut status = Status::new();
        status.network = network;
        status.battery = battery;
        status.clock = "07:05".into();
        status
    }

    fn shapes(status: &Status, width: f32) -> Vec<Rect> {
        status
            .commands(width)
            .into_iter()
            .filter_map(|command| match command {
                DrawCommand::Fill { bounds, .. } | DrawCommand::RoundedFill { bounds, .. } => {
                    Some(bounds)
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_clock_sits_on_the_left_and_icons_on_the_right() {
        let width = 500.0;
        let status = status(
            Some(NetworkSnapshot {
                wifi: Some(80),
                cellular: Some(50),
                cellular_available: true,
                ..Default::default()
            }),
            Some(BatterySnapshot {
                percentage: 75,
                charging: false,
            }),
        );

        let clock = status
            .commands(width)
            .into_iter()
            .find_map(|command| match command {
                DrawCommand::Text { bounds, text, .. } if text == "07:05" => Some(bounds),
                _ => None,
            })
            .expect("the clock is drawn");
        assert_eq!(clock.origin.x, INSET);
        assert!(clock.origin.x + CLOCK_WIDTH < width / 2.0, "clock stays left");

        let shapes = shapes(&status, width);
        assert!(!shapes.is_empty(), "icons are drawn as shapes");
        assert!(
            shapes.iter().all(|bounds| bounds.origin.x > width / 2.0),
            "every icon sits on the right half"
        );
        assert!(
            shapes
                .iter()
                .all(|bounds| bounds.origin.x + bounds.size.width <= width - INSET + 0.01),
            "icons stay inside the inset"
        );
    }

    #[test]
    fn the_battery_keeps_the_right_hand_corner() {
        let width = 500.0;
        let full = status(
            Some(NetworkSnapshot {
                wifi: Some(80),
                cellular: Some(50),
                cellular_available: true,
                wired: true,
                ..Default::default()
            }),
            Some(BatterySnapshot {
                percentage: 75,
                charging: false,
            }),
        );
        assert_eq!(
            full.icons(),
            vec![Icon::Wifi, Icon::Wired, Icon::Cellular, Icon::Battery],
            "the bar's order, battery last"
        );

        // Dropping the radios must not move the battery off the corner.
        let sparse = status(
            None,
            Some(BatterySnapshot {
                percentage: 40,
                charging: true,
            }),
        );
        let rightmost = |status: &Status| {
            shapes(status, width)
                .into_iter()
                .map(|bounds| bounds.origin.x + bounds.size.width)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        assert!((rightmost(&full) - rightmost(&sparse)).abs() < 0.01);
    }

    #[test]
    fn unavailable_readings_simply_have_no_icon() {
        let empty = status(None, None);
        assert!(empty.icons().is_empty());
        assert!(shapes(&empty, 500.0).is_empty(), "nothing but the clock");
    }

    #[test]
    fn a_disabled_radio_is_not_given_a_slot() {
        assert!(!wifi_visible(&NetworkSnapshot {
            wifi_available: true,
            wifi_enabled: false,
            ..Default::default()
        }));
        assert!(wifi_visible(&NetworkSnapshot {
            wifi_available: true,
            wifi_enabled: true,
            ..Default::default()
        }));
        assert!(!cellular_visible(&NetworkSnapshot::default()));
        assert!(cellular_visible(&NetworkSnapshot {
            cellular_available: true,
            ..Default::default()
        }));
    }
}
