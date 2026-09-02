//! The status strip along the top of a shell composition.
//!
//! Someone standing at a login screen, or picking up a locked phone, wants the
//! same things the shell's bar shows them: the time, how much battery is left,
//! and whether the device is actually on a network — a phone that silently
//! dropped its SIM or Wi-Fi is worth knowing about *before* signing in. This
//! reuses `patin-icons`, and keeps the bar's sides: clock on the left, radios
//! and battery on the right.
//!
//! Every reading is optional. A composition may run as an unprivileged user
//! that cannot reach UPower or NetworkManager at all, so a provider that
//! returns nothing simply drops its icon rather than showing a wrong or
//! placeholder value.
//!
//! The strip is opt-in and configurable rather than one fixed row, because the
//! two consumers want different things from it. The greeter has no other
//! clock, so it keeps this one; the lock screen already draws a large centred
//! clock and asks for icons only. Nothing here is constructed by the toolkit:
//! a composition builds a [`Status`], polls it on whatever schedule it already
//! has, and appends [`Status::commands`] to its own scene.

use std::time::{Duration, Instant};

use chrono::{Local, Timelike};
use patin::service::Provider;
use patin::ui::{DrawCommand, FontFamily, FontWeight, Rect, TextAlign};
use patin_icons::{
    IconPalette, VolumeLevel, WifiSignal, battery, cellular_signal, volume, wifi_signal, wired,
};
use patin_service_network::{NetworkProvider, NetworkSnapshot};
use patin_service_upower::{BatteryProvider, BatterySnapshot};
use patin_service_volume::{VolumeProvider, VolumeSnapshot};

/// Distance from the screen edges, matching the bar's own inset.
const INSET: f32 = 16.0;
const STRIP_TOP: f32 = 14.0;
const STRIP_HEIGHT: f32 = 28.0;
/// Where a composition's own content must start below, so a short screen
/// cannot stack a header or a clock on top of the strip.
pub const STRIP_BOTTOM: f32 = STRIP_TOP + STRIP_HEIGHT;
const ICON_WIDTH: f32 = 30.0;
const ICON_GAP: f32 = 6.0;
const CLOCK_WIDTH: f32 = 96.0;

/// Time between provider refreshes. Consumers tick at very different rates —
/// the greeter every 200ms so a sign-in result lands promptly, the lock every
/// 50ms — so the interval is measured against the clock rather than counted in
/// ticks. Battery and signal move far slower than either rate, and each read is
/// a D-Bus round trip, so they are sampled every couple of seconds.
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

pub struct Status {
    palette: IconPalette,
    clock_shown: bool,
    battery_provider: BatteryProvider,
    network_provider: NetworkProvider,
    /// Absent unless the composition asked for volume. Polling it spawns
    /// `wpctl`/`pactl`, so a composition that does not show it must not pay
    /// for it — nor construct a module belonging only to another composition.
    volume_provider: Option<VolumeProvider>,
    battery: Option<BatterySnapshot>,
    network: Option<NetworkSnapshot>,
    volume: Option<VolumeSnapshot>,
    clock: String,
    /// Providers are not read during construction: the first D-Bus round trip
    /// would delay the composition's first frame, and on a machine with no
    /// system bus reachable it would delay it for nothing.
    last_refresh: Option<Instant>,
}

impl Status {
    /// A strip with the clock, the radios, and the battery — the greeter's set.
    ///
    /// The palette's `background` must match the fill drawn behind the strip:
    /// several glyphs punch holes in that colour rather than being transparent.
    pub fn new(palette: IconPalette) -> Self {
        Self {
            palette,
            clock_shown: true,
            battery_provider: BatteryProvider::new(),
            network_provider: NetworkProvider::new(),
            volume_provider: None,
            battery: None,
            network: None,
            volume: None,
            clock: current_clock(),
            last_refresh: None,
        }
    }

    /// Draw the clock, or leave the left side empty for a composition that
    /// already shows the time somewhere else.
    pub fn with_clock(mut self, clock: bool) -> Self {
        self.clock_shown = clock;
        self
    }

    /// Add the volume icon, constructing its provider only when asked.
    pub fn with_volume(mut self, shown: bool) -> Self {
        self.volume_provider = shown.then(VolumeProvider::new);
        if !shown {
            self.volume = None;
        }
        self
    }

    /// Advance one tick, returning whether anything visible changed.
    ///
    /// Cheap to call at any rate: the clock is compared every time, while the
    /// providers are read at most once per [`REFRESH_INTERVAL`].
    pub fn update(&mut self) -> bool {
        let mut changed = false;

        let clock = current_clock();
        if self.clock != clock {
            self.clock = clock;
            changed = true;
        }

        let now = Instant::now();
        if let Some(last) = self.last_refresh
            && now.duration_since(last) < REFRESH_INTERVAL
        {
            return changed;
        }
        self.last_refresh = Some(now);

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
        if let Some(provider) = &mut self.volume_provider {
            let volume = provider.poll();
            if self.volume != volume {
                self.volume = volume;
                changed = true;
            }
        }
        changed
    }

