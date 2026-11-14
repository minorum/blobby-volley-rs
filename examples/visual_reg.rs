#[path = "common/mod.rs"]
mod pak_ref;

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use blobby_volley_rs::assets::{tint_pixel, GameAssets, BLOBBY_COLORS};
use blobby_volley_rs::procedural::{
    generate_background, generate_ball_frames, generate_ball_shadow, generate_blobby_frames,
    generate_blobby_shadows, generate_cursor, generate_ending_splash, generate_font_glyphs,
    generate_hit_wav, generate_title_logo, generate_whistle_wav,
};
use blobby_volley_rs::render::{Framebuffer, Sprite, HEIGHT, WIDTH};
use blobby_volley_rs::sim::{MatchState, PlayerControl, PlayerInput};
use pak_ref::{load_pak_file, parse_bmp, BmpImage};

struct Metrics {
    fg_mae_rgb: [f64; 3],
    fg_mae: f64,
    psnr: f64,
    iou: f64,
    boundary_f1: f64,
    ssim: f64,
}

fn ssim_local_luma(a: &[u32], b: &[u32], w: usize, h: usize) -> f64 {
    let luma = |p: u32| -> f64 {
        let r = ((p >> 16) & 0xFF) as f64;
        let g = ((p >> 8) & 0xFF) as f64;
        let bl = (p & 0xFF) as f64;
        0.299 * r + 0.587 * g + 0.114 * bl
    };
    let la: Vec<f64> = a.iter().map(|&p| luma(p)).collect();
    let lb: Vec<f64> = b.iter().map(|&p| luma(p)).collect();
    let win = 8usize.min(w).min(h);
    let step = if w * h > 50_000 { 4 } else { 2 };
    let c1 = (0.01_f64 * 255.0).powi(2);
    let c2 = (0.03_f64 * 255.0).powi(2);
    let mut sum_ssim = 0.0;
    let mut count = 0usize;
    let wn = (win * win) as f64;

    for y0 in (0..=h.saturating_sub(win)).step_by(step) {
        for x0 in (0..=w.saturating_sub(win)).step_by(step) {
            let (mut sa, mut sb) = (0.0, 0.0);
            for dy in 0..win {
                let row = (y0 + dy) * w + x0;
                for dx in 0..win {
                    sa += la[row + dx];
                    sb += lb[row + dx];
                }
            }
            let mu_a = sa / wn;
            let mu_b = sb / wn;
            let (mut va, mut vb, mut cov) = (0.0, 0.0, 0.0);
            for dy in 0..win {
                let row = (y0 + dy) * w + x0;
                for dx in 0..win {
                    let da = la[row + dx] - mu_a;
                    let db = lb[row + dx] - mu_b;
                    va += da * da;
                    vb += db * db;
                    cov += da * db;
                }
            }
            va /= wn;
            vb /= wn;
            cov /= wn;
            let s = ((2.0 * mu_a * mu_b + c1) * (2.0 * cov + c2))
                / ((mu_a * mu_a + mu_b * mu_b + c1) * (va + vb + c2));
            sum_ssim += s;
            count += 1;
        }
    }
    if count > 0 { sum_ssim / count as f64 } else { 1.0 }
}

fn boundary_f1(ref_px: &[u32], proc_px: &[u32], w: usize, h: usize) -> f64 {
    let is_edge = |px: &[u32], x: usize, y: usize| -> bool {
        if px[y * w + x] == 0 {
            return false;
        }
        x == 0 || x + 1 == w || y == 0 || y + 1 == h
            || px[y * w + x - 1] == 0
            || px[y * w + x + 1] == 0
            || px[(y - 1) * w + x] == 0
            || px[(y + 1) * w + x] == 0
    };
    let mut ref_edges = vec![false; w * h];
    let mut proc_edges = vec![false; w * h];
    let (mut r_cnt, mut p_cnt) = (0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if is_edge(ref_px, x, y) {
                ref_edges[y * w + x] = true;
                r_cnt += 1;
            }
            if is_edge(proc_px, x, y) {
                proc_edges[y * w + x] = true;
                p_cnt += 1;
            }
        }
    }
    if r_cnt == 0 && p_cnt == 0 {
        return 1.0;
    }
    if r_cnt == 0 || p_cnt == 0 {
        return 0.0;
    }
    let has_near = |grid: &[bool], x: usize, y: usize| -> bool {
        let y0 = y.saturating_sub(1);
        let y1 = (y + 1).min(h - 1);
        let x0 = x.saturating_sub(1);
        let x1 = (x + 1).min(w - 1);
        (y0..=y1).any(|yy| (x0..=x1).any(|xx| grid[yy * w + xx]))
    };
    let mut tp_r = 0usize;
    let mut tp_p = 0usize;
    for y in 0..h {
        for x in 0..w {
            if ref_edges[y * w + x] && has_near(&proc_edges, x, y) {
                tp_r += 1;
            }
            if proc_edges[y * w + x] && has_near(&ref_edges, x, y) {
                tp_p += 1;
            }
        }
    }
    let rec = tp_r as f64 / r_cnt as f64;
    let prec = tp_p as f64 / p_cnt as f64;
    if rec + prec > 0.0 { 2.0 * rec * prec / (rec + prec) } else { 0.0 }
}

