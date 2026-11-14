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

#[inline]
fn hash2(ix: i32, iy: i32) -> f32 {
    let mut n = (ix.wrapping_mul(374761393))
        .wrapping_add(iy.wrapping_mul(668265263))
        .wrapping_add(1442695041);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) as u32 & 0xFFFF) as f32 / 65535.0
}

fn value_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash2(ix, iy);
    let b = hash2(ix + 1, iy);
    let c = hash2(ix, iy + 1);
    let d = hash2(ix + 1, iy + 1);
    (a + (b - a) * ux) + ((c + (d - c) * ux) - (a + (b - a) * ux)) * uy
}

fn fbm(mut x: f32, mut y: f32, octaves: usize) -> f32 {
    let mut sum = 0.0_f32;
    let mut amp = 0.5_f32;
    let mut norm = 0.0_f32;
    for _ in 0..octaves {
        sum += amp * value_noise(x, y);
        norm += amp;
        x *= 2.03;
        y *= 2.01;
        amp *= 0.5;
    }
    sum / norm
}

#[inline]
fn rgb(r: f32, g: f32, b: f32) -> u32 {
    let rr = r.round().clamp(0.0, 255.0) as u32;
    let gg = g.round().clamp(0.0, 255.0) as u32;
    let bb = b.round().clamp(0.0, 255.0) as u32;
    (rr << 16) | (gg << 8) | bb
}

#[inline]
fn blend_pixel(dst: &mut u32, r: f32, g: f32, b: f32, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    let dr = ((*dst >> 16) & 0xFF) as f32;
    let dg = ((*dst >> 8) & 0xFF) as f32;
    let db = (*dst & 0xFF) as f32;
    *dst = rgb(dr + (r - dr) * a, dg + (g - dg) * a, db + (b - db) * a);
}

#[inline]
fn darken_pixel(dst: &mut u32, factor: f32) {
    let dr = ((*dst >> 16) & 0xFF) as f32;
    let dg = ((*dst >> 8) & 0xFF) as f32;
    let db = (*dst & 0xFF) as f32;
    *dst = rgb(dr * factor, dg * factor, db * factor);
}

