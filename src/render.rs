use cosmic_text::{
    Align, Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight,
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect as SkiaRect, Transform};

use crate::ui::{DrawCommand, FontFamily, Rect, TextAlign};

pub const SCALE_DENOMINATOR: u32 = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scale(u32);

impl Scale {
    pub const ONE: Self = Self(SCALE_DENOMINATOR);

    pub fn from_120ths(value: u32) -> Self {
        Self(value.max(1))
    }

    pub fn from_integer(value: i32) -> Self {
        let value = u32::try_from(value).unwrap_or(1).max(1);
        Self(value.saturating_mul(SCALE_DENOMINATOR))
    }

    pub fn physical(self, logical: u32) -> u32 {
        let physical = u64::from(logical)
            .saturating_mul(u64::from(self.0))
            .div_ceil(u64::from(SCALE_DENOMINATOR));
        u32::try_from(physical).unwrap_or(u32::MAX)
    }

    pub fn factor(self) -> f32 {
        self.0 as f32 / SCALE_DENOMINATOR as f32
    }
}

impl Default for Scale {
    fn default() -> Self {
        Self::ONE
    }
}

pub struct CpuRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Default for CpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuRenderer {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub fn render_bar(
        &mut self,
        canvas: &mut [u8],
        width: u32,
        height: u32,
        scale: Scale,
        commands: &[DrawCommand],
    ) -> Result<(), &'static str> {
        let mut pixmap = Pixmap::new(width, height).ok_or("invalid render target dimensions")?;
        pixmap.fill(Color::TRANSPARENT);

        for command in commands {
            match command {
                DrawCommand::Fill { bounds, color } => {
                    let bounds = physical_rect(*bounds, scale);
                    let Some(rect) = SkiaRect::from_xywh(bounds.0, bounds.1, bounds.2, bounds.3)
                    else {
                        continue;
                    };
                    let mut paint = Paint::default();
                    paint.set_color(Color::from_rgba8(color.0, color.1, color.2, color.3));
                    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                }
                DrawCommand::RoundedFill {
                    bounds,
                    color,
                    radius,
                } => {
                    let bounds = physical_rect(*bounds, scale);
                    let Some(path) = rounded_rect_path(bounds, *radius * scale.factor()) else {
                        continue;
                    };
                    let mut paint = Paint::default();
                    paint.set_color(Color::from_rgba8(color.0, color.1, color.2, color.3));
                    paint.anti_alias = true;
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                DrawCommand::Text {
                    bounds,
                    text,
                    color,
                    font_size,
                    line_height,
                    family,
                    align,
                } => self.draw_text(
                    &mut pixmap,
                    scale,
                    *bounds,
                    text,
                    TextColor::rgba(color.0, color.1, color.2, color.3),
                    *font_size,
                    *line_height,
                    *family,
                    *align,
                ),
            }
        }
        copy_rgba_to_argb(pixmap.data(), canvas);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &mut self,
        pixmap: &mut Pixmap,
        scale: Scale,
        bounds: Rect,
        text: &str,
        color: TextColor,
        font_size: f32,
        line_height: f32,
        family: FontFamily,
        align: TextAlign,
    ) {
        let factor = scale.factor();
        let physical = physical_rect(bounds, scale);
        let line_height = line_height * factor;
        let mut buffer = TextBuffer::new(
            &mut self.font_system,
            Metrics::new(font_size * factor, line_height),
        );
        buffer.set_size(Some(physical.2.max(1.0)), Some(physical.3.max(1.0)));
        buffer.set_text(
            text,
            &Attrs::new()
                .family(match family {
                    FontFamily::SansSerif => Family::SansSerif,
                    FontFamily::Monospace => Family::Monospace,
                })
                .weight(Weight::SEMIBOLD),
            Shaping::Advanced,
            Some(match align {
                TextAlign::Center => Align::Center,
                TextAlign::End => Align::End,
            }),
        );

        let x_offset = physical.0.round() as i32;
        let y_offset = (physical.1 + (physical.3 - line_height) / 2.0).round() as i32;
        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            color,
            |x, y, width, height, color| {
                let Some(rect) = SkiaRect::from_xywh(
                    (x + x_offset) as f32,
                    (y + y_offset) as f32,
                    width as f32,
                    height as f32,
                ) else {
                    return;
                };
                let (red, green, blue, alpha) = color.as_rgba_tuple();
                let mut paint = Paint::default();
                paint.set_color_rgba8(red, green, blue, alpha);
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            },
        );
    }
}

/// Cubic-bezier approximation constant for a quarter circle (4/3 * (sqrt(2) - 1)).
const CIRCLE_KAPPA: f32 = 0.552_284_8;

fn rounded_rect_path(bounds: (f32, f32, f32, f32), radius: f32) -> Option<tiny_skia::Path> {
    let (x, y, width, height) = bounds;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let radius = radius.max(0.0).min(width.min(height) / 2.0);
    let k = radius * CIRCLE_KAPPA;
    let mut path = PathBuilder::new();
    path.move_to(x + radius, y);
    path.line_to(x + width - radius, y);
    path.cubic_to(
        x + width - radius + k,
        y,
        x + width,
        y + radius - k,
        x + width,
        y + radius,
    );
    path.line_to(x + width, y + height - radius);
    path.cubic_to(
        x + width,
        y + height - radius + k,
        x + width - radius + k,
        y + height,
        x + width - radius,
        y + height,
    );
    path.line_to(x + radius, y + height);
    path.cubic_to(
        x + radius - k,
        y + height,
        x,
        y + height - radius + k,
        x,
        y + height - radius,
    );
    path.line_to(x, y + radius);
    path.cubic_to(x, y + radius - k, x + radius - k, y, x + radius, y);
    path.close();
    path.finish()
}

fn physical_rect(rect: Rect, scale: Scale) -> (f32, f32, f32, f32) {
    let factor = scale.factor();
    (
        rect.origin.x * factor,
        rect.origin.y * factor,
        rect.size.width * factor,
        rect.size.height * factor,
    )
}

fn copy_rgba_to_argb(source: &[u8], destination: &mut [u8]) {
    assert!(
        destination.len() >= source.len(),
        "render target must fit the rendered pixels"
    );

    for (rgba, argb) in source
        .chunks_exact(4)
        .zip(destination[..source.len()].chunks_exact_mut(4))
    {
        argb.copy_from_slice(&[rgba[2], rgba[1], rgba[0], rgba[3]]);
    }
}

#[cfg(test)]
mod tests {
    use super::{Scale, copy_rgba_to_argb};

    #[test]
    fn rounds_fractional_physical_size_up() {
        let scale = Scale::from_120ths(180);

        assert_eq!(scale.physical(1), 2);
        assert_eq!(scale.physical(32), 48);
        assert_eq!(scale.physical(509), 764);
    }

    #[test]
    fn converts_rgba_pixels_and_ignores_slot_padding() {
        let mut destination = [0; 12];

        copy_rgba_to_argb(
            &[0x11, 0x22, 0x33, 0xff, 0x44, 0x55, 0x66, 0x80],
            &mut destination,
        );

        assert_eq!(
            destination,
            [0x33, 0x22, 0x11, 0xff, 0x66, 0x55, 0x44, 0x80, 0, 0, 0, 0]
        );
    }
}
