use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, Rect, Size, TextAlign},
};

use crate::apps::Application;

const PANEL_INSET: f32 = 2.0;
const LIST_PADDING: f32 = 10.0;
const ROW_HEIGHT: f32 = 44.0;
const ICON_SIZE: f32 = 28.0;
const SCROLL_STEP: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ListRow {
    bounds: Rect,
    application: usize,
}

pub struct Launcher {
    size: Size,
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
        let content_height = (self.size.height - LIST_PADDING * 2.0).max(ROW_HEIGHT);
        self.visible_count = ((content_height / ROW_HEIGHT).floor() as usize).max(1);
        let max_first = self.applications.len().saturating_sub(self.visible_count);
        self.first_visible = self.first_visible.min(max_first);
        let end = (self.first_visible + self.visible_count).min(self.applications.len());
        self.rows = (self.first_visible..end)
            .enumerate()
            .map(|(slot, application)| ListRow {
                bounds: Rect::new(
                    LIST_PADDING,
                    LIST_PADDING + slot as f32 * ROW_HEIGHT,
                    (self.size.width - LIST_PADDING * 2.0).max(1.0),
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
                PANEL_INSET,
                PANEL_INSET,
                (self.size.width - PANEL_INSET * 2.0).max(1.0),
                (self.size.height - PANEL_INSET * 2.0).max(1.0),
            ),
            Color(27, 27, 30, 250),
            12.0,
        )];

        for row in &self.rows {
            let application = &self.applications[row.application];
            let icon_bounds = Rect::new(
                row.bounds.origin.x + 6.0,
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
                commands.push(rounded(icon_bounds, Color(72, 72, 78, 255), 7.0));
            }
            commands.push(text(
                Rect::new(
                    row.bounds.origin.x + 44.0,
                    row.bounds.origin.y,
                    row.bounds.size.width - 54.0,
                    ROW_HEIGHT,
                ),
                &application.name,
                15.0,
                TextAlign::Start,
                Color(238, 238, 241, 255),
            ));
        }

        if self.applications.is_empty() {
            commands.push(text(
                Rect::new(
                    LIST_PADDING,
                    LIST_PADDING,
                    self.size.width - 24.0,
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
                    LIST_PADDING,
                    self.size.height - 42.0,
                    self.size.width - 32.0,
                    34.0,
                ),
                error,
                12.0,
                TextAlign::Start,
                Color(245, 130, 150, 255),
            ));
        }
        if self.applications.len() > self.visible_count {
            let track_height = self.size.height - LIST_PADDING * 2.0;
            let thumb_height = (track_height * self.visible_count as f32
                / self.applications.len() as f32)
                .max(24.0);
            let max_first = self.applications.len() - self.visible_count;
            let thumb_y = LIST_PADDING
                + (track_height - thumb_height) * self.first_visible as f32 / max_first as f32;
            commands.push(rounded(
                Rect::new(self.size.width - 6.0, thumb_y, 3.0, thumb_height),
                Color(105, 105, 112, 220),
                1.5,
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
        align,
    }
}

#[cfg(test)]
mod tests {
    use patin::{platform::Shell, ui::Size};

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
    fn fixed_window_lists_only_rows_that_fit() {
        let mut launcher = launcher(40);
        launcher.resize(Size {
            width: 380.0,
            height: 540.0,
        });
        assert_eq!(launcher.visible_count, 11);
        assert_eq!(launcher.rows.len(), 11);
        assert!(launcher.rows.iter().all(|row| {
            row.bounds.origin.y >= 0.0 && row.bounds.origin.y + row.bounds.size.height <= 540.0
        }));
    }

    #[test]
    fn scrolling_moves_the_visible_window_and_clamps_at_each_end() {
        let mut launcher = launcher(20);
        launcher.resize(Size {
            width: 380.0,
            height: 300.0,
        });
        assert!(launcher.scroll_by(60.0));
        assert_eq!(launcher.first_visible, 2);
        assert!(launcher.scroll_by(10_000.0));
        assert_eq!(launcher.first_visible, 14);
        assert!(!launcher.scroll_by(30.0));
        assert!(launcher.scroll_by(-10_000.0));
        assert_eq!(launcher.first_visible, 0);
    }

    #[test]
    fn empty_space_requests_composition_exit() {
        let mut launcher = launcher(1);
        launcher.resize(Size {
            width: 380.0,
            height: 540.0,
        });
        launcher.activate_at((20.0, 100.0));
        assert!(launcher.close_requested());
    }
}
