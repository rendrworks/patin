//! Placing the controls: the per-page row/button geometry, and the small
//! helpers that register a button's hit target as it is laid out.

use patin::ui::{Rect, TextAlign};
use patin_icons::WifiSignal;
use patin_service_network::{HotspotBand, HotspotSecurity};

use super::{Action, Button, Editing, NetworkSettings, Page, ROW};

impl NetworkSettings {
    pub(super) fn layout(&mut self) {
        self.buttons.clear();
        self.wifi_icons.clear();
        let width = self.size.width.clamp(1.0, 520.0);
        let left = (self.size.width - width) / 2.0;
        self.centered_button(Rect::new(left + 12.0, 14.0, 48.0, 40.0), "×", Action::Close);
        let tab_width = (width - 90.0) / 3.0;
        self.button(
            Rect::new(left + 72.0, 14.0, tab_width, 40.0),
            "Wi-Fi",
            Action::WifiPage,
        );
        self.button(
            Rect::new(left + 72.0 + tab_width, 14.0, tab_width, 40.0),
            "Cellular",
            Action::CellularPage,
        );
        self.button(
            Rect::new(left + 72.0 + tab_width * 2.0, 14.0, tab_width, 40.0),
            "Hotspot",
            Action::HotspotPage,
        );
        let mut y = 70.0;
        match self.page {
            Page::Wifi => {
                let row_width = width - 32.0;
                let disconnect_width = 112.0;
                let connected = self.snapshot.wifi.is_some();
                self.button(
                    Rect::new(
                        left + 16.0,
                        y,
                        if connected {
                            row_width - disconnect_width - 6.0
                        } else {
                            row_width
                        },
                        ROW,
                    ),
                    if self.initial_refresh_pending {
                        "Wi-Fi: loading…"
                    } else if self.snapshot.wifi_enabled {
                        "Wi-Fi: on"
                    } else {
                        "Wi-Fi: off"
                    },
                    Action::ToggleWifi,
                );
                if connected {
                    self.button(
                        Rect::new(
                            left + 16.0 + row_width - disconnect_width,
                            y,
                            disconnect_width,
                            ROW,
                        ),
                        "Disconnect",
                        Action::Disconnect,
                    );
                }
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    if self.scan_pending {
                        "Scanning for new networks…"
                    } else {
                        "Scan for new networks"
                    },
                    Action::ScanWifi,
                );
                y += ROW + 6.0;
                let row_capacity =
                    ((self.size.height - y + 6.0).max(0.0) / (ROW + 6.0)).floor() as usize;
                let max_rows = row_capacity.min(if self.editing.is_some() { 2 } else { 5 });
                let visible_networks = self
                    .networks
                    .iter()
                    .enumerate()
                    .filter(|(_, network)| network.available)
                    .take(max_rows)
                    .map(|(index, network)| (index, network.clone()))
                    .collect::<Vec<_>>();
                for (index, network) in visible_networks {
                    let forget_width = 82.0;
                    let row_width = width - 32.0;
                    let main_width = if network.known {
                        row_width - forget_width - 6.0
                    } else {
                        row_width
                    };
                    let main_bounds = Rect::new(left + 16.0, y, main_width, ROW);
                    let signal = WifiSignal::from_percentage(network.strength);
                    let label = if network.active {
                        format!("{} • connected", network.ssid)
                    } else {
                        network.ssid.clone()
                    };
                    self.wifi_icons.push((
                        Rect::new(main_bounds.origin.x + 8.0, y + 14.0, 24.0, 24.0),
                        signal,
                    ));
                    self.indented_button(main_bounds, &label, Action::Connect(index), 42.0);
                    if network.known {
                        self.centered_button(
                            Rect::new(left + 16.0 + row_width - forget_width, y, forget_width, ROW),
                            "Forget",
                            Action::Forget(index),
                        );
                    }
                    y += ROW + 6.0;
                }
            }
            Page::Cellular => self.button(
                Rect::new(left + 16.0, y, width - 32.0, ROW),
                if self.initial_refresh_pending {
                    "Mobile data: loading…"
                } else if self.snapshot.cellular_enabled {
                    "Mobile data: on"
                } else {
                    "Mobile data: off"
                },
                Action::ToggleCellular,
            ),
            Page::Hotspot => {
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    if self.initial_refresh_pending {
                        "Hotspot: loading…"
                    } else if self.snapshot.hotspot_active {
                        "Hotspot: on"
                    } else {
                        "Hotspot: off"
                    },
                    Action::ToggleHotspot,
                );
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    match self.hotspot.security {
                        HotspotSecurity::Open => "Security: open",
                        HotspotSecurity::WpaPersonal => "Security: WPA2/WPA3 personal",
                    },
                    Action::ToggleHotspotSecurity,
                );
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    match self.hotspot.band {
                        HotspotBand::Automatic => "Band: automatic",
                        HotspotBand::Ghz2_4 => "Band: 2.4 GHz",
                        HotspotBand::Ghz5 => "Band: 5 GHz",
                    },
                    Action::ToggleHotspotBand,
                );
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    &format!(
                        "{}SSID: {}",
                        if self.editing == Some(Editing::HotspotSsid) {
                            "Editing • "
                        } else {
                            ""
                        },
                        self.hotspot.ssid
                    ),
                    Action::EditHotspotSsid,
                );
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    if self.editing == Some(Editing::HotspotPassword) {
                        "Editing • Password: ••••••••"
                    } else if self.hotspot_password.is_empty() {
                        "Set hotspot password"
                    } else {
                        "Password: ••••••••"
                    },
                    Action::EditHotspotPassword,
                );
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    "Save hotspot settings",
                    Action::SaveHotspot,
                );
            }
        }
    }

    fn button(&mut self, bounds: Rect, label: &str, action: Action) {
        self.buttons.push(Button {
            bounds,
            label: label.into(),
            action,
            text_align: TextAlign::Start,
            text_inset: 10.0,
        });
    }
    fn indented_button(&mut self, bounds: Rect, label: &str, action: Action, text_inset: f32) {
        self.buttons.push(Button {
            bounds,
            label: label.into(),
            action,
            text_align: TextAlign::Start,
            text_inset,
        });
    }
    fn centered_button(&mut self, bounds: Rect, label: &str, action: Action) {
        self.buttons.push(Button {
            bounds,
            label: label.into(),
            action,
            text_align: TextAlign::Center,
            text_inset: 0.0,
        });
    }
}