    pub fn commands(&self, width: f32) -> Vec<DrawCommand> {
        let mut commands = Vec::new();
        if self.clock_shown {
            commands.push(DrawCommand::Text {
                bounds: Rect::new(INSET, STRIP_TOP, CLOCK_WIDTH, STRIP_HEIGHT),
                text: self.clock.clone(),
                color: self.palette.foreground,
                font_size: 16.0,
                line_height: 21.0,
                family: FontFamily::Monospace,
                weight: FontWeight::Semibold,
                align: TextAlign::Start,
            });
        }

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
        if self.volume.is_some() {
            icons.push(Icon::Volume);
        }
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
            Icon::Volume => match self.volume {
                Some(status) => volume(
                    bounds,
                    VolumeLevel::from_percentage(status.percentage, status.muted),
                    self.palette,
                ),
                None => Vec::new(),
            },
            Icon::Wifi => {
                let signal = network
                    .wifi
                    .map(WifiSignal::from_percentage)
                    .unwrap_or(WifiSignal::Unavailable);
                wifi_signal(bounds, signal, self.palette)
            }
            Icon::Wired => wired(bounds, self.palette),
            Icon::Cellular => cellular_signal(bounds, network.cellular.unwrap_or(0), self.palette),
            Icon::Battery => match self.battery {
                Some(status) => battery(bounds, status.percentage, status.charging, self.palette),
                None => Vec::new(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Icon {
    Volume,
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

fn current_clock() -> String {
    let now = Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
}

#[cfg(test)]
mod tests {
    use super::{CLOCK_WIDTH, INSET, Icon, Status, cellular_visible, wifi_visible};
    use patin::ui::{Color, DrawCommand, Rect};
    use patin_icons::IconPalette;
    use patin_service_network::NetworkSnapshot;
    use patin_service_upower::BatterySnapshot;
    use patin_service_volume::VolumeSnapshot;

    fn palette() -> IconPalette {
        IconPalette {
            foreground: Color(236, 244, 248, 255),
            muted: Color(132, 152, 168, 255),
            background: Color(11, 15, 24, 255),
            accent: Color(82, 196, 186, 255),
            unavailable: Color(232, 150, 177, 255),
        }
    }

    fn status(network: Option<NetworkSnapshot>, battery: Option<BatterySnapshot>) -> Status {
        let mut status = Status::new(palette());
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
        assert!(
            clock.origin.x + CLOCK_WIDTH < width / 2.0,
            "clock stays left"
        );

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

    #[test]
    fn the_clock_can_be_left_out_for_a_composition_that_has_one() {
        let with_clock = status(None, None);
        assert!(
            with_clock
                .commands(500.0)
                .iter()
                .any(|command| matches!(command, DrawCommand::Text { .. })),
            "the greeter's strip keeps its clock"
        );

        let mut without = status(None, None);
        without = without.with_clock(false);
        assert!(
            without.commands(500.0).is_empty(),
            "the lock's strip draws no text at all when it has nothing else to show"
        );
    }

    #[test]
    fn volume_is_absent_until_it_is_asked_for() {
        // A reading alone is not enough: without `with_volume` the provider is
        // never constructed, so the snapshot stays empty and no slot appears.
        let silent = status(None, None);
        assert!(silent.volume_provider.is_none());
        assert!(!silent.icons().contains(&Icon::Volume));

        let mut loud = status(None, None).with_volume(true);
        assert!(loud.volume_provider.is_some());
        loud.volume = Some(VolumeSnapshot {
            percentage: 60,
            muted: false,
        });
        assert_eq!(loud.icons(), vec![Icon::Volume]);
    }

    #[test]
    fn volume_leads_the_row_and_battery_still_ends_it() {
        let mut status = status(
            Some(NetworkSnapshot {
                wifi: Some(80),
                cellular_available: true,
                ..Default::default()
            }),
            Some(BatterySnapshot {
                percentage: 75,
                charging: false,
            }),
        )
        .with_volume(true);
        status.volume = Some(VolumeSnapshot {
            percentage: 0,
            muted: true,
        });

        assert_eq!(
            status.icons(),
            vec![Icon::Volume, Icon::Wifi, Icon::Cellular, Icon::Battery],
            "volume joins on the inward side, battery keeps the corner"
        );
    }

    #[test]
    fn turning_volume_back_off_drops_its_reading() {
        let mut status = status(None, None).with_volume(true);
        status.volume = Some(VolumeSnapshot {
            percentage: 60,
            muted: false,
        });
        let status = status.with_volume(false);
        assert!(status.volume.is_none(), "a stale reading cannot linger");
        assert!(status.icons().is_empty());
    }
}
