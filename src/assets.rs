use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::pak::{load_pak_file, parse_bmp, BmpImage};
use crate::procedural::generate_ball_frames;

pub const BLOBBY_COLORS: [u32; 8] = [
    0xFF0000, // 0: Red
    0xFF7F00, // 1: Orange
    0xFFFF00, // 2: Yellow
    0x00FF00, // 3: Green
    0x00ECFF, // 4: Cyan
    0x0000FF, // 5: Blue
    0xC000FF, // 6: Purple
    0xFF00D8, // 7: Magenta
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    French = 0,
    German = 1,
    English = 2,
}

impl Language {
    pub fn next(self) -> Self {
        match self {
            Language::French => Language::German,
            Language::German => Language::English,
            Language::English => Language::French,
        }
    }
}

pub struct LocalizedText {
    sections: HashMap<String, Vec<String>>,
}

impl LocalizedText {
    pub fn parse(raw: &[u8]) -> Self {
        // Text files in text.pak are Windows-1252 / ISO-8859-1 encoded with CRLF.
        let text: String = raw.iter().map(|&b| b as char).collect();
        let mut sections: HashMap<String, Vec<String>> = HashMap::new();
        let mut current = String::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with('@') {
                current = line.to_string();
                sections.entry(current.clone()).or_default();
            } else if !current.is_empty() {
                sections
                    .entry(current.clone())
                    .or_default()
                    .push(line.to_string());
            }
        }
        Self { sections }
    }

    pub fn get(&self, section: &str, idx: usize) -> &str {
        self.sections
            .get(section)
            .and_then(|v| v.get(idx))
            .map(|s| s.as_str())
            .unwrap_or("")
    }
}

pub struct GameAssets {
    pub bg: Vec<u32>,                       // 800x600 top-down (matches Framebuffer)
    pub bg_dark: Vec<u32>,                  // 800x600 top-down (1/8 brightness for menus)
    pub ball_frames: Vec<BmpImage>,         // 16 frames of 64x64
    pub blobby_base: Vec<BmpImage>,         // 5 frames of 148x101 (grayscale base)
    pub blobby_colored: [Vec<BmpImage>; 2], // 5 frames per player
    pub font_glyphs: Vec<BmpImage>,         // 55 glyphs of 24x24
    pub font_icon_base: BmpImage,           // base glyph 51 (0x33) for player color icons
    pub blobby_shadows: Vec<BmpImage>,      // 10 frames of 129x25 (actually 5 mirrored/frames)
    pub ball_shadow: BmpImage,              // 69x17
    pub cursor: BmpImage,                   // 48x48
    pub title_logo: BmpImage,               // entry 89 (410x230)
    pub ending_splash: BmpImage,            // entry 90 (400x300)
    pub sounds: [Arc<[u8]>; 2],             // 0: whistle, 1: hit/menu click
    pub texts: [LocalizedText; 3],          // 0: FR, 1: DE, 2: EN
}

pub fn find_asset_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("graph.pak").exists() {
        return cwd;
    }
    let orig = cwd.join("original");
    if orig.join("graph.pak").exists() {
        return orig;
    }
    let manifest_orig = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("original");
    if manifest_orig.join("graph.pak").exists() {
        return manifest_orig;
    }
    cwd
}

pub fn tint_pixel(color: u32, pixel: u32) -> u32 {
    let b = (pixel & 0xFF) as i32;
    let g = ((pixel >> 8) & 0xFF) as i32;
    let r = ((pixel >> 16) & 0xFF) as i32;
    let highlight = (b * 5 - 0x4B2).max(0);
    let cb = (color & 0xFF) as i32;
    let cg = ((color >> 8) & 0xFF) as i32;
    let cr = ((color >> 16) & 0xFF) as i32;
    let out_r = (((r * cr) >> 8) + highlight).min(255) as u32;
    let out_g = (((g * cg) >> 8) + highlight).min(255) as u32;
    let out_b = (((b * cb) >> 8) + highlight).min(255) as u32;
    (out_r << 16) | (out_g << 8) | out_b
}

impl GameAssets {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let graph = load_pak_file(&dir.join("graph.pak"))?;
        if graph.len() < 91 {
            return Err(format!("Unexpected graph.pak entry count: {}", graph.len()));
        }

        // Background (with optional bvbg.bmp override if 800x600)
        let mut bg_img = parse_bmp(&graph[0])?;
        let bvbg_path = dir.join("bvbg.bmp");
        if bvbg_path.exists() {
            if let Ok(bytes) = fs::read(&bvbg_path) {
                if let Ok(custom) = parse_bmp(&bytes) {
                    if custom.width == 800 && custom.height == 600 {
                        bg_img = custom;
                    }
                }
            }
        }
        // Flip background to top-down once so per-frame copy_bg is a single memcpy.
        let mut bg = vec![0u32; 800 * 600];
        for y in 0..600 {
            bg[(599 - y) * 800..(600 - y) * 800]
                .copy_from_slice(&bg_img.pixels[y * 800..(y + 1) * 800]);
        }
        let bg_dark: Vec<u32> = bg.iter().map(|&p| (p & 0x00F8F8F8) >> 3).collect();

