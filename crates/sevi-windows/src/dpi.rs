//! Window layouts use 96-DPI logical units; Win32 coordinates and native fonts use pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale(u32);
impl Scale {
    pub const fn new(dpi: u32) -> Self {
        Self(if dpi == 0 { 96 } else { dpi })
    }
    pub const fn dpi(self) -> u32 {
        self.0
    }
    pub fn factor(self) -> f32 {
        self.0 as f32 / 96.0
    }
    pub fn pixels(self, logical: i32) -> i32 {
        rounded_ratio(logical, self.0, 96)
    }
    pub fn logical(self, pixels: i32) -> i32 {
        rounded_ratio(pixels, 96, self.0)
    }
    pub fn contains(self, bounds: [i32; 4], x: i32, y: i32) -> bool {
        x >= self.pixels(bounds[0])
            && x < self.pixels(bounds[2])
            && y >= self.pixels(bounds[1])
            && y < self.pixels(bounds[3])
    }
}
fn rounded_ratio(value: i32, numerator: u32, denominator: u32) -> i32 {
    let product = i64::from(value) * i64::from(numerator);
    let half = i64::from(denominator) / 2;
    ((product + if product < 0 { -half } else { half }) / i64::from(denominator)) as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_and_text_follow_standard_and_custom_windows_scales() {
        for (dpi, width, height, body) in [
            (96, 460, 600, 15),
            (120, 575, 750, 19),
            (144, 690, 900, 23),
            (168, 805, 1050, 26),
            (192, 920, 1200, 30),
            (216, 1035, 1350, 34),
            (288, 1380, 1800, 45),
        ] {
            let s = Scale::new(dpi);
            assert_eq!(
                (s.pixels(460), s.pixels(600), s.pixels(15)),
                (width, height, body)
            );
        }
        assert_eq!(Scale::new(132).pixels(460), 633);
        assert_eq!(Scale::new(0), Scale::new(96));
    }
    #[test]
    fn painted_link_and_physical_hit_target_agree_at_fractional_scales() {
        for dpi in [96, 120, 132, 144, 168, 192, 216, 240, 288, 384] {
            let s = Scale::new(dpi);
            let (x, y) = (s.logical(s.pixels(65)), s.logical(s.pixels(222)));
            assert!((24..100).contains(&x) && (213..235).contains(&y));
            assert_eq!(s.logical(s.pixels(-24)), -24);
            // Hit testing compares physical bounds, avoiding inverse-rounding gaps at the last pixel.
            let bounds = [24, 213, 100, 235];
            assert!(s.contains(bounds, s.pixels(100) - 1, s.pixels(235) - 1));
            assert!(!s.contains(bounds, s.pixels(100), s.pixels(222)));
            assert!(!s.contains(bounds, s.pixels(24) - 1, s.pixels(222)));
        }
    }
}
