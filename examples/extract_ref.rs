use std::fs;
use std::path::PathBuf;

use blobby_volley_rs::pak::{load_pak_file, parse_bmp};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let orig_dir = manifest_dir.join("original");
    let out_dir = manifest_dir.join("target/ref_assets");
    fs::create_dir_all(&out_dir).unwrap();

    let graph = load_pak_file(&orig_dir.join("graph.pak")).unwrap();
    println!("graph.pak: {} entries", graph.len());
    for (i, entry) in graph.iter().enumerate() {
        let raw_path = out_dir.join(format!("graph_{i:02}.bin"));
        fs::write(&raw_path, entry).unwrap();
        if entry.len() >= 2 && &entry[0..2] == b"BM" {
            let bpp = u16::from_le_bytes(entry[28..30].try_into().unwrap());
            let img = parse_bmp(entry).unwrap();
            println!(
                "  [{i:02}] BMP {}x{} ({bpp} bpp, {} bytes)",
                img.width,
                img.height,
                entry.len()
            );
            // Save as PPM (top-down) for easy inspection with PIL/etc.
            let ppm_path = out_dir.join(format!("graph_{i:02}.ppm"));
            let mut ppm = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
            for y in (0..img.height).rev() {
                for x in 0..img.width {
                    let p = img.pixels[y * img.width + x];
                    ppm.push(((p >> 16) & 0xFF) as u8);
                    ppm.push(((p >> 8) & 0xFF) as u8);
                    ppm.push((p & 0xFF) as u8);
                }
            }
            fs::write(&ppm_path, ppm).unwrap();
        } else {
            let head: Vec<String> = entry.iter().take(16).map(|b| format!("{b:02X}")).collect();
            println!("  [{i:02}] NON-BMP ({} bytes): {}", entry.len(), head.join(" "));
        }
    }

    let sound = load_pak_file(&orig_dir.join("sound.pak")).unwrap();
    println!("sound.pak: {} entries", sound.len());
    for (i, entry) in sound.iter().enumerate() {
        let wav_path = out_dir.join(format!("sound_{i:02}.wav"));
        fs::write(&wav_path, entry).unwrap();
        println!("  [{i:02}] {} bytes", entry.len());
    }

    let text = load_pak_file(&orig_dir.join("text.pak")).unwrap();
    println!("text.pak: {} entries", text.len());
    for (i, entry) in text.iter().enumerate() {
        let txt_path = out_dir.join(format!("text_{i:02}.txt"));
        fs::write(&txt_path, entry).unwrap();
        println!("  [{i:02}] {} bytes", entry.len());
    }
}
