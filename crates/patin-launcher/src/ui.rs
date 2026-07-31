use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, Rect, Size, TextAlign},
};

use crate::apps::Application;

const OUTER_MARGIN: f32 = 12.0;
const LIST_MAX_WIDTH: f32 = 640.0;
const FOOTER_HEIGHT: f32 = 32.0;
const ROW_HEIGHT: f32 = 38.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ListRow {
    bounds: Rect,
    application: usize,
}

pub struct Launcher {
    size: Size,
    applications: Vec<Application>,
    page: usize,
    page_size: usize,
    page_count: usize,
    list_bounds: Rect,
    rows: Vec<ListRow>,
    previous_bounds: Option<Rect>,
    next_bounds: Option<Rect>,
    close: bool,
    error: Option<String>,
    damage: Vec<Rect>,
}

impl Launcher {
    pub fn new(applications: Vec<Application>) -> Self {
        Self {
            size: Size::default(),
            applications,
            page: 0,
            page_size: 1,
            page_count: 1,
            list_bounds: Rect::default(),
            rows: Vec::new(),
            previous_bounds: None,
            next_bounds: None,
            close: false,
            error: None,
            damage: Vec::new(),
        }
    }

    fn layout(&mut self) {
        let list_width = (self.size.width - OUTER_MARGIN * 2.0).clamp(1.0, LIST_MAX_WIDTH);
        let list_x = (self.size.width - list_width) / 2.0;
        self.list_bounds = Rect::new(
            list_x,
            OUTER_MARGIN,
            list_width,
            (self.size.height - OUTER_MARGIN * 2.0).max(1.0),
        );

        let content_height = (self.list_bounds.size.height - FOOTER_HEIGHT).max(ROW_HEIGHT);
        self.page_size = ((content_height / ROW_HEIGHT).floor() as usize).max(1);
        self.page_count = self.applications.len().div_ceil(self.page_size).max(1);
        self.page = self.page.min(self.page_count - 1);

        let start = self.page * self.page_size;
        let end = (start + self.page_size).min(self.applications.len());
        self.rows = (start..end)
            .enumerate()
            .map(|(slot, application)| ListRow {
                bounds: Rect::new(
                    list_x,
                    OUTER_MARGIN + slot as f32 * ROW_HEIGHT,
                    list_width,
                    ROW_HEIGHT,
                ),
                application,
            })
            .collect();

        if self.page_count > 1 {
            let y = OUTER_MARGIN + self.list_bounds.size.height - FOOTER_HEIGHT;
            self.previous_bounds =
                (self.page > 0).then(|| Rect::new(list_x, y, list_width / 2.0, FOOTER_HEIGHT));
            self.next_bounds = (self.page + 1 < self.page_count).then(|| {
                Rect::new(
                    list_x + list_width / 2.0,
                    y,
                    list_width / 2.0,
                    FOOTER_HEIGHT,
                )
            });
        } else {
            self.previous_bounds = None;
            self.next_bounds = None;
        }
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
        if self
            .previous_bounds
            .is_some_and(|bounds| bounds.contains(position))
        {
            self.page = self.page.saturating_sub(1);
            self.layout();
            self.damage_all();
            return true;
        }
        if self
            .next_bounds
            .is_some_and(|bounds| bounds.contains(position))
        {
            self.page += 1;
            self.layout();
            self.damage_all();
            return true;
        }
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

    fn close_requested(&self) -> bool {
        self.close
    }

    fn commands(&self) -> Vec<DrawCommand> {
        let mut commands = vec![fill(
            Rect::new(0.0, 0.0, self.size.width, self.size.height),
            Color(12, 12, 12, 248),
        )];

        for row in &self.rows {
            let application = &self.applications[row.application];
            commands.push(text(
                Rect::new(
                    row.bounds.origin.x + 6.0,
                    row.bounds.origin.y,
                    row.bounds.size.width - 12.0,
                    row.bounds.size.height,
                ),
                &application.name,
                16.0,
                TextAlign::Start,
                Color(224, 224, 224, 255),
            ));
        }

        if self.applications.is_empty() {
            commands.push(text(
                Rect::new(
                    self.list_bounds.origin.x,
                    self.size.height * 0.42,
                    self.list_bounds.size.width,
                    60.0,
                ),
                "No launchable applications found",
                17.0,
                TextAlign::Center,
                Color(183, 172, 198, 255),
            ));
        }
        if let Some(error) = &self.error {
            commands.push(text(
                Rect::new(
                    self.list_bounds.origin.x + 8.0,
                    self.list_bounds.origin.y,
                    self.list_bounds.size.width - 16.0,
                    ROW_HEIGHT,
                ),
                error,
                12.0,
                TextAlign::Start,
                Color(239, 140, 171, 255),
            ));
        }
        if self.page_count > 1 {
            commands.push(text(
                Rect::new(
                    self.size.width / 2.0 - 50.0,
                    self.list_bounds.origin.y + self.list_bounds.size.height - FOOTER_HEIGHT,
                    100.0,
                    40.0,
                ),
                &format!("‹    {} / {}    ›", self.page + 1, self.page_count),
                13.0,
                TextAlign::Center,
                Color(170, 170, 170, 255),
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

fn fill(bounds: Rect, color: Color) -> DrawCommand {
    DrawCommand::Fill { bounds, color }
}

fn text(bounds: Rect, value: &str, font_size: f32, align: TextAlign, color: Color) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color,
        font_size,
        line_height: font_size * 1.25,
        family: FontFamily::Monospace,
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
    fn phone_layout_pages_apps_and_keeps_rows_inside_output() {
        let mut launcher = launcher(40);
        launcher.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        assert!(launcher.page_count > 1);
        assert!(launcher.rows.iter().all(|row| {
            row.bounds.origin.x >= 0.0
                && row.bounds.origin.y >= 0.0
                && row.bounds.origin.x + row.bounds.size.width <= 509.0
                && row.bounds.origin.y + row.bounds.size.height <= 1020.0
        }));
    }

    #[test]
    fn desktop_layout_keeps_the_list_narrow_and_centered() {
        let mut launcher = launcher(5);
        launcher.resize(Size {
            width: 1920.0,
            height: 1080.0,
        });
        assert_eq!(launcher.list_bounds.size.width, 640.0);
        assert_eq!(launcher.list_bounds.origin.x, 640.0);
        assert!(
            launcher
                .rows
                .iter()
                .all(|row| row.bounds.size.width == 640.0)
        );
    }

    #[test]
    fn page_controls_and_empty_space_close_without_launching() {
        let mut launcher = launcher(40);
        launcher.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        let next = launcher.next_bounds.unwrap();
        launcher.activate_at((next.origin.x as f64 + 1.0, next.origin.y as f64 + 1.0));
        assert_eq!(launcher.page, 1);
        assert!(!launcher.close_requested());

        launcher.activate_at((1.0, 500.0));
        assert!(launcher.close_requested());
    }

    #[test]
    fn empty_space_requests_composition_exit() {
        let mut launcher = launcher(1);
        launcher.resize(Size {
            width: 1280.0,
            height: 720.0,
        });
        let Rect { origin, size } = launcher.rows[0].bounds;
        launcher.activate_at((
            origin.x as f64 + 1.0,
            origin.y as f64 + size.height as f64 + 1.0,
        ));
        assert!(launcher.close_requested());
    }
}
