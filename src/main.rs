use minifb::{Window, WindowOptions};

use blobby_volley_rs::app::App;
use blobby_volley_rs::assets::{find_settings_dir, GameAssets};
use blobby_volley_rs::render::{HEIGHT, WIDTH};

fn main() {
    let settings_dir = find_settings_dir();
    let assets = GameAssets::generate();

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

    let mut app = App::new(assets, settings_dir);
    while window.is_open() && !app.should_quit {
        app.update(&window);
        if let Err(e) = window.update_with_buffer(app.framebuffer(), WIDTH, HEIGHT) {
            eprintln!("Window update error: {e}");
            break;
        }
    }
}
