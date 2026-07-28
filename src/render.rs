use cosmic_text::{
    Align, Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight,
};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

pub const SCALE_DENOMINATOR: u32 = 120;

const CLOCK_COLOR: TextColor = TextColor::rgb(245, 243, 255);

const CLOCK_FONT_SIZE: f32 = 15.0;
const CLOCK_LINE_HEIGHT: f32 = 20.0;
const HORIZONTAL_PADDING: f32 = 12.0;
const ACCENT_HEIGHT: f32 = 2.0;

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

    fn factor(self) -> f32 {
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
        clock: &str,
    ) -> Result<(), &'static str> {
        let mut pixmap = Pixmap::new(width, height).ok_or("invalid render target dimensions")?;
        pixmap.fill(Color::from_rgba8(20, 17, 29, 255));

        draw_accent(&mut pixmap, scale);
        self.draw_clock(&mut pixmap, scale, clock);
        copy_rgba_to_argb(pixmap.data(), canvas);

        Ok(())
    }

    fn draw_clock(&mut self, pixmap: &mut Pixmap, scale: Scale, clock: &str) {
        let factor = scale.factor();
        let padding = HORIZONTAL_PADDING * factor;
        let line_height = CLOCK_LINE_HEIGHT * factor;
        let text_width = (pixmap.width() as f32 - padding * 2.0).max(1.0);

        let mut buffer = TextBuffer::new(
            &mut self.font_system,
            Metrics::new(CLOCK_FONT_SIZE * factor, line_height),
        );
        buffer.set_size(Some(text_width), Some(line_height));
        buffer.set_text(
            clock,
            &Attrs::new()
                .family(Family::Monospace)
                .weight(Weight::SEMIBOLD),
            Shaping::Advanced,
            Some(Align::End),
        );

        let y_offset = ((pixmap.height() as f32 - line_height) / 2.0).round() as i32;
        let x_offset = padding.round() as i32;

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            CLOCK_COLOR,
            |x, y, width, height, color| {
                let Some(rect) = Rect::from_xywh(
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

fn draw_accent(pixmap: &mut Pixmap, scale: Scale) {
    let height = (ACCENT_HEIGHT * scale.factor()).ceil().max(1.0);
    let y = pixmap.height() as f32 - height;
    let Some(rect) = Rect::from_xywh(0.0, y, pixmap.width() as f32, height) else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(124, 58, 237, 255));
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
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
