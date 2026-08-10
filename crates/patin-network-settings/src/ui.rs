use patin::{
    platform::{KeyInput, Shell, TextInputPurpose},
    service::Provider,
    ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, Size, TextAlign},
};
use patin_service_network::{
    HotspotBand, HotspotConfig, HotspotSecurity, NetworkProvider, NetworkSnapshot, WifiNetwork,
    WifiSecurity,
};
use zeroize::Zeroizing;

const ROW: f32 = 52.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Wifi,
    Cellular,
    Hotspot,
}

#[derive(Clone, Debug)]
enum Action {
    Close,
    WifiPage,
    CellularPage,
    HotspotPage,
    ToggleWifi,
    ScanWifi,
    ToggleCellular,
    Connect(usize),
    Disconnect,
    Forget(usize),
    EditHotspotSsid,
    EditHotspotPassword,
    ToggleHotspotSecurity,
    ToggleHotspotBand,
    SaveHotspot,
    ToggleHotspot,
}

#[derive(Clone, Debug)]
struct Button {
    bounds: Rect,
    label: String,
    action: Action,
    text_align: TextAlign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Editing {
    WifiPassword(usize),
    HotspotSsid,
    HotspotPassword,
}

pub struct NetworkSettings {
    size: Size,
    page: Page,
    provider: NetworkProvider,
    snapshot: NetworkSnapshot,
    networks: Vec<WifiNetwork>,
    hotspot: HotspotConfig,
    hotspot_password: Zeroizing<String>,
    wifi_password: Zeroizing<String>,
    editing: Option<Editing>,
    buttons: Vec<Button>,
    error: Option<String>,
    initial_refresh_pending: bool,
    scan_pending: bool,
    close: bool,
    damage: Vec<Rect>,
}

impl NetworkSettings {
    pub fn new(page: Option<&str>) -> Self {
        let provider = NetworkProvider::new();
        let mut settings = Self {
            size: Size::default(),
            page: match page {
                Some("cellular") => Page::Cellular,
                Some("hotspot") => Page::Hotspot,
                _ => Page::Wifi,
            },
            provider,
            snapshot: NetworkSnapshot::default(),
            networks: Vec::new(),
            hotspot: HotspotConfig::default(),
            hotspot_password: Zeroizing::new(String::new()),
            wifi_password: Zeroizing::new(String::new()),
            editing: None,
            buttons: Vec::new(),
            error: None,
            initial_refresh_pending: true,
            scan_pending: false,
            close: false,
            damage: Vec::new(),
        };
        settings.layout();
        settings
    }

