# Blobby Volley (v1.8.0.48) Reverse Engineering Log

## Target Binary
- **Archive**: `blobby.zip`
- **Executable**: `original/volley.exe` (PE32 GUI, x86, Borland Delphi VCL, version `1.8.0.48`, SHA-256 `a16f4b950b263a036be928a6f773056eee0fc602be740584198f08da94b031c2`)
- **Asset Archives**:
  - `graph.pak` (619,758 bytes, 93 LZW-compressed BMPs)
  - `sound.pak` (20,826 bytes, 2 LZW-compressed WAVs: whistle + ball hit / menu click)
  - `text.pak` (3,822 bytes, 3 LZW-compressed localized text tables: French, German, English)

## Recovered Subsystems & Evidence

### 1. `.pak` Compression (`FUN_004418a8`, `FUN_0044176c`, `FUN_004416b0`, `FUN_0044173c`)
- 12-bit LZW stream, packed 2 codes per 3 bytes (little-endian 24-bit word: low 12 bits first, high 12 bits second).
- `0xFFE` resets the dictionary to 256 single-byte entries.
- `0xFFF` terminates an entry; remaining unread half-word in the 3-byte pair is discarded.
- Verified by decompressing all 93 entries of `graph.pak`, 2 entries of `sound.pak`, and 3 entries of `text.pak` to exact EOF.

### 2. Asset Loading & Recoloring (`FUN_00445aa0`, `FUN_0044a784`, `FUN_0044a6f0`)
- Framebuffer is 800×600 bottom-up (`y = 0` at bottom, `y = 599` at top).
- Optional `bvbg.bmp` (800×600) overrides `graph.pak` entry 0.
- Darkened menu background is `(pixel & 0xF8F8F8) >> 3`.
- 8 Blobby colors (`0..7`): Red (`0xFF0000`), Orange (`0xFF7F00`), Yellow (`0xFFFF00`), Green (`0x00FF00`), Cyan (`0x00ECFF`), Blue (`0x0000FF`), Purple (`0xC000FF`), Magenta (`0xFF00D8`).
- Recoloring formula (`FUN_0044a6f0`): specular highlight `max(0, b * 5 - 0x4B2)` added to `(channel * color_channel) >> 8`, clamped to 255.
- Font (`graph.pak` entries 22..76, 24×24): mapped via `FUN_0044708c`; lowercase letters set bit `0x8000` and are scaled vertically to 3/4 height (`(row * 3) / 4`).

### 3. Physics & Collision (`FUN_004443e8`, `FUN_004441d0`, `FUN_00443bf0`, `FUN_00444100`)
- Fixed 33 ms tick (~30 Hz, `0x978234 = 0x21`).
- Ball coordinates `(ball_x, ball_y)` are fixed-point `× 32`. Serve starts at `((server * 400 + 200) * 32, 300 * 32)` with `active = false`.
- `FUN_004441d0` and `FUN_00443bf0` are both invoked twice per frame (before and after player movement), with `FUN_004441d0` running 2 substeps per call (`grav = substep + 2`, air drag `-(|vy| / 3) / 32` when falling).
- Ground bounce at `y < 132 * 32` (`0x1080`): `vy = (-vy * 3) / 4`, `vx = (vx * 3) / 4`, triggers rally end if not already ended.
- Walls at `x = 32 * 32` (`0x400`) and `x = 768 * 32` (`0x6000`).
- Net vertical sides below `y = 355` and top cap at `(400, 329)` with radius 37 (ball 32 + net 5), bounce speed `4.0 * 32`.
- Blobby collision (`FUN_00443bf0`): two circles per player — head (`r = 25`, `r_sum = 57`, `57^2 = 3249`, `HEAD_CY = [36, 41, 45, 50, 54]`) and body (`r = 32`, `r_sum = 64`, `64^2 = 4096`, `BODY_CY = [69, 71, 74, 76, 79]`).
- Hit detection: `dot(v, n) <= -0.7 * dist * speed` (`0x4440cc`), bounce speed `8.0 * 32 = 256`.
- Player movement: `±12 px/frame`, P1 `x in [0, 360]`, P2 `x in [440, 800]`.
- Jump: `vy = 10000`, `y_frac += vy`, `y += y_frac / 256`, `vy -= 800` (and another `-800` if jump key released), ground at `y = 100`.
- Rules (`FUN_00445890`, `FUN_004443e8`): first rally determines serve; side-out scoring to 15, win by 2; max 3 hits per side (4th hit = fault) or >40 frames continuous contact = fault; 2-second whistle cooldown before next rally.

### 4. AI (`FUN_004438f4`, `FUN_004443e8`)
- Quadratic landing predictor `FUN_004438f4` using effective gravity `-3.5`.
- Easy (Adam): adds random stun frames (`7..13` frames) when ball rises.
- Medium (Bert): tracks predicted landing with wall-bounce reflection.
- Hard (Carl): adds spike prediction at `y = 350` and jump-spike timing.

### 5. Menus, Stats, & Match End (`FUN_00447e0c`, `FUN_004486fc`, `FUN_00447948`, `FUN_0044a570`, `FUN_00449d3c`, `FUN_00448e70`)
- Main menu, Settings, Key definition, Statistics (5 categories, sortable), Multiplayer Host/Join menus, Quit confirmation overlay, Match Win bounce animation (`vy = 12.0, grav = 0.8`), and Ending Splash.
