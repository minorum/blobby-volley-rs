use crate::assets::GameAssets;
use crate::sim::MatchState;

/// 32-bit `0x00RRGGBB` sprite stored bottom-up (row 0 = bottom scanline).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 600;
pub const CHAR_ADVANCE: i32 = 20; // DAT_0044db4c = 0x14
pub const LINE_HEIGHT: i32 = 24; // DAT_0044db50 = 0x18

pub struct Framebuffer {
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }

    #[inline]
    fn clip_rect_bu(
        x0: i32,
        y0: i32,
        w: i32,
        h: i32,
    ) -> Option<(usize, usize, usize, usize, usize, usize)> {
        let x_start = x0.max(0);
        let x_end = (x0 + w).min(WIDTH as i32);
        let y_start = y0.max(0);
        let y_end = (y0 + h).min(HEIGHT as i32);
        if x_start >= x_end || y_start >= y_end {
            return None;
        }
        Some((
            x_start as usize,
            x_end as usize,
            y_start as usize,
            y_end as usize,
            (x_start - x0) as usize,
            (y_start - y0) as usize,
        ))
    }

    pub fn copy_bg(&mut self, bg_top_down: &[u32]) {
        self.pixels.copy_from_slice(bg_top_down);
    }

    pub fn darken_rect_bu(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, shift: u32) {
        let Some((xs, xe, ys, ye, _, _)) = Self::clip_rect_bu(x0, y0, x1 - x0, y1 - y0) else {
            return;
        };
        let mask = match shift {
            1 => 0x00FEFEFE,
            2 => 0x00FCFCFC,
            _ => 0x00F8F8F8,
        };
        for sy in (HEIGHT - ye)..(HEIGHT - ys) {
            let row = &mut self.pixels[sy * WIDTH + xs..sy * WIDTH + xe];
            for p in row {
                *p = (*p & mask) >> shift;
            }
        }
    }

    pub fn draw_title_logo(&mut self, logo: &Sprite, dim_shift: u32) {
        let Some((xs, xe, ys, ye, src_x0, src_y0)) =
            Self::clip_rect_bu(30, 360, logo.width as i32, logo.height as i32)
        else {
            return;
        };
        let mask = match dim_shift {
            0 => 0xFFFFFFFF,
            1 => 0x00FEFEFE,
            _ => 0x00FCFCFC,
        };
        let span = xe - xs;
        for (i, y) in (ys..ye).enumerate() {
            let sy = HEIGHT - 1 - y;
            let src_off = (src_y0 + i) * logo.width + src_x0;
            let dst = &mut self.pixels[sy * WIDTH + xs..sy * WIDTH + xe];
            let src = &logo.pixels[src_off..src_off + span];
            for (d, &p) in dst.iter_mut().zip(src) {
                let sum = (p & 0xFF) + ((p >> 8) & 0xFF) + ((p >> 16) & 0xFF);
                if sum > 0x14 {
                    *d = (p & mask) >> dim_shift;
                }
            }
        }
    }

    pub fn draw_sprite_bu(&mut self, img: &Sprite, x0: i32, y0: i32) {
        let Some((xs, xe, ys, ye, src_x0, src_y0)) =
            Self::clip_rect_bu(x0, y0, img.width as i32, img.height as i32)
        else {
            return;
        };
        let span = xe - xs;
        for (i, y) in (ys..ye).enumerate() {
            let sy = HEIGHT - 1 - y;
            let src_off = (src_y0 + i) * img.width + src_x0;
            let dst = &mut self.pixels[sy * WIDTH + xs..sy * WIDTH + xe];
            let src = &img.pixels[src_off..src_off + span];
            for (d, &p) in dst.iter_mut().zip(src) {
                if p != 0 {
                    *d = p;
                }
            }
        }
    }

    pub fn draw_shadow_bu(&mut self, img: &Sprite, x0: i32, y0: i32) {
        let Some((xs, xe, ys, ye, src_x0, src_y0)) =
            Self::clip_rect_bu(x0, y0, img.width as i32, img.height as i32)
        else {
            return;
        };
        let span = xe - xs;
        for (i, y) in (ys..ye).enumerate() {
            let sy = HEIGHT - 1 - y;
            let src_off = (src_y0 + i) * img.width + src_x0;
            let dst = &mut self.pixels[sy * WIDTH + xs..sy * WIDTH + xe];
            let src = &img.pixels[src_off..src_off + span];
            for (d, &p) in dst.iter_mut().zip(src) {
                if p != 0 {
                    *d = (*d & 0x00FCFCFC) >> 1;
                }
            }
        }
    }

    pub fn draw_cursor(&mut self, cursor: &Sprite, mx: i32, my_bu: i32) {
        self.draw_sprite_bu(cursor, mx, my_bu - 47);
    }

    pub fn draw_text(
        &mut self,
        assets: &GameAssets,
        x: i32,
        y: i32,
        small: bool,
        highlight: bool,
        text: &str,
    ) {
        for (line_idx, line) in text.split('\n').enumerate() {
            let line_y = y - (line_idx as i32) * LINE_HEIGHT;
            for (i, ch) in line.chars().enumerate() {
                let cx = x + (i as i32) * CHAR_ADVANCE;
                let Some((glyph_idx, is_lower)) = char_to_glyph(ch) else {
                    continue;
                };
                let glyph = &assets.font_glyphs[glyph_idx];
                let denom = if is_lower { 4 } else { 3 };
                for row in 0..24i32 {
                    let gy = (row * 3) / denom + line_y;
                    if gy < 0 || gy >= HEIGHT as i32 {
                        continue;
                    }
                    let sy = HEIGHT - 1 - gy as usize;
                    let col_start = (-cx).max(0);
                    let col_end = (WIDTH as i32 - cx).min(24);
                    for col in col_start..col_end {
                        let gx = (cx + col) as usize;
                        let p = glyph.pixels[(row as usize) * 24 + col as usize];
                        if (p & 0x00E0E0E0) == 0 {
                            continue;
                        }
                        let out = if !small {
                            if highlight {
                                let r = (((p >> 16) & 0xFF) * 2 + 0x1E).min(255);
                                let g = (((p >> 8) & 0xFF) * 2 + 0x1E).min(255);
                                let b = ((p & 0xFF) * 2 + 0x1E).min(255);
                                (r << 16) | (g << 8) | b
                            } else {
                                p
                            }
                        } else {
                            let b = p & 0xFF;
                            let g = (p >> 8) & 0xFF;
                            let r = (p >> 16) & 0xFF;
                            let lum = ((b * 3 + g * 6 + r) / 10) >> 1;
                            let base = if highlight { 0xBF } else { 0x7F };
                            let v = (base - lum as i32).clamp(0, 255) as u32;
                            (v << 16) | (v << 8) | v
                        };
                        self.pixels[sy * WIDTH + gx] = out;
                    }
                }
            }
        }
    }

    pub fn draw_match(
        &mut self,
        assets: &GameAssets,
        m: &MatchState,
        names: &[String; 2],
        tick_ms: u32,
    ) {
        self.copy_bg(&assets.bg);

        let r = &m.rally;
        // Player shadows
        for i in 0..2 {
            let p = &r.players[i];
            let sx = (p.x + ((p.y - 100) >> 2) + 24 - (i as i32) * 24) & !1;
            let sy = ((p.y - 100) >> 3) + 97;
            let frame = (p.anim_frame.clamp(0, 4) as usize) + i * 5;
            self.draw_shadow_bu(&assets.blobby_shadows[frame], sx - 64, sy - 12);
        }

        // Ball shadow
        let bx_px = r.ball.x / 32;
        let by_px = r.ball.y / 32;
        let bsy_off = by_px - 100;
        let bsx = bx_px + (bsy_off / 3);
        let bsy = (bsy_off >> 3) + 97;
        self.draw_shadow_bu(&assets.ball_shadow, bsx - 34, bsy - 8);

        // Score & serve indicator
        let s0 = r.players[0].score;
        let s1 = r.players[1].score;
        let serve0 = !r.is_first_rally && r.server == 0;
        let serve1 = !r.is_first_rally && r.server == 1;
        let mp = m.is_match_point();
        let blink_on = (tick_ms & 0x100) == 0;

        let score_x = [100, 636];
        let scores = [s0, s1];
        let serves = [serve0, serve1];
        for i in 0..2 {
            let s = scores[i].clamp(0, 99) as u8;
            let d0 = [b'0' + s / 10];
            let d1 = [b'0' + s % 10];
            self.draw_text(
                assets,
                score_x[i],
                540,
                false,
                false,
                std::str::from_utf8(&d0).unwrap(),
            );
            self.draw_text(
                assets,
                score_x[i] + 24,
                540,
                false,
                false,
                std::str::from_utf8(&d1).unwrap(),
            );
            if serves[i] {
                self.draw_text(assets, score_x[i] + 48, 540, false, false, "!");
                if mp && blink_on {
                    self.draw_text(assets, score_x[i] + 52, 540, false, false, "!");
                }
            }
        }

        // Player names at bottom (y = 0x23a? Actually 570 from bottom = top, or bottom!)
        // In FUN_004443e8 line 701: y = 0x23a (570) -> top of screen!
        for i in 0..2 {
            let label = match r.controls[i] {
                crate::sim::PlayerControl::AiEasy => assets.texts[2].get("@names", 0),
                crate::sim::PlayerControl::AiMedium => assets.texts[2].get("@names", 1),
                crate::sim::PlayerControl::AiHard => assets.texts[2].get("@names", 2),
                _ => names[i].as_str(),
            };
            let half_w = ((label.chars().count() as i32 - 2) * CHAR_ADVANCE) / 2;
            self.draw_text(assets, score_x[i] - half_w, 570, false, false, label);
        }

        // Ball
        let ball_frame = ((r.ball.spin_phase >> 3) & 0xF) as usize;
        self.draw_sprite_bu(&assets.ball_frames[ball_frame], bx_px - 32, by_px - 32);

        // Players
        for i in 0..2 {
            let p = &r.players[i];
            let frame = p.anim_frame.clamp(0, 4) as usize;
            self.draw_sprite_bu(&assets.blobby_colored[i][frame], p.x - 74, p.y);
        }
    }
}

