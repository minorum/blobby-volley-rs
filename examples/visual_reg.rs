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
use blobby_volley_rs::sim::{MatchState, PlayerControl};
use pak_ref::{load_pak_file, parse_bmp, BmpImage};

struct Metrics {
    fg_mae_rgb: [f64; 3],
    fg_mae: f64,
    psnr: f64,
    iou: f64,
    ssim: f64,
}

fn ssim_luma(a: &[u32], b: &[u32]) -> f64 {
    let n = a.len() as f64;
    let luma = |p: u32| -> f64 {
        let r = ((p >> 16) & 0xFF) as f64;
        let g = ((p >> 8) & 0xFF) as f64;
        let bl = (p & 0xFF) as f64;
        0.299 * r + 0.587 * g + 0.114 * bl
    };
    let mut sum_a = 0.0;
    let mut sum_b = 0.0;
    for (&pa, &pb) in a.iter().zip(b) {
        sum_a += luma(pa);
        sum_b += luma(pb);
    }
    let mu_a = sum_a / n;
    let mu_b = sum_b / n;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    let mut cov = 0.0;
    for (&pa, &pb) in a.iter().zip(b) {
        let da = luma(pa) - mu_a;
        let db = luma(pb) - mu_b;
        var_a += da * da;
        var_b += db * db;
        cov += da * db;
    }
    var_a /= n;
    var_b /= n;
    cov /= n;
    let c1 = (0.01_f64 * 255.0).powi(2);
    let c2 = (0.03_f64 * 255.0).powi(2);
    ((2.0 * mu_a * mu_b + c1) * (2.0 * cov + c2))
        / ((mu_a * mu_a + mu_b * mu_b + c1) * (var_a + var_b + c2))
}

