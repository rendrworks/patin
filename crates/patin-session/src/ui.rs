use patin::{
    platform::Shell,
    ui::{Color, DrawCommand, FontFamily, FontWeight, Rect, Size, TextAlign},
};

use patin_icons::{IconPalette, logout, power, reboot};

use patin_lua::Config;

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

const PANEL: Color = Color(20, 17, 29, 248);
const LABEL: Color = Color(245, 243, 255, 255);
const MUTED: Color = Color(120, 110, 140, 255);
const ACCENT: Color = Color(124, 58, 237, 255);
const ERROR: Color = Color(245, 130, 150, 255);

/// The menu's four colours, defaulting to the ones it shipped with.
///
/// `session.*` names one for this menu alone; `theme.*` names it for every
/// composition at once, and the specific key wins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub panel: Color,
    pub label: Color,
    pub accent: Color,
    pub error: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            panel: PANEL,
            label: LABEL,
            accent: ACCENT,
            error: ERROR,
        }
    }
}

impl Palette {
    pub fn from_config(config: &Config) -> Self {
        let mut palette = Palette::default();
        if let Some(color) = config.color(&["session.panel", "theme.background"]) {
            palette.panel = color;
        }
        if let Some(color) = config.color(&["session.label", "theme.foreground"]) {
            palette.label = color;
        }
        if let Some(color) = config.color(&["theme.accent"]) {
            palette.accent = color;
        }
        if let Some(color) = config.color(&["session.error", "theme.error"]) {
            palette.error = color;
        }
        palette
    }

    /// The icons are drawn against the panel, so their "background" — the
    /// colour they punch holes with — has to be the panel's fill, not the
    /// surface's, and opaque: a hole punched with a translucent colour shows
    /// whatever is behind the menu.
    fn icons(&self) -> IconPalette {
        IconPalette {
            foreground: self.label,
            muted: MUTED,
            background: Color(self.panel.0, self.panel.1, self.panel.2, 255),
            accent: self.accent,
            unavailable: self.error,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActionRow {
    bounds: Rect,
    action: usize,
}

pub struct SessionMenu {
    size: Size,
    actions: Vec<Action>,
    palette: Palette,
    panel_bounds: Rect,
    rows: Vec<ActionRow>,
    close: bool,
    error: Option<String>,
    damage: Vec<Rect>,
}

impl SessionMenu {
    pub fn new(actions: Vec<Action>, palette: Palette) -> Self {
        Self {
            size: Size::default(),
            actions,
            palette,
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
            self.palette.panel,
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
            let icons = self.palette.icons();
            commands.extend(match action.kind {
                ActionKind::LogOut => logout(icon_bounds, icons),
                ActionKind::Reboot => reboot(icon_bounds, icons),
                ActionKind::ShutDown => power(icon_bounds, icons),
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
                self.palette.label,
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
                self.palette.error,
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

    use super::{Color, Palette, SessionMenu};
    use crate::actions::Action;
    use patin_lua::Config;

    fn menu() -> SessionMenu {
        SessionMenu::new(
            vec![
                Action::fixture("Log out"),
                Action::fixture("Reboot"),
                Action::fixture("Shut down"),
            ],
            Palette::default(),
        )
    }

    #[test]
    fn an_empty_config_reproduces_the_menu_this_crate_shipped_with() {
        assert_eq!(Palette::from_config(&Config::empty()), Palette::default());
    }

    #[test]
    fn a_shared_theme_colour_reaches_the_menu_unless_it_names_its_own() {
        let config = Config::from_source(
            "init.lua",
            r##"
            patin.theme.background = "#101018"
            patin.theme.foreground = "#eeeeff"
            patin.session.label = "#00ff00"
            "##,
        )
        .unwrap();
        let palette = Palette::from_config(&config);
        assert_eq!(palette.panel, Color(16, 16, 24, 255));
        assert_eq!(palette.label, Color(0, 255, 0, 255));
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
