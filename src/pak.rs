use std::fs;
use std::path::Path;

/// Decompresses all entries from a Blobby Volley `.pak` archive (12-bit LZW).
///
/// Format recovered from `FUN_004418a8` and `FUN_0044176c` in `volley.exe`:
/// - 12-bit codes packed 2 codes per 3 bytes (little-endian 24-bit word:
///   low 12 bits first, high 12 bits second).
/// - `0xFFE` resets the dictionary to 256 entries (`0x00..=0xFF`).
/// - `0xFFF` terminates the current entry; any unused second half of the
///   3-byte pair is discarded, and the next entry starts on the next byte.
pub fn unpack_pak(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut entries = Vec::new();
    let mut pos = 0usize;

    while pos < data.len() {
        let (entry, next_pos) = unpack_lzw_entry(data, pos)?;
        if entry.is_empty() && next_pos == pos {
            break;
        }
        entries.push(entry);
        pos = next_pos;
    }

    Ok(entries)
}

fn unpack_lzw_entry(data: &[u8], start: usize) -> Result<(Vec<u8>, usize), String> {
    let mut pos = start;
    let mut half = false;
    let mut word24: u32 = 0;

    let mut next_code = || -> Result<u16, String> {
        if !half {
            if pos >= data.len() {
                return Err("Unexpected EOF in LZW stream".to_string());
            }
            let b0 = data[pos] as u32;
            let b1 = *data.get(pos + 1).unwrap_or(&0) as u32;
            let b2 = *data.get(pos + 2).unwrap_or(&0) as u32;
            pos = (pos + 3).min(data.len());
            word24 = b0 | (b1 << 8) | (b2 << 16);
            half = true;
            Ok((word24 & 0x0FFF) as u16)
        } else {
            half = false;
            Ok(((word24 >> 12) & 0x0FFF) as u16)
        }
    };

    // Dictionary entries for codes >= 0x100: (parent_code, appended_byte)
    let mut dict: Vec<(u16, u8)> = Vec::with_capacity(4096 - 256);
    let mut out = Vec::new();
    let mut stack = Vec::with_capacity(256);

    let mut old_code = next_code()?;
    while old_code != 0x0FFF {
        let new_code = next_code()?;
        if old_code == 0x0FFE {
            dict.clear();
            old_code = new_code;
            continue;
        }

        if new_code != 0x0FFF {
            // Insert placeholder before traversing `new_code` so KwKwK (`new_code == 0x100 + dict.len()`) resolves.
            let new_idx = dict.len();
            dict.push((old_code, 0));
            let mut c = new_code;
            while c > 0x00FF {
                let idx = (c - 0x0100) as usize;
                c = dict
                    .get(idx)
                    .ok_or_else(|| format!("Invalid LZW code {c}"))?
                    .0;
            }
            dict[new_idx].1 = c as u8;
        }

        stack.clear();
        let mut c = old_code;
        while c > 0x00FF {
            let idx = (c - 0x0100) as usize;
            let (parent, ch) = *dict
                .get(idx)
                .ok_or_else(|| format!("Invalid LZW code {c}"))?;
            stack.push(ch);
            c = parent;
        }
        stack.push(c as u8);
        for &b in stack.iter().rev() {
            out.push(b);
        }

        old_code = new_code;
    }

    Ok((out, pos))
}

/// Decoded 32-bit `0x00RRGGBB` image stored bottom-up (row 0 = bottom scanline, matching BMP).
#[derive(Clone, Debug)]
pub struct BmpImage {
    pub width: usize,
    pub height: usize,
    /// Bottom-up pixels in `0x00RRGGBB` format (`width * height` elements).
    pub pixels: Vec<u32>,
}

