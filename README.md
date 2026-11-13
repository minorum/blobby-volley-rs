# blobby-volley-rs

A clean-room Rust rewrite of the classic **Blobby Volley** (v1.8.0.48) game, reconstructed via reverse engineering of `volley.exe` (see [REVERSE_ENGINEERING.md](REVERSE_ENGINEERING.md)).

## Assets Setup

This repository does not redistribute the original proprietary game assets. To run the game or the asset-unpacking test:

1. Obtain the original `blobby.zip` archive.
2. Either extract it into `original/` inside the repository root (so `original/graph.pak`, `original/sound.pak`, and `original/text.pak` exist), or place `graph.pak`, `sound.pak`, and `text.pak` in the working directory.

## Build & Run

```bash
cargo test
cargo run --release
```

## Controls (Defaults)

- **Player 1 (Left):** Arrow keys (`Left` / `Right` / `Up`) or Mouse
- **Player 2 (Right):** `A` / `D` / `W` (when playing 2-player local)
- **Menu / Pause:** `Esc`, `Enter` / `Space`, Arrow keys
