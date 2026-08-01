use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, Size, TextAlign},
};

use crate::apps::Application;

const PANEL_INSET: f32 = 2.0;
const PANEL_WIDTH: f32 = 280.0;
const PANEL_HEIGHT: f32 = 350.0;
const HORIZONTAL_PADDING: f32 = 18.0;
const VERTICAL_PADDING: f32 = 20.0;
const INNER_PADDING: f32 = 10.0;
const ROW_HEIGHT: f32 = 31.0;
const ICON_SIZE: f32 = 18.0;
const SCROLL_STEP: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ListRow {
    bounds: Rect,
    application: usize,
}

pub struct Launcher {
    size: Size,
    panel_bounds: Rect,
    applications: Vec<Application>,
    first_visible: usize,
    visible_count: usize,
    scroll_remainder: f64,
    rows: Vec<ListRow>,
    close: bool,
    error: Option<String>,
    damage: Vec<Rect>,
}

impl Launcher {
    pub fn new(applications: Vec<Application>) -> Self {
        Self {
            size: Size::default(),
            panel_bounds: Rect::default(),
            applications,
            first_visible: 0,
            visible_count: 1,
            scroll_remainder: 0.0,
            rows: Vec::new(),
            close: false,
            error: None,
            damage: Vec::new(),
        }
    }

    fn layout(&mut self) {
        let panel_width = self.size.width.clamp(1.0, PANEL_WIDTH);
        let panel_height = self.size.height.clamp(1.0, PANEL_HEIGHT);
        self.panel_bounds = Rect::new(
            (self.size.width - panel_width) / 2.0,
            (self.size.height - panel_height) / 2.0,
            panel_width,
            panel_height,
        );
        let content_height = (panel_height - VERTICAL_PADDING * 2.0).max(ROW_HEIGHT);
        self.visible_count = ((content_height / ROW_HEIGHT).floor() as usize).max(1);
        let max_first = self.applications.len().saturating_sub(self.visible_count);
        self.first_visible = self.first_visible.min(max_first);
        let end = (self.first_visible + self.visible_count).min(self.applications.len());
        self.rows = (self.first_visible..end)
            .enumerate()
            .map(|(slot, application)| ListRow {
                bounds: Rect::new(
                    self.panel_bounds.origin.x + HORIZONTAL_PADDING,
                    self.panel_bounds.origin.y + VERTICAL_PADDING + slot as f32 * ROW_HEIGHT,
                    (panel_width - HORIZONTAL_PADDING * 2.0).max(1.0),
                    ROW_HEIGHT,
                ),
                application,
            })
            .collect();
    }

    fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
}

impl Shell for Launcher {
    fn resize(&mut self, size: Size) {
        if self.size != size {
            self.size = size;
            self.layout();
            self.damage_all();
        }
    }

    fn update(&mut self) -> bool {
        false
    }

    fn activate_at(&mut self, position: (f64, f64)) -> bool {
        if let Some(row) = self.rows.iter().find(|row| row.bounds.contains(position)) {
            match self.applications[row.application].launch() {
                Ok(()) => self.close = true,
                Err(error) => {
                    self.error = Some(error);
                    self.damage_all();
                    return true;
                }
            }
            return false;
        }
        self.close = true;
        false
    }

    fn scroll_by(&mut self, delta_y: f64) -> bool {
        self.scroll_remainder += delta_y;
        let steps = (self.scroll_remainder / SCROLL_STEP).trunc() as isize;
        if steps == 0 {
            return false;
        }
        self.scroll_remainder -= steps as f64 * SCROLL_STEP;
        let max_first = self.applications.len().saturating_sub(self.visible_count);
        let next = self
            .first_visible
            .saturating_add_signed(steps)
            .min(max_first);
        if next == self.first_visible {
            return false;
        }
        self.first_visible = next;
        self.layout();
        self.damage_all();
        true
    }

    fn close_requested(&self) -> bool {
        self.close
    }