    fn layout(&mut self) {
        self.buttons.clear();
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
                for index in 0..self.networks.len().min(max_rows) {
                    let network = &self.networks[index];
                    let suffix = if network.active { " • connected" } else { "" };
                    self.button(
                        Rect::new(left + 16.0, y, width - 92.0, ROW),
                        &format!("{}  {}%{suffix}", network.ssid, network.strength),
                        Action::Connect(index),
                    );
                    self.button(
                        Rect::new(left + width - 70.0, y, 54.0, ROW),
                        "Forget",
                        Action::Forget(index),
                    );
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
        });
    }
    fn centered_button(&mut self, bounds: Rect, label: &str, action: Action) {
        self.buttons.push(Button {
            bounds,
            label: label.into(),
            action,
            text_align: TextAlign::Center,
        });
    }
    fn redraw(&mut self) {
        self.layout();
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
    fn result(&mut self, result: Result<(), impl std::fmt::Display>) {
        self.error = result.err().map(|error| error.to_string());
        self.redraw();
    }

    fn edit_input(&mut self, input: KeyInput) {
        let Some(editing) = self.editing else { return };
        let target: &mut String = match editing {
            Editing::WifiPassword(_) => &mut self.wifi_password,
            Editing::HotspotSsid => &mut self.hotspot.ssid,
            Editing::HotspotPassword => &mut self.hotspot_password,
        };
        match input {
            KeyInput::Text(text) => {
                for character in text.chars() {
                    if target.len() + character.len_utf8() <= 63 {
                        target.push(character);
                    }
                }
            }
            KeyInput::Backspace => {
                target.pop();
            }
            KeyInput::Enter => self.submit_edit(),
            KeyInput::Escape => {
                self.editing = None;
            }
        }
        self.redraw();
    }

    fn submit_edit(&mut self) {
        if let Some(Editing::WifiPassword(index)) = self.editing
            && let Some(network) = self.networks.get(index)
        {
            let result = self
                .provider
                .connect_wifi(&network.ssid, Some(&self.wifi_password));
            if result.is_ok() {
                self.mark_connected(index);
            }
            self.wifi_password.clear();
            self.editing = None;
            self.result(result);
            return;
        }
        self.editing = None;
    }

    fn mark_connected(&mut self, index: usize) {
        let strength = self.networks.get(index).map(|network| network.strength);
        for (candidate_index, network) in self.networks.iter_mut().enumerate() {
            network.active = candidate_index == index;
        }
        self.snapshot.wifi = strength;
    }

    fn mark_disconnected(&mut self) {
        for network in &mut self.networks {
            network.active = false;
        }
        self.snapshot.wifi = None;
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Close => {
                self.editing = None;
                self.close = true;
            }
            Action::WifiPage => {
                self.page = Page::Wifi;
                self.editing = None;
                self.redraw();
            }
            Action::CellularPage => {
                self.page = Page::Cellular;
                self.editing = None;
                self.redraw();
            }
            Action::HotspotPage => {
                self.page = Page::Hotspot;
                self.editing = None;
                self.redraw();
            }
            Action::ToggleWifi => {
                let result = self.provider.set_wifi_enabled(!self.snapshot.wifi_enabled);
                self.result(result);
            }
            Action::ScanWifi => {
                if !self.scan_pending {
                    self.scan_pending = true;
                    self.error = None;
                    self.redraw();
                }
            }
            Action::ToggleCellular => {
                let result = self
                    .provider
                    .set_cellular_enabled(!self.snapshot.cellular_enabled);
                self.result(result);
            }
            Action::Disconnect => {
                let result = self.provider.disconnect_wifi();
                if result.is_ok() {
                    self.mark_disconnected();
                }
                self.result(result);
            }
            Action::Forget(index) => {
                if let Some(network) = self.networks.get(index) {
                    let result = self.provider.forget_wifi(&network.ssid);
                    if result.is_ok() {
                        self.networks.remove(index);
                    }
                    self.result(result);
                }
            }
            Action::Connect(index) => {
                if let Some(network) = self.networks.get(index) {
                    match network.security {
                        WifiSecurity::Open => {
                            let result = self.provider.connect_wifi(&network.ssid, None);
                            if result.is_ok() {
                                self.mark_connected(index);
                            }
                            self.result(result);
                        }
                        WifiSecurity::Personal => {
                            let result = self.provider.connect_wifi(&network.ssid, None);
                            if result.is_ok() {
                                self.mark_connected(index);
                                self.result(result);
                            } else {
                                self.wifi_password.clear();
                                self.editing = Some(Editing::WifiPassword(index));
                                self.redraw();
                            }
                        }
                        WifiSecurity::Unsupported => {
                            self.error = Some(
                                "This network needs unsupported enterprise credentials".into(),
                            );
                            self.redraw();
                        }
                    }
                }
            }
            Action::EditHotspotSsid => {
                self.editing = Some(Editing::HotspotSsid);
                self.redraw();
            }
            Action::EditHotspotPassword => {
                self.hotspot_password.clear();
                self.editing = Some(Editing::HotspotPassword);
                self.redraw();
            }
            Action::ToggleHotspotSecurity => {
                self.hotspot.security = match self.hotspot.security {
                    HotspotSecurity::Open => HotspotSecurity::WpaPersonal,
                    HotspotSecurity::WpaPersonal => HotspotSecurity::Open,
                };
                self.redraw();
            }
            Action::ToggleHotspotBand => {
                self.hotspot.band = match self.hotspot.band {
                    HotspotBand::Automatic => HotspotBand::Ghz2_4,
                    HotspotBand::Ghz2_4 => HotspotBand::Ghz5,
                    HotspotBand::Ghz5 => HotspotBand::Automatic,
                };
                self.redraw();
            }
            Action::SaveHotspot => {
                let password =
                    (!self.hotspot_password.is_empty()).then_some(self.hotspot_password.as_str());
                let result = self.provider.save_hotspot(&self.hotspot, password);
                if result.is_ok() && password.is_some() {
                    self.hotspot.password_configured = true;
                    self.hotspot_password.clear();
                }
                self.result(result);
            }
            Action::ToggleHotspot => {
                let result = self
                    .provider
                    .set_hotspot_enabled(!self.snapshot.hotspot_active);
                self.result(result);
            }
        }
    }
}

