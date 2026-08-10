use patin::{
    keyboard::{Key, KeyboardMode, TouchKeyboard},
    platform::{KeyInput, Shell},
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
}

#[derive(Clone, Debug)]
enum Action {
    Close,
    WifiPage,
    CellularPage,
    ToggleWifi,
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
    keyboard: TouchKeyboard,
    buttons: Vec<Button>,
    error: Option<String>,
    close: bool,
    damage: Vec<Rect>,
}

impl NetworkSettings {
    pub fn new(page: Option<&str>) -> Self {
        let mut provider = NetworkProvider::new();
        let snapshot = provider.poll().unwrap_or_default();
        let networks = provider.wifi_networks().unwrap_or_default();
        let hotspot = provider.hotspot_config();
        let mut settings = Self {
            size: Size::default(),
            page: if page == Some("cellular") {
                Page::Cellular
            } else {
                Page::Wifi
            },
            provider,
            snapshot,
            networks,
            hotspot,
            hotspot_password: Zeroizing::new(String::new()),
            wifi_password: Zeroizing::new(String::new()),
            editing: None,
            keyboard: TouchKeyboard::new(KeyboardMode::Full),
            buttons: Vec::new(),
            error: None,
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
        self.button(Rect::new(left + 12.0, 14.0, 48.0, 40.0), "‹", Action::Close);
        self.button(
            Rect::new(left + 72.0, 14.0, (width - 90.0) / 2.0, 40.0),
            "Wi-Fi",
            Action::WifiPage,
        );
        self.button(
            Rect::new(
                left + 72.0 + (width - 90.0) / 2.0,
                14.0,
                (width - 90.0) / 2.0,
                40.0,
            ),
            "Cellular",
            Action::CellularPage,
        );
        let mut y = 70.0;
        match self.page {
            Page::Wifi => {
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    if self.snapshot.wifi_enabled {
                        "Wi-Fi: on"
                    } else {
                        "Wi-Fi: off"
                    },
                    Action::ToggleWifi,
                );
                y += ROW + 6.0;
                if self.snapshot.wifi.is_some() {
                    self.button(
                        Rect::new(left + 16.0, y, width - 32.0, ROW),
                        "Disconnect current Wi-Fi",
                        Action::Disconnect,
                    );
                    y += ROW + 6.0;
                }
                let max_rows = if self.editing.is_some() { 2 } else { 5 };
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
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    if self.snapshot.hotspot_active {
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
                    &format!("SSID: {}", self.hotspot.ssid),
                    Action::EditHotspotSsid,
                );
                y += ROW + 6.0;
                self.button(
                    Rect::new(left + 16.0, y, width - 32.0, ROW),
                    if self.hotspot_password.is_empty() {
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
            Page::Cellular => self.button(
                Rect::new(left + 16.0, y, width - 32.0, ROW),
                if self.snapshot.cellular_enabled {
                    "Mobile data: on"
                } else {
                    "Mobile data: off"
                },
                Action::ToggleCellular,
            ),
        }
    }

    fn button(&mut self, bounds: Rect, label: &str, action: Action) {
        self.buttons.push(Button {
            bounds,
            label: label.into(),
            action,
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

    fn edit_key(&mut self, key: Key) {
        let Some(editing) = self.editing else { return };
        let target: &mut String = match editing {
            Editing::WifiPassword(_) => &mut self.wifi_password,
            Editing::HotspotSsid => &mut self.hotspot.ssid,
            Editing::HotspotPassword => &mut self.hotspot_password,
        };
        match key {
            Key::Character(character) if target.len() + character.len_utf8() <= 63 => {
                target.push(character)
            }
            Key::Space if target.len() < 63 => target.push(' '),
            Key::Backspace => {
                target.pop();
            }
            Key::Enter => self.submit_edit(),
            _ => {}
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
            self.wifi_password.clear();
            self.editing = None;
            self.result(result);
            return;
        }
        self.editing = None;
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Close => self.close = true,
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
            Action::ToggleWifi => {
                let result = self.provider.set_wifi_enabled(!self.snapshot.wifi_enabled);
                self.result(result);
            }
            Action::ToggleCellular => {
                let result = self
                    .provider
                    .set_cellular_enabled(!self.snapshot.cellular_enabled);
                self.result(result);
            }
            Action::Disconnect => {
                let result = self.provider.disconnect_wifi();
                self.result(result);
            }
            Action::Forget(index) => {
                if let Some(network) = self.networks.get(index) {
                    let result = self.provider.forget_wifi(&network.ssid);
                    self.result(result);
                }
            }
            Action::Connect(index) => {
                if let Some(network) = self.networks.get(index) {
                    match network.security {
                        WifiSecurity::Open => {
                            let result = self.provider.connect_wifi(&network.ssid, None);
                            self.result(result);
                        }
                        WifiSecurity::Personal => {
                            if self.provider.connect_wifi(&network.ssid, None).is_err() {
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
        if self.editing.is_some()
            && let Some(key) = self
                .keyboard
                .key_at(self.size.width, self.size.height, position)
                .and_then(|key| self.keyboard.press(key))
        {
            self.edit_key(key);
            return true;
        }
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
        match input {
            KeyInput::Text(text) => {
                for character in text.chars() {
                    self.edit_key(Key::Character(character));
                }
            }
            KeyInput::Backspace => self.edit_key(Key::Backspace),
            KeyInput::Enter => self.edit_key(Key::Enter),
            KeyInput::Escape => {
                if self.editing.take().is_none() {
                    self.close = true;
                }
            }
        }
        true
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
            commands.push(text(button.bounds.inset(10.0), &button.label, 15.0));
        }
        if let Some(error) = &self.error {
            commands.push(text(
                Rect::new(16.0, self.size.height * 0.45, self.size.width - 32.0, 56.0),
                error,
                13.0,
            ));
        }
        if self.editing.is_some() {
            commands.extend(
                self.keyboard
                    .commands(self.size.width, self.size.height, false),
            );
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
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color: Color(245, 243, 255, 255),
        font_size: size,
        line_height: size * 1.25,
        family: FontFamily::SansSerif,
        weight: FontWeight::Normal,
        align: TextAlign::Start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_page_is_selected() {
        assert_eq!(NetworkSettings::new(Some("cellular")).page, Page::Cellular);
    }
    #[test]
    fn escape_closes_when_not_editing() {
        let mut ui = NetworkSettings::new(Some("wifi"));
        ui.key_input(KeyInput::Escape);
        assert!(ui.close_requested());
    }
}
