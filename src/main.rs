mod app;
mod assets;
mod audio;
mod net;
mod pak;
mod render;
mod settings;
mod sim;

use minifb::{Window, WindowOptions};

use crate::app::App;
use crate::assets::{find_asset_dir, GameAssets};
use crate::render::{HEIGHT, WIDTH};

fn main() {
    let asset_dir = find_asset_dir();
    let assets = GameAssets::load(&asset_dir).unwrap_or_else(|e| {
        eprintln!(
            "Failed to load game assets from {}: {e}",
            asset_dir.display()
        );
        std::process::exit(1);
    });

    let mut window = Window::new(
        "Blobby Volley",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to create window: {e}");
        std::process::exit(1);
    });

    window.set_target_fps(60);
    window.set_cursor_visibility(false);

    let mut app = App::new(assets, asset_dir);
    while window.is_open() && !app.should_quit {
        app.update(&window);
        if let Err(e) = window.update_with_buffer(app.framebuffer(), WIDTH, HEIGHT) {
            eprintln!("Window update error: {e}");
            break;
        }
    }
}
