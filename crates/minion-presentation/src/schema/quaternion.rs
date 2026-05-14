/// Quaternion represented as [w, x, y, z].
pub type Quat = [f64; 4];

/// Convert Euler angles (degrees, XYZ order) to a unit quaternion [w, x, y, z].
pub fn euler_deg_to_quaternion(rx_deg: f64, ry_deg: f64, rz_deg: f64) -> Quat {
    let rx = rx_deg.to_radians() * 0.5;
    let ry = ry_deg.to_radians() * 0.5;
    let rz = rz_deg.to_radians() * 0.5;

    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    let (sz, cz) = rz.sin_cos();

    // XYZ intrinsic rotation order
    [
        cx * cy * cz + sx * sy * sz,  // w
        sx * cy * cz - cx * sy * sz,  // x
        cx * sy * cz + sx * cy * sz,  // y
        cx * cy * sz - sx * sy * cz,  // z
    ]
}

/// Extract Euler angles (degrees, XYZ order) from a quaternion.
/// Returns (rx_deg, ry_deg, rz_deg).
pub fn quaternion_to_euler_deg(q: &Quat) -> (f64, f64, f64) {
    let [w, x, y, z] = *q;

    // Normalise
    let n = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if n > 1e-10 {
        (w / n, x / n, y / n, z / n)
    } else {
        (1.0, 0.0, 0.0, 0.0)
    };

    // Detect gimbal lock (singularity at pitch ±90°)
    let sin_ry = 2.0 * (w * y - z * x);
    let sin_ry = sin_ry.clamp(-1.0, 1.0);

    let ry = sin_ry.asin();

    let (rx, rz) = if (1.0 - sin_ry * sin_ry).sqrt() > 1e-6 {
        let rx = (2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y));
        let rz = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z));
        (rx, rz)
    } else {
        // Gimbal lock — freeze roll, compute yaw
        let rx = 0.0;
        let rz = (2.0 * (x * z - w * y)).atan2(1.0 - 2.0 * (y * y + z * z));
        (rx, rz)
    };

    (rx.to_degrees(), ry.to_degrees(), rz.to_degrees())
}

/// Produce a CSS `rotate3d(x,y,z,angle)` string from a quaternion.
/// Uses axis-angle decomposition. Safe for CSS `transform` property.
pub fn quaternion_to_css_rotate3d(q: &Quat) -> String {
    let [w, x, y, z] = *q;

    // Normalise
    let n = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if n > 1e-10 {
        (w / n, x / n, y / n, z / n)
    } else {
        (1.0, 0.0, 0.0, 0.0)
    };

    let angle_rad = 2.0 * w.clamp(-1.0, 1.0).acos();
    // Clamp to [0, 1] before sqrt — floating-point rounding can push w*w
    // slightly above 1.0 after normalization, making the argument negative → NaN.
    let sin_half = (1.0 - w * w).max(0.0).sqrt();

    if sin_half < 1e-6 {
        return "rotate3d(0,0,1,0deg)".into();
    }

    let (ax, ay, az) = (x / sin_half, y / sin_half, z / sin_half);
    let angle_deg = angle_rad.to_degrees();

    format!("rotate3d({:.6},{:.6},{:.6},{:.4}deg)", ax, ay, az, angle_deg)
}
