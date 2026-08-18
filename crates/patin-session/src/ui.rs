use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, Size, TextAlign},
};

use patin_icons::{IconPalette, logout, power, reboot};

use crate::actions::{Action, ActionKind};

const PANEL_WIDTH: f32 = 240.0;
const PANEL_INSET: f32 = 2.0;
const HORIZONTAL_PADDING: f32 = 18.0;
const VERTICAL_PADDING: f32 = 18.0;
const ROW_HEIGHT: f32 = 36.0;
/// Matches the launcher's row icons, so the two menus feel like one system.
const ICON_SIZE: f32 = 18.0;
const ICON_GAP: f32 = 12.0;
const ERROR_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActionRow {
    bounds: Rect,
    action: usize,
}

pub struct SessionMenu {
    size: Size,
    actions: Vec<Action>,
    panel_bounds: Rect,
    rows: Vec<ActionRow>,
    close: bool,
    error: Option<String>,
    damage: Vec<Rect>,
}

impl SessionMenu {
    pub fn new(actions: Vec<Action>) -> Self {
        Self {
            size: Size::default(),
            actions,
            panel_bounds: Rect::default(),
            rows: Vec::new(),
            close: false,
            error: None,
            damage: Vec::new(),
        }
    }

    fn layout(&mut self) {
        let panel_width = self.size.width.clamp(1.0, PANEL_WIDTH);
        let error_height = self.error.as_ref().map_or(0.0, |_| ERROR_HEIGHT);
        let wanted_height =
            VERTICAL_PADDING * 2.0 + ROW_HEIGHT * self.actions.len() as f32 + error_height;
        let panel_height = self.size.height.clamp(1.0, wanted_height);
        self.panel_bounds = Rect::new(
            (self.size.width - panel_width) / 2.0,
            (self.size.height - panel_height) / 2.0,
            panel_width,
            panel_height,
        );
        self.rows = self
            .actions
            .iter()
            .enumerate()
            .map(|(action, _)| ActionRow {
                bounds: Rect::new(
                    self.panel_bounds.origin.x + HORIZONTAL_PADDING,
                    self.panel_bounds.origin.y + VERTICAL_PADDING + action as f32 * ROW_HEIGHT,
                    (panel_width - HORIZONTAL_PADDING * 2.0).max(1.0),
                    ROW_HEIGHT,
                ),
                action,
            })
            .collect();
    }

    fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }
}

impl Shell for SessionMenu {
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
            match self.actions[row.action].launch() {
                Ok(()) => self.close = true,
                Err(error) => {
                    eprintln!("patin-session: {error}");
                    self.error = Some(error);
                    self.layout();
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
        let mut commands = vec![rounded(
            self.panel_bounds.inset(PANEL_INSET),
            Color(20, 17, 29, 248),
            14.0,
        )];
        for row in &self.rows {
            let action = &self.actions[row.action];
            let icon_bounds = Rect::new(
                row.bounds.origin.x,
                row.bounds.origin.y + (ROW_HEIGHT - ICON_SIZE) / 2.0,
                ICON_SIZE,
                ICON_SIZE,
            );
            commands.extend(match action.kind {
                ActionKind::LogOut => logout(icon_bounds, icon_palette()),
                ActionKind::Reboot => reboot(icon_bounds, icon_palette()),
                ActionKind::ShutDown => power(icon_bounds, icon_palette()),
            });
            commands.push(text(
                Rect::new(
                    row.bounds.origin.x + ICON_SIZE + ICON_GAP,
                    row.bounds.origin.y,
                    (row.bounds.size.width - ICON_SIZE - ICON_GAP).max(1.0),
                    row.bounds.size.height,
                ),
                &action.label,
                14.0,
                Color(245, 243, 255, 255),
            ));
        }
        if let Some(error) = &self.error {
            commands.push(text(
                Rect::new(
                    self.panel_bounds.origin.x + HORIZONTAL_PADDING,
                    self.panel_bounds.origin.y + self.panel_bounds.size.height - ERROR_HEIGHT,
                    self.panel_bounds.size.width - HORIZONTAL_PADDING * 2.0,
                    ERROR_HEIGHT,
                ),
                error,
                11.0,
                Color(245, 130, 150, 255),
            ));
        }
        commands
    }

    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }

    fn damage_all(&mut self) {
        SessionMenu::damage_all(self);
    }
}

/// The icons are drawn against the panel, so their "background" — the colour
/// they punch holes with — has to be the panel's fill, not the surface's.
fn icon_palette() -> IconPalette {
    IconPalette {
        foreground: Color(245, 243, 255, 255),
        muted: Color(120, 110, 140, 255),
        background: Color(20, 17, 29, 255),
        accent: Color(124, 58, 237, 255),
        unavailable: Color(245, 130, 150, 255),
    }
}

fn rounded(bounds: Rect, color: Color, radius: f32) -> DrawCommand {
    DrawCommand::RoundedFill {
        bounds,
        color,
        radius,
    }
}

fn text(bounds: Rect, value: &str, font_size: f32, color: Color) -> DrawCommand {
    DrawCommand::Text {
        bounds,
        text: value.into(),
        color,
        font_size,
        line_height: font_size * 1.25,
        family: FontFamily::SansSerif,
        weight: FontWeight::Normal,
        align: TextAlign::Start,
    }
}

#[cfg(test)]
mod tests {
    use patin::{
        platform::Shell,
        ui::{Rect, Size},
    };

    use super::SessionMenu;
    use crate::actions::Action;

    fn menu() -> SessionMenu {
        SessionMenu::new(vec![
            Action::fixture("Log out"),
            Action::fixture("Reboot"),
            Action::fixture("Shut down"),
        ])
    }

    #[test]
    fn centers_three_rows_in_a_compact_panel() {
        let mut menu = menu();
        menu.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        assert_eq!(menu.panel_bounds, Rect::new(134.5, 438.0, 240.0, 144.0));
        assert_eq!(menu.rows.len(), 3);
        assert!(menu.rows.iter().all(|row| {
            menu.panel_bounds
                .contains((row.bounds.origin.x as f64, row.bounds.origin.y as f64))
        }));
    }

    #[test]
    fn tap_outside_panel_requests_exit_without_an_action() {
        let mut menu = menu();
        menu.resize(Size {
            width: 509.0,
            height: 1020.0,
        });
        menu.activate_at((10.0, 10.0));
        assert!(menu.close_requested());
    }
}
