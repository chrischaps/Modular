//! Reading between samples.

/// The 4-point, 3rd-order Hermite curve through `x1` (at 0) and `x2` (at 1),
/// shaped by its neighbours `x0` and `x3`: smooth enough for audio read at
/// any rate, and cheap enough to run per sample per voice.
#[inline]
pub fn hermite(x0: f32, x1: f32, x2: f32, x3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x2 - x0);
    let c2 = x0 - 2.5 * x1 + 2.0 * x2 - 0.5 * x3;
    let c3 = 0.5 * (x3 - x0) + 1.5 * (x1 - x2);
    ((c3 * t + c2) * t + c1) * t + x1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_its_middle_points() {
        assert_eq!(hermite(3.0, 1.0, 2.0, 5.0, 0.0), 1.0);
        assert!((hermite(3.0, 1.0, 2.0, 5.0, 1.0) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn follows_a_straight_line_exactly() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!((hermite(-1.0, 0.0, 1.0, 2.0, t) - t).abs() < 1e-6);
        }
    }
}
