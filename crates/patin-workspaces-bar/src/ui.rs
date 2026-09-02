use std::time::Duration;

use patin::{
    platform::Shell,
    service::Provider,
    ui::{Color, DrawCommand, Length, Rect, Size, row},
};
use patin_lua::Config;
use patin_service_workspaces::{WorkspacesProvider, WorkspacesSnapshot};

const PILL_WIDTH: f32 = 32.0;
const PILL_HEIGHT: f32 = 10.0;
const GAP: f32 = 12.0;
const RADIUS: f32 = 5.0;
/// Breathing room above and below the pills; the strip's height, and so its
/// exclusive zone, is the pill plus this unless a config names a height.
const PADDING: f32 = 4.0;
const POLL_INTERVAL: Duration = Duration::from_millis(200);

const FOCUSED: Color = Color(124, 58, 237, 255);
const OCCUPIED: Color = Color(148, 138, 165, 220);
const EMPTY: Color = Color(148, 138, 165, 90);

/// Everything about the strip a config may move.
///
/// Defaults are the values the bar shipped with, so an absent `init.lua` and
/// an empty one produce exactly the frame this crate always drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub height: f32,
    pub pill_width: f32,
    pub pill_height: f32,
    pub gap: f32,
    pub radius: f32,
    pub focused: Color,
    pub occupied: Color,
    pub empty: Color,
    pub poll_interval: Duration,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            height: PILL_HEIGHT + PADDING,
            pill_width: PILL_WIDTH,
            pill_height: PILL_HEIGHT,
            gap: GAP,
            radius: RADIUS,
            focused: FOCUSED,
            occupied: OCCUPIED,
            empty: EMPTY,
            poll_interval: POLL_INTERVAL,
        }
    }
}

/// Namespaces this composition is responsible for, and the settings it knows.
/// Anything else written under them is reported as a probable typo — the one
/// real cost of assigning settings straight onto the module table.
const OWNED: &[&str] = &["bar"];
const KNOWN: &[&str] = &[
    "bar.height",
    "bar.pill.width",
    "bar.pill.height",
    "bar.pill.gap",
    "bar.pill.radius",
    "bar.focused",
    "bar.occupied",
    "bar.empty",
    "bar.poll_interval",
];

impl Style {
    pub fn from_config(config: &Config) -> Self {
        config.warn_unknown(OWNED, KNOWN);
        let mut style = Style::default();
        if let Some(value) = config.number(&["bar.pill.width"]) {
            style.pill_width = value.max(1.0);
        }
        if let Some(value) = config.number(&["bar.pill.height"]) {
            style.pill_height = value.max(1.0);
            style.height = style.pill_height + PADDING;
        }
        if let Some(value) = config.number(&["bar.height"]) {
            style.height = value.max(style.pill_height);
        }
        if let Some(value) = config.number(&["bar.pill.gap"]) {
            style.gap = value.max(0.0);
        }
        if let Some(value) = config.number(&["bar.pill.radius"]) {
            style.radius = value.max(0.0);
        }
        if let Some(color) = config.color(&["bar.focused", "theme.accent"]) {
            style.focused = color;
        }
        // The shared muted colour keeps the strip's own two opacities: an
        // occupied workspace and an empty one are the same hue at different
        // strengths, and a config that names one grey means both.
        if let Some(color) = config.color(&["bar.occupied", "theme.muted"]) {
            style.occupied =
                with_alpha(color, OCCUPIED.3, config.color(&["bar.occupied"]).is_some());
        }
        if let Some(color) = config.color(&["bar.empty", "theme.muted"]) {
            style.empty = with_alpha(color, EMPTY.3, config.color(&["bar.empty"]).is_some());
        }
        if let Some(value) = config.seconds(&["bar.poll_interval"]) {
            style.poll_interval = value.max(Duration::from_millis(16));
        }
        style
    }
}

/// A colour named for this bar is taken as written; one inherited from the
/// shared theme is faded to the strength that slot has always had.
fn with_alpha(color: Color, alpha: u8, explicit: bool) -> Color {
    if explicit {
        color
    } else {
        Color(color.0, color.1, color.2, alpha)
    }
}

/// Workspace number shown at visual position `position` (0-indexed, left to
/// right) out of `count` total pills. Workspace 1 always lands in the middle
/// position, with the rest fanning out around it (wrapping), so the default
/// starting workspace reads as centered rather than pinned to the far left.
fn workspace_at(position: usize, count: usize) -> usize {
    let center = count / 2;
    ((position + count - center) % count) + 1
}

pub struct WorkspacesBarShell {
    provider: WorkspacesProvider,
    snapshot: Option<WorkspacesSnapshot>,
    style: Style,
    size: Size,
    damage: Vec<Rect>,
}

impl WorkspacesBarShell {
    pub fn new(style: Style) -> Self {
        let mut provider = WorkspacesProvider::new();
        let snapshot = provider.poll();
        Self {
            provider,
            snapshot,
            style,
            size: Size::default(),
            damage: Vec::new(),
        }
    }

