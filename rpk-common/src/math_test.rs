use super::*;

extern crate std;

pub fn alt_bezier(t: f32, c0: f32, c1: f32, c2: f32, c3: f32) -> (f32, f32) {
    let (a, b, c, d) = (0.0, c0, c2, 1.0); // x values
    let (e, f, g, h) = (0.0, c1, c3, 1.0); // y values

    // Precompute powers of t and (1 - t)
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    (
        // Calculate the x-coordinate using the cubic Bézier formula
        mt3 * a + 3.0 * mt2 * t * b + 3.0 * mt * t2 * c + t3 * d,
        // Calculate the y-coordinate (output value) with the same formula
        mt3 * e + 3.0 * mt2 * t * f + 3.0 * mt * t2 * g + t3 * h,
    )
}

macro_rules! assert_curve {
    ($c0:expr,$c1:expr,$c2:expr,$c3:expr,$e:expr) => {{
        let a: std::vec::Vec<f32> = (0..=10)
            .map(|t| {
                let tf = t as f32 / 10.0;
                cubic_bezier(tf, $c1, $c3)
            })
            .collect();

        let b: std::vec::Vec<f32> = (0..=10)
            .map(|t| {
                let tf = t as f32 / 10.0;
                alt_bezier(tf, $c0, $c1, $c2, $c3).1
            })
            .collect();

        let a = std::format!("{a:.2?}");
        let b = std::format!("{b:.2?}");
        let e = std::format!("{:.2?}", $e);

        assert_eq!(a, b);
        assert_eq!(a, e);
    }};
}

#[test]
fn cubic_bezier_test() {
    assert_curve!(
        1.0,
        0.86,
        0.63,
        1.38,
        [0.00, 0.25, 0.47, 0.67, 0.83, 0.96, 1.06, 1.11, 1.12, 1.09, 1.00]
    );

    assert_curve!(
        0.0,
        0.0,
        1.0,
        1.0,
        [0.00, 0.03, 0.10, 0.22, 0.35, 0.50, 0.65, 0.78, 0.90, 0.97, 1.00]
    );

    //cubic-bezier(0.8, 0.36, 0.01, 0.41)

    assert_curve!(
        0.0,
        0.2,
        0.0,
        0.5,
        [0.00, 0.06, 0.13, 0.21, 0.29, 0.39, 0.49, 0.60, 0.72, 0.86, 1.00]
    );

    assert_curve!(
        0.8,
        0.36,
        0.01,
        0.41,
        [0.00, 0.10, 0.19, 0.26, 0.34, 0.41, 0.50, 0.59, 0.70, 0.84, 1.00]
    );
}