fn compare_images(ref_px: &[u32], proc_px: &[u32], w: usize, h: usize, has_alpha: bool) -> Metrics {
    assert_eq!(ref_px.len(), w * h);
    assert_eq!(proc_px.len(), w * h);
    let mut fg_abs = [0u64; 3];
    let mut sq_sum = 0u64;
    let mut inter = 0u64;
    let mut union = 0u64;

    for (&rp, &pp) in ref_px.iter().zip(proc_px) {
        let r_fg = !has_alpha || rp != 0;
        let p_fg = !has_alpha || pp != 0;
        if r_fg && p_fg {
            inter += 1;
        }
        if r_fg || p_fg {
            union += 1;
            for (c_idx, shift) in [16, 8, 0].iter().enumerate() {
                let rc = ((rp >> shift) & 0xFF) as i64;
                let pc = ((pp >> shift) & 0xFF) as i64;
                let d = (rc - pc).abs();
                fg_abs[c_idx] += d as u64;
                sq_sum += (d * d) as u64;
            }
        }
    }

    let denom = (union.max(1) * 3) as f64;
    let fg_mae_rgb = [
        fg_abs[0] as f64 / union.max(1) as f64,
        fg_abs[1] as f64 / union.max(1) as f64,
        fg_abs[2] as f64 / union.max(1) as f64,
    ];
    let fg_mae = (fg_abs[0] + fg_abs[1] + fg_abs[2]) as f64 / denom;
    let mse = sq_sum as f64 / denom;
    let psnr = if mse > 0.0 {
        10.0 * (255.0 * 255.0 / mse).log10()
    } else {
        f64::INFINITY
    };
    let iou = if union > 0 {
        inter as f64 / union as f64
    } else {
        1.0
    };
    let bf1 = if has_alpha { boundary_f1(ref_px, proc_px, w, h) } else { 1.0 };
    let ssim = ssim_local_luma(ref_px, proc_px, w, h);
    Metrics {
        fg_mae_rgb,
        fg_mae,
        psnr,
        iou,
        boundary_f1: bf1,
        ssim,
    }
}

fn save_side_by_side_bu(path: &PathBuf, w: usize, h: usize, ref_px: &[u32], proc_px: &[u32]) {
    let out_w = w * 3;
    let mut ppm = format!("P6\n{out_w} {h}\n255\n").into_bytes();
    for y in (0..h).rev() {
        for x in 0..w {
            let p = ref_px[y * w + x];
            ppm.push(((p >> 16) & 0xFF) as u8);
            ppm.push(((p >> 8) & 0xFF) as u8);
            ppm.push((p & 0xFF) as u8);
        }
        for x in 0..w {
            let p = proc_px[y * w + x];
            ppm.push(((p >> 16) & 0xFF) as u8);
            ppm.push(((p >> 8) & 0xFF) as u8);
            ppm.push((p & 0xFF) as u8);
        }
        for x in 0..w {
            let rp = ref_px[y * w + x];
            let pp = proc_px[y * w + x];
            for shift in [16, 8, 0] {
                let rc = ((rp >> shift) & 0xFF) as i32;
                let pc = ((pp >> shift) & 0xFF) as i32;
                ppm.push(((rc - pc).abs() * 4).min(255) as u8);
            }
        }
    }
    fs::write(path, ppm).unwrap();
}