    fn damage_all(&mut self) {
        self.damage = vec![Rect::new(0.0, 0.0, self.size.width, self.size.height)];
    }

    fn pills(&self) -> Vec<DrawCommand> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let count = snapshot.occupied.len();
        if count == 0 {
            return Vec::new();
        }
        let style = self.style;
        let content_width = count as f32 * style.pill_width + (count - 1) as f32 * style.gap;
        let bounds = Rect::new(
            (self.size.width - content_width) / 2.0,
            (self.size.height - style.pill_height) / 2.0,
            content_width,
            style.pill_height,
        );
        let lengths = vec![Length::Fixed(style.pill_width); count];
        row(bounds, style.gap, &lengths)
            .into_iter()
            .enumerate()
            .map(|(position, pill)| {
                let workspace = workspace_at(position, count);
                let color = if workspace == snapshot.focused {
                    style.focused
                } else if snapshot.occupied[workspace - 1] {
                    style.occupied
                } else {
                    style.empty
                };
                DrawCommand::RoundedFill {
                    bounds: pill,
                    color,
                    radius: style.radius,
                }
            })
            .collect()
    }
}

impl Shell for WorkspacesBarShell {
    fn resize(&mut self, size: Size) {
        if self.size != size {
            self.size = size;
            self.damage_all();
        }
    }

    fn poll_interval(&self) -> Duration {
        self.style.poll_interval
    }

    fn update(&mut self) -> bool {
        let next = self.provider.poll();
        if next != self.snapshot {
            self.snapshot = next;
            self.damage_all();
            true
        } else {
            false
        }
    }

    fn activate_at(&mut self, _position: (f64, f64)) -> bool {
        false
    }

    fn commands(&self) -> Vec<DrawCommand> {
        self.pills()
    }

    fn take_damage(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.damage)
    }

    fn damage_all(&mut self) {
        WorkspacesBarShell::damage_all(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_with(snapshot: Option<WorkspacesSnapshot>) -> WorkspacesBarShell {
        let style = Style::default();
        let mut shell = WorkspacesBarShell::new(style);
        shell.snapshot = snapshot;
        shell.resize(Size {
            width: 400.0,
            height: style.height,
        });
        shell
    }

    #[test]
    fn no_snapshot_draws_nothing() {
        let shell = shell_with(None);
        assert!(shell.commands().is_empty());
    }

    #[test]
    fn one_pill_per_workspace_colored_by_state() {
        // workspace_at(position, 3) => [3, 1, 2] left to right (workspace 1
        // centered). Workspace 3 is empty, 1 is occupied, 2 is focused.
        let shell = shell_with(Some(WorkspacesSnapshot {
            focused: 2,
            occupied: vec![true, true, false],
        }));
        let commands = shell.commands();
        assert_eq!(commands.len(), 3);
        let colors: Vec<Color> = commands
            .iter()
            .map(|command| match command {
                DrawCommand::RoundedFill { color, .. } => *color,
                other => panic!("expected RoundedFill, got {other:?}"),
            })
            .collect();
        assert_eq!(colors, vec![EMPTY, OCCUPIED, FOCUSED]);
    }

    #[test]
    fn workspace_one_is_always_centered() {
        assert_eq!(workspace_at(4, 9), 1);
        assert_eq!(
            (0..9)
                .map(|position| workspace_at(position, 9))
                .collect::<Vec<_>>(),
            vec![6, 7, 8, 9, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn pills_are_centered_within_the_surface() {
        let shell = shell_with(Some(WorkspacesSnapshot {
            focused: 1,
            occupied: vec![true, false],
        }));
        let commands = shell.commands();
        let first = match &commands[0] {
            DrawCommand::RoundedFill { bounds, .. } => *bounds,
            _ => unreachable!(),
        };
        let last = match &commands[commands.len() - 1] {
            DrawCommand::RoundedFill { bounds, .. } => *bounds,
            _ => unreachable!(),
        };
        let left_margin = first.origin.x;
        let right_margin = shell.size.width - (last.origin.x + last.size.width);
        assert!((left_margin - right_margin).abs() < 0.01);
    }

    #[test]
    fn resize_damages_the_whole_surface_once() {
        let mut shell = WorkspacesBarShell::new(Style::default());
        let size = Size {
            width: 400.0,
            height: Style::default().height,
        };
        shell.resize(size);
        assert_eq!(
            shell.take_damage(),
            vec![Rect::new(0.0, 0.0, size.width, size.height)]
        );
        assert_eq!(shell.take_damage(), Vec::new());

        shell.resize(size);
        assert_eq!(
            shell.take_damage(),
            Vec::new(),
            "unchanged size stays undamaged"
        );
    }

    #[test]
    fn tapping_never_activates_anything() {
        let mut shell = shell_with(Some(WorkspacesSnapshot {
            focused: 1,
            occupied: vec![true],
        }));
        assert!(!shell.activate_at((10.0, 10.0)));
    }

    #[test]
    fn an_empty_config_reproduces_the_bar_this_crate_shipped_with() {
        assert_eq!(Style::from_config(&Config::empty()), Style::default());
    }
}
