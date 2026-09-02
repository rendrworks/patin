//! Where each key sits: the per-mode geometry that turns a surface size
//! into a list of laid-out keys, and the fixed-point solve that reports
//! how tall a mode needs a standalone surface to be.

use patin::ui::Rect;

use super::{Key, KeyLayout, KeyboardMode, Page};

pub(crate) fn keyboard(
    mode: KeyboardMode,
    page: Page,
    shift: bool,
    width: f32,
    height: f32,
) -> Vec<KeyLayout> {
    match mode {
        KeyboardMode::Full => keyboard_full(page, shift, width, height),
        KeyboardMode::Numeric => keyboard_numeric(width, height),
        KeyboardMode::Extended => keyboard_extended(page, shift, width, height),
    }
}

/// The height a keyboard of the given mode occupies at the given width,
/// independent of any surrounding canvas. Lets a standalone surface (one
/// that *is* the keyboard, rather than embedding it above other content)
/// size itself exactly, with no dead space and no clipped keys.
pub fn footprint_height(mode: KeyboardMode, width: f32) -> f32 {
    // Row/key sizing and the bottom safe-area margin are themselves clamped
    // functions of height (so the keyboard stays usable on very short or
    // very tall screens), so "the height that exactly fits this keyboard"
    // is a fixed point, not something derivable from one arbitrary
    // reference height — the clamps can land on different values there
    // than they do at the real (much smaller) docked height, leaving dead
    // space above the keys and an undersized margin below them.
    //
    // `height - top` at a given height *is* this mode's natural footprint
    // if that height were the container (top = height - rows - margin, so
    // height - top = rows + margin) — as long as `height` is large enough
    // that `top` doesn't hit its own `.max(0.0)` safety clamp. Starting
    // from a height that's comfortably large and iterating downward keeps
    // `top` on its natural (positive) branch throughout, so this converges
    // to the fixed point in a handful of steps.
    // Measured against `visual_bounds`, not `hit_bounds`: the numeric
    // keypad's hit targets are padded outward by half a gap for touch
    // generosity, so using `hit_bounds` here would converge with the
    // *touch target* flush at y=0 while the actually-drawn key sits
    // several pixels lower — invisible slack that looks like dead space.
    let mut height = 4000.0_f32;
    for _ in 0..8 {
        let layout = keyboard(mode, Page::Letters, false, width, height);
        let top = layout
            .iter()
            .map(|key| key.visual_bounds.origin.y)
            .fold(f32::INFINITY, f32::min);
        let next = height - top;
        if (next - height).abs() < 0.01 {
            return next;
        }
        height = next;
    }
    height
}

fn keyboard_numeric(width: f32, height: f32) -> Vec<KeyLayout> {
    let gap = 14.0;
    let bottom_margin = bottom_margin(height);
    let key_size = ((width - 60.0 - gap * 2.0) / 3.0)
        .min((height * 0.51 - bottom_margin - gap * 3.0) / 4.0)
        .clamp(44.0, 72.0);
    let left = (width - (key_size * 3.0 + gap * 2.0)) / 2.0;
    let top = height - (key_size * 4.0 + gap * 3.0) - bottom_margin;
    let rows = [
        [
            ('1', Key::Character('1')),
            ('2', Key::Character('2')),
            ('3', Key::Character('3')),
        ],
        [
            ('4', Key::Character('4')),
            ('5', Key::Character('5')),
            ('6', Key::Character('6')),
        ],
        [
            ('7', Key::Character('7')),
            ('8', Key::Character('8')),
            ('9', Key::Character('9')),
        ],
        [
            ('⌫', Key::Backspace),
            ('0', Key::Character('0')),
            ('✓', Key::Enter),
        ],
    ];
    let mut keys = Vec::new();
    for (row, values) in rows.into_iter().enumerate() {
        for (column, (label, key)) in values.into_iter().enumerate() {
            let visual_bounds = Rect::new(
                left + column as f32 * (key_size + gap),
                top + row as f32 * (key_size + gap),
                key_size,
                key_size,
            );
            keys.push(KeyLayout {
                hit_bounds: Rect::new(
                    visual_bounds.origin.x - gap / 2.0,
                    visual_bounds.origin.y - gap / 2.0,
                    key_size + gap,
                    key_size + gap,
                ),
                visual_bounds,
                label: label.to_string(),
                key,
            });
        }
    }
    keys
}