impl Shell for NetworkSettings {
    fn resize(&mut self, size: Size) {
        if self.size != size {
            self.size = size;
            self.redraw();
        }
    }
    fn update(&mut self) -> bool {
        if self.initial_refresh_pending {
            self.initial_refresh_pending = false;
            self.snapshot = self.provider.poll().unwrap_or_default();
            match self.provider.known_wifi_networks() {
                Ok(networks) => self.networks = networks,
                Err(error) => self.error = Some(error.to_string()),
            }
            self.hotspot = self.provider.hotspot_config();
            self.redraw();
            return true;
        }
        if self.scan_pending {
            self.scan_pending = false;
            match self.provider.scan_wifi_networks() {
                Ok(networks) => {
                    self.networks = networks;
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            self.redraw();
            return true;
        }
        let next = self.provider.poll().unwrap_or_default();
        if next != self.snapshot {
            self.snapshot = next;
            self.redraw();
            true
        } else {
            false
        }
    }
    fn activate_at(&mut self, position: (f64, f64)) -> bool {
        if let Some(action) = self
            .buttons
            .iter()
            .find(|button| button.bounds.contains(position))
            .map(|button| button.action.clone())
        {
            self.act(action);
            return true;
        }
        false
    }
    fn key_input(&mut self, input: KeyInput) -> bool {
        if self.editing.is_some() {
            self.edit_input(input);
        } else if input == KeyInput::Escape {
            self.close = true;
        }
        true
    }
    fn text_input(&self) -> Option<TextInputPurpose> {
        match self.editing {
            Some(Editing::HotspotSsid) => Some(TextInputPurpose::Normal),
            Some(Editing::WifiPassword(_) | Editing::HotspotPassword) => {
                Some(TextInputPurpose::Password)
            }
            None => None,
        }
    }
    fn close_requested(&self) -> bool {
        self.close
    }
    fn commands(&self) -> Vec<DrawCommand> {
        let mut commands = vec![DrawCommand::Fill {
            bounds: Rect::new(0.0, 0.0, self.size.width, self.size.height),
            color: Color(20, 17, 29, 255),
        }];
        for button in &self.buttons {
            commands.push(DrawCommand::RoundedFill {
                bounds: button.bounds,
                color: Color(57, 48, 75, 255),
                radius: 10.0,
            });
            commands.push(aligned_text(
                if button.text_align == TextAlign::Center {
                    button.bounds
                } else {
                    button.bounds.inset(10.0)
                },
                &button.label,
                15.0,
                button.text_align,
            ));
        }
        if let Some(error) = &self.error {
            commands.push(text(
                Rect::new(16.0, self.size.height * 0.45, self.size.width - 32.0, 56.0),
                error,
                13.0,
            ));
        }
        if let Some(Editing::WifiPassword(index)) = self.editing
            && let Some(network) = self.networks.get(index)
        {
            commands.push(DrawCommand::RoundedFill {
                bounds: Rect::new(16.0, self.size.height - 76.0, self.size.width - 32.0, 52.0),
                color: Color(75, 62, 98, 255),
                radius: 10.0,
            });
            commands.push(text(
                Rect::new(28.0, self.size.height - 76.0, self.size.width - 56.0, 52.0),
                &format!(
                    "Password for {}: {}",
                    network.ssid,
                    "•".repeat(self.wifi_password.chars().count())
                ),
                14.0,
            ));
        }
        commands
    }
    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }
    fn damage_all(&mut self) {
        self.redraw();
    }
}

fn text(bounds: Rect, value: &str, size: f32) -> DrawCommand {
    aligned_text(bounds, value, size, TextAlign::Start)
}

fn aligned_text(bounds: Rect, value: &str, size: f32, align: TextAlign) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color: Color(245, 243, 255, 255),
        font_size: size,
        line_height: size * 1.25,
        family: FontFamily::SansSerif,
        weight: FontWeight::Normal,
        align,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_page_is_selected() {
        assert_eq!(NetworkSettings::new(Some("cellular")).page, Page::Cellular);
        assert_eq!(NetworkSettings::new(Some("hotspot")).page, Page::Hotspot);
        assert_eq!(NetworkSettings::new(Some("unknown")).page, Page::Wifi);
    }