/// Exact character-to-glyph mapping from `FUN_0044708c`.
/// Returns `(glyph_index, is_lowercase)`.
pub fn char_to_glyph(ch: char) -> Option<(usize, bool)> {
    let code = ch as u32;
    let idx = match code {
        0x30..=0x39 => (code - 0x30) as usize, // '0'..'9' -> 0..9
        0x41..=0x5A => (code - 0x37) as usize, // 'A'..'Z' -> 10..35
        0x61..=0x7A => return Some(((code - 0x57) as usize, true)), // 'a'..'z' -> 10..35 lowercase
        0x2E => 0x24,                          // '.'
        0x21 => 0x25,                          // '!'
        0x28 => 0x26,                          // '('
        0x29 => 0x27,                          // ')'
        0xDF => 0x28,                          // 'ß'
        0xC4 => 0x29,                          // 'Ä'
        0xD6 => 0x2A,                          // 'Ö'
        0xDC => 0x2B,                          // 'Ü'
        0xE4 => return Some((0x29, true)),     // 'ä'
        0xF6 => return Some((0x2A, true)),     // 'ö'
        0xFC => return Some((0x2B, true)),     // 'ü'
        0x27 => 0x2C,                          // '\''
        0x3A => 0x2D,                          // ':'
        0x3B => 0x2E,                          // ';'
        0x3F => 0x2F,                          // '?'
        0x2C => 0x30,                          // ','
        0x2F => 0x31,                          // '/'
        0x2D => 0x32,                          // '-'
        0x3C => 0x33,                          // '<' (P1 icon)
        0x3E => 0x34,                          // '>' (P2 icon)
        0xB2 => 0x35,                          // '²'
        0xB3 => 0x36,                          // '³'
        _ => return None,
    };
    Some((idx, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_sprite_and_darken_clip_cleanly_at_edges() {
        let mut fb = Framebuffer::new();
        fb.pixels.fill(0x00808080);
        fb.darken_rect_bu(-10, -10, 10, 10, 1);
        // Bottom-left corner pixel (y_bu = 0 -> sy = HEIGHT - 1, x = 0) should be halved
        assert_eq!(fb.pixels[(HEIGHT - 1) * WIDTH], 0x00404040);
        // Outside rect stays unchanged
        assert_eq!(fb.pixels[(HEIGHT - 1) * WIDTH + 10], 0x00808080);

        let img = Sprite {
            width: 4,
            height: 4,
            pixels: vec![0x00FF0000; 16],
        };
        fb.draw_sprite_bu(&img, -2, -2);
        assert_eq!(fb.pixels[(HEIGHT - 1) * WIDTH], 0x00FF0000);
        assert_eq!(fb.pixels[(HEIGHT - 1) * WIDTH + 2], 0x00404040);
    }

    #[test]
    fn draw_text_splits_newlines_across_vertical_lines() {
        let assets = GameAssets::generate();
        let mut fb = Framebuffer::new();
        fb.draw_text(&assets, 100, 300, false, false, "A\nB");
        // Line 0 ('A') renders near y_bu = 300..324 (sy = 276..299); Line 1 ('B') near y_bu = 276..300 (sy = 300..323)
        let top_line_lit = (276..300).any(|sy| (100..124).any(|x| fb.pixels[sy * WIDTH + x] != 0));
        let bot_line_lit = (300..324).any(|sy| (100..124).any(|x| fb.pixels[sy * WIDTH + x] != 0));
        // Nothing should be drawn at x >= 120 on line 0 (i.e. no '\\n' characters concatenated horizontally)
        let trailing_lit = (276..300).any(|sy| (124..180).any(|x| fb.pixels[sy * WIDTH + x] != 0));
        assert!(top_line_lit);
        assert!(bot_line_lit);
        assert!(!trailing_lit);
    }

    #[test]
    fn char_to_glyph_maps_ascii_and_german_umlauts() {
        assert_eq!(char_to_glyph('0'), Some((0, false)));
        assert_eq!(char_to_glyph('A'), Some((10, false)));
        assert_eq!(char_to_glyph('a'), Some((10, true)));
        assert_eq!(char_to_glyph('ä'), Some((0x29, true)));
        assert_eq!(char_to_glyph(' '), None);
    }
}