fn draw_disc(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32, col: u32) {
    let x0 = (cx - r).floor().max(0.0) as usize;
    let x1 = (cx + r).ceil().min((w - 1) as f32) as usize;
    let y0 = (cy - r).floor().max(0.0) as usize;
    let y1 = (cy + r).ceil().min((h - 1) as f32) as usize;
    let r2 = r * r;
    for y in y0..=y1 {
        let dy = y as f32 - cy;
        for x in x0..=x1 {
            let dx = x as f32 - cx;
            if dx * dx + dy * dy <= r2 {
                buf[y * w + x] = col;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_palm_tree(
    buf: &mut [u32],
    w: usize,
    h: usize,
    bx: f32,
    by: f32,
    mx: f32,
    my: f32,
    tx: f32,
    ty: f32,
    base_r: f32,
    top_r: f32,
    frond_len: f32,
    seed: f32,
) {
    // Trunk along quadratic Bezier
    let steps = 260;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let it = 1.0 - t;
        let x = it * it * bx + 2.0 * it * t * mx + t * t * tx;
        let y = it * it * by + 2.0 * it * t * my + t * t * ty;
        let r = base_r + (top_r - base_r) * t;
        let ring = ((t * 42.0).sin() * 0.5 + 0.5) * 22.0;
        let x0 = (x - r).floor().max(0.0) as usize;
        let x1 = (x + r).ceil().min((w - 1) as f32) as usize;
        let y0 = (y - r).floor().max(0.0) as usize;
        let y1 = (y + r).ceil().min((h - 1) as f32) as usize;
        for py in y0..=y1 {
            let dy = py as f32 - y;
            for px in x0..=x1 {
                let dx = px as f32 - x;
                if dx * dx + dy * dy <= r * r {
                    let lit = (0.5 - dx / (2.0 * r.max(1.0))).clamp(0.0, 1.0);
                    let cr = 46.0 + lit * 44.0 + ring * 0.7;
                    let cg = 34.0 + lit * 30.0 + ring * 0.5;
                    let cb = 24.0 + lit * 20.0 + ring * 0.3;
                    buf[py * w + px] = rgb(cr, cg, cb);
                }
            }
        }
    }

    // Feathery palm fronds radiating from crown (tx, ty)
    let num_fronds = 13;
    for f in 0..num_fronds {
        let ang = (f as f32 / num_fronds as f32) * std::f32::consts::TAU + seed;
        let dir_x = ang.cos();
        let dir_y = ang.sin() * 0.72 - 0.16;
        let len = frond_len * (0.85 + 0.30 * ((f as f32 * 2.1 + seed).sin() * 0.5 + 0.5));
        let droop = 28.0 + 22.0 * (1.0 - dir_y.abs());

        let spine_steps = 52;
        for s in 2..=spine_steps {
            let t = s as f32 / spine_steps as f32;
            let sx = tx + dir_x * len * t;
            let sy = ty + dir_y * len * t + droop * t * t;
            let leaf_len = (1.0 - (t - 0.35).abs() * 1.1).clamp(0.18, 1.0) * (len * 0.28);
            let shade = if dir_y < 0.0 { 1.0 } else { 0.72 };

            draw_disc(
                buf,
                w,
                h,
                sx,
                sy,
                1.3,
                rgb(12.0 * shade, 72.0 * shade, 8.0),
            );

            // Leaflets on both sides of spine
            let perp_x = -dir_y;
            let perp_y = dir_x;
            for side in [-1.0_f32, 1.0_f32] {
                let lx_dir = (perp_x * side * 0.80 + dir_x * 0.50) * leaf_len;
                let ly_dir = (perp_y * side * 0.55 + dir_y * 0.35 + 0.48) * leaf_len;
                let l_steps = 16;
                for ls in 1..=l_steps {
                    let lt = ls as f32 / l_steps as f32;
                    let px = sx + lx_dir * lt;
                    let py = sy + ly_dir * lt + 3.5 * lt * lt;
                    let ix = px.round() as i32;
                    let iy = py.round() as i32;
                    if (0..w as i32).contains(&ix) && (0..h as i32).contains(&iy) {
                        let g = (52.0 + 96.0 * (1.0 - lt * 0.45) * shade).clamp(0.0, 255.0);
                        let r = (6.0 + 26.0 * (1.0 - lt) * shade).clamp(0.0, 255.0);
                        buf[iy as usize * w + ix as usize] = rgb(r, g, 6.0);
                    }
                }
            }
        }
    }
}

fn draw_palm_shadow(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, rx: f32, ry: f32) {
    let x0 = (cx - rx).floor().max(0.0) as usize;
    let x1 = (cx + rx).ceil().min((w - 1) as f32) as usize;
    let y0 = (cy - ry).floor().max(0.0) as usize;
    let y1 = (cy + ry).ceil().min((h - 1) as f32) as usize;
    for y in y0..=y1 {
        let dy = (y as f32 - cy) / ry;
        for x in x0..=x1 {
            let dx = (x as f32 - cx) / rx;
            let r = (dx * dx + dy * dy).sqrt();
            if r > 1.0 {
                continue;
            }
            let ang = dy.atan2(dx);
            let n = fbm(x as f32 * 0.04, y as f32 * 0.10, 2);
            let frond_wave = (ang * 7.0 + n * 2.5).cos() * 0.5 + 0.5;
            let core = (1.0 - r / 0.30).clamp(0.0, 1.0);
            let mask = (core + (1.0 - r) * frond_wave * (0.45 + 0.55 * n)).clamp(0.0, 1.0);
            if mask > 0.25 {
                darken_pixel(&mut buf[y * w + x], 1.0 - 0.55 * mask);
            }
        }
    }
}

/// Generates the 800x600 top-down beach background procedurally.
pub fn generate_background() -> Vec<u32> {
    const W: usize = 800;
    const H: usize = 600;
    let mut buf = vec![0u32; W * H];

    let shoreline = |x: f32| -> f32 {
        338.0 - 19.0 * (x / 520.0).clamp(0.0, 1.0)
            + 3.0 * (x * 0.014).sin()
            + 1.5 * (x * 0.037).cos()
    };

    // 1. Sky, Ocean, and Sand base layers
    for y in 0..H {
        let yf = y as f32;
        for x in 0..W {
            let xf = x as f32;
            let sh_y = shoreline(xf);
            let col = if yf < 285.0 {
                // Deep cobalt sky with sparse wispy fBm clouds
                let t = yf / 285.0;
                let cloud = fbm(xf * 0.007 + 2.1, yf * 0.022 + 0.4, 4);
                let wisp = ((cloud - 0.54) * 2.6).clamp(0.0, 1.0);
                let side_bright = (xf / 800.0) * 16.0;
                let r = 8.0 + t * 44.0 + side_bright + wisp * 82.0;
                let g = 34.0 + t * 56.0 + side_bright * 0.8 + wisp * 78.0;
                let b = 212.0 + t * 24.0 + wisp * 24.0;
                rgb(r, g, b)
            } else if yf < sh_y {
                // Ocean with horizon darkening and horizontal wave ripples
                let depth = ((yf - 285.0) / (sh_y - 285.0).max(1.0)).clamp(0.0, 1.0);
                let ripple = fbm(xf * 0.055, yf * 0.42, 3);
                let glint = if ripple > 0.72 && depth > 0.25 {
                    (ripple - 0.72) * 290.0
                } else {
                    0.0
                };
                let horiz = if yf < 289.0 { 0.72 } else { 1.0 };
                let r = (66.0 + depth * 32.0 + (ripple - 0.5) * 44.0 + glint) * horiz;
                let g = (94.0 + depth * 38.0 + (ripple - 0.5) * 52.0 + glint) * horiz;
                let b = (238.0 + depth * 14.0 + (ripple - 0.5) * 16.0 + glint * 0.4) * horiz;
                rgb(r, g, b)
            } else {
                // Sandy beach with subtle shading and grain
                let sand_t = ((yf - sh_y) / (600.0 - sh_y)).clamp(0.0, 1.0);
                let grain = (hash2(x as i32, y as i32) - 0.5) * 12.0;
                let dune = (fbm(xf * 0.006, yf * 0.012, 3) - 0.5) * 14.0;
                let r = 174.0 + sand_t * 14.0 + grain + dune - (1.0 - xf / 800.0) * 16.0;
                let g = 160.0 + sand_t * 12.0 + grain + dune - (1.0 - xf / 800.0) * 16.0;
                let b = 90.0 + sand_t * 8.0 + grain * 0.7 + dune * 0.6;
                rgb(r, g, b)
            };
            buf[y * W + x] = col;
        }
    }

    // 2. Distant left cliffs and sea stack
    for x in 0..265 {
        let xf = x as f32;
        let top_y = if x < 198 {
            170.0 + xf * 0.21 + (fbm(xf * 0.08, 1.0, 3) - 0.5) * 18.0
        } else if x < 208 {
            170.0 + 198.0 * 0.21 + (xf - 198.0) * 8.5
        } else if x >= 224 && x <= 258 {
            let d = ((xf - 241.0) / 17.0).abs();
            260.0 + d * d * 32.0 + (fbm(xf * 0.2, 2.0, 2) - 0.5) * 6.0
        } else {
            999.0
        };
        let bot_y = 294.0 - (xf / 265.0) * 4.0;
        let y0 = (top_y.floor() as usize).min(H);
        let y1 = (bot_y.ceil() as usize).min(H);
        for y in y0..y1 {
            let stria = fbm(xf * 0.16, y as f32 * 0.025, 3);
            let lit = if x > 224 { 0.85 + 0.25 * (241.0 - xf) / 17.0 } else { 0.9 };
            let cr = (62.0 + stria * 58.0) * lit;
            let cg = (54.0 + stria * 52.0) * lit;
            let cb = (48.0 + stria * 46.0) * lit;
            buf[y * W + x] = rgb(cr, cg, cb);
        }
    }

    // 3. Palm shadows on sand
    draw_palm_shadow(&mut buf, W, H, 165.0, 378.0, 145.0, 38.0);
    draw_palm_shadow(&mut buf, W, H, 655.0, 368.0, 125.0, 32.0);

    // 4. Palm trees (2 left, 3 right)
    draw_palm_tree(&mut buf, W, H, 62.0, 385.0, 44.0, 225.0, 22.0, 68.0, 7.0, 3.0, 122.0, 0.2);
    draw_palm_tree(&mut buf, W, H, 14.0, 338.0, 74.0, 220.0, 152.0, 124.0, 6.5, 2.8, 110.0, 0.7);
    draw_palm_tree(&mut buf, W, H, 600.0, 344.0, 588.0, 268.0, 572.0, 195.0, 5.2, 2.2, 76.0, 0.4);
    draw_palm_tree(&mut buf, W, H, 620.0, 340.0, 636.0, 240.0, 656.0, 148.0, 5.8, 2.4, 92.0, 1.1);
    draw_palm_tree(&mut buf, W, H, 732.0, 342.0, 736.0, 260.0, 742.0, 185.0, 5.2, 2.2, 80.0, 0.8);

    // 5. Net shadow on sand (perspective quad from (407, 404)..(465, 397) to (407, 600)..(524, 577))
    for y in 397..H {
        let yf = y as f32;
        let x_left = 407.0;
        let t_right = (yf - 397.0) / 180.0;
        let x_right = 465.0 + t_right * 59.0;
        let y_top = 404.0 - 7.0 * ((yf - 404.0) / 58.0).clamp(0.0, 1.0);
        let y_bot = 600.0 - 23.0 * ((x_right - 407.0) / 117.0);
        let _ = y_bot;
        for x in (x_left as usize)..=(x_right.ceil() as usize).min(W - 1) {
            let u = ((x as f32 - x_left) / (x_right - x_left).max(1.0)).clamp(0.0, 1.0);
            let y_top_u = 404.0 - 7.0 * u;
            let y_bot_u = 600.0 - 23.0 * u;
            if yf < y_top_u || yf > y_bot_u {
                continue;
            }
            let v = (yf - y_top_u) / (y_bot_u - y_top_u).max(1.0);
            let _ = y_top;
            let gu = (u * 5.0).fract();
            let gv = (v * 16.0).fract();
            let on_vline = gu < 0.045 || gu > 0.955;
            let on_hline = gv < 0.045 || gv > 0.955;
            let on_border = u > 0.94 || v < 0.015 || v > 0.965;
            if on_border {
                darken_pixel(&mut buf[y * W + x], 0.42);
            } else if on_vline || on_hline {
                darken_pixel(&mut buf[y * W + x], 0.68);
            }
        }
    }

    // 6. Center volleyball net pole (x = 393..407, y = 278..600)
    for y in 278..H {
        let half_w = if y < 285 {
            let dy = (285 - y) as f32 / 7.0;
            7.0 * (1.0 - dy * dy).max(0.0).sqrt()
        } else {
            7.0
        };
        let x0 = (400.0 - half_w).floor() as usize;
        let x1 = (400.0 + half_w).ceil() as usize;
        for x in x0..=x1 {
            let nx = ((x as f32 - 400.0) / 7.0).clamp(-1.0, 1.0);
            let shade = 152.0 - nx * 46.0 + (1.0 - (nx + 0.35).abs()).max(0.0) * 40.0;
            buf[y * W + x] = rgb(shade, shade, shade + 2.0);
        }
    }

    // Suppress unused helper warning if blend_pixel is only used in later assets
    let _ = blend_pixel;
    buf
}

#[inline]
fn dist_to_segment(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let vx = x1 - x0;
    let vy = y1 - y0;
    let wx = px - x0;
    let wy = py - y0;
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 1e-6 {
        ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = px - (x0 + t * vx);
    let dy = py - (y0 + t * vy);
    (dx * dx + dy * dy).sqrt()
}

fn dist_to_polylines(px: f32, py: f32, strokes: &[&[(f32, f32)]]) -> f32 {
    let mut best = 1e9_f32;
    for poly in strokes {
        if poly.len() == 1 {
            let (x0, y0) = poly[0];
            best = best.min(((px - x0).powi(2) + (py - y0).powi(2)).sqrt());
        } else {
            for win in poly.windows(2) {
                best = best.min(dist_to_segment(
                    px, py, win[0].0, win[0].1, win[1].0, win[1].1,
                ));
            }
        }
    }
    best
}

fn glyph_strokes(idx: usize) -> &'static [&'static [(f32, f32)]] {
    match idx {
        // 0..=9
        0 => &[&[(12.0, 3.0), (17.0, 4.5), (18.5, 11.5), (15.5, 19.0), (10.5, 20.5), (6.5, 18.0), (6.0, 10.5), (8.5, 4.5), (12.0, 3.0)]],
        1 => &[&[(9.0, 7.5), (14.5, 3.5), (11.0, 20.5)]],
        2 => &[&[(8.0, 6.5), (11.5, 3.5), (16.5, 4.0), (17.5, 8.0), (13.5, 12.5), (6.0, 20.0), (16.5, 20.0)]],
        3 => &[&[(8.5, 4.5), (14.5, 3.5), (17.5, 6.5), (14.5, 10.5), (10.5, 11.2)], &[(14.5, 10.5), (17.0, 14.5), (13.5, 19.5), (7.5, 20.0), (5.5, 17.5)]],
        4 => &[&[(15.5, 20.0), (16.5, 3.5), (6.5, 14.5), (18.5, 14.5)]],
        5 => &[&[(17.5, 4.0), (9.5, 4.0), (8.5, 10.5), (14.5, 10.5), (17.0, 14.0), (14.0, 19.5), (8.0, 20.0), (5.5, 18.0)]],
        6 => &[&[(16.5, 4.0), (11.0, 5.0), (7.0, 11.5), (6.5, 17.5), (10.5, 20.5), (15.5, 18.5), (16.5, 14.0), (12.5, 11.5), (7.5, 13.0)]],
        7 => &[&[(7.0, 4.0), (18.0, 4.0), (8.5, 20.5)], &[(9.5, 11.5), (15.5, 11.5)]],
        8 => &[&[(12.5, 11.2), (17.0, 8.5), (16.5, 4.5), (12.0, 3.5), (8.5, 6.5), (10.5, 10.5), (12.5, 11.2), (16.5, 14.5), (14.5, 19.5), (9.5, 20.5), (6.0, 17.5), (7.5, 13.5), (12.5, 11.2)]],
        9 => &[&[(17.0, 11.0), (12.0, 13.0), (8.0, 10.5), (9.0, 5.5), (13.5, 3.5), (17.5, 6.0), (16.5, 13.5), (12.0, 19.5), (7.0, 20.5)]],
        // A..Z (10..=35)
        10 => &[&[(5.0, 20.5), (14.5, 3.5), (17.5, 20.5)], &[(8.5, 14.5), (16.5, 14.5)]],
        11 => &[&[(6.0, 20.0), (10.0, 4.0), (16.0, 4.5), (17.0, 8.5), (13.5, 11.5), (8.5, 11.5)], &[(13.5, 11.5), (16.5, 14.5), (14.5, 19.5), (6.0, 20.0)]],
        12 => &[&[(18.0, 6.0), (14.0, 3.5), (9.0, 6.0), (6.5, 13.0), (7.5, 19.0), (12.5, 20.5), (16.5, 18.0)]],
        13 => &[&[(6.0, 20.0), (10.0, 4.0), (15.5, 4.5), (18.0, 9.5), (16.5, 16.5), (11.5, 20.0), (6.0, 20.0)]],
        14 => &[&[(17.5, 4.0), (9.5, 4.0), (6.0, 20.0), (15.0, 20.0)], &[(8.0, 11.5), (14.5, 11.5)]],
        15 => &[&[(17.5, 4.0), (9.5, 4.0), (6.0, 20.0)], &[(8.0, 11.5), (14.5, 11.5)]],
        16 => &[&[(17.5, 6.0), (13.5, 3.5), (8.5, 6.5), (6.5, 13.5), (8.0, 19.5), (13.5, 20.0), (16.5, 16.5), (17.0, 12.5), (12.5, 12.5)]],
        17 => &[&[(6.0, 20.0), (10.0, 4.0)], &[(14.5, 20.0), (18.5, 4.0)], &[(8.0, 11.5), (16.5, 11.5)]],
        18 => &[&[(10.0, 20.0), (14.0, 4.0)]],
        19 => &[&[(16.5, 4.0), (14.0, 16.5), (10.5, 20.0), (6.5, 19.0)]],
        20 => &[&[(6.0, 20.0), (10.0, 4.0)], &[(17.5, 4.0), (8.0, 12.5), (15.5, 20.0)]],
        21 => &[&[(10.0, 4.0), (6.0, 20.0), (15.5, 20.0)]],
        22 => &[&[(4.5, 20.0), (8.5, 4.0), (11.5, 15.0), (18.0, 4.0), (15.5, 20.0)]],
        23 => &[&[(5.5, 20.0), (9.5, 4.0), (15.0, 20.0), (19.0, 4.0)]],
        24 => &[&[(12.5, 3.5), (17.5, 6.0), (17.0, 14.0), (12.5, 20.0), (7.5, 18.0), (7.0, 10.0), (12.5, 3.5)]],
        25 => &[&[(6.0, 20.0), (10.0, 4.0), (16.0, 4.5), (17.5, 8.5), (14.5, 12.5), (8.0, 12.5)]],
        26 => &[&[(12.5, 3.5), (17.5, 6.0), (17.0, 14.0), (12.5, 19.5), (7.5, 18.0), (7.0, 10.0), (12.5, 3.5)], &[(13.0, 16.0), (17.0, 21.0)]],
        27 => &[&[(6.0, 20.0), (10.0, 4.0), (16.0, 4.5), (17.5, 8.5), (14.5, 12.0), (8.0, 12.0)], &[(12.5, 12.0), (16.5, 20.0)]],
        28 => &[&[(17.0, 5.5), (13.0, 3.5), (9.0, 6.0), (10.5, 10.0), (15.5, 13.0), (15.0, 18.0), (10.5, 20.5), (6.5, 18.5)]],
        29 => &[&[(7.5, 4.0), (18.5, 4.0)], &[(13.0, 4.0), (9.5, 20.0)]],
        30 => &[&[(9.0, 4.0), (6.5, 16.0), (9.5, 20.0), (14.0, 19.5), (17.5, 4.0)]],
        31 => &[&[(8.0, 4.0), (10.0, 20.0), (18.0, 4.0)]],
        32 => &[&[(6.0, 4.0), (7.5, 20.0), (12.0, 9.5), (14.5, 20.0), (19.0, 4.0)]],
        33 => &[&[(7.5, 4.0), (16.0, 20.0)], &[(17.5, 4.0), (6.0, 20.0)]],
        34 => &[&[(8.0, 4.0), (11.5, 12.5), (18.0, 4.0)], &[(11.5, 12.5), (9.0, 20.0)]],
        35 => &[&[(8.0, 4.0), (18.0, 4.0), (6.5, 20.0), (16.5, 20.0)]],
        // 36..=50: . ! ( ) ß Ä Ö Ü ' : ; ? , / -
        36 => &[&[(9.0, 19.0)]],
        37 => &[&[(13.5, 4.0), (11.5, 14.5)], &[(10.5, 19.0)]],
        38 => &[&[(15.0, 3.0), (11.0, 8.0), (9.5, 14.0), (11.5, 20.5)]],
        39 => &[&[(12.5, 3.0), (14.5, 8.5), (13.0, 15.0), (9.5, 20.5)]],
        40 => &[&[(6.5, 20.0), (10.0, 5.0), (14.5, 3.5), (17.0, 6.5), (13.5, 10.5), (16.5, 14.5), (14.5, 19.5), (10.0, 19.5)]],
        41 => &[&[(5.5, 20.5), (14.5, 5.5), (17.0, 20.5)], &[(8.5, 15.0), (16.0, 15.0)], &[(11.5, 2.5)], &[(16.0, 2.5)]],
        42 => &[&[(12.5, 5.5), (17.0, 8.0), (16.5, 15.0), (12.0, 20.0), (7.5, 18.0), (7.0, 11.0), (12.5, 5.5)], &[(11.0, 2.5)], &[(15.5, 2.5)]],
        43 => &[&[(9.0, 6.0), (6.5, 16.5), (9.5, 20.0), (14.0, 19.5), (17.5, 6.0)], &[(11.0, 2.5)], &[(15.5, 2.5)]],
        44 => &[&[(13.5, 3.0), (12.0, 7.5)]],
        45 => &[&[(13.0, 9.0)], &[(11.0, 18.0)]],
        46 => &[&[(13.0, 9.0)], &[(11.0, 17.5), (9.5, 20.5)]],
        47 => &[&[(9.5, 6.5), (13.0, 3.5), (17.0, 5.5), (16.0, 9.5), (12.0, 13.5)], &[(11.0, 18.5)]],
        48 => &[&[(11.5, 17.5), (10.0, 21.0)]],
        49 => &[&[(16.5, 3.5), (7.5, 20.5)]],
        50 => &[&[(8.5, 12.5), (15.5, 12.5)]],
        _ => &[],
    }
}

/// Generates all 55 font glyphs (24x24) procedurally:
/// - 0..=50: beveled italic glyphs with colorful plasma gradients
/// - 51..=52: grayscale 24x24 Blobby player icons (for P1/P2 color tinting)
/// - 53..=54: rainbow up/down arrows ('²' and '³')
pub fn generate_font_glyphs(blobby_base0: &BmpImage) -> Vec<BmpImage> {
    const W: usize = 24;
    const H: usize = 24;
    let mut glyphs = Vec::with_capacity(55);

    for idx in 0..55 {
        let mut pixels = vec![0u32; W * H];
        if idx <= 50 {
            let strokes = glyph_strokes(idx);
            let phase = idx as f32 * 0.73;
            let eval_d = |px: f32, py: f32| {
                let sx = 12.0 + (px - 12.0) / 1.25;
                dist_to_polylines(sx, py, strokes) - 1.55
            };
            for by in 0..H {
                let py = (H - 1 - by) as f32 + 0.5;
                for x in 0..W {
                    let px = x as f32 + 0.5;
                    let d = eval_d(px, py);
                    if d > 0.35 {
                        continue;
                    }
                    let cov = (1.0 - (d + 0.35) / 0.70).clamp(0.0, 1.0);
                    let gx = eval_d(px + 0.5, py) - eval_d(px - 0.5, py);
                    let gy = eval_d(px, py + 0.5) - eval_d(px, py - 0.5);
                    let bevel = ((-gx - gy) * 0.45).clamp(-0.30, 0.45);

                    let u = px / 24.0;
                    let v = py / 24.0;
                    let r = 140.0 + 115.0 * (u * 2.4 + v * 1.4 + phase).sin();
                    let g = 140.0 + 115.0 * (u * 1.9 - v * 2.2 + phase * 1.3 + 2.1).sin();
                    let b = 140.0 + 115.0 * (-u * 2.1 + v * 1.8 + phase * 0.8 + 4.2).sin();
                    let lit = (0.90 + bevel) * cov;
                    let col = rgb(
                        r * lit + bevel.max(0.0) * 110.0,
                        g * lit + bevel.max(0.0) * 110.0,
                        b * lit + bevel.max(0.0) * 110.0,
                    );
                    let sum = (col & 0xFF) + ((col >> 8) & 0xFF) + ((col >> 16) & 0xFF);
                    if sum >= 0x28 {
                        pixels[by * W + x] = col;
                    }
                }
            }
        } else if idx == 51 || idx == 52 {
            // Downsample 148x101 blobby_base[0] (bounds x=41..106, y=12..101 -> ~65x89) into 24x24
            for by in 0..H {
                let sy = ((by as f32 + 0.5) / H as f32 * 90.0) as usize;
                let src_y = sy.min(blobby_base0.height - 1);
                for x in 0..W {
                    let sx = 37 + ((x as f32 + 0.5) / W as f32 * 74.0) as usize;
                    let p = blobby_base0.pixels[src_y * blobby_base0.width + sx.min(blobby_base0.width - 1)];
                    let sum = (p & 0xFF) + ((p >> 8) & 0xFF) + ((p >> 16) & 0xFF);
                    if sum >= 0x28 {
                        pixels[by * W + x] = p;
                    }
                }
            }
        } else {
            // 53: Up arrow, 54: Down arrow
            let is_up = idx == 53;
            for by in 0..H {
                let py = (H - 1 - by) as f32 + 0.5;
                let y_dir = if is_up { py } else { 24.0 - py };
                for x in 0..W {
                    let px = x as f32 + 0.5;
                    let dx = (px - 12.0).abs();
                    let in_head = (2.0..=11.0).contains(&y_dir) && dx <= (y_dir - 2.0) * 1.1;
                    let in_shaft = (11.0..=21.5).contains(&y_dir) && dx <= 4.0;
                    if in_head || in_shaft {
                        let t = py / 24.0;
                        let (r, g, b) = if is_up {
                            (255.0 * (1.0 - t * 0.5), 40.0 + 60.0 * (1.0 - t), 40.0 + 215.0 * t)
                        } else {
                            (220.0 * (1.0 - t), 220.0 * t, 255.0 * (1.0 - (t - 0.5).abs() * 1.5).max(0.2))
                        };
                        pixels[by * W + x] = rgb(r, g, b);
                    }
                }
            }
        }
        glyphs.push(BmpImage {
            width: W,
            height: H,
            pixels,
        });
    }
    glyphs
}
