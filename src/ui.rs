#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    pub fn contains(self, position: (f64, f64)) -> bool {
        let (x, y) = (position.0 as f32, position.1 as f32);
        x >= self.origin.x
            && y >= self.origin.y
            && x < self.origin.x + self.size.width
            && y < self.origin.y + self.size.height
    }

    pub fn inset(self, amount: f32) -> Self {
        Self::new(
            self.origin.x + amount,
            self.origin.y + amount,
            (self.size.width - amount * 2.0).max(0.0),
            (self.size.height - amount * 2.0).max(0.0),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Fixed(f32),
    Fill(f32),
}

pub fn row(bounds: Rect, gap: f32, lengths: &[Length]) -> Vec<Rect> {
    linear_layout(bounds, gap, lengths, false)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn column(bounds: Rect, gap: f32, lengths: &[Length]) -> Vec<Rect> {
    linear_layout(bounds, gap, lengths, true)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn stack(bounds: Rect, count: usize) -> Vec<Rect> {
    vec![bounds; count]
}

fn linear_layout(bounds: Rect, gap: f32, lengths: &[Length], vertical: bool) -> Vec<Rect> {
    if lengths.is_empty() {
        return Vec::new();
    }

    let main_size = if vertical {
        bounds.size.height
    } else {
        bounds.size.width
    };
    let gaps = gap.max(0.0) * lengths.len().saturating_sub(1) as f32;
    let available = (main_size - gaps).max(0.0);
    let fixed_total: f32 = lengths
        .iter()
        .map(|length| match length {
            Length::Fixed(value) => value.max(0.0),
            Length::Fill(_) => 0.0,
        })
        .sum();
    let fill_total: f32 = lengths
        .iter()
        .map(|length| match length {
            Length::Fill(weight) => weight.max(0.0),
            Length::Fixed(_) => 0.0,
        })
        .sum();
    let fixed_scale = if fixed_total > available && fixed_total > 0.0 {
        available / fixed_total
    } else {
        1.0
    };
    let fill_space = (available - fixed_total).max(0.0);
    let mut cursor = 0.0;

    lengths
        .iter()
        .map(|length| {
            let main = match length {
                Length::Fixed(value) => value.max(0.0) * fixed_scale,
                Length::Fill(weight) if fill_total > 0.0 => {
                    fill_space * weight.max(0.0) / fill_total
                }
                Length::Fill(_) => 0.0,
            };
            let rect = if vertical {
                Rect::new(
                    bounds.origin.x,
                    bounds.origin.y + cursor,
                    bounds.size.width,
                    main,
                )
            } else {
                Rect::new(
                    bounds.origin.x + cursor,
                    bounds.origin.y,
                    main,
                    bounds.size.height,
                )
            };
            cursor += main + gap.max(0.0);
            rect
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAlign {
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontFamily {
    SansSerif,
    Monospace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    Fill {
        bounds: Rect,
        color: Color,
    },
    Text {
        bounds: Rect,
        text: String,
        color: Color,
        font_size: f32,
        line_height: f32,
        family: FontFamily,
        align: TextAlign,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Toggle,
}

#[derive(Clone, Copy, Debug)]
pub struct BarStyle {
    pub background: Color,
    pub accent: Color,
    pub toggle_off: Color,
    pub toggle_on: Color,
    pub text: Color,
    pub padding: f32,
    pub gap: f32,
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

pub struct BarScene {
    size: Size,
    style: BarStyle,
    toggle_bounds: Rect,
    battery_bounds: Option<Rect>,
    volume_bounds: Option<Rect>,
    clock_bounds: Rect,
    toggle_active: bool,
    battery: Option<String>,
    volume: Option<String>,
    clock: String,
    damage: Vec<Rect>,
}

impl BarScene {
    pub fn new(clock: String) -> Self {
        Self {
            size: Size::default(),
            style: BarStyle::default(),
            toggle_bounds: Rect::default(),
            battery_bounds: None,
            volume_bounds: None,
            clock_bounds: Rect::default(),
            toggle_active: false,
            battery: None,
            volume: None,
            clock,
            damage: Vec::new(),
        }
    }

    pub fn resize(&mut self, size: Size) {
        if self.size == size {
            return;
        }
        self.size = size;
        self.layout();
        self.damage_all();
    }

    fn layout(&mut self) {
        let mut lengths = vec![Length::Fixed(180.0), Length::Fill(1.0)];
        if self.battery.is_some() {
            lengths.push(Length::Fixed(76.0));
        }
        if self.volume.is_some() {
            lengths.push(Length::Fixed(84.0));
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
        self.clock_bounds = children[index];
    }

    pub fn set_clock(&mut self, clock: String) -> bool {
        if self.clock == clock {
            return false;
        }
        self.clock = clock;
        self.damage.push(self.clock_bounds);
        true
    }

    pub fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }

    pub fn set_status(&mut self, battery: Option<String>, volume: Option<String>) -> bool {
        if self.battery == battery && self.volume == volume {
            return false;
        }

        let layout_changed = self.battery.is_some() != battery.is_some()
            || self.volume.is_some() != volume.is_some();
        let battery_changed = self.battery != battery;
        let volume_changed = self.volume != volume;
        self.battery = battery;
        self.volume = volume;

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
        }
        true
    }

    pub fn hit_test(&self, position: (f64, f64)) -> Option<Action> {
        self.toggle_bounds
            .contains(position)
            .then_some(Action::Toggle)
    }

    pub fn activate(&mut self, action: Action) {
        match action {
            Action::Toggle => {
                self.toggle_active = !self.toggle_active;
                self.damage.push(self.toggle_bounds);
            }
        }
    }

    pub fn toggle_active(&self) -> bool {
        self.toggle_active
    }

    pub fn commands(&self) -> Vec<DrawCommand> {
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

    pub fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, BarScene, DrawCommand, Length, Rect, Size, column, row, stack};

    #[test]
    fn row_assigns_remaining_space_to_fill_children() {
        let children = row(
            Rect::new(0.0, 0.0, 300.0, 32.0),
            8.0,
            &[Length::Fixed(100.0), Length::Fill(1.0), Length::Fixed(50.0)],
        );

        assert_eq!(children[0], Rect::new(0.0, 0.0, 100.0, 32.0));
        assert_eq!(children[1], Rect::new(108.0, 0.0, 134.0, 32.0));
        assert_eq!(children[2], Rect::new(250.0, 0.0, 50.0, 32.0));
    }

    #[test]
    fn column_and_stack_preserve_cross_axis_and_layer_bounds() {
        let bounds = Rect::new(2.0, 3.0, 80.0, 100.0);
        let children = column(bounds, 4.0, &[Length::Fill(1.0), Length::Fill(1.0)]);

        assert_eq!(children[0], Rect::new(2.0, 3.0, 80.0, 48.0));
        assert_eq!(children[1], Rect::new(2.0, 55.0, 80.0, 48.0));
        assert_eq!(stack(bounds, 2), vec![bounds, bounds]);
    }

    #[test]
    fn narrow_rows_shrink_fixed_children_without_negative_sizes() {
        let children = row(
            Rect::new(0.0, 0.0, 100.0, 32.0),
            8.0,
            &[Length::Fixed(180.0), Length::Fill(1.0), Length::Fixed(72.0)],
        );

        assert!(children.iter().all(|child| child.size.width >= 0.0));
        assert!(children[2].origin.x + children[2].size.width <= 100.0);
    }

    #[test]
    fn scene_hit_testing_and_damage_follow_component_bounds() {
        let mut scene = BarScene::new("12:00".into());
        scene.resize(Size {
            width: 500.0,
            height: 32.0,
        });
        scene.take_damage();

        assert_eq!(scene.hit_test((20.0, 16.0)), Some(Action::Toggle));
        assert_eq!(scene.hit_test((300.0, 16.0)), None);
        scene.activate(Action::Toggle);

        let damage = scene.take_damage();
        assert_eq!(damage.len(), 1);
        assert_eq!(damage[0], Rect::new(0.0, 0.0, 180.0, 32.0));
    }

    #[test]
    fn optional_status_components_relayout_and_render() {
        let mut scene = BarScene::new("12:00".into());
        scene.resize(Size {
            width: 509.0,
            height: 32.0,
        });
        scene.take_damage();

        assert!(scene.set_status(Some("BAT 55%+".into()), Some("VOL 65%".into())));
        assert_eq!(scene.take_damage(), vec![Rect::new(0.0, 0.0, 509.0, 32.0)]);
        let labels = scene
            .commands()
            .into_iter()
            .filter_map(|command| match command {
                DrawCommand::Text { text, .. } => Some(text),
                DrawCommand::Fill { .. } => None,
            })
            .collect::<Vec<_>>();

        assert!(labels.contains(&"BAT 55%+".to_owned()));
        assert!(labels.contains(&"VOL 65%".to_owned()));
    }
}
