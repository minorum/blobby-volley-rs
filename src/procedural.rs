use crate::pak::BmpImage;

/// Generates all 16 rotation frames (64x64) of the volleyball procedurally.
///
/// Uses a unit-sphere shading model with wrap diffuse lighting from the top-left
/// and an 18-panel seam pattern rotated by `22.5` degrees per frame, matching the
/// original `sum < 0xB4` transparency threshold.
pub fn generate_ball_frames() -> Vec<BmpImage> {
    const W: usize = 64;
    const H: usize = 64;
    const R: f32 = 32.45;
    const CX: f32 = 31.5;
    const CY: f32 = 31.5;

    // Normalized top-left key light direction
    let (lx, ly, lz) = {
        let (x, y, z) = (-0.40_f32, 0.85_f32, 0.34_f32);
        let len = (x * x + y * y + z * z).sqrt();
        (x / len, y / len, z / len)
    };

    let mut frames = Vec::with_capacity(16);
    for k in 0..16 {
        let ang = -((k as f32) - 1.0) * std::f32::consts::FRAC_PI_8;
        let (ca, sa) = (ang.cos(), ang.sin());
        let mut pixels = vec![0u32; W * H];

        // Store bottom-up (row 0 = bottom scanline) to match BmpImage convention.
        for by in 0..H {
            let y_td = (H - 1 - by) as f32;
            let ny0 = (CY - y_td) / R;
            for x in 0..W {
                let nx0 = (x as f32 - CX) / R;
                let nz_sq = 1.0 - nx0 * nx0 - ny0 * ny0;
                if nz_sq <= 0.0 {
                    continue;
                }
                let nz0 = nz_sq.sqrt();

                let ndotl = nx0 * lx + ny0 * ly + nz0 * lz;
                let w_ndotl = ((ndotl + 0.4) / 1.4).max(0.0);
                let shade = 98.0 + 168.0 * w_ndotl;

                let rx = nx0 * ca + ny0 * sa;
                let ry = -nx0 * sa + ny0 * ca;

                let lon_off = 0.20_f32;
                let lat_bound = 0.585_f32;
                let d_lat_b = (ry.abs() - lat_bound).abs();
                let d = if ry.abs() < lat_bound {
                    let d_lon_b = (rx - lon_off).abs();
                    let d_strip = if rx < lon_off {
                        (ry - lat_bound / 3.0)
                            .abs()
                            .min((ry + lat_bound / 3.0).abs())
                    } else {
                        (rx - (lon_off + 0.38))
                            .abs()
                            .min((rx - (lon_off + 0.68)).abs())
                    };
                    d_lat_b.min(d_lon_b.min(d_strip))
                } else {
                    let d_cap = if ry > 0.0 {
                        (rx + 0.48).abs().min((rx - 0.11).abs())
                    } else {
                        (rx + 0.20).abs().min((rx - 0.32).abs())
                    };
                    d_lat_b.min(d_cap)
                };

                let seam = (-(d / 0.06).powi(2)).exp();
                let cushion = 1.0 + 0.04 * ((d / 0.15).min(1.0) - 0.5);
                let val = (shade * cushion * (1.0 - 0.17 * seam)).round().clamp(0.0, 255.0) as u32;
                if val >= 60 {
                    pixels[by * W + x] = (val << 16) | (val << 8) | val;
                }
            }
        }
        frames.push(BmpImage {
            width: W,
            height: H,
            pixels,
        });
    }
    frames
}
