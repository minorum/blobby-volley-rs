# blobby-volley-rs

A clean-room Rust rewrite of the classic **Blobby Volley** (v1.8.0.48) game, reconstructed via reverse engineering of `volley.exe` (see [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md)).

## Assets

All game graphics, sound effects, and localized strings are generated procedurally at startup (`src/procedural.rs`, `src/assets.rs`)—no external `.pak` or proprietary asset files are required to build or run the game.

Optional development-only reference comparison tools (`examples/extract_ref.rs`, `examples/visual_reg.rs`) can compare procedural output against original `.pak` files placed in `original/`.

## Build & Run

```bash
cargo test
cargo run --release
```

## Controls (Defaults)

- **Player 1 (Left):** Arrow keys (`Left` / `Right` / `Up`) or Mouse
- **Player 2 (Right):** `A` / `D` / `W` (when playing 2-player local)
- **Menu / Pause:** `Esc`, `Enter` / `Space`, Arrow keys
