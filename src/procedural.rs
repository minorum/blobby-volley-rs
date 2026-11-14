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

#[inline]
fn sd_ellipsoid(
    px: f32,
    py: f32,
    pz: f32,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rz: f32,
) -> f32 {
    let qx = (px - cx) / rx;
    let qy = (py - cy) / ry;
    let qz = pz / rz;
    let k0 = (qx * qx + qy * qy + qz * qz).sqrt();
    let k1 = ((qx / rx).powi(2) + (qy / ry).powi(2) + (qz / rz).powi(2)).sqrt();
    if k1 > 1e-6 {
        k0 * (k0 - 1.0) / k1
    } else {
        -rx.min(ry).min(rz)
    }
}

#[inline]
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

#[inline]
fn normalize3(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let len = (x * x + y * y + z * z).sqrt().max(1e-8);
    (x / len, y / len, z / len)
}

/// Generates all 5 squash-and-stretch base frames (148x101 grayscale) of the Blobby character.
///
/// Uses an orthographic raymarched 3D smooth-union SDF of two ellipsoids (head + body)
/// with two-light Phong shading matching the original specular highlights and base tones.
pub fn generate_blobby_frames() -> Vec<BmpImage> {
    const W: usize = 148;
    const H: usize = 101;
    const CX: f32 = 73.0;

    let l1 = normalize3(-0.34, 0.86, 0.38);
    let h1 = normalize3(l1.0, l1.1, l1.2 + 1.0);
    let l2 = normalize3(0.36, -0.78, 0.51);
    let h2 = normalize3(l2.0, l2.1, l2.2 + 1.0);

    let mut frames = Vec::with_capacity(5);
    for k in 0..5 {
        let t = (k as f32) / 4.0;
        let head_cy = 37.5 + 15.2 * t;
        let head_rx = 25.2 - 1.8 * t;
        let head_ry = 25.5 - 2.8 * t;
        let body_cy = 69.5 + 11.5 * t;
        let body_rx = 32.5 + 5.0 * t;
        let body_ry = 31.2 - 11.5 * t;
        let sk = 15.0 - 5.5 * t;

        let sdf = |px: f32, py: f32, pz: f32| -> f32 {
            let d1 = sd_ellipsoid(px, py, pz, CX, head_cy, head_rx, head_ry, head_rx);
            let d2 = sd_ellipsoid(px, py, pz, CX, body_cy, body_rx, body_ry, body_rx);
            smin(d1, d2, sk)
        };

        let mut pixels = vec![0u32; W * H];
        // Bounding box of Blobby is x in [34..113], y_td in [10..101].
        for by in 0..H {
            let py = (H - 1 - by) as f32;
            if py < 10.0 {
                continue;
            }
            for x in 34..114 {
                let px = x as f32;
                let mut pz = 45.0_f32;
                for _ in 0..30 {
                    let d = sdf(px, py, pz);
                    pz -= d * 0.85;
                }
                if sdf(px, py, pz) >= 0.35 {
                    continue;
                }

                let eps = 0.25_f32;
                let gx = sdf(px + eps, py, pz) - sdf(px - eps, py, pz);
                let gy = -(sdf(px, py + eps, pz) - sdf(px, py - eps, pz));
                let gz = sdf(px, py, pz + eps) - sdf(px, py, pz - eps);
                let (nx, ny, nz) = normalize3(gx, gy, gz);

                let d1 = (nx * l1.0 + ny * l1.1 + nz * l1.2).max(0.0);
                let s1 = (nx * h1.0 + ny * h1.1 + nz * h1.2).max(0.0).powf(95.0);
                let d2 = (nx * l2.0 + ny * l2.1 + nz * l2.2).max(0.0);
                let s2 = (nx * h2.0 + ny * h2.1 + nz * h2.2).max(0.0).powf(100.0);

                let val = (166.0 + 34.0 * d1 + 10.0 * d2 + 78.0 * s1 + 78.0 * s2)
                    .round()
                    .clamp(0.0, 255.0) as u32;
                if val >= 14 {
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

/// Generates the 69x17 checkerboard-dithered ball shadow mask.
pub fn generate_ball_shadow() -> BmpImage {
    const W: usize = 69;
    const H: usize = 17;
    let mut pixels = vec![0u32; W * H];
    for by in 0..H {
        let y = H - 1 - by;
        let dy = (y as f32 - 8.0) / 8.8;
        for x in 0..W {
            if (x + y) % 2 != 1 {
                continue;
            }
            let dx = (x as f32 - 34.0) / 34.8;
            if dx * dx + dy * dy <= 1.0 {
                pixels[by * W + x] = 0x00202020;
            }
        }
    }
    BmpImage {
        width: W,
        height: H,
        pixels,
    }
}

/// Generates all 10 checkerboard-dithered Blobby shadow masks (129x25)
/// from the 5 base Blobby silhouettes (0..5 for P1, 5..10 mirrored for P2).
pub fn generate_blobby_shadows(blobby_base: &[BmpImage]) -> Vec<BmpImage> {
    const W: usize = 129;
    const H: usize = 25;
    let mut shadows = Vec::with_capacity(10);

    for player in 0..2 {
        for (k, base) in blobby_base.iter().take(5).enumerate() {
            let parity = usize::from(k < 2);
            let mut pixels = vec![0u32; W * H];
            for by in 0..H {
                let y = H - 1 - by;
                let yb_td = 100 - ((24 - y) as f32 * 3.8).round() as i32;
                if !(0..101).contains(&yb_td) {
                    continue;
                }
                let yb_bu = (base.height as i32 - 1 - yb_td) as usize;
                for x in 0..W {
                    let src_x = if player == 0 { x } else { W - 1 - x };
                    if (src_x + y) % 2 != parity {
                        continue;
                    }
                    let xb = src_x as i32 - 52 + 73;
                    if (0..148).contains(&xb) && base.pixels[yb_bu * base.width + xb as usize] != 0
                    {
                        pixels[by * W + x] = 0x00202020;
                    }
                }
            }
            shadows.push(BmpImage {
                width: W,
                height: H,
                pixels,
            });
        }
    }
    shadows
}
