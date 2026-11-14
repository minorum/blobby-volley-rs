# Blobby Volley 2 Bitmap Font Assets (`font00.bmp`–`font49.bmp`, `font52.bmp`)

## Provenance
- **Upstream repository**: <https://github.com/danielknobe/blobbyvolley2>
- **Upstream commit**: `c28c5fa87872b7592f34f5f86196e93d127b6cf9`
- **Upstream paths**: `data/gfx/font00.bmp` through `data/gfx/font49.bmp`, plus `data/gfx/font52.bmp` (`'-'`, mapped to glyph index 50 in Blobby Volley 1.8).
- **Comparison against Blobby Volley 1.8 (`graph.pak` entries 22–76)**:
  - Glyphs `0..=23` and `25..=49` are pixel-for-pixel identical to the Blobby Volley 1.8 reference glyphs.
  - Glyph `24` (`'O'`) differs in 11 pixels (`max_diff = 29`, `raw_MAE = 0.20`, `IoU = 0.9805`).
  - Blobby Volley 1.8 glyph `50` (`'-'`) corresponds to `font52.bmp` in Blobby Volley 2 (while `font50.bmp` is `'_'` and `font51.bmp` is `' '`).
  - Blobby Volley 1.8 glyphs `51..=54` (P1/P2 Blobby player icons and up/down menu arrows) are not present in Blobby Volley 2 (`font53..58` were repurposed for `%`, `+`, `ì`, `¿`, eye, lock) and remain procedurally generated in `src/procedural.rs`.

## Copyright, License & Distribution Terms
- **Original Graphics Author**: **Silvio Mummert** (`mummertathome@t-online.de`), as credited in Blobby Volley 2's `AUTHORS` file.
- **Combined Program License**: Because these 51 BMPs are compiled into the executable via `include_bytes!`, the combined program and font assets are distributed under the **GNU General Public License, version 2 (GPL-2.0)**, whose complete text is included in [`../../COPYING`](../../COPYING).
- **Source & Binary Redistribution**:
  - Any source or binary redistribution of `blobby-volley-rs` containing these assets must preserve Silvio Mummert's copyright attribution, retain this notice and [`../../COPYING`](../../COPYING), and comply with GPL-2.0 source-availability obligations.
- **Upstream Asset Caveat**:
  - The `danielknobe/blobbyvolley2` repository ships `data/gfx/` alongside its root GPL-2.0 `COPYING` without a separate per-file asset license grant. If distributing binaries outside GPL-2.0 or if downstream packagers require an explicit written grant from Silvio Mummert, obtain separate permission from the graphics author or replace these bitmaps.