        // Ball frames (procedural, 16 frames of 64x64)
        let ball_frames = generate_ball_frames();

        // Blobby base frames (17..=21)
        let mut blobby_base = Vec::with_capacity(5);
        for entry in &graph[17..=21] {
            blobby_base.push(parse_bmp(entry)?);
        }

        // Font glyphs (22..=76)
        let mut font_glyphs = Vec::with_capacity(55);
        for entry in &graph[22..=76] {
            let mut img = parse_bmp(entry)?;
            for p in &mut img.pixels {
                let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
                if sum < 0x28 {
                    *p = 0;
                }
            }
            font_glyphs.push(img);
        }
        let font_icon_base = font_glyphs[0x33].clone();

        // Blobby shadows (77..=86)
        let mut blobby_shadows = Vec::with_capacity(10);
        for entry in &graph[77..=86] {
            let mut img = parse_bmp(entry)?;
            for p in &mut img.pixels {
                let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
                if sum < 0x28 {
                    *p = 0;
                } else {
                    *p = (*p & 0x00FCFCFC) >> 1;
                }
            }
            blobby_shadows.push(img);
        }

        // Ball shadow (87)
        let mut ball_shadow = parse_bmp(&graph[87])?;
        for p in &mut ball_shadow.pixels {
            let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
            if sum < 0x28 {
                *p = 0;
            } else {
                *p = (*p & 0x00FCFCFC) >> 2;
            }
        }

        // Cursor (88), title logo (89), ending splash (90)
        let cursor = parse_bmp(&graph[88])?;
        let title_logo = parse_bmp(&graph[89])?;
        let ending_splash = parse_bmp(&graph[90])?;

        // Sounds
        let sound_entries = load_pak_file(&dir.join("sound.pak"))?;
        if sound_entries.len() < 2 {
            return Err("sound.pak has fewer than 2 entries".to_string());
        }
        let sounds = [
            Arc::from(sound_entries[0].as_slice()),
            Arc::from(sound_entries[1].as_slice()),
        ];

        // Texts
        let text_entries = load_pak_file(&dir.join("text.pak"))?;
        if text_entries.len() < 3 {
            return Err("text.pak has fewer than 3 entries".to_string());
        }
        let texts = [
            LocalizedText::parse(&text_entries[0]),
            LocalizedText::parse(&text_entries[1]),
            LocalizedText::parse(&text_entries[2]),
        ];

        let mut assets = Self {
            bg,
            bg_dark,
            ball_frames,
            blobby_base,
            blobby_colored: [Vec::new(), Vec::new()],
            font_glyphs,
            font_icon_base,
            blobby_shadows,
            ball_shadow,
            cursor,
            title_logo,
            ending_splash,
            sounds,
            texts,
        };
        assets.recolor_player(0, 0);
        assets.recolor_player(1, 3);
        Ok(assets)
    }

    pub fn recolor_player(&mut self, player_idx: usize, color_idx: usize) {
        let color = BLOBBY_COLORS[color_idx % BLOBBY_COLORS.len()];
        let mut colored_frames = Vec::with_capacity(5);
        for base in &self.blobby_base {
            let mut img = base.clone();
            for p in &mut img.pixels {
                let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
                if sum < 0x28 {
                    *p = 0;
                } else {
                    *p = tint_pixel(color, *p);
                    if *p == 0 {
                        *p = 1;
                    }
                }
            }
            colored_frames.push(img);
        }
        self.blobby_colored[player_idx] = colored_frames;

        // Also tint the player's little blobby icon in font_glyphs[0x33 + player_idx]
        let mut icon = self.font_icon_base.clone();
        for p in &mut icon.pixels {
            if *p != 0 {
                *p = tint_pixel(color, *p);
            }
        }
        self.font_glyphs[0x33 + player_idx] = icon;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_text_parses_sections_and_crlf() {
        let raw = b"@main\r\nStart\r\nQuit\r\n@names\r\nAdam\r\n";
        let lt = LocalizedText::parse(raw);
        assert_eq!(lt.get("@main", 0), "Start");
        assert_eq!(lt.get("@main", 1), "Quit");
        assert_eq!(lt.get("@names", 0), "Adam");
        assert_eq!(lt.get("@missing", 0), "");
    }

    #[test]
    fn tint_pixel_preserves_black_and_adds_highlight() {
        assert_eq!(tint_pixel(0xFF0000, 0x000000), 0);
        // White pixel (255,255,255) with red color gets highlight on all channels
        let tinted = tint_pixel(0xFF0000, 0xFFFFFF);
        assert_eq!(tinted >> 16, 255);
        assert!(tinted & 0xFF > 0);
    }
}