    #[test]
    fn construction_defers_network_discovery_until_after_window_creation() {
        let ui = NetworkSettings::new(Some("wifi"));
        assert!(ui.initial_refresh_pending);
        assert!(ui.networks.is_empty());
        assert_eq!(ui.hotspot, HotspotConfig::default());
        assert!(
            ui.buttons
                .iter()
                .any(|button| button.label == "Wi-Fi: loading…")
        );
    }

    #[test]
    fn scan_is_explicit_and_close_is_centered() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        assert!(!ui.scan_pending);
        assert!(ui.buttons.iter().any(|button| {
            matches!(button.action, Action::ScanWifi) && button.label == "Scan for new networks"
        }));
        assert!(ui.buttons.iter().any(|button| {
            matches!(button.action, Action::Close)
                && button.label == "×"
                && button.text_align == TextAlign::Center
        }));

        ui.act(Action::ScanWifi);
        assert!(ui.scan_pending);
        assert!(
            ui.buttons
                .iter()
                .any(|button| button.label == "Scanning for new networks…")
        );
    }

    #[test]
    fn scan_row_keeps_network_buttons_inside_minimum_height() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.initial_refresh_pending = false;
        ui.snapshot.wifi = Some(80);
        ui.networks = (0..5)
            .map(|index| WifiNetwork {
                ssid: format!("Network {index}"),
                strength: 50,
                security: WifiSecurity::Personal,
                active: index == 0,
            })
            .collect();
        ui.resize(Size {
            width: 320.0,
            height: 480.0,
        });

        assert!(ui.buttons.iter().all(|button| {
            button.bounds.origin.y + button.bounds.size.height <= ui.size.height
        }));
    }

    #[test]
    fn disconnect_sits_right_of_wifi_toggle_when_connected() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.initial_refresh_pending = false;
        ui.snapshot.wifi = Some(69);
        ui.resize(Size {
            width: 320.0,
            height: 480.0,
        });

        let wifi = ui
            .buttons
            .iter()
            .find(|button| matches!(button.action, Action::ToggleWifi))
            .unwrap();
        let disconnect = ui
            .buttons
            .iter()
            .find(|button| matches!(button.action, Action::Disconnect))
            .unwrap();
        assert_eq!(wifi.bounds.origin.y, disconnect.bounds.origin.y);
        assert!(disconnect.bounds.origin.x > wifi.bounds.origin.x);
        assert_eq!(disconnect.label, "Disconnect");
    }

    #[test]
    fn successful_connection_actions_update_visible_active_state_immediately() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.networks = vec![
            WifiNetwork {
                ssid: "DELTA".into(),
                strength: 69,
                security: WifiSecurity::Personal,
                active: true,
            },
            WifiNetwork {
                ssid: "Corner".into(),
                strength: 42,
                security: WifiSecurity::Personal,
                active: false,
            },
        ];

        ui.mark_disconnected();
        assert_eq!(ui.snapshot.wifi, None);
        assert!(ui.networks.iter().all(|network| !network.active));

        ui.mark_connected(1);
        assert_eq!(ui.snapshot.wifi, Some(42));
        assert!(!ui.networks[0].active);
        assert!(ui.networks[1].active);
    }

    #[test]
    fn hotspot_controls_are_only_on_the_hotspot_page() {
        let wifi = NetworkSettings::new(Some("wifi"));
        assert!(
            !wifi
                .buttons
                .iter()
                .any(|button| matches!(button.action, Action::ToggleHotspot))
        );

        let hotspot = NetworkSettings::new(Some("hotspot"));
        assert!(
            hotspot
                .buttons
                .iter()
                .any(|button| matches!(button.action, Action::ToggleHotspot))
        );
        assert!(
            !hotspot
                .buttons
                .iter()
                .any(|button| matches!(button.action, Action::ToggleWifi))
        );
    }
    #[test]
    fn escape_closes_when_not_editing() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.key_input(KeyInput::Escape);
        assert!(ui.close_requested());
    }

    #[test]
    fn editing_exposes_system_text_input_purpose() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        assert_eq!(ui.text_input(), None);

        ui.editing = Some(Editing::HotspotSsid);
        assert_eq!(ui.text_input(), Some(TextInputPurpose::Normal));

        ui.editing = Some(Editing::HotspotPassword);
        assert_eq!(ui.text_input(), Some(TextInputPurpose::Password));

        ui.key_input(KeyInput::Escape);
        assert_eq!(ui.text_input(), None);
        assert!(!ui.close_requested());
    }
}