fn compare_images(ref_px: &[u32], proc_px: &[u32], has_alpha: bool) -> Metrics {
    assert_eq!(ref_px.len(), proc_px.len());
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
    let ssim = ssim_luma(ref_px, proc_px);
    Metrics {
        fg_mae_rgb,
        fg_mae,
        psnr,
        iou,
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

fn wav_stats(wav: &[u8]) -> (usize, f64, f64) {
    let samples = &wav[44..];
    let n = samples.len();
    let mean = samples.iter().map(|&s| s as f64).sum::<f64>() / n as f64;
    let rms = (samples
        .iter()
        .map(|&s| {
            let d = s as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n as f64)
        .sqrt();
    let peak = samples
        .iter()
        .map(|&s| (s as f64 - mean).abs())
        .fold(0.0_f64, f64::max);
    (n, rms, peak)
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
    let (mut b_mae, mut b_iou, mut b_ssim) = (0.0, 0.0, 0.0);
    for (k, proc_img) in proc_ball.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[1 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0xB4 {
                *p = 0;
            }
        }
        let m = compare_images(&ref_img.pixels, &proc_img.pixels, true);
        b_mae += m.fg_mae;
        b_iou += m.iou;
        b_ssim += m.ssim;
        if k < 4 {
            save_side_by_side_bu(&out_dir.join(format!("ball_{k:02}.ppm")), 64, 64, &ref_img.pixels, &proc_img.pixels);
        }
    }
    println!(
        "Ball (16f):      fg_MAE={:5.2}, IoU={:.4}, SSIM={:.4}",
        b_mae / 16.0,
        b_iou / 16.0,
        b_ssim / 16.0
    );

    // 2. Blobby base frames (17..=21) + tinted P1 check
    let proc_blobby = generate_blobby_frames();
    let (mut bl_mae, mut bl_iou, mut bl_ssim) = (0.0, 0.0, 0.0);
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
        let m = compare_images(&ref_img.pixels, &proc_tinted, true);
        bl_mae += m.fg_mae;
        bl_iou += m.iou;
        bl_ssim += m.ssim;
        save_side_by_side_bu(&out_dir.join(format!("blobby_{k:02}.ppm")), 148, 101, &ref_img.pixels, &proc_tinted);
    }
    println!(
        "Blobby (5f,red): fg_MAE={:5.2}, IoU={:.4}, SSIM={:.4}",
        bl_mae / 5.0,
        bl_iou / 5.0,
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
        bsh_iou += compare_images(&ref_img.pixels, &proc_img.pixels, true).iou;
    }
    let mut ref_ball_sh = parse_bmp(&graph[87]).unwrap();
    for p in &mut ref_ball_sh.pixels {
        let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
        *p = if sum < 0x28 { 0 } else { 0x00202020 };
    }
    let proc_ball_sh = generate_ball_shadow();
    let m_bsh = compare_images(&ref_ball_sh.pixels, &proc_ball_sh.pixels, true);
    println!(
        "Shadows:         blobby IoU={:.4}, ball IoU={:.4}",
        bsh_iou / 10.0,
        m_bsh.iou
    );

    // 4. Background (800x600)
    let ref_bg = parse_bmp(&graph[0]).unwrap();
    let proc_bg_bu = flip_td_to_bu(&generate_background(), 800, 600);
    let m_bg = compare_images(&ref_bg.pixels, &proc_bg_bu, false);
    save_side_by_side_bu(&out_dir.join("bg.ppm"), 800, 600, &ref_bg.pixels, &proc_bg_bu);
    println!(
        "Background:      MAE={:5.2} (RGB {:.1}/{:.1}/{:.1}), PSNR={:.2} dB, SSIM={:.4}",
        m_bg.fg_mae, m_bg.fg_mae_rgb[0], m_bg.fg_mae_rgb[1], m_bg.fg_mae_rgb[2], m_bg.psnr, m_bg.ssim
    );

    // 5. Font (55 glyphs)
    let proc_font = generate_font_glyphs(&proc_blobby[0]);
    let (mut f_mae, mut f_iou) = (0.0, 0.0);
    for (k, proc_img) in proc_font.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[22 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            }
        }
        let m = compare_images(&ref_img.pixels, &proc_img.pixels, true);
        f_mae += m.fg_mae;
        f_iou += m.iou;
    }
    println!(
        "Font (55g):      fg_MAE={:5.2}, IoU={:.4}",
        f_mae / 55.0,
        f_iou / 55.0
    );

    // 6. Cursor, Title logo, Ending splash
    let ref_cursor = parse_bmp(&graph[88]).unwrap();
    let proc_cursor = generate_cursor();
    let m_cur = compare_images(&ref_cursor.pixels, &proc_cursor.pixels, true);
    save_side_by_side_bu(&out_dir.join("cursor.ppm"), 48, 48, &ref_cursor.pixels, &proc_cursor.pixels);
    println!(
        "Cursor:          fg_MAE={:5.2}, IoU={:.4}, SSIM={:.4}",
        m_cur.fg_mae, m_cur.iou, m_cur.ssim
    );

    let ref_logo = parse_bmp(&graph[89]).unwrap();
    let proc_logo = generate_title_logo();
    let m_logo = compare_images(&ref_logo.pixels, &proc_logo.pixels, true);
    save_side_by_side_bu(&out_dir.join("title_logo.ppm"), 410, 230, &ref_logo.pixels, &proc_logo.pixels);
    println!(
        "Title logo:      fg_MAE={:5.2}, IoU={:.4}, SSIM={:.4}",
        m_logo.fg_mae, m_logo.iou, m_logo.ssim
    );

    let ref_end = parse_bmp(&graph[90]).unwrap();
    let proc_end = generate_ending_splash();
    let m_end = compare_images(&ref_end.pixels, &proc_end.pixels, false);
    save_side_by_side_bu(&out_dir.join("ending_splash.ppm"), 400, 300, &ref_end.pixels, &proc_end.pixels);
    println!(
        "Ending splash:   MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_end.fg_mae, m_end.psnr, m_end.ssim
    );

    // 7. Full-frame gameplay & menu screenshots
    let ref_assets = build_ref_assets(&graph);
    let proc_assets = GameAssets::generate();
    let mut fb_ref = Framebuffer::new();
    let mut fb_proc = Framebuffer::new();
    let m = MatchState::new([PlayerControl::AiMedium, PlayerControl::AiMedium], 1);
    let names = ["Player 1".to_string(), "Player 2".to_string()];

    fb_ref.draw_match(&ref_assets, &m, &names, 0);
    fb_proc.draw_match(&proc_assets, &m, &names, 0);
    let ref_match_bu = flip_td_to_bu(&fb_ref.pixels, WIDTH, HEIGHT);
    let proc_match_bu = flip_td_to_bu(&fb_proc.pixels, WIDTH, HEIGHT);
    let m_match = compare_images(&ref_match_bu, &proc_match_bu, false);
    save_side_by_side_bu(&out_dir.join("scene_match.ppm"), WIDTH, HEIGHT, &ref_match_bu, &proc_match_bu);
    println!(
        "Scene (match):   MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_match.fg_mae, m_match.psnr, m_match.ssim
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
    let m_menu = compare_images(&ref_menu_bu, &proc_menu_bu, false);
    save_side_by_side_bu(&out_dir.join("scene_menu.ppm"), WIDTH, HEIGHT, &ref_menu_bu, &proc_menu_bu);
    println!(
        "Scene (menu):    MAE={:5.2}, PSNR={:.2} dB, SSIM={:.4}",
        m_menu.fg_mae, m_menu.psnr, m_menu.ssim
    );

    // 8. Audio WAV comparison
    let proc_w = generate_whistle_wav();
    let proc_h = generate_hit_wav();
    let (rn0, rrms0, rpk0) = wav_stats(&sound[0]);
    let (pn0, prms0, ppk0) = wav_stats(&proc_w);
    let (rn1, rrms1, rpk1) = wav_stats(&sound[1]);
    let (pn1, prms1, ppk1) = wav_stats(&proc_h);
    println!("Audio whistle:   ref=({rn0} smp, rms={rrms0:.1}, pk={rpk0:.1}) vs proc=({pn0} smp, rms={prms0:.1}, pk={ppk0:.1})");
    println!("Audio hit:       ref=({rn1} smp, rms={rrms1:.1}, pk={rpk1:.1}) vs proc=({pn1} smp, rms={prms1:.1}, pk={ppk1:.1})");

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
