//! The [`Shell`] implementation: the compositor-facing lifecycle, the
//! polling tick that refreshes network state, and the draw commands for
//! whichever page is showing.

use patin::{
    service::Provider,
    platform::{KeyInput, Shell, TextInputPurpose},
    ui::{Color, DrawCommand, Rect, Size, TextAlign},
};
use patin_icons::{IconPalette, wifi_signal};

use super::{Editing, NetworkSettings, aligned_text, text, wifi_refresh_due, wifi_scan_due};

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
            self.wifi_refresh_ticks = 0;
            self.wifi_scan_ticks = 0;
            self.redraw();
            return true;
        }
        if self.scan_pending {
            self.scan_pending = false;
            match self.provider.scan_wifi_networks() {
                Ok(networks) => {
                    self.networks = networks;
                    self.error = None;
                    self.show_discovered = true;
                    self.wifi_refresh_ticks = 0;
                    self.wifi_scan_ticks = 0;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            self.redraw();
            return true;
        }
        let next = self.provider.poll().unwrap_or_default();
        let mut changed = false;
        if next != self.snapshot {
            self.snapshot = next;
            changed = true;
        }
        if wifi_scan_due(self.page, &mut self.wifi_scan_ticks)
            && let Err(error) = self.provider.request_wifi_scan()
        {
            let error = error.to_string();
            if self.error.as_deref() != Some(&error) {
                self.error = Some(error);
                changed = true;
            }
        }
        if wifi_refresh_due(self.page, &mut self.wifi_refresh_ticks) {
            match self
                .provider
                .refresh_wifi_networks(&self.networks, self.show_discovered)
            {
                Ok(networks) if networks != self.networks => {
                    self.networks = networks;
                    self.error = None;
                    changed = true;
                }
                Ok(_) => {}
                Err(error) => {
                    let error = error.to_string();
                    if self.error.as_deref() != Some(&error) {
                        self.error = Some(error);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.redraw();
        }
        changed
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
                    Rect::new(
                        button.bounds.origin.x + button.text_inset,
                        button.bounds.origin.y + 10.0,
                        (button.bounds.size.width - button.text_inset - 10.0).max(0.0),
                        (button.bounds.size.height - 20.0).max(0.0),
                    )
                },
                &button.label,
                15.0,
                button.text_align,
            ));
        }
        for (bounds, signal) in &self.wifi_icons {
            commands.extend(wifi_signal(
                *bounds,
                *signal,
                IconPalette {
                    foreground: Color(245, 243, 255, 255),
                    muted: Color(112, 102, 132, 255),
                    background: Color(57, 48, 75, 255),
                    accent: Color(124, 58, 237, 255),
                    unavailable: Color(239, 96, 119, 255),
                },
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
