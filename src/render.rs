pub const BAR_COLOR_ARGB: u32 = 0xff_7c_3a_ed;

pub fn fill_solid_argb(canvas: &mut [u8], color: u32) {
    let pixel = color.to_le_bytes();
    let mut pixels = canvas.chunks_exact_mut(pixel.len());

    for destination in &mut pixels {
        destination.copy_from_slice(&pixel);
    }

    assert!(
        pixels.into_remainder().is_empty(),
        "ARGB canvas length must be a multiple of four"
    );
}

#[cfg(test)]
mod tests {
    use super::fill_solid_argb;

    #[test]
    fn fills_every_pixel_with_little_endian_argb() {
        let mut canvas = [0; 8];

        fill_solid_argb(&mut canvas, 0xff_11_22_33);

        assert_eq!(canvas, [0x33, 0x22, 0x11, 0xff, 0x33, 0x22, 0x11, 0xff]);
    }

    #[test]
    #[should_panic(expected = "ARGB canvas length must be a multiple of four")]
    fn rejects_partial_pixels() {
        fill_solid_argb(&mut [0; 3], 0);
    }
}