const QWERTY_GAP: f32 = 5.0;

fn qwerty_row_height(height: f32) -> f32 {
    (height * 0.058).clamp(44.0, 52.0)
}

fn qwerty_bounds(width: f32) -> (f32, f32) {
    let keyboard_width = (width - 12.0).clamp(0.0, 720.0);
    let keyboard_left = (width - keyboard_width) / 2.0;
    (keyboard_width, keyboard_left)
}

fn keyboard_full(page: Page, shift: bool, width: f32, height: f32) -> Vec<KeyLayout> {
    let row_height = qwerty_row_height(height);
    let top = (height - (row_height * 4.0 + QWERTY_GAP * 3.0) - bottom_margin(height)).max(0.0);
    qwerty_and_bottom_rows(page, shift, width, top, row_height)
}

/// [`KeyboardMode::Full`]'s letter/symbol rows plus its bottom function row,
/// starting at `top`. Shared with [`keyboard_extended`], which stacks an
/// extra row of keys above this block instead of duplicating it.
fn qwerty_and_bottom_rows(
    page: Page,
    shift: bool,
    width: f32,
    top: f32,
    row_height: f32,
) -> Vec<KeyLayout> {
    let rows: &[&str] = match page {
        Page::Letters => &["qwertyuiop", "asdfghjkl", "zxcvbnm"],
        Page::Symbols => &["1234567890", "@#$%&*-+=", "!?_/:;()"],
    };
    let gap = QWERTY_GAP;
    let (keyboard_width, keyboard_left) = qwerty_bounds(width);
    let mut keys = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let characters: Vec<char> = row.chars().collect();
        let inset = match row_index {
            1 => keyboard_width * 0.04,
            2 => keyboard_width * 0.1,
            _ => 0.0,
        };
        let row_width = keyboard_width - inset * 2.0;
        let key_width = (row_width - gap * (characters.len() - 1) as f32) / characters.len() as f32;
        for (index, character) in characters.into_iter().enumerate() {
            let hit_bounds = Rect::new(
                keyboard_left + inset + index as f32 * (key_width + gap),
                top + row_index as f32 * (row_height + gap),
                key_width,
                row_height,
            );
            keys.push(KeyLayout {
                hit_bounds,
                visual_bounds: hit_bounds.inset(1.5),
                label: if shift {
                    character.to_ascii_uppercase()
                } else {
                    character
                }
                .to_string(),
                key: Key::Character(character),
            });
        }
    }
    let y = top + (row_height + gap) * 3.0;
    let labels = [
        (
            if page == Page::Letters { "⇧" } else { "ABC" },
            if page == Page::Letters {
                Key::Shift
            } else {
                Key::Symbols
            },
        ),
        ("?123", Key::Symbols),
        ("space", Key::Space),
        ("⌫", Key::Backspace),
        ("✓", Key::Enter),
    ];
    let mut x = keyboard_left;
    for ((label, key), fraction) in labels.into_iter().zip([0.18, 0.16, 0.32, 0.16, 0.18]) {
        let part_width = keyboard_width * fraction;
        let hit_bounds = Rect::new(x + gap / 2.0, y, part_width - gap, row_height);
        keys.push(KeyLayout {
            hit_bounds,
            visual_bounds: hit_bounds.inset(1.5),
            label: label.into(),
            key,
        });
        x += part_width;
    }
    keys
}

fn keyboard_extended(page: Page, shift: bool, width: f32, height: f32) -> Vec<KeyLayout> {
    let row_height = qwerty_row_height(height);
    let top = (height - (row_height * 5.0 + QWERTY_GAP * 4.0) - bottom_margin(height)).max(0.0);
    let mut keys = extra_keys_row(width, top, row_height);
    keys.extend(qwerty_and_bottom_rows(
        page,
        shift,
        width,
        top + row_height + QWERTY_GAP,
        row_height,
    ));
    keys
}

