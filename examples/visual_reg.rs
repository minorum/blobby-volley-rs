#[path = "common/mod.rs"]
mod pak_ref;

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use blobby_volley_rs::assets::GameAssets;
use blobby_volley_rs::procedural::{
    generate_background, generate_ball_frames, generate_ball_shadow, generate_blobby_frames,
    generate_blobby_shadows, generate_cursor, generate_ending_splash, generate_font_glyphs,
    generate_title_logo,
};
use blobby_volley_rs::render::{Framebuffer, Sprite};
use blobby_volley_rs::sim::{MatchState, PlayerControl};
use pak_ref::{load_pak_file, parse_bmp, BmpImage};

struct Metrics {
    mae: f64,
    psnr: f64,
    iou: f64,
}

fn compare_images(ref_img: &BmpImage, proc_img: &Sprite) -> Metrics {
    assert_eq!(ref_img.width, proc_img.width);
    assert_eq!(ref_img.height, proc_img.height);
    let mut abs_sum = 0u64;
    let mut sq_sum = 0u64;
    let mut inter = 0u64;
    let mut union = 0u64;
    let n = ref_img.pixels.len();

    for (&rp, &pp) in ref_img.pixels.iter().zip(&proc_img.pixels) {
        if rp != 0 && pp != 0 {
            inter += 1;
        }
        if rp != 0 || pp != 0 {
            union += 1;
        }
        for shift in [0, 8, 16] {
            let rc = ((rp >> shift) & 0xFF) as i64;
            let pc = ((pp >> shift) & 0xFF) as i64;
            let d = (rc - pc).abs();
            abs_sum += d as u64;
            sq_sum += (d * d) as u64;
        }
    }

    let total_ch = (n * 3) as f64;
    let mae = abs_sum as f64 / total_ch;
    let mse = sq_sum as f64 / total_ch;
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
    Metrics { mae, psnr, iou }
}