    fn commands(&self) -> Vec<DrawCommand> {
        let mut commands = vec![rounded(
            Rect::new(
                self.panel_bounds.origin.x + PANEL_INSET,
                self.panel_bounds.origin.y + PANEL_INSET,
                (self.panel_bounds.size.width - PANEL_INSET * 2.0).max(1.0),
                (self.panel_bounds.size.height - PANEL_INSET * 2.0).max(1.0),
            ),
            Color(20, 17, 29, 248),
            14.0,
        )];

        for row in &self.rows {
            let application = &self.applications[row.application];
            let icon_bounds = Rect::new(
                row.bounds.origin.x,
                row.bounds.origin.y + (ROW_HEIGHT - ICON_SIZE) / 2.0,
                ICON_SIZE,
                ICON_SIZE,
            );
            if let Some(icon) = &application.icon {
                commands.push(DrawCommand::Image {
                    bounds: icon_bounds,
                    width: icon.width,
                    height: icon.height,
                    rgba: icon.rgba.clone(),
                });
            } else {
                commands.push(rounded(icon_bounds, Color(124, 58, 237, 255), 5.0));
            }
            commands.push(text(
                Rect::new(
                    row.bounds.origin.x + ICON_SIZE + INNER_PADDING,
                    row.bounds.origin.y,
                    row.bounds.size.width - ICON_SIZE - INNER_PADDING,
                    ROW_HEIGHT,
                ),
                &application.name,
                14.0,
                TextAlign::Start,
                Color(245, 243, 255, 255),
            ));
        }

        if self.applications.is_empty() {
            commands.push(text(
                Rect::new(
                    self.panel_bounds.origin.x + HORIZONTAL_PADDING,
                    self.panel_bounds.origin.y + VERTICAL_PADDING,
                    self.panel_bounds.size.width - HORIZONTAL_PADDING * 2.0,
                    ROW_HEIGHT,
                ),
                "No applications found",
                16.0,
                TextAlign::Start,
                Color(175, 175, 180, 255),
            ));
        }
        if let Some(error) = &self.error {
            commands.push(text(
                Rect::new(
                    self.panel_bounds.origin.x + HORIZONTAL_PADDING,
                    self.panel_bounds.origin.y + self.panel_bounds.size.height - 42.0,
                    self.panel_bounds.size.width - HORIZONTAL_PADDING * 2.0,
                    34.0,
                ),
                error,
                12.0,
                TextAlign::Start,
                Color(245, 130, 150, 255),
            ));
        }
        commands
    }

    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }

    fn damage_all(&mut self) {
        Launcher::damage_all(self);
    }
}

fn rounded(bounds: Rect, color: Color, radius: f32) -> DrawCommand {
    DrawCommand::RoundedFill {
        bounds,
        color,
        radius,
    }
}

fn text(bounds: Rect, value: &str, font_size: f32, align: TextAlign, color: Color) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color,
        font_size,
        line_height: font_size * 1.25,
        family: FontFamily::SansSerif,
        weight: FontWeight::Normal,
        align,
    }
}

#[cfg(test)]
mod tests {
    use patin::{
        platform::Shell,
        ui::{Rect, Size},
    };

    use super::Launcher;
    use crate::apps::Application;

    fn launcher(count: usize) -> Launcher {
        Launcher::new(
            (0..count)
                .map(|index| Application::fixture(&format!("Application {index}")))
                .collect(),
        )
    }

    #[test]
    fn full_output_surface_centers_fixed_panel_and_rows() {
        let mut launcher = launcher(40);
        launcher.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        assert_eq!(launcher.panel_bounds, Rect::new(114.5, 335.0, 280.0, 350.0));
        assert_eq!(launcher.visible_count, 10);
        assert_eq!(launcher.rows.len(), 10);
        assert!(launcher.rows.iter().all(|row| {
            row.bounds.origin.y >= launcher.panel_bounds.origin.y
                && row.bounds.origin.y + row.bounds.size.height
                    <= launcher.panel_bounds.origin.y + launcher.panel_bounds.size.height
        }));
    }

    #[test]
    fn scrolling_moves_the_visible_window_and_clamps_at_each_end() {
        let mut launcher = launcher(20);
        launcher.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        assert!(launcher.scroll_by(60.0));
        assert_eq!(launcher.first_visible, 2);
        assert!(launcher.scroll_by(10_000.0));
        assert_eq!(launcher.first_visible, 10);
        assert!(!launcher.scroll_by(30.0));
        assert!(launcher.scroll_by(-10_000.0));
        assert_eq!(launcher.first_visible, 0);
    }

    #[test]
    fn tap_outside_panel_requests_composition_exit() {
        let mut launcher = launcher(1);
        launcher.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        launcher.activate_at((10.0, 10.0));
        assert!(launcher.close_requested());
    }
}