fn extra_keys_row(width: f32, top: f32, row_height: f32) -> Vec<KeyLayout> {
    let (keyboard_width, keyboard_left) = qwerty_bounds(width);
    let labels = [
        ("Esc", Key::Escape),
        ("Tab", Key::Tab),
        ("Ctrl", Key::Ctrl),
        ("Alt", Key::Alt),
        ("←", Key::ArrowLeft),
        ("↓", Key::ArrowDown),
        ("↑", Key::ArrowUp),
        ("→", Key::ArrowRight),
    ];
    let key_width = (keyboard_width - QWERTY_GAP * (labels.len() - 1) as f32) / labels.len() as f32;
    let mut keys = Vec::new();
    let mut x = keyboard_left;
    for (label, key) in labels {
        let hit_bounds = Rect::new(x, top, key_width, row_height);
        keys.push(KeyLayout {
            hit_bounds,
            visual_bounds: hit_bounds.inset(1.5),
            label: label.into(),
            key,
        });
        x += key_width + QWERTY_GAP;
    }
    keys
}

fn bottom_margin(height: f32) -> f32 {
    // The lower bound only ever binds for a compact, already-docked-at-the-
    // bottom-edge surface — there's no other content below to keep clear of
    // there, just a small touch-safety gap, not a full gesture-nav
    // reservation. A docked deployment (e.g. patin-osk under 0xin) sits
    // above patin-workspaces-bar's own reserved strip rather than the true
    // screen edge, so that strip already provides real separation and this
    // only needs to be a thin touch-safety pad.
    (height * 0.06).clamp(8.0, 40.0)
}

#[cfg(test)]
mod tests {
    use crate::{KeyboardMode, TouchKeyboard, footprint_height};
    use patin::ui::DrawCommand;

    #[test]
    fn layouts_stay_within_common_outputs() {
        for (width, height) in [
            (320.0, 500.0),
            (509.0, 1020.0),
            (1920.0, 1080.0),
            (400.0, 360.0),
        ] {
            for mode in [
                KeyboardMode::Numeric,
                KeyboardMode::Full,
                KeyboardMode::Extended,
            ] {
                let commands = TouchKeyboard::new(mode).commands(width, height, false);
                assert!(!commands.is_empty());
                assert!(commands.iter().all(|command| match command {
                    DrawCommand::RoundedFill { bounds, .. } | DrawCommand::Text { bounds, .. } =>
                        bounds.origin.x >= 0.0
                            && bounds.origin.y >= 0.0
                            && bounds.origin.x + bounds.size.width <= width
                            && bounds.origin.y + bounds.size.height <= height,
                    _ => true,
                }));
            }
        }
    }

    #[test]
    fn footprint_height_fits_a_standalone_surface_exactly() {
        for width in [320.0, 400.0, 509.0, 1080.0] {
            for mode in [
                KeyboardMode::Numeric,
                KeyboardMode::Full,
                KeyboardMode::Extended,
            ] {
                let height = footprint_height(mode, width);
                let commands = TouchKeyboard::new(mode).commands(width, height, false);
                assert!(!commands.is_empty());
                assert!(commands.iter().all(|command| match command {
                    DrawCommand::RoundedFill { bounds, .. } | DrawCommand::Text { bounds, .. } =>
                        bounds.origin.x >= 0.0
                            && bounds.origin.y >= 0.0
                            && bounds.origin.x + bounds.size.width <= width
                            && bounds.origin.y + bounds.size.height <= height,
                    _ => true,
                }));

                // No dead space above the first row: the clamps that decide
                // row/key height and the bottom margin must be evaluated at
                // this *real* footprint height, not at some other height,
                // or the keys land well below y=0 with room to spare.
                let top = commands
                    .iter()
                    .filter_map(|command| match command {
                        DrawCommand::RoundedFill { bounds, .. } => Some(bounds.origin.y),
                        _ => None,
                    })
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    top < 5.0,
                    "{mode:?} at width {width} has {top}px of dead space above its keys \
                     (footprint_height returned {height})"
                );
            }
        }
    }
}
