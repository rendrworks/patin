pub const TOGGLE_WIDTH: f64 = 180.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn contains(self, position: (f64, f64)) -> bool {
        let (x, y) = position;
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

pub fn toggle_target(bar_height: u32) -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: TOGGLE_WIDTH,
        height: f64::from(bar_height),
    }
}

#[cfg(test)]
mod tests {
    use super::toggle_target;

    #[test]
    fn hit_test_uses_logical_half_open_bounds() {
        let target = toggle_target(32);

        assert!(target.contains((0.0, 0.0)));
        assert!(target.contains((179.99, 31.99)));
        assert!(!target.contains((180.0, 12.0)));
        assert!(!target.contains((40.0, 32.0)));
        assert!(!target.contains((-0.1, 12.0)));
    }
}
