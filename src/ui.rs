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

pub fn column(bounds: Rect, gap: f32, lengths: &[Length]) -> Vec<Rect> {
    linear_layout(bounds, gap, lengths, true)
}

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
    let gap = gap.max(0.0);
    let available = (main_size - gap * lengths.len().saturating_sub(1) as f32).max(0.0);
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
            cursor += main + gap;
            rect
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAlign {
    Start,
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
    RoundedFill {
        bounds: Rect,
        color: Color,
        radius: f32,
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

#[cfg(test)]
mod tests {
    use super::{Length, Rect, column, row, stack};

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
}