fn to_sprite(b: BmpImage) -> Sprite {
    Sprite {
        width: b.width,
        height: b.height,
        pixels: b.pixels,
    }
}

fn build_ref_assets(graph: &[Vec<u8>]) -> GameAssets {
    let mut assets = GameAssets::generate();
    let bg_img = parse_bmp(&graph[0]).unwrap();
    for y in 0..600 {
        assets.bg[(599 - y) * 800..(600 - y) * 800]
            .copy_from_slice(&bg_img.pixels[y * 800..(y + 1) * 800]);
    }
    assets.bg_dark = assets.bg.iter().map(|&p| (p & 0x00F8F8F8) >> 3).collect();

    for (k, entry) in graph[1..=16].iter().enumerate() {
        let mut img = to_sprite(parse_bmp(entry).unwrap());
        for p in &mut img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0xB4 {
                *p = 0;
            } else if *p == 0 {
                *p = 1;
            }
        }
        assets.ball_frames[k] = img;
    }
    for (k, entry) in graph[17..=21].iter().enumerate() {
        assets.blobby_base[k] = to_sprite(parse_bmp(entry).unwrap());
    }
    for (k, entry) in graph[22..=76].iter().enumerate() {
        let mut img = to_sprite(parse_bmp(entry).unwrap());
        for p in &mut img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            }
        }
        assets.font_glyphs[k] = img;
    }
    assets.font_icon_base = assets.font_glyphs[0x33].clone();
    for (k, entry) in graph[77..=86].iter().enumerate() {
        let mut img = to_sprite(parse_bmp(entry).unwrap());
        for p in &mut img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            *p = if sum < 0x28 { 0 } else { (*p & 0x00FCFCFC) >> 1 };
        }
        assets.blobby_shadows[k] = img;
    }
    let mut bsh = to_sprite(parse_bmp(&graph[87]).unwrap());
    for p in &mut bsh.pixels {
        let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
        *p = if sum < 0x28 { 0 } else { (*p & 0x00FCFCFC) >> 2 };
    }
    assets.ball_shadow = bsh;
    assets.cursor = to_sprite(parse_bmp(&graph[88]).unwrap());
    assets.title_logo = to_sprite(parse_bmp(&graph[89]).unwrap());
    assets.ending_splash = to_sprite(parse_bmp(&graph[90]).unwrap());
    assets.recolor_player(0, 0);
    assets.recolor_player(1, 3);
    assets
}

fn flip_td_to_bu(td: &[u32], w: usize, h: usize) -> Vec<u32> {
    let mut bu = vec![0u32; w * h];
    for y in 0..h {
        bu[(h - 1 - y) * w..(h - y) * w].copy_from_slice(&td[y * w..(y + 1) * w]);
    }
    bu
}

struct WavData {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    samples: Vec<f64>,
}

fn parse_wav(wav: &[u8]) -> Result<WavData, String> {
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("Not a RIFF/WAVE file".to_string());
    }
    let riff_sz = u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize;
    let riff_end = 8usize.checked_add(riff_sz).ok_or("RIFF size overflow")?;
    if riff_end > wav.len() {
        return Err("Truncated RIFF container".to_string());
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data_bytes: Option<&[u8]> = None;
    while pos + 8 <= riff_end {
        let id = &wav[pos..pos + 4];
        let sz = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body_start = pos + 8;
        let body_end = body_start.checked_add(sz).ok_or("Chunk size overflow")?;
        if body_end > riff_end {
            return Err("Truncated WAV chunk".to_string());
        }
        if id == b"fmt " {
            if sz < 16 || body_end < body_start + 16 {
                return Err("Truncated fmt chunk".to_string());
            }
            let audio_fmt = u16::from_le_bytes([wav[body_start], wav[body_start + 1]]);
            let channels = u16::from_le_bytes([wav[body_start + 2], wav[body_start + 3]]);
            let sr = u32::from_le_bytes(wav[body_start + 4..body_start + 8].try_into().unwrap());
            let bps = u16::from_le_bytes([wav[body_start + 14], wav[body_start + 15]]);
            fmt = Some((audio_fmt, channels, sr, bps));
        } else if id == b"data" {
            data_bytes = Some(&wav[body_start..body_end]);
        }
        pos = body_end.saturating_add(sz & 1);
    }
    let (audio_fmt, channels, sample_rate, bits_per_sample) =
        fmt.ok_or_else(|| "Missing fmt chunk".to_string())?;
    if audio_fmt != 1 || channels != 1 || bits_per_sample != 8 {
        return Err(format!("Unsupported WAV format: fmt={audio_fmt} ch={channels} bps={bits_per_sample}"));
    }
    let raw = data_bytes.ok_or_else(|| "Missing data chunk".to_string())?;
    let samples = raw.iter().map(|&b| b as f64 - 128.0).collect();
    Ok(WavData {
        sample_rate,
        channels,
        bits_per_sample,
        samples,
    })
}