/// Parses an uncompressed 8-bit paletted or 24-bit/32-bit Windows BMP.
pub fn parse_bmp(data: &[u8]) -> Result<BmpImage, String> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err("Not a valid BMP file".to_string());
    }
    let pixel_offset = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
    let header_size = u32::from_le_bytes(data[14..18].try_into().unwrap()) as usize;
    if header_size < 40 || data.len() < 14 + header_size {
        return Err("Unsupported BMP header".to_string());
    }
    let width = i32::from_le_bytes(data[18..22].try_into().unwrap());
    let raw_height = i32::from_le_bytes(data[22..26].try_into().unwrap());
    let bpp = u16::from_le_bytes(data[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(data[30..34].try_into().unwrap());
    if compression != 0 {
        return Err(format!("Unsupported BMP compression {compression}"));
    }
    if width <= 0 || raw_height == 0 {
        return Err("Invalid BMP dimensions".to_string());
    }
    let w = width as usize;
    let top_down = raw_height < 0;
    let h = raw_height.unsigned_abs() as usize;

    let mut pixels = vec![0u32; w * h];

    match bpp {
        8 => {
            let palette_offset = 14 + header_size;
            let colors_used = u32::from_le_bytes(data[46..50].try_into().unwrap()) as usize;
            let num_colors = if colors_used == 0 { 256 } else { colors_used.min(256) };
            if data.len() < palette_offset + num_colors * 4 {
                return Err("Truncated BMP palette".to_string());
            }
            let mut palette = [0u32; 256];
            for (i, entry) in palette.iter_mut().take(num_colors).enumerate() {
                let off = palette_offset + i * 4;
                let b = data[off] as u32;
                let g = data[off + 1] as u32;
                let r = data[off + 2] as u32;
                *entry = (r << 16) | (g << 8) | b;
            }
            let stride = (w + 3) & !3;
            for y in 0..h {
                let dst_y = if top_down { h - 1 - y } else { y };
                let row_off = pixel_offset + y * stride;
                if row_off + w > data.len() {
                    return Err("Truncated 8-bit BMP pixel data".to_string());
                }
                for x in 0..w {
                    pixels[dst_y * w + x] = palette[data[row_off + x] as usize];
                }
            }
        }
        24 | 32 => {
            let step = (bpp / 8) as usize;
            let stride = (w * step + 3) & !3;
            for y in 0..h {
                let dst_y = if top_down { h - 1 - y } else { y };
                let row_off = pixel_offset + y * stride;
                if row_off + w * step > data.len() {
                    return Err(format!("Truncated {bpp}-bit BMP pixel data"));
                }
                for x in 0..w {
                    let off = row_off + x * step;
                    let b = data[off] as u32;
                    let g = data[off + 1] as u32;
                    let r = data[off + 2] as u32;
                    pixels[dst_y * w + x] = (r << 16) | (g << 8) | b;
                }
            }
        }
        _ => return Err(format!("Unsupported BMP bit depth {bpp}")),
    }

    Ok(BmpImage {
        width: w,
        height: h,
        pixels,
    })
}

pub fn load_pak_file(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    unpack_pak(&data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn orig_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("original")
            .join(name)
    }

    #[test]
    fn unpack_all_original_paks() {
        let graph = load_pak_file(&orig_path("graph.pak")).unwrap();
        assert_eq!(graph.len(), 93);
        let bg = parse_bmp(&graph[0]).unwrap();
        assert_eq!((bg.width, bg.height), (800, 600));
        let ball = parse_bmp(&graph[1]).unwrap();
        assert_eq!((ball.width, ball.height), (64, 64));
        let blobby = parse_bmp(&graph[17]).unwrap();
        assert_eq!((blobby.width, blobby.height), (148, 101));

        let sound = load_pak_file(&orig_path("sound.pak")).unwrap();
        assert_eq!(sound.len(), 2);
        assert_eq!(&sound[0][0..4], b"RIFF");
        assert_eq!(&sound[1][0..4], b"RIFF");

        let text = load_pak_file(&orig_path("text.pak")).unwrap();
        assert_eq!(text.len(), 3);
        assert!(text[2].starts_with(b"@main"));
    }
}
