use std::time::Duration;

use patin::{
    platform::Shell,
    service::Provider,
    ui::{Color, DrawCommand, Length, Rect, Size, row},
};
use patin_service_workspaces::{WorkspacesProvider, WorkspacesSnapshot};

pub const BAR_HEIGHT: f32 = 28.0;
const PILL_WIDTH: f32 = 32.0;
const PILL_HEIGHT: f32 = 10.0;
const GAP: f32 = 12.0;
const RADIUS: f32 = 5.0;

/// Workspace number shown at visual position `position` (0-indexed, left to
/// right) out of `count` total pills. Workspace 1 always lands in the middle
/// position, with the rest fanning out around it (wrapping), so the default
/// starting workspace reads as centered rather than pinned to the far left.
fn workspace_at(position: usize, count: usize) -> usize {
    let center = count / 2;
    ((position + count - center) % count) + 1
}

const FOCUSED: Color = Color(124, 58, 237, 255);
const OCCUPIED: Color = Color(148, 138, 165, 220);
const EMPTY: Color = Color(148, 138, 165, 90);

pub struct WorkspacesBarShell {
    provider: WorkspacesProvider,
    snapshot: Option<WorkspacesSnapshot>,
    size: Size,
    damage: Vec<Rect>,
}

impl WorkspacesBarShell {
    pub fn new() -> Self {
        let mut provider = WorkspacesProvider::new();
        let snapshot = provider.poll();
        Self {
            provider,
            snapshot,
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
        let content_width = count as f32 * PILL_WIDTH + (count - 1) as f32 * GAP;
        let bounds = Rect::new(
            (self.size.width - content_width) / 2.0,
            (self.size.height - PILL_HEIGHT) / 2.0,
            content_width,
            PILL_HEIGHT,
        );
        let lengths = vec![Length::Fixed(PILL_WIDTH); count];
        row(bounds, GAP, &lengths)
            .into_iter()
            .enumerate()
            .map(|(position, pill)| {
                let workspace = workspace_at(position, count);
                let color = if workspace == snapshot.focused {
                    FOCUSED
                } else if snapshot.occupied[workspace - 1] {
                    OCCUPIED
                } else {
                    EMPTY
                };
                DrawCommand::RoundedFill {
                    bounds: pill,
                    color,
                    radius: RADIUS,
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
        Duration::from_millis(200)
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
        let mut shell = WorkspacesBarShell::new();
        shell.snapshot = snapshot;
        shell.resize(Size {
            width: 400.0,
            height: BAR_HEIGHT,
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
            (0..9).map(|position| workspace_at(position, 9)).collect::<Vec<_>>(),
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
        let mut shell = WorkspacesBarShell::new();
        let size = Size {
            width: 400.0,
            height: BAR_HEIGHT,
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
}