fn save_side_by_side(path: &PathBuf, ref_img: &BmpImage, proc_img: &Sprite) {
    let w = ref_img.width;
    let h = ref_img.height;
    let out_w = w * 3;
    let mut ppm = format!("P6\n{out_w} {h}\n255\n").into_bytes();

    for y in (0..h).rev() {
        // Reference
        for x in 0..w {
            let p = ref_img.pixels[y * w + x];
            ppm.push(((p >> 16) & 0xFF) as u8);
            ppm.push(((p >> 8) & 0xFF) as u8);
            ppm.push((p & 0xFF) as u8);
        }
        // Procedural
        for x in 0..w {
            let p = proc_img.pixels[y * w + x];
            ppm.push(((p >> 16) & 0xFF) as u8);
            ppm.push(((p >> 8) & 0xFF) as u8);
            ppm.push((p & 0xFF) as u8);
        }
        // 4x Amplified Diff
        for x in 0..w {
            let rp = ref_img.pixels[y * w + x];
            let pp = proc_img.pixels[y * w + x];
            for shift in [16, 8, 0] {
                let rc = ((rp >> shift) & 0xFF) as i32;
                let pc = ((pp >> shift) & 0xFF) as i32;
                let d = ((rc - pc).abs() * 4).min(255) as u8;
                ppm.push(d);
            }
        }
    }
    fs::write(path, ppm).unwrap();
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let orig_dir = manifest_dir.join("original");
    let out_dir = manifest_dir.join("target/visual_reg");
    fs::create_dir_all(&out_dir).unwrap();

    let graph = load_pak_file(&orig_dir.join("graph.pak")).unwrap();

    // 1. Ball frames (1..=16)
    let proc_ball = generate_ball_frames();
    let mut ball_mae = 0.0;
    let mut ball_psnr = 0.0;
    let mut ball_iou = 0.0;
    for (k, proc_img) in proc_ball.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[1 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0xB4 {
                *p = 0;
            }
        }
        let m = compare_images(&ref_img, proc_img);
        ball_mae += m.mae;
        ball_psnr += m.psnr;
        ball_iou += m.iou;
        save_side_by_side(&out_dir.join(format!("ball_{k:02}.ppm")), &ref_img, proc_img);
    }
    println!(
        "Ball (16 frames): mean MAE={:.2}, mean PSNR={:.2} dB, mean IoU={:.4}",
        ball_mae / 16.0,
        ball_psnr / 16.0,
        ball_iou / 16.0
    );

    // 2. Blobby base frames (17..=21)
    let proc_blobby = generate_blobby_frames();
    let mut blobby_mae = 0.0;
    let mut blobby_psnr = 0.0;
    let mut blobby_iou = 0.0;
    for (k, proc_img) in proc_blobby.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[17 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            }
        }
        let m = compare_images(&ref_img, proc_img);
        blobby_mae += m.mae;
        blobby_psnr += m.psnr;
        blobby_iou += m.iou;
        save_side_by_side(&out_dir.join(format!("blobby_{k:02}.ppm")), &ref_img, proc_img);
    }
    println!(
        "Blobby (5 frames): mean MAE={:.2}, mean PSNR={:.2} dB, mean IoU={:.4}",
        blobby_mae / 5.0,
        blobby_psnr / 5.0,
        blobby_iou / 5.0
    );

    // 3. Shadows (77..=86 blobby, 87 ball)
    let proc_bshadows = generate_blobby_shadows(&proc_blobby);
    let mut bshadow_iou = 0.0;
    for (k, proc_img) in proc_bshadows.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[77 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            *p = if sum < 0x28 { 0 } else { 0x00202020 };
        }
        let m = compare_images(&ref_img, proc_img);
        bshadow_iou += m.iou;
        save_side_by_side(&out_dir.join(format!("bshadow_{k:02}.ppm")), &ref_img, proc_img);
    }
    let mut ref_ball_sh = parse_bmp(&graph[87]).unwrap();
    for p in &mut ref_ball_sh.pixels {
        let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
        *p = if sum < 0x28 { 0 } else { 0x00202020 };
    }
    let proc_ball_sh = generate_ball_shadow();
    let m_bsh = compare_images(&ref_ball_sh, &proc_ball_sh);
    save_side_by_side(&out_dir.join("ball_shadow.ppm"), &ref_ball_sh, &proc_ball_sh);
    println!(
        "Shadows: blobby mean IoU={:.4}, ball IoU={:.4}",
        bshadow_iou / 10.0,
        m_bsh.iou
    );

    // 4. Background (0, 800x600)
    let ref_bg = parse_bmp(&graph[0]).unwrap();
    let proc_bg_td = generate_background();
    let mut proc_bg_bu = vec![0u32; 800 * 600];
    for y in 0..600 {
        proc_bg_bu[(599 - y) * 800..(600 - y) * 800]
            .copy_from_slice(&proc_bg_td[y * 800..(y + 1) * 800]);
    }
    let proc_bg = Sprite {
        width: 800,
        height: 600,
        pixels: proc_bg_bu,
    };
    let m_bg = compare_images(&ref_bg, &proc_bg);
    save_side_by_side(&out_dir.join("bg.ppm"), &ref_bg, &proc_bg);
    println!("Background: MAE={:.2}, PSNR={:.2} dB", m_bg.mae, m_bg.psnr);

    // 5. Font glyphs (22..=76, 55 glyphs of 24x24)
    let proc_font = generate_font_glyphs(&proc_blobby[0]);
    let mut font_iou = 0.0;
    let mut font_sheet_proc = Sprite {
        width: 24 * 11,
        height: 24 * 5,
        pixels: vec![0u32; 24 * 11 * 24 * 5],
    };
    let mut font_sheet_ref = BmpImage {
        width: 24 * 11,
        height: 24 * 5,
        pixels: vec![0u32; 24 * 11 * 24 * 5],
    };
    for (k, proc_img) in proc_font.iter().enumerate() {
        let mut ref_img = parse_bmp(&graph[22 + k]).unwrap();
        for p in &mut ref_img.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            }
        }
        font_iou += compare_images(&ref_img, proc_img).iou;
        let gx = (k % 11) * 24;
        let gy = 4 - (k / 11);
        for y in 0..24 {
            for x in 0..24 {
                let dst_idx = (gy * 24 + y) * (24 * 11) + (gx + x);
                font_sheet_ref.pixels[dst_idx] = ref_img.pixels[y * 24 + x];
                font_sheet_proc.pixels[dst_idx] = proc_img.pixels[y * 24 + x];
            }
        }
    }
    save_side_by_side(
        &out_dir.join("font_sheet.ppm"),
        &font_sheet_ref,
        &font_sheet_proc,
    );
    println!("Font (55 glyphs): mean mask IoU={:.4}", font_iou / 55.0);

    // 6. Cursor (88), Title logo (89), Ending splash (90)
    let ref_cursor = parse_bmp(&graph[88]).unwrap();
    let proc_cursor = generate_cursor();
    let m_cur = compare_images(&ref_cursor, &proc_cursor);
    save_side_by_side(&out_dir.join("cursor.ppm"), &ref_cursor, &proc_cursor);
    println!("Cursor: MAE={:.2}, IoU={:.4}", m_cur.mae, m_cur.iou);

    let ref_logo = parse_bmp(&graph[89]).unwrap();
    let proc_logo = generate_title_logo();
    let m_logo = compare_images(&ref_logo, &proc_logo);
    save_side_by_side(&out_dir.join("title_logo.ppm"), &ref_logo, &proc_logo);
    println!("Title logo: MAE={:.2}, IoU={:.4}", m_logo.mae, m_logo.iou);

    let ref_end = parse_bmp(&graph[90]).unwrap();
    let proc_end = generate_ending_splash();
    let m_end = compare_images(&ref_end, &proc_end);
    save_side_by_side(&out_dir.join("ending_splash.ppm"), &ref_end, &proc_end);
    println!("Ending splash: MAE={:.2}, PSNR={:.2} dB", m_end.mae, m_end.psnr);

    // 7. Benchmark procedural asset generation and per-frame rendering
    let t0 = Instant::now();
    let assets = GameAssets::generate();
    let gen_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let mut fb = Framebuffer::new();
    let m = MatchState::new([PlayerControl::AiMedium, PlayerControl::AiMedium], 1);
    let names = ["Player 1".to_string(), "Player 2".to_string()];
    let t1 = Instant::now();
    for tick in 0..1000u32 {
        fb.draw_match(&assets, &m, &names, tick * 16);
    }
    let render_us = t1.elapsed().as_secs_f64() * 1_000_000.0 / 1000.0;
    println!(
        "Benchmark: GameAssets::generate() = {gen_ms:.2} ms, draw_match() = {render_us:.1} us/frame"
    );
}