fn pearson_corr(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let ma = a[..n].iter().sum::<f64>() / n as f64;
    let mb = b[..n].iter().sum::<f64>() / n as f64;
    let (mut num, mut va, mut vb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let da = a[i] - ma;
        let db = b[i] - mb;
        num += da * db;
        va += da * da;
        vb += db * db;
    }
    if va > 0.0 && vb > 0.0 { num / (va * vb).sqrt() } else { 0.0 }
}

fn compare_audio(name: &str, ref_bytes: &[u8], proc_bytes: &[u8]) {
    let r = parse_wav(ref_bytes).unwrap();
    let p = parse_wav(proc_bytes).unwrap();
    assert_eq!(r.sample_rate, p.sample_rate);
    assert_eq!(r.channels, p.channels);
    assert_eq!(r.bits_per_sample, p.bits_per_sample);

    let rms = |s: &[f64]| (s.iter().map(|&x| x * x).sum::<f64>() / s.len() as f64).sqrt();
    let pk = |s: &[f64]| s.iter().map(|&x| x.abs()).fold(0.0_f64, f64::max);
    let dc = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;

    // 5 ms RMS envelope correlation
    let win = (r.sample_rate as usize / 200).max(1);
    let n_common = r.samples.len().min(p.samples.len());
    let env_r: Vec<f64> = r.samples[..n_common].chunks_exact(win).map(rms).collect();
    let env_p: Vec<f64> = p.samples[..n_common].chunks_exact(win).map(rms).collect();
    let env_corr = pearson_corr(&env_r, &env_p);

    // Best-lag aligned waveform correlation (-20..=20 samples)
    let mut best_wave_corr = -1.0_f64;
    for lag in -20isize..=20 {
        let (rs, ps) = if lag >= 0 {
            (&r.samples[lag as usize..n_common], &p.samples[..n_common - lag as usize])
        } else {
            (&r.samples[..n_common - (-lag) as usize], &p.samples[(-lag) as usize..n_common])
        };
        best_wave_corr = best_wave_corr.max(pearson_corr(rs, ps));
    }

    // Windowed time-frequency band relative magnitude error (4 time windows x 4 freq bands)
    let n_wins = 4usize;
    let win_len = (n_common / n_wins).max(64).min(512);
    let bands = [(0.0, 400.0), (400.0, 1500.0), (1500.0, 4000.0), (4000.0, 12000.0)];
    let mut band_num = [0.0_f64; 4];
    let mut band_den = [0.0_f64; 4];
    for w_idx in 0..n_wins {
        let start = w_idx * (n_common - win_len) / (n_wins - 1).max(1);
        for k in 1..(win_len / 2) {
            let freq = k as f64 * r.sample_rate as f64 / win_len as f64;
            let Some(b_idx) = bands.iter().position(|&(lo, hi)| freq >= lo && freq < hi) else {
                continue;
            };
            let w = std::f64::consts::TAU * k as f64 / win_len as f64;
            let (mut rr, mut ri, mut pr, mut pi) = (0.0, 0.0, 0.0, 0.0);
            for i in 0..win_len {
                let (s, c) = (w * i as f64).sin_cos();
                rr += r.samples[start + i] * c;
                ri -= r.samples[start + i] * s;
                pr += p.samples[start + i] * c;
                pi -= p.samples[start + i] * s;
            }
            let mr = rr.hypot(ri);
            let mp = pr.hypot(pi);
            band_num[b_idx] += (mr - mp).abs();
            band_den[b_idx] += mr;
        }
    }
    let band_rel: Vec<f64> = (0..4)
        .map(|i| if band_den[i] > 1e-6 { band_num[i] / band_den[i] } else { 0.0 })
        .collect();
    let onset_lag = {
        let r_on = r.samples.iter().position(|x| x.abs() > 25.0).unwrap_or(0) as isize;
        let p_on = p.samples.iter().position(|x| x.abs() > 25.0).unwrap_or(0) as isize;
        p_on - r_on
    };

    println!(
        "{name:15}  ref=({} smp, {}Hz, rms={:.1}, pk={:.1}, dc={:+.2}) vs proc=({} smp, {}Hz, rms={:.1}, pk={:.1}, dc={:+.2}) | wave_r={:+.3}, env_r={:.3}, onset_lag={}smp, stft_rel_err=[{:.2},{:.2},{:.2},{:.2}]",
        r.samples.len(), r.sample_rate, rms(&r.samples), pk(&r.samples), dc(&r.samples),
        p.samples.len(), p.sample_rate, rms(&p.samples), pk(&p.samples), dc(&p.samples),
        best_wave_corr, env_corr, onset_lag, band_rel[0], band_rel[1], band_rel[2], band_rel[3]
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wav_rejects_truncated_fmt_chunk() {
        // Declared fmt chunk of 16 bytes, but only 8 bytes actually follow
        let mut bad = Vec::new();
        bad.extend_from_slice(b"RIFF");
        bad.extend_from_slice(&20u32.to_le_bytes());
        bad.extend_from_slice(b"WAVE");
        bad.extend_from_slice(b"fmt ");
        bad.extend_from_slice(&16u32.to_le_bytes());
        bad.extend_from_slice(&[0u8; 8]);
        assert!(parse_wav(&bad).is_err());
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let orig_dir = manifest_dir.join("original");
    let out_dir = manifest_dir.join("target/visual_reg");
    fs::create_dir_all(&out_dir).unwrap();

    let graph = load_pak_file(&orig_dir.join("graph.pak")).unwrap();
    let sound = load_pak_file(&orig_dir.join("sound.pak")).unwrap();

    // 1. Ball frames (1..=16)
    let proc_ball = generate_ball_frames();
    let (mut b_mae, mut b_iou, mut b_bf1, mut b_ssim) = (0.0, 0.0, 0.0, 0.0);
    for (k, proc_img) in proc_ball.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[1 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0xB4 {
                *p = 0;
            }
        }
        let m = compare_images(&ref_img.pixels, &proc_img.pixels, 64, 64, true);
        b_mae += m.fg_mae;
        b_iou += m.iou;
        b_bf1 += m.boundary_f1;
        b_ssim += m.ssim;
        if k < 4 {
            save_side_by_side_bu(&out_dir.join(format!("ball_{k:02}.ppm")), 64, 64, &ref_img.pixels, &proc_img.pixels);
        }
    }
    println!(
        "Ball (16f):      fg_MAE={:5.2}, IoU={:.4}, bF1={:.4}, SSIM={:.4}",
        b_mae / 16.0,
        b_iou / 16.0,
        b_bf1 / 16.0,
        b_ssim / 16.0
    );

    // 2. Blobby base frames (17..=21) + tinted P1 check
    let proc_blobby = generate_blobby_frames();
    let (mut bl_mae, mut bl_iou, mut bl_bf1, mut bl_ssim) = (0.0, 0.0, 0.0, 0.0);
    for (k, proc_img) in proc_blobby.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[17 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            } else {
                *p = tint_pixel(BLOBBY_COLORS[0], *p);
            }
        }
        let mut proc_tinted = proc_img.pixels.clone();
        for p in &mut proc_tinted {
            if *p != 0 {
                *p = tint_pixel(BLOBBY_COLORS[0], *p);
            }
        }
        let m = compare_images(&ref_img.pixels, &proc_tinted, 148, 101, true);
        bl_mae += m.fg_mae;
        bl_iou += m.iou;
        bl_bf1 += m.boundary_f1;
        bl_ssim += m.ssim;
        save_side_by_side_bu(&out_dir.join(format!("blobby_{k:02}.ppm")), 148, 101, &ref_img.pixels, &proc_tinted);
    }
    println!(
        "Blobby (5f,red): fg_MAE={:5.2}, IoU={:.4}, bF1={:.4}, SSIM={:.4}",
        bl_mae / 5.0,
        bl_iou / 5.0,
        bl_bf1 / 5.0,
        bl_ssim / 5.0
    );

    // 3. Shadows
    let proc_bshadows = generate_blobby_shadows(&proc_blobby);
    let mut bsh_iou = 0.0;
    for (k, proc_img) in proc_bshadows.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[77 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            *p = if sum < 0x28 { 0 } else { 0x00202020 };
        }
        bsh_iou += compare_images(&ref_img.pixels, &proc_img.pixels, 129, 25, true).iou;
    }
    let mut ref_ball_sh = parse_bmp(&graph[87]).unwrap();
    for p in &mut ref_ball_sh.pixels {
        let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
        *p = if sum < 0x28 { 0 } else { 0x00202020 };
    }
    let proc_ball_sh = generate_ball_shadow();
    let m_bsh = compare_images(&ref_ball_sh.pixels, &proc_ball_sh.pixels, 69, 17, true);
    println!(
        "Shadows:         blobby IoU={:.4}, ball IoU={:.4}",
        bsh_iou / 10.0,
        m_bsh.iou
    );

    // 4. Background (800x600)
    let ref_bg = parse_bmp(&graph[0]).unwrap();
    let proc_bg_bu = flip_td_to_bu(&generate_background(), 800, 600);
    let m_bg = compare_images(&ref_bg.pixels, &proc_bg_bu, 800, 600, false);
    save_side_by_side_bu(&out_dir.join("bg.ppm"), 800, 600, &ref_bg.pixels, &proc_bg_bu);
    println!(
        "Background:      MAE={:5.2} (RGB {:.1}/{:.1}/{:.1}), PSNR={:.2} dB, SSIM={:.4}",
        m_bg.fg_mae, m_bg.fg_mae_rgb[0], m_bg.fg_mae_rgb[1], m_bg.fg_mae_rgb[2], m_bg.psnr, m_bg.ssim
    );

    // 5. Font (55 glyphs)
    let proc_font = generate_font_glyphs(&proc_blobby[0]);
    let (mut f_mae, mut f_iou, mut f_bf1) = (0.0, 0.0, 0.0);
    for (k, proc_img) in proc_font.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[22 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            }
        }
        let m = compare_images(&ref_img.pixels, &proc_img.pixels, 24, 24, true);
        f_mae += m.fg_mae;
        f_iou += m.iou;
        f_bf1 += m.boundary_f1;
    }
    println!(
        "Font (55g):      fg_MAE={:5.2}, IoU={:.4}, bF1={:.4}",
        f_mae / 55.0,
        f_iou / 55.0,
        f_bf1 / 55.0
    );

    // 6. Cursor, Title logo, Ending splash
    let ref_cursor = parse_bmp(&graph[88]).unwrap();
    let proc_cursor = generate_cursor();
    let m_cur = compare_images(&ref_cursor.pixels, &proc_cursor.pixels, 48, 48, true);
    save_side_by_side_bu(&out_dir.join("cursor.ppm"), 48, 48, &ref_cursor.pixels, &proc_cursor.pixels);
    println!(
        "Cursor:          fg_MAE={:5.2}, IoU={:.4}, bF1={:.4}, SSIM={:.4}",
        m_cur.fg_mae, m_cur.iou, m_cur.boundary_f1, m_cur.ssim
    );

    let ref_logo = parse_bmp(&graph[89]).unwrap();
    let proc_logo = generate_title_logo();
    let m_logo = compare_images(&ref_logo.pixels, &proc_logo.pixels, 410, 230, true);
    save_side_by_side_bu(&out_dir.join("title_logo.ppm"), 410, 230, &ref_logo.pixels, &proc_logo.pixels);
    println!(
        "Title logo:      fg_MAE={:5.2}, IoU={:.4}, bF1={:.4}, SSIM={:.4}",
        m_logo.fg_mae, m_logo.iou, m_logo.boundary_f1, m_logo.ssim
    );

    let ref_end = parse_bmp(&graph[90]).unwrap();
    let proc_end = generate_ending_splash();
    let m_end = compare_images(&ref_end.pixels, &proc_end.pixels, 400, 300, false);
    save_side_by_side_bu(&out_dir.join("ending_splash.ppm"), 400, 300, &ref_end.pixels, &proc_end.pixels);
    println!(
        "Ending splash:   MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_end.fg_mae, m_end.psnr, m_end.ssim
    );

    // 7. Asset-composited gameplay & menu scenes (rendered via Framebuffer)
    let ref_assets = build_ref_assets(&graph);
    let proc_assets = GameAssets::generate();
    let mut fb_ref = Framebuffer::new();
    let mut fb_proc = Framebuffer::new();
    let mut m = MatchState::new([PlayerControl::AiMedium, PlayerControl::AiMedium], 1);
    let names = ["Player 1".to_string(), "Player 2".to_string()];

    fb_ref.draw_match(&ref_assets, &m, &names, 0);
    fb_proc.draw_match(&proc_assets, &m, &names, 0);
    let ref_match_bu = flip_td_to_bu(&fb_ref.pixels, WIDTH, HEIGHT);
    let proc_match_bu = flip_td_to_bu(&fb_proc.pixels, WIDTH, HEIGHT);
    let m_match = compare_images(&ref_match_bu, &proc_match_bu, WIDTH, HEIGHT, false);
    save_side_by_side_bu(&out_dir.join("scene_match.ppm"), WIDTH, HEIGHT, &ref_match_bu, &proc_match_bu);
    println!(
        "Asset-composited scene (serve): MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_match.fg_mae, m_match.psnr, m_match.ssim
    );

    for _ in 0..75 {
        m.step([PlayerInput::default(); 2], 0, false);
    }
    fb_ref.draw_match(&ref_assets, &m, &names, 75 * 16);
    fb_proc.draw_match(&proc_assets, &m, &names, 75 * 16);
    let ref_rally_bu = flip_td_to_bu(&fb_ref.pixels, WIDTH, HEIGHT);
    let proc_rally_bu = flip_td_to_bu(&fb_proc.pixels, WIDTH, HEIGHT);
    let m_rally = compare_images(&ref_rally_bu, &proc_rally_bu, WIDTH, HEIGHT, false);
    println!(
        "Asset-composited scene (rally): MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_rally.fg_mae, m_rally.psnr, m_rally.ssim
    );

    fb_ref.copy_bg(&ref_assets.bg_dark);
    fb_ref.draw_title_logo(&ref_assets.title_logo, 0);
    for (i, label) in ["Start Game", "Network Match", "Statistics", "Settings", "Francais", "Quit"].iter().enumerate() {
        fb_ref.draw_text(&ref_assets, 50, 260 - (i as i32) * 32, false, i == 0, label);
    }
    fb_ref.draw_cursor(&ref_assets.cursor, 200, 250);

    fb_proc.copy_bg(&proc_assets.bg_dark);
    fb_proc.draw_title_logo(&proc_assets.title_logo, 0);
    for (i, label) in ["Start Game", "Network Match", "Statistics", "Settings", "Francais", "Quit"].iter().enumerate() {
        fb_proc.draw_text(&proc_assets, 50, 260 - (i as i32) * 32, false, i == 0, label);
    }
    fb_proc.draw_cursor(&proc_assets.cursor, 200, 250);

    let ref_menu_bu = flip_td_to_bu(&fb_ref.pixels, WIDTH, HEIGHT);
    let proc_menu_bu = flip_td_to_bu(&fb_proc.pixels, WIDTH, HEIGHT);
    let m_menu = compare_images(&ref_menu_bu, &proc_menu_bu, WIDTH, HEIGHT, false);
    save_side_by_side_bu(&out_dir.join("scene_menu.ppm"), WIDTH, HEIGHT, &ref_menu_bu, &proc_menu_bu);
    println!(
        "Asset-composited scene (menu):  MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_menu.fg_mae, m_menu.psnr, m_menu.ssim
    );

    // 8. Audio WAV comparison
    let proc_w = generate_whistle_wav();
    let proc_h = generate_hit_wav();
    compare_audio("Audio whistle:", &sound[0], &proc_w);
    compare_audio("Audio hit:", &sound[1], &proc_h);

    // 9. Benchmark
    let t0 = Instant::now();
    let assets = GameAssets::generate();
    let gen_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t1 = Instant::now();
    for tick in 0..1000u32 {
        fb_proc.draw_match(&assets, &m, &names, tick * 16);
    }
    let render_us = t1.elapsed().as_secs_f64() * 1_000_000.0 / 1000.0;
    println!("Benchmark:       generate()={gen_ms:.2} ms, draw_match()={render_us:.1} us/frame");
}
