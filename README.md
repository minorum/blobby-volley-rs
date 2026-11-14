# blobby-volley-rs

A Rust rewrite of the classic **Blobby Volley** (v1.8.0.48) game, reconstructed via reverse engineering of `volley.exe` (see [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md)).

## Assets & Licensing

No external `.pak` files are required at runtime:
- Most graphics, sound effects, and localized strings are generated procedurally at startup (`src/procedural.rs`, `src/assets.rs`).
- Font glyphs `0..=50` embed matching 24×24 bitmap glyphs from [Blobby Volley 2](https://github.com/danielknobe/blobbyvolley2) (`assets/bv2_font/*.bmp`, authored by Silvio Mummert) via `include_bytes!`.

Because the Blobby Volley 2 font assets are compiled into the binary, the combined source and binary distribution is governed by the **GNU General Public License, version 2 (GPL-2.0)** — see [COPYING](COPYING) and [assets/bv2_font/NOTICE.md](assets/bv2_font/NOTICE.md) for provenance, attribution, and redistribution details.

Optional development-only reference comparison tools (`examples/extract_ref.rs`, `examples/visual_reg.rs`) can compare generated output against original `.pak` files placed in `original/`.

## Build & Run

```bash
cargo test
cargo run --release
```

## Controls (Defaults)

- **Player 1 (Left):** Arrow keys (`Left` / `Right` / `Up`) or Mouse
- **Player 2 (Right):** `A` / `D` / `W` (when playing 2-player local)
- **Menu / Pause:** `Esc`, `Enter` / `Space`, Arrow keys
