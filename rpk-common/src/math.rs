/// Computes the x-coordinate of a point on a cubic Bezier curve at a given parameter `t`.
///
/// # Parameters
/// - `t`: The parameter along the curve, where 0.0 represents the start of the curve
///   and 1.0 represents the end.
/// - `c0`: control point defining the curve near t = 0.
/// - `c1`: control point defining the curve near t = 1.
///
/// # Returns
/// - The x-coordinate of the point on the curve at `t`.
pub fn cubic_bezier(t: f32, c0: f32, c1: f32) -> f32 {
    let r = 1.0 - t; // Complement of `t`
    let r2 = r * r; // `r` squared
    let t2 = t * t; // `t` squared
    let t3 = t * t2; // `t` cubed

    // Calculate the x-coordinate using the cubic Bezier formula.
    3.0 * r2 * t * c0 + 3.0 * r * t2 * c1 + t3
}

#[cfg(test)]
#[path = "math_test.rs"]
mod test;
