use crate::render::Sprite;

/// Generates all 16 rotation frames (64x64) of the volleyball procedurally.
///
/// Uses a unit-sphere shading model with wrap diffuse lighting from the top-left
/// and an 18-panel seam pattern rotated by `22.5` degrees per frame, matching the
/// original `sum < 0xB4` transparency threshold.
pub fn generate_ball_frames() -> Vec<Sprite> {
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

        // Store bottom-up (row 0 = bottom scanline) to match Sprite convention.
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
                let val = (shade * cushion * (1.0 - 0.17 * seam))
                    .round()
                    .clamp(0.0, 255.0) as u32;
                if val >= 60 {
                    pixels[by * W + x] = (val << 16) | (val << 8) | val;
                }
            }
        }
        frames.push(Sprite {
            width: W,
            height: H,
            pixels,
        });
    }
    frames
}

#[inline]
fn sd_ellipsoid(p: [f32; 3], c: [f32; 3], r: [f32; 3]) -> f32 {
    let qx = (p[0] - c[0]) / r[0];
    let qy = (p[1] - c[1]) / r[1];
    let qz = (p[2] - c[2]) / r[2];
    let k0 = (qx * qx + qy * qy + qz * qz).sqrt();
    let k1 = ((qx / r[0]).powi(2) + (qy / r[1]).powi(2) + (qz / r[2]).powi(2)).sqrt();
    if k1 > 1e-6 {
        k0 * (k0 - 1.0) / k1
    } else {
        -r[0].min(r[1]).min(r[2])
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
pub fn generate_blobby_frames() -> Vec<Sprite> {
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
            let d1 = sd_ellipsoid(
                [px, py, pz],
                [CX, head_cy, 0.0],
                [head_rx, head_ry, head_rx],
            );
            let d2 = sd_ellipsoid(
                [px, py, pz],
                [CX, body_cy, 0.0],
                [body_rx, body_ry, body_rx],
            );
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
        frames.push(Sprite {
            width: W,
            height: H,
            pixels,
        });
    }
    frames
}

/// Generates the 69x17 checkerboard-dithered ball shadow mask.
pub fn generate_ball_shadow() -> Sprite {
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
    Sprite {
        width: W,
        height: H,
        pixels,
    }
}

/// Generates all 10 checkerboard-dithered Blobby shadow masks (129x25)
/// from the 5 base Blobby silhouettes (0..5 for P1, 5..10 mirrored for P2).
pub fn generate_blobby_shadows(blobby_base: &[Sprite]) -> Vec<Sprite> {
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
            shadows.push(Sprite {
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
        let len = frond_len * (0.85 + 0.25 * ((f as f32 * 2.1 + seed).sin() * 0.5 + 0.5));
        let droop = 26.0 + 20.0 * (1.0 - dir_y.abs());

        let spine_steps = 58;
        for s in 2..=spine_steps {
            let t = s as f32 / spine_steps as f32;
            let sx = tx + dir_x * len * t;
            let sy = ty + dir_y * len * t + droop * t * t;
            let leaf_len = (1.0 - (t - 0.35).abs() * 1.1).clamp(0.18, 1.0) * (len * 0.30);
            let shade = if dir_y < 0.0 { 0.95 } else { 0.68 };

            draw_disc(buf, w, h, sx, sy, 1.4, rgb(10.0 * shade, 58.0 * shade, 6.0));

            // Leaflets on both sides of spine
            let perp_x = -dir_y;
            let perp_y = dir_x;
            for side in [-1.0_f32, 1.0_f32] {
                let lx_dir = (perp_x * side * 0.80 + dir_x * 0.50) * leaf_len;
                let ly_dir = (perp_y * side * 0.55 + dir_y * 0.35 + 0.50) * leaf_len;
                let l_steps = 20;
                for ls in 1..=l_steps {
                    let lt = ls as f32 / l_steps as f32;
                    let px = sx + lx_dir * lt;
                    let py = sy + ly_dir * lt + 4.0 * lt * lt;
                    let ix = px.round() as i32;
                    let iy = py.round() as i32;
                    if (0..w as i32).contains(&ix) && (0..h as i32).contains(&iy) {
                        let g = (42.0 + 78.0 * (1.0 - lt * 0.45) * shade).clamp(0.0, 255.0);
                        let r = (5.0 + 22.0 * (1.0 - lt) * shade).clamp(0.0, 255.0);
                        buf[iy as usize * w + ix as usize] = rgb(r, g, 5.0);
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
            let n = fbm(x as f32 * 0.045, y as f32 * 0.12, 3);
            let streak = fbm(x as f32 * 0.09, y as f32 * 0.03, 2);
            let falloff = (1.0 - r).powf(1.5);
            let mask = (falloff * (0.45 + 0.55 * n) * (0.65 + 0.35 * streak)).clamp(0.0, 1.0);
            if mask > 0.12 {
                darken_pixel(&mut buf[y * w + x], 1.0 - 0.36 * mask);
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
        344.0 - 16.0 * (x / 620.0).clamp(0.0, 1.0)
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
                // Deep cobalt sky with wispy clouds weighted toward horizon & right
                let t = yf / 285.0;
                let cloud = fbm(xf * 0.007 + 2.1, yf * 0.022 + 0.4, 4);
                let region = ((xf / 800.0) * 0.40 + t * 0.60).clamp(0.05, 1.0)
                    * ((yf - 25.0) / 80.0).clamp(0.0, 1.0);
                let wisp = ((cloud - 0.50) * 2.6 * region).clamp(0.0, 1.0);
                let r = 23.0 + t * 56.0 + wisp * 102.0;
                let g = 66.0 + t * 48.0 + wisp * 92.0;
                let b = 227.0 + t * 5.0 + wisp * 24.0;
                rgb(r, g, b)
            } else if yf < sh_y {
                // Ocean with crisp 1px horizon and bright cobalt water
                let depth = ((yf - 285.0) / (sh_y - 285.0).max(1.0)).clamp(0.0, 1.0);
                let ripple = fbm(xf * 0.055, yf * 0.42, 3);
                let glint = if ripple > 0.72 && depth > 0.25 {
                    (ripple - 0.72) * 220.0
                } else {
                    0.0
                };
                let horiz = if yf < 286.0 { 0.88 } else { 1.0 };
                let surf = if depth > 0.82 {
                    (depth - 0.82) / 0.18
                } else {
                    0.0
                };
                let r = (84.0 + depth * 18.0 + surf * 42.0 + (ripple - 0.5) * 30.0 + glint) * horiz;
                let g =
                    (106.0 + depth * 18.0 + surf * 36.0 + (ripple - 0.5) * 34.0 + glint) * horiz;
                let b = (244.0 + depth * 4.0 - surf * 48.0 + (ripple - 0.5) * 14.0 + glint * 0.4)
                    * horiz;
                rgb(r, g, b)
            } else {
                // Sandy beach with subtle shading and grain
                let sand_t = ((yf - sh_y) / (600.0 - sh_y)).clamp(0.0, 1.0);
                let grain = (hash2(x as i32, y as i32) - 0.5) * 10.0;
                let dune = (fbm(xf * 0.006, yf * 0.012, 3) - 0.5) * 12.0;
                let r = 171.0 + sand_t * 14.0 + grain + dune - (1.0 - xf / 800.0) * 14.0;
                let g = 159.0 + sand_t * 12.0 + grain + dune - (1.0 - xf / 800.0) * 14.0;
                let b = 84.0 + sand_t * 6.0 + grain * 0.7 + dune * 0.5;
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
        } else if (224..=258).contains(&x) {
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
            let lit = if x > 224 {
                0.85 + 0.25 * (241.0 - xf) / 17.0
            } else {
                0.9
            };
            let cr = (62.0 + stria * 58.0) * lit;
            let cg = (54.0 + stria * 52.0) * lit;
            let cb = (48.0 + stria * 46.0) * lit;
            buf[y * W + x] = rgb(cr, cg, cb);
        }
    }

    // 3. Palm shadows on sand
    draw_palm_shadow(&mut buf, W, H, 105.0, 386.0, 155.0, 34.0);
    draw_palm_shadow(&mut buf, W, H, 605.0, 356.0, 120.0, 26.0);

    // 4. Palm trees (2 left, 3 right)
    draw_palm_tree(
        &mut buf, W, H, 62.0, 385.0, 46.0, 225.0, 32.0, 78.0, 7.0, 3.0, 74.0, 0.2,
    );
    draw_palm_tree(
        &mut buf, W, H, 14.0, 338.0, 74.0, 220.0, 150.0, 125.0, 6.5, 2.8, 72.0, 0.7,
    );
    draw_palm_tree(
        &mut buf, W, H, 600.0, 344.0, 588.0, 268.0, 582.0, 185.0, 5.2, 2.2, 50.0, 0.4,
    );
    draw_palm_tree(
        &mut buf, W, H, 620.0, 340.0, 636.0, 240.0, 655.0, 148.0, 5.8, 2.4, 58.0, 1.1,
    );
    draw_palm_tree(
        &mut buf, W, H, 732.0, 342.0, 736.0, 260.0, 742.0, 168.0, 5.2, 2.2, 50.0, 0.8,
    );

    // 5. Net shadow on sand (perspective quad from (407, 370)..(442, 368) to (407, 600)..(498, 590))
    for y in 368..H {
        let yf = y as f32;
        let x_left = 407.0;
        let t_right = (yf - 368.0) / 222.0;
        let x_right = 442.0 + t_right * 56.0;
        for x in (x_left as usize)..=(x_right.ceil() as usize).min(W - 1) {
            let u = ((x as f32 - x_left) / (x_right - x_left).max(1.0)).clamp(0.0, 1.0);
            let y_top_u = 370.0 - 2.0 * u;
            let y_bot_u = 600.0 - 10.0 * u;
            if yf < y_top_u || yf > y_bot_u {
                continue;
            }
            let v = (yf - y_top_u) / (y_bot_u - y_top_u).max(1.0);
            let gu = (u * 5.0).fract();
            let gv = (v * 18.0).fract();
            let on_vline = gu < 0.032 || gu > 0.968;
            let on_hline = gv < 0.032 || gv > 0.968;
            let on_border = u > 0.955 || v < 0.012 || v > 0.975;
            if on_border {
                darken_pixel(&mut buf[y * W + x], 0.42);
            } else if on_vline || on_hline {
                darken_pixel(&mut buf[y * W + x], 0.68);
            }
        }
    }

    // 6. Center volleyball net pole (x = 393..407, y = 264..600)
    // Top cap centered at top-down y=271 (bottom-up y=328, matching sim.rs net cap at y=329, r=5).
    for y in 264..H {
        let half_w = if y < 271 {
            let dy = (271 - y) as f32 / 7.0;
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

/// Blobby Volley 2 bitmap glyphs (`0..=49` and `52` for `'-'`, authored by Silvio Mummert;
/// see `assets/bv2_font/NOTICE.md` for provenance and GPL-2.0 licensing details).
const BV2_FONT_BMPS: [&[u8]; 51] = [
    include_bytes!("../assets/bv2_font/font00.bmp"),
    include_bytes!("../assets/bv2_font/font01.bmp"),
    include_bytes!("../assets/bv2_font/font02.bmp"),
    include_bytes!("../assets/bv2_font/font03.bmp"),
    include_bytes!("../assets/bv2_font/font04.bmp"),
    include_bytes!("../assets/bv2_font/font05.bmp"),
    include_bytes!("../assets/bv2_font/font06.bmp"),
    include_bytes!("../assets/bv2_font/font07.bmp"),
    include_bytes!("../assets/bv2_font/font08.bmp"),
    include_bytes!("../assets/bv2_font/font09.bmp"),
    include_bytes!("../assets/bv2_font/font10.bmp"),
    include_bytes!("../assets/bv2_font/font11.bmp"),
    include_bytes!("../assets/bv2_font/font12.bmp"),
    include_bytes!("../assets/bv2_font/font13.bmp"),
    include_bytes!("../assets/bv2_font/font14.bmp"),
    include_bytes!("../assets/bv2_font/font15.bmp"),
    include_bytes!("../assets/bv2_font/font16.bmp"),
    include_bytes!("../assets/bv2_font/font17.bmp"),
    include_bytes!("../assets/bv2_font/font18.bmp"),
    include_bytes!("../assets/bv2_font/font19.bmp"),
    include_bytes!("../assets/bv2_font/font20.bmp"),
    include_bytes!("../assets/bv2_font/font21.bmp"),
    include_bytes!("../assets/bv2_font/font22.bmp"),
    include_bytes!("../assets/bv2_font/font23.bmp"),
    include_bytes!("../assets/bv2_font/font24.bmp"),
    include_bytes!("../assets/bv2_font/font25.bmp"),
    include_bytes!("../assets/bv2_font/font26.bmp"),
    include_bytes!("../assets/bv2_font/font27.bmp"),
    include_bytes!("../assets/bv2_font/font28.bmp"),
    include_bytes!("../assets/bv2_font/font29.bmp"),
    include_bytes!("../assets/bv2_font/font30.bmp"),
    include_bytes!("../assets/bv2_font/font31.bmp"),
    include_bytes!("../assets/bv2_font/font32.bmp"),
    include_bytes!("../assets/bv2_font/font33.bmp"),
    include_bytes!("../assets/bv2_font/font34.bmp"),
    include_bytes!("../assets/bv2_font/font35.bmp"),
    include_bytes!("../assets/bv2_font/font36.bmp"),
    include_bytes!("../assets/bv2_font/font37.bmp"),
    include_bytes!("../assets/bv2_font/font38.bmp"),
    include_bytes!("../assets/bv2_font/font39.bmp"),
    include_bytes!("../assets/bv2_font/font40.bmp"),
    include_bytes!("../assets/bv2_font/font41.bmp"),
    include_bytes!("../assets/bv2_font/font42.bmp"),
    include_bytes!("../assets/bv2_font/font43.bmp"),
    include_bytes!("../assets/bv2_font/font44.bmp"),
    include_bytes!("../assets/bv2_font/font45.bmp"),
    include_bytes!("../assets/bv2_font/font46.bmp"),
    include_bytes!("../assets/bv2_font/font47.bmp"),
    include_bytes!("../assets/bv2_font/font48.bmp"),
    include_bytes!("../assets/bv2_font/font49.bmp"),
    include_bytes!("../assets/bv2_font/font52.bmp"),
];

fn decode_bv2_font_bmp(bmp: &[u8]) -> Vec<u32> {
    let off = u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]) as usize;
    let mut pixels = vec![0u32; 24 * 24];
    for (i, px) in pixels.iter_mut().enumerate() {
        let p = off + i * 3;
        let b = bmp[p] as u32;
        let g = bmp[p + 1] as u32;
        let r = bmp[p + 2] as u32;
        if r + g + b >= 0x28 {
            *px = (r << 16) | (g << 8) | b;
        }
    }
    pixels
}

/// Generates all 55 font glyphs (24x24):
/// - 0..=50: embedded Blobby Volley 2 bitmap glyphs (Silvio Mummert)
/// - 51..=52: grayscale 24x24 Blobby player icons (for P1/P2 color tinting)
/// - 53..=54: rainbow up/down arrows ('²' and '³')
pub fn generate_font_glyphs(blobby_base0: &Sprite) -> Vec<Sprite> {
    const W: usize = 24;
    const H: usize = 24;
    let mut glyphs = Vec::with_capacity(55);

    for idx in 0..55 {
        let mut pixels = if let Some(&bmp) = BV2_FONT_BMPS.get(idx) {
            decode_bv2_font_bmp(bmp)
        } else {
            vec![0u32; W * H]
        };
        if idx <= 50 {
            // Decoded above from embedded BV2 BMPs.
        } else if idx == 51 || idx == 52 {
            // Downsample 148x101 blobby_base[0] into 24x24 (bounds x=3..20, y=0..23)
            for by in 0..H {
                let sy = ((by as f32 + 0.5) / H as f32 * 86.0) as usize;
                let src_y = sy.min(blobby_base0.height - 1);
                for x in 0..W {
                    let sx = 33 + ((x as f32 + 0.5) / W as f32 * 82.0) as usize;
                    let p = blobby_base0.pixels
                        [src_y * blobby_base0.width + sx.min(blobby_base0.width - 1)];
                    let sum = (p & 0xFF) + ((p >> 8) & 0xFF) + ((p >> 16) & 0xFF);
                    if sum >= 0x28 {
                        pixels[by * W + x] = p;
                    }
                }
            }
        } else {
            // 53: Up arrow, 54: Down arrow (exact 24x24 silhouette: 12-row head + 10-row shaft)
            let is_up = idx == 53;
            for by in 0..H {
                let top_y = H - 1 - by;
                let dir_y = if is_up { top_y } else { by };
                let hw = if (1..=12).contains(&dir_y) {
                    dir_y
                } else if (13..=22).contains(&dir_y) {
                    6
                } else {
                    0
                };
                if hw == 0 {
                    continue;
                }
                let v = top_y as f32 / 24.0;
                for x in (12 - hw)..(12 + hw) {
                    let u = x as f32 / 24.0;
                    let (r, g, b) = if is_up {
                        let a_rg = -0.5 * (u + v);
                        let a_b = -6.0 * u - 7.0 * v;
                        (
                            -3449.3 - 1434.4 * a_rg.sin() + 3423.9 * a_rg.cos(),
                            1759.5 + 983.0 * a_rg.sin() - 1465.7 * a_rg.cos(),
                            140.6 - 99.4 * a_b.sin() + 3.3 * a_b.cos(),
                        )
                    } else {
                        let a_r = -2.0 * u - 7.0 * v;
                        let a_g = -4.5 * u - 8.0 * v;
                        let a_b = -4.0 * u - 5.5 * v;
                        (
                            135.9 - 106.8 * a_r.sin() - 59.6 * a_r.cos(),
                            93.0 - 75.4 * a_g.sin() + 42.0 * a_g.cos(),
                            160.9 - 43.5 * a_b.sin() - 102.4 * a_b.cos(),
                        )
                    };
                    pixels[by * W + x] = rgb(r, g, b);
                }
            }
        }
        glyphs.push(Sprite {
            width: W,
            height: H,
            pixels,
        });
    }
    glyphs
}

#[inline]
fn point_in_triangle_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    let d1 = dist_to_segment(px, py, a.0, a.1, b.0, b.1);
    let d2 = dist_to_segment(px, py, b.0, b.1, c.0, c.1);
    let d3 = dist_to_segment(px, py, c.0, c.1, a.0, a.1);
    let edge = d1.min(d2).min(d3);
    let s1 = (px - b.0) * (a.1 - b.1) - (a.0 - b.0) * (py - b.1);
    let s2 = (px - c.0) * (b.1 - c.1) - (b.0 - c.0) * (py - c.1);
    let s3 = (px - a.0) * (c.1 - a.1) - (c.0 - a.0) * (py - a.1);
    let inside = (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0);
    if inside {
        -edge
    } else {
        edge
    }
}

/// Generates the 48x48 beveled green pointer cursor.
pub fn generate_cursor() -> Sprite {
    const W: usize = 48;
    const H: usize = 48;
    let mut pixels = vec![0u32; W * H];

    let cursor_sdf = |px: f32, py: f32| -> f32 {
        // Arrow head: tip (3.0, 0.5), right wing (33.0, 16.5), left wing (13.5, 32.5)
        let d_head = point_in_triangle_dist(px, py, (3.0, 0.5), (33.0, 16.5), (13.5, 32.5));
        // Flat-ended rectangular stem along (20.5, 21.5) -> (39.5, 44.5), half-width 4.3
        let (ax, ay) = (20.5_f32, 21.5_f32);
        let (bx, by) = (39.5_f32, 44.5_f32);
        let (vx, vy) = (bx - ax, by - ay);
        let len = (vx * vx + vy * vy).sqrt();
        let (ux, uy) = (vx / len, vy / len);
        let (rx, ry) = (px - ax, py - ay);
        let along = rx * ux + ry * uy;
        let perp = (rx * -uy + ry * ux).abs();
        let d_along = (-along).max(along - len);
        let d_perp = perp - 4.3;
        let d_stem = d_along.max(0.0).hypot(d_perp.max(0.0)) + d_along.max(d_perp).min(0.0);
        d_head.min(d_stem)
    };

    for by in 0..H {
        let py = (H - 1 - by) as f32 + 0.5;
        for x in 0..W {
            let px = x as f32 + 0.5;
            let d = cursor_sdf(px, py);
            if d > 0.6 {
                continue;
            }
            let cov = (1.0 - (d + 0.2) / 0.8).clamp(0.0, 1.0);
            let gx = cursor_sdf(px + 0.5, py) - cursor_sdf(px - 0.5, py);
            let gy = cursor_sdf(px, py + 0.5) - cursor_sdf(px, py - 0.5);
            // Bright cream-yellow bevel along the upper-right face
            let bevel = ((gx - gy) * 0.72).clamp(-0.35, 1.0);
            let hi = (bevel.max(0.0)).powf(1.25);
            let mottle = fbm(px * 0.22, py * 0.22, 3);
            let rim = (1.0 - (-d / 3.8).clamp(0.0, 1.0)).powi(2);

            let r = (55.0 + mottle * 70.0 + hi * 175.0 + rim * 35.0) * cov;
            let g = (150.0 + mottle * 75.0 + hi * 95.0 + rim * 25.0) * cov;
            let b = (40.0 + mottle * 45.0 + hi * 140.0 + rim * 20.0) * cov;
            let col = rgb(r, g, b);
            if col != 0 {
                pixels[by * W + x] = col;
            }
        }
    }
    Sprite {
        width: W,
        height: H,
        pixels,
    }
}

fn title_word_dist(px: f32, py: f32) -> (f32, bool) {
    // Letter strokes fitted to the 11 segmented components of graph_89.png (410x230).
    let blobby_strokes: &[&[(f32, f32)]] = &[
        // B
        &[
            (21.0, 121.0),
            (33.0, 59.0),
            (48.0, 54.0),
            (58.0, 61.0),
            (56.0, 76.0),
            (29.0, 85.0),
        ],
        &[(58.0, 79.0), (70.0, 96.0), (52.0, 116.0), (19.0, 125.0)],
        // l
        &[(97.0, 43.0), (85.0, 110.0)],
        // o
        &[
            (124.0, 58.0),
            (132.0, 69.0),
            (126.0, 97.0),
            (109.0, 106.0),
            (103.0, 92.0),
            (111.0, 66.0),
            (124.0, 58.0),
        ],
        // b1
        &[(157.0, 28.0), (140.0, 97.0)],
        &[
            (147.0, 74.0),
            (168.0, 49.0),
            (178.0, 57.0),
            (164.0, 79.0),
            (145.0, 96.0),
        ],
        // b2
        &[(202.0, 17.0), (185.0, 87.0)],
        &[
            (190.0, 61.0),
            (217.0, 37.0),
            (222.0, 52.0),
            (204.0, 73.0),
            (191.0, 85.0),
        ],
        // y
        &[
            (236.0, 43.0),
            (228.0, 80.0),
            (248.0, 58.0),
            (262.0, 30.0),
            (248.0, 89.0),
            (236.0, 126.0),
        ],
    ];
    let d1 = dist_to_polylines(px, py, blobby_strokes);

    let volley_strokes: &[&[(f32, f32)]] = &[
        // V
        &[(185.0, 135.0), (207.0, 194.0), (230.0, 126.0)],
        // l1
        &[(296.0, 110.0), (284.0, 177.0)],
        // l2
        &[(316.0, 105.0), (305.0, 172.0)],
        // e
        &[
            (331.0, 150.0),
            (346.0, 135.0),
            (343.0, 123.0),
            (336.0, 126.0),
            (324.0, 146.0),
            (326.0, 170.0),
            (344.0, 156.0),
        ],
        // y
        &[
            (363.0, 123.0),
            (355.0, 158.0),
            (368.0, 151.0),
            (388.0, 115.0),
            (373.0, 175.0),
            (368.0, 194.0),
        ],
    ];
    let d2 = dist_to_polylines(px, py, volley_strokes);

    // Miniature volleyball at (251.6, 164.7) radius 24.3
    let d_ball = ((px - 251.6).powi(2) + (py - 164.7).powi(2)).sqrt() - 24.3;
    (d1.min(d2).min(d_ball + 6.0), d_ball <= 0.0)
}

/// Generates the 410x230 "Blobby Volley" title logo with bumpy blue outline,
/// speckled green lettering, and embedded miniature volleyball.
pub fn generate_title_logo() -> Sprite {
    const W: usize = 410;
    const H: usize = 230;
    let mut pixels = vec![0u32; W * H];

    for by in 0..H {
        let py = (H - 1 - by) as f32 + 0.5;
        for x in 0..W {
            let px = x as f32 + 0.5;
            let (d, in_ball) = title_word_dist(px, py);

            if in_ball {
                // Miniature volleyball inside the 'o' of "Volley", rotated ~20 deg
                let nx = (px - 251.6) / 24.3;
                let ny = (164.7 - py) / 24.3;
                let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
                let ndotl = (nx * -0.40 + ny * 0.85 + nz * 0.34 + 0.4).max(0.0) / 1.4;
                let shade = 105.0 + 150.0 * ndotl;
                let rx = nx * 0.94 - ny * 0.34;
                let ry = nx * 0.34 + ny * 0.94;
                let seam_d = (ry.abs() - 0.55)
                    .abs()
                    .min((rx - 0.18).abs())
                    .min((ry - 0.18).abs());
                let seam = (-(seam_d / 0.07).powi(2)).exp();
                let v = (shade * (1.0 - 0.16 * seam)).clamp(40.0, 255.0);
                pixels[by * W + x] = rgb(v, v, v);
                continue;
            }

            if d > 18.7 {
                continue;
            }
            let speckle = (hash2(x as i32, y_td(by, H) as i32) - 0.5) * 16.0;
            let bump = (fbm(px * 0.14, py * 0.14, 2) - 0.5) * 28.0;

            let rim = ((4.5 - d) / 1.9).clamp(0.0, 1.0);
            let hi = ((2.6 - d) / 2.0).clamp(0.0, 1.0) * (speckle.max(0.0) * 1.4);
            let gr = (34.0 + rim * 52.0 + hi + speckle * 0.5).clamp(0.0, 255.0);
            let gg = (122.0 + rim * 94.0 + hi * 0.8 + speckle).clamp(0.0, 255.0);
            let gb = (34.0 + rim * 50.0 + hi * 0.7 + speckle * 0.4).clamp(0.0, 255.0);

            let t = ((18.7 - d) / 14.2).clamp(0.0, 1.0);
            let br = (52.0 + t * 56.0 + bump * 0.6 + speckle * 0.7).clamp(0.0, 255.0);
            let bg = (46.0 + t * 64.0 + bump * 0.7 + speckle * 0.7).clamp(0.0, 255.0);
            let bb = (185.0 + t * 70.0 + bump * 0.5 + speckle * 0.4).clamp(0.0, 255.0);

            // Smooth teal/cyan transition between inner green letter and outer blue bubble
            let blend = ((d - 3.9) / 1.2).clamp(0.0, 1.0);
            let mut r = gr * (1.0 - blend) + br * blend;
            let mut g = gg * (1.0 - blend) + bg * blend;
            let mut b = gb * (1.0 - blend) + bb * blend;

            // Desaturated blue-gray outer rim matching BV1.8's sum >= 142 colorkey contract
            let outer = ((d - 17.4) / 1.3).clamp(0.0, 1.0);
            r = r * (1.0 - outer) + (54.0 + speckle * 0.4) * outer;
            g = g * (1.0 - outer) + (58.0 + speckle * 0.4) * outer;
            b = b * (1.0 - outer) + (82.0 + speckle * 0.5) * outer;
            pixels[by * W + x] = rgb(r, g, b);
        }
    }
    Sprite {
        width: W,
        height: H,
        pixels,
    }
}

#[inline]
fn y_td(by: usize, h: usize) -> usize {
    h - 1 - by
}

const ARIMO_REGULAR_TTF: &[u8] = include_bytes!("../assets/arimo/Arimo.ttf");
const ARIMO_ITALIC_TTF: &[u8] = include_bytes!("../assets/arimo/Arimo-Italic.ttf");

/// Separable Gaussian blur over a top-down `w x h` mask in `0.0..=1.0`.
fn gaussian_blur_f32(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma <= 0.05 {
        return src.to_vec();
    }
    let radius = (sigma * 3.0).ceil() as isize;
    let mut kernel = Vec::with_capacity((radius * 2 + 1) as usize);
    let mut sum = 0.0_f32;
    let inv2s2 = 1.0 / (2.0 * sigma * sigma);
    for k in -radius..=radius {
        let v = (-(k * k) as f32 * inv2s2).exp();
        kernel.push(v);
        sum += v;
    }
    for v in &mut kernel {
        *v /= sum;
    }

    let mut tmp = vec![0.0_f32; w * h];
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let mut acc = 0.0_f32;
            for (ki, &kv) in kernel.iter().enumerate() {
                let sx = (x as isize + ki as isize - radius).clamp(0, w as isize - 1) as usize;
                acc += src[row + sx] * kv;
            }
            tmp[row + x] = acc;
        }
    }

    let mut dst = vec![0.0_f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0_f32;
            for (ki, &kv) in kernel.iter().enumerate() {
                let sy = (y as isize + ki as isize - radius).clamp(0, h as isize - 1) as usize;
                acc += tmp[sy * w + x] * kv;
            }
            dst[y * w + x] = acc;
        }
    }
    dst
}

/// Rasterizes a text line with `swash` into a tight buffer, scales horizontally to `target_w`,
/// and composites its max coverage into the top-down `dst` (`w x h`) mask at `(dst_x, dst_y)`.
#[allow(clippy::too_many_arguments)]
fn draw_arimo_line_into(
    dst: &mut [f32],
    w: usize,
    h: usize,
    shape_ctx: &mut swash::shape::ShapeContext,
    scale_ctx: &mut swash::scale::ScaleContext,
    font_bytes: &[u8],
    px_size: f32,
    weight: f32,
    dst_x: isize,
    dst_y: isize,
    target_w: usize,
    text: &str,
) {
    let font =
        swash::FontRef::from_index(font_bytes, 0).expect("embedded Arimo font must be valid");
    let mut shaper = shape_ctx
        .builder(font)
        .size(px_size)
        .variations(&[("wght", weight)])
        .build();
    shaper.add_str(text);

    let mut glyphs = Vec::new();
    shaper.shape_with(|cluster| {
        for g in cluster.glyphs {
            glyphs.push((g.id, g.x, g.y, g.advance));
        }
    });

    let mut scaler = scale_ctx
        .builder(font)
        .size(px_size)
        .variations(&[("wght", weight)])
        .build();

    let mut placed = Vec::new();
    let mut pen_x = 0.0_f32;
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for (gid, gx, gy, adv) in glyphs {
        if let Some(img) = swash::scale::Render::new(&[swash::scale::Source::Outline])
            .format(swash::zeno::Format::Alpha)
            .render(&mut scaler, gid)
        {
            let x = (pen_x + gx).round() as i32 + img.placement.left;
            let y = (-gy).round() as i32 - img.placement.top;
            let gw = img.placement.width as i32;
            let gh = img.placement.height as i32;
            if gw > 0 && gh > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + gw);
                max_y = max_y.max(y + gh);
                placed.push((x, y, gw as usize, gh as usize, img.data));
            }
        }
        pen_x += adv;
    }

    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let bw = (max_x - min_x) as usize;
    let bh = (max_y - min_y) as usize;
    let mut line_buf = vec![0.0_f32; bw * bh];
    for (gx, gy, gw, gh, data) in placed {
        let ox = (gx - min_x) as usize;
        let oy = (gy - min_y) as usize;
        for row in 0..gh {
            for col in 0..gw {
                let a = data[row * gw + col] as f32 / 255.0;
                let idx = (oy + row) * bw + (ox + col);
                line_buf[idx] = line_buf[idx].max(a);
            }
        }
    }

    if target_w == 0 {
        return;
    }
    let scale_x = bw as f32 / target_w as f32;
    for row in 0..bh {
        let py = dst_y + row as isize;
        if py < 0 || py >= h as isize {
            continue;
        }
        for tx in 0..target_w {
            let px = dst_x + tx as isize;
            if px < 0 || px >= w as isize {
                continue;
            }
            let sx = (tx as f32 + 0.5) * scale_x - 0.5;
            let x0 = sx.floor() as isize;
            let frac = sx - x0 as f32;
            let s0 = if x0 >= 0 && (x0 as usize) < bw {
                line_buf[row * bw + x0 as usize]
            } else {
                0.0
            };
            let s1 = if x0 + 1 >= 0 && ((x0 + 1) as usize) < bw {
                line_buf[row * bw + (x0 + 1) as usize]
            } else {
                0.0
            };
            let cov = s0 * (1.0 - frac) + s1 * frac;
            let idx = (py as usize) * w + (px as usize);
            dst[idx] = dst[idx].max(cov);
        }
    }
}

/// Generates the 400x300 woven-paper ending credits splash card.
pub fn generate_ending_splash() -> Sprite {
    const W: usize = 400;
    const H: usize = 300;
    const TAU: f32 = std::f32::consts::TAU;
    let mut pixels = vec![0u32; W * H];

    for by in 0..H {
        let y = H - 1 - by;
        let yf = y as f32;
        for x in 0..W {
            if x == 0 || x == W - 1 || y == 0 || y == H - 1 {
                pixels[by * W + x] = rgb(12.0, 13.0, 13.0);
                continue;
            }
            let xf = x as f32;
            // Smooth washed-out beach backdrop (left cliff/sand warm shadow, bright central sky, right 3 palm crowns)
            let cliff = (1.0 - ((xf - 108.0) / 52.0).clamp(0.0, 1.0)) * 15.0;
            let right_palms = {
                let mut p = 0.0_f32;
                for &(px, py, pr) in &[
                    (288.0_f32, 106.0_f32, 34.0_f32),
                    (325.0, 90.0, 38.0),
                    (368.0, 100.0, 34.0),
                ] {
                    let dx = xf - px;
                    let dy = yf - py;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < pr {
                        let ang = dy.atan2(dx);
                        let frond = 0.55 + 0.45 * (ang * 7.0).cos().abs();
                        p = p.max((1.0 - d / pr) * frond * 12.0);
                    }
                }
                p
            };
            let base = 236.0 - cliff - right_palms;

            // Coherent rotated halftone screen matching graph.pak entry 90
            let v1 = (TAU * (0.26855 * xf - 0.09277 * yf) + 2.2906).cos();
            let v2 = (TAU * (0.08691 * xf + 0.28711 * yf) - 1.4885).cos();
            let screen = (18.0 * v1 + 16.0 * v2).min(10.0);

            let lum = (base + screen).clamp(8.0, 246.0);
            pixels[by * W + x] = rgb(lum, lum - 1.5, lum - 2.2);
        }
    }

    let mut raw_fg = vec![0.0_f32; W * H];
    let mut shape_ctx = swash::shape::ShapeContext::new();
    let mut scale_ctx = swash::scale::ScaleContext::new();

    type CreditLine<'a> = (&'a [u8], f32, f32, isize, isize, usize, &'a str);
    let lines: &[CreditLine<'_>] = &[
        (ARIMO_ITALIC_TTF, 38.0, 450.0, 13, 13, 216, "Blobby Volley"),
        (
            ARIMO_ITALIC_TTF,
            30.0,
            550.0,
            26,
            94,
            230,
            "programmiert von",
        ),
        (
            ARIMO_ITALIC_TTF,
            31.0,
            450.0,
            93,
            131,
            279,
            "Daniel Skoraszewsky",
        ),
        (ARIMO_ITALIC_TTF, 30.0, 450.0, 29, 170, 131, "Grafik von"),
        (
            ARIMO_ITALIC_TTF,
            31.0,
            500.0,
            100,
            207,
            205,
            "Silvio Mummert",
        ),
        (
            ARIMO_REGULAR_TTF,
            15.0,
            600.0,
            23,
            263,
            334,
            "www.t-online.de/home/skoraszewsky/",
        ),
        (
            ARIMO_REGULAR_TTF,
            15.0,
            600.0,
            23,
            282,
            360,
            "www.t-online.de/home/mummertathome/",
        ),
    ];
    for &(font_bytes, px_size, weight, x, y, target_w, text) in lines {
        draw_arimo_line_into(
            &mut raw_fg,
            W,
            H,
            &mut shape_ctx,
            &mut scale_ctx,
            font_bytes,
            px_size,
            weight,
            x,
            y,
            target_w,
            text,
        );
    }

    let fg_blur = gaussian_blur_f32(&raw_fg, W, H, 0.75);
    let glow_blur = gaussian_blur_f32(&raw_fg, W, H, 1.9);
    let mut shifted = vec![0.0_f32; W * H];
    for y in 2..H {
        for x in 2..W {
            shifted[y * W + x] = raw_fg[(y - 2) * W + (x - 2)];
        }
    }
    let shadow_blur = gaussian_blur_f32(&shifted, W, H, 1.6);

    for y in 0..H {
        let by = H - 1 - y;
        for x in 0..W {
            let td = y * W + x;
            let idx = by * W + x;
            let ga = (glow_blur[td] * 0.45).clamp(0.0, 1.0);
            let sa = (shadow_blur[td] * 0.50).clamp(0.0, 1.0);
            let dark_factor = (1.0 - ga) * (1.0 - sa);
            if dark_factor < 1.0 {
                darken_pixel(&mut pixels[idx], dark_factor);
            }
            let fa = fg_blur[td].clamp(0.0, 1.0);
            if fa > 0.0 {
                blend_pixel(&mut pixels[idx], 38.0, 41.0, 45.0, fa);
            }
        }
    }

    Sprite {
        width: W,
        height: H,
        pixels,
    }
}

fn encode_wav_u8_mono(sample_rate: u32, samples: &[u8]) -> Vec<u8> {
    let data_len = samples.len() as u32;
    let riff_len = 36 + data_len;
    let mut wav = Vec::with_capacity(44 + samples.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes()); // byte rate (8-bit mono)
    wav.extend_from_slice(&1u16.to_le_bytes()); // block align
    wav.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(samples);
    wav
}

fn symmetric_noise(idx: i32) -> f32 {
    hash2(idx, 7) * 2.0 - 1.0
}

/// Synthesizes the referee whistle WAV (44.1 kHz, 8-bit mono, 20000 samples).
/// Matches the original two-tone beat whistle (~2526.5 Hz + ~2667.0 Hz) with
/// hard clipping at [-128, 127], producing the ~140.5 Hz pea-whistle warble.
/// Note: the original `sound_00.wav` starts at full amplitude and ends mid-beat
/// without an attack/release taper, which this matches directly.
pub fn generate_whistle_wav() -> Vec<u8> {
    const SR: u32 = 44100;
    const N: usize = 20000;
    let mut samples = Vec::with_capacity(N);
    for i in 0..N {
        let t = i as f32 / SR as f32;
        let s1 = (t * 2526.5 * std::f32::consts::TAU).sin();
        let s2 = (t * 2667.0 * std::f32::consts::TAU).sin();
        let sig = (125.0 * (s1 + s2)).clamp(-128.0, 127.0);
        samples.push((128.0 + sig).round().clamp(0.0, 255.0) as u8);
    }
    encode_wav_u8_mono(SR, &samples)
}

/// Synthesizes the ball hit / menu click WAV (44.1 kHz, 8-bit mono, 2408 samples).
/// Combines a ~1 ms soft pre-attack, a damped dual-mode volleyball thump (95 Hz + 186 Hz),
/// and a fast-decaying ~2.35/2.74 kHz impact snap with symmetric zero-mean air noise.
pub fn generate_hit_wav() -> Vec<u8> {
    const SR: u32 = 44100;
    const N: usize = 2408;
    const HIT_START: f32 = 46.0 / SR as f32;
    // Fitted modal table (freq Hz, decay s^-1, amplitude, phase rad) from sound_01.wav
    const MODES: [(f32, f32, f32, f32); 9] = [
        (105.0, 48.0, 38.0, 2.284),
        (190.0, 75.0, 22.0, -2.053),
        (318.0, 95.0, 13.0, 2.745),
        (565.0, 120.0, 12.0, -2.141),
        (1470.0, 180.0, 14.5, 0.102),
        (1810.0, 210.0, 18.0, -1.525),
        (2164.0, 230.0, 49.73, 0.371),
        (2379.0, 240.0, 62.52, -2.743),
        (2660.0, 260.0, 37.91, 2.091),
    ];
    let mut samples = Vec::with_capacity(N);
    for i in 0..N {
        let t = i as f32 / SR as f32;
        let val = if t < HIT_START {
            14.0 * (t * 2088.7 * std::f32::consts::TAU).sin()
                * (std::f32::consts::PI * t / HIT_START).sin()
        } else {
            let th = t - HIT_START;
            let mut sum = symmetric_noise(i as i32) * 14.0 * (-th * 130.0).exp();
            for &(freq, damp, amp, phase) in &MODES {
                sum += amp * (th * freq * std::f32::consts::TAU + phase).sin() * (-th * damp).exp();
            }
            sum
        };
        samples.push((128.0 + val.clamp(-128.0, 127.0)).round().clamp(0.0, 255.0) as u8);
    }
    encode_wav_u8_mono(SR, &samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_noise_is_bounded_and_near_zero_mean() {
        let n = 4096;
        let mut sum = 0.0_f64;
        for i in 0..n {
            let v = symmetric_noise(i);
            assert!((-1.0..=1.0).contains(&v), "noise out of [-1, 1]: {v}");
            sum += v as f64;
        }
        let mean = sum / n as f64;
        assert!(mean.abs() < 0.03, "noise DC bias too large: {mean}");
    }

    #[test]
    fn wav_generators_produce_valid_pcm_without_dc_bias() {
        for (wav, expected_samples) in [(generate_whistle_wav(), 20000), (generate_hit_wav(), 2408)]
        {
            assert_eq!(&wav[0..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
            assert_eq!(wav.len(), 44 + expected_samples);
            let mean: f64 =
                wav[44..].iter().map(|&b| b as f64 - 128.0).sum::<f64>() / expected_samples as f64;
            assert!(mean.abs() < 2.0, "WAV DC offset too large: {mean}");
        }
    }

    #[test]
    fn ending_splash_has_expected_dimensions_border_and_text_bands() {
        let a = generate_ending_splash();
        let b = generate_ending_splash();
        assert_eq!((a.width, a.height), (400, 300));
        assert_eq!(a.pixels, b.pixels);

        let luma = |px: u32| -> f64 {
            let r = ((px >> 16) & 0xFF) as f64;
            let g = ((px >> 8) & 0xFF) as f64;
            let b = (px & 0xFF) as f64;
            0.299 * r + 0.587 * g + 0.114 * b
        };

        // Dark 1px frame
        assert!(luma(a.pixels[0]) < 20.0);
        assert!(luma(a.pixels[a.width * a.height - 1]) < 20.0);

        // Text-free band (top-down y=55..88 -> bottom-up by=212..245) should stay bright
        let mut blank_sum = 0.0;
        let mut blank_n = 0usize;
        for y in 55..88 {
            let by = a.height - 1 - y;
            for x in 10..390 {
                blank_sum += luma(a.pixels[by * a.width + x]);
                blank_n += 1;
            }
        }
        let blank_mean = blank_sum / blank_n as f64;
        assert!(
            (200.0..=235.0).contains(&blank_mean),
            "unexpected blank mean {blank_mean}"
        );

        // Each of the 7 credit/URL bands must contain dark ink pixels (< 140 luma)
        for (y0, y1) in [
            (12, 50),
            (92, 124),
            (130, 161),
            (168, 194),
            (206, 233),
            (262, 277),
            (280, 294),
        ] {
            let mut dark_cnt = 0usize;
            for y in y0..=y1 {
                let by = a.height - 1 - y;
                for x in 10..390 {
                    if luma(a.pixels[by * a.width + x]) < 140.0 {
                        dark_cnt += 1;
                    }
                }
            }
            assert!(
                dark_cnt > 500,
                "missing text ink in band {y0}..={y1}: {dark_cnt}"
            );
        }
    }

    #[test]
    fn embedded_arimo_fonts_parse_and_shape_credit_lines() {
        let mut shape_ctx = swash::shape::ShapeContext::new();
        let credit_lines: &[(&str, &[u8], f32, f32, &str)] = &[
            (
                "Arimo Italic",
                ARIMO_ITALIC_TTF,
                38.0,
                450.0,
                "Blobby Volley",
            ),
            (
                "Arimo Italic",
                ARIMO_ITALIC_TTF,
                30.0,
                550.0,
                "programmiert von",
            ),
            (
                "Arimo Italic",
                ARIMO_ITALIC_TTF,
                31.0,
                450.0,
                "Daniel Skoraszewsky",
            ),
            ("Arimo Italic", ARIMO_ITALIC_TTF, 30.0, 450.0, "Grafik von"),
            (
                "Arimo Italic",
                ARIMO_ITALIC_TTF,
                31.0,
                500.0,
                "Silvio Mummert",
            ),
            (
                "Arimo Regular",
                ARIMO_REGULAR_TTF,
                15.0,
                600.0,
                "www.t-online.de/home/skoraszewsky/",
            ),
            (
                "Arimo Regular",
                ARIMO_REGULAR_TTF,
                15.0,
                600.0,
                "www.t-online.de/home/mummertathome/",
            ),
        ];
        for &(label, bytes, px_size, weight, text) in credit_lines {
            let font = swash::FontRef::from_index(bytes, 0)
                .unwrap_or_else(|| panic!("{label} failed to parse"));
            let mut shaper = shape_ctx
                .builder(font)
                .size(px_size)
                .variations(&[("wght", weight)])
                .build();
            shaper.add_str(text);
            let mut glyph_count = 0usize;
            let mut advance = 0.0_f32;
            shaper.shape_with(|cluster| {
                glyph_count += cluster.glyphs.len();
                for g in cluster.glyphs {
                    advance += g.advance;
                }
            });
            assert!(
                glyph_count > 0 && advance > 0.0,
                "{label} failed to shape '{text}'"
            );
        }
    }
}
