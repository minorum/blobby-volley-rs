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

## Copyright & Licensing
- **Original Graphics Author**: **Silvio Mummert** (`mummertathome@t-online.de`), as credited in Blobby Volley 2's `AUTHORS` file.
- **Repository License**: The `danielknobe/blobbyvolley2` repository distributes its source code and `data/gfx/` assets together under the **GNU General Public License, version 2 (GPL-2.0)** (`COPYING`), without a separate per-asset license file in `data/gfx/`.
- **Licensing Obligations**:
  - Do not assume these bitmaps are permissively licensed or public domain.
  - Retaining and redistributing these BMPs (including compiled-in via `include_bytes!`) requires preserving Silvio Mummert's attribution and complying with GPL-2.0 terms (or obtaining separate permission from the graphics copyright holder if distributing under a non-GPL-compatible license).
