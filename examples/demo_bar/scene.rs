use chrono::{Local, Timelike};
use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, Length, Rect, Size, TextAlign, row},
};

use super::services::SystemStatus;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Toggle,
}

#[derive(Clone, Copy, Debug)]
struct BarStyle {
    background: Color,
    accent: Color,
    toggle_off: Color,
    toggle_on: Color,
    text: Color,
    padding: f32,
    gap: f32,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            background: Color(20, 17, 29, 255),
            accent: Color(124, 58, 237, 255),
            toggle_off: Color(76, 67, 92, 255),
            toggle_on: Color(22, 163, 74, 255),
            text: Color(245, 243, 255, 255),
            padding: 5.0,
            gap: 8.0,
        }
    }
}

pub struct DemoBar {
    size: Size,
    style: BarStyle,
    toggle_bounds: Rect,
    battery_bounds: Option<Rect>,
    volume_bounds: Option<Rect>,
    brightness_bounds: Option<Rect>,
    network_bounds: Option<Rect>,
    clock_bounds: Rect,
    toggle_active: bool,
    battery: Option<String>,
    volume: Option<String>,
    brightness: Option<String>,
    network: Option<String>,
    clock: String,
    damage: Vec<Rect>,
    status: SystemStatus,
}

impl DemoBar {
    pub fn new() -> Self {
        let mut status = SystemStatus::new();
        let snapshot = status.poll();
        eprintln!(
            "demo_bar: status providers: battery={}, volume={}, brightness={}, network={}",
            snapshot.battery.as_deref().unwrap_or("unavailable"),
            snapshot.volume.as_deref().unwrap_or("unavailable"),
            snapshot.brightness.as_deref().unwrap_or("unavailable"),
            snapshot.network.as_deref().unwrap_or("unavailable")
        );
        let mut bar = Self {
            size: Size::default(),
            style: BarStyle::default(),
            toggle_bounds: Rect::default(),
            battery_bounds: None,
            volume_bounds: None,
            brightness_bounds: None,
            network_bounds: None,
            clock_bounds: Rect::default(),
            toggle_active: false,
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
        let mut lengths = vec![Length::Fixed(180.0), Length::Fill(1.0)];
        if self.battery.is_some() {
            lengths.push(Length::Fixed(76.0));
        }
        if self.volume.is_some() {
            lengths.push(Length::Fixed(84.0));
        }
        if self.brightness.is_some() {
            lengths.push(Length::Fixed(76.0));
        }
        if self.network.is_some() {
            lengths.push(Length::Fixed(76.0));
        }
        lengths.push(Length::Fixed(72.0));
        let children = row(
            Rect::new(0.0, 0.0, self.size.width, self.size.height),
            self.style.gap,
            &lengths,
        );
        self.toggle_bounds = children[0];
        let mut index = 2;
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
        self.clock_bounds = children[index];
    }

    fn set_status(
        &mut self,
        battery: Option<String>,
        volume: Option<String>,
        brightness: Option<String>,
        network: Option<String>,
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

    fn action_at(&self, position: (f64, f64)) -> Option<Action> {
        self.toggle_bounds
            .contains(position)
            .then_some(Action::Toggle)
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

    fn activate_at(&mut self, position: (f64, f64)) -> bool {
        let Some(Action::Toggle) = self.action_at(position) else {
            return false;
        };
        self.toggle_active = !self.toggle_active;
        self.damage.push(self.toggle_bounds);
        eprintln!(
            "demo_bar: toggle state is {}",
            if self.toggle_active { "on" } else { "off" }
        );
        true
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
            DrawCommand::Fill {
                bounds: self.toggle_bounds.inset(self.style.padding),
                color: if self.toggle_active {
                    self.style.toggle_on
                } else {
                    self.style.toggle_off
                },
            },
            DrawCommand::Text {
                bounds: self.toggle_bounds.inset(self.style.padding * 2.0),
                text: if self.toggle_active {
                    "SHELL ON".into()
                } else {
                    "SHELL OFF".into()
                },
                color: self.style.text,
                font_size: 12.0,
                line_height: 20.0,
                family: FontFamily::SansSerif,
                align: TextAlign::Center,
            },
        ];
        for (bounds, text) in [
            (self.battery_bounds, self.battery.as_ref()),
            (self.volume_bounds, self.volume.as_ref()),
            (self.brightness_bounds, self.brightness.as_ref()),
            (self.network_bounds, self.network.as_ref()),
        ] {
            if let (Some(bounds), Some(text)) = (bounds, text) {
                commands.push(DrawCommand::Text {
                    bounds: bounds.inset(self.style.padding),
                    text: text.clone(),
                    color: self.style.text,
                    font_size: 12.0,
                    line_height: 20.0,
                    family: FontFamily::SansSerif,
                    align: TextAlign::Center,
                });
            }
        }
        commands.push(DrawCommand::Text {
            bounds: self.clock_bounds.inset(self.style.padding),
            text: self.clock.clone(),
            color: self.style.text,
            font_size: 15.0,
            line_height: 20.0,
            family: FontFamily::Monospace,
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

fn current_clock() -> String {
    let now = Local::now();
    format_clock(now.hour(), now.minute())
}

fn format_clock(hour: u32, minute: u32) -> String {
    format!("{hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::{DemoBar, format_clock};
    use patin::{
        platform::Shell,
        ui::{DrawCommand, Rect, Size},
    };

    #[test]
    fn formats_clock_with_leading_zeroes() {
        assert_eq!(format_clock(7, 5), "07:05");
        assert_eq!(format_clock(23, 59), "23:59");
    }

    #[test]
    fn demo_hit_testing_and_damage_follow_toggle_bounds() {
        let mut bar = DemoBar::new();
        bar.resize(Size {
            width: 500.0,
            height: 32.0,
        });
        bar.take_damage();
        assert!(bar.activate_at((20.0, 16.0)));
        assert!(!bar.activate_at((300.0, 16.0)));
        let damage = bar.take_damage();
        assert_eq!(damage.len(), 1);
        assert_eq!(damage[0].origin, Rect::default().origin);
        assert!(damage[0].size.width > 0.0);
        assert_eq!(damage[0].size.height, 32.0);
        assert!(bar.commands().iter().any(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "SHELL ON")
        ));
    }
}
