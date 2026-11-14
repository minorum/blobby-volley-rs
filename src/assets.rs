use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::render::Sprite;
use crate::procedural::{
    generate_background, generate_ball_frames, generate_ball_shadow, generate_blobby_frames,
    generate_blobby_shadows, generate_cursor, generate_ending_splash, generate_font_glyphs,
    generate_hit_wav, generate_title_logo, generate_whistle_wav,
};

pub const BLOBBY_COLORS: [u32; 8] = [
    0xFF0000, // 0: Red
    0xFF7F00, // 1: Orange
    0xFFFF00, // 2: Yellow
    0x00FF00, // 3: Green
    0x00ECFF, // 4: Cyan
    0x0000FF, // 5: Blue
    0xC000FF, // 6: Purple
    0xFF00D8, // 7: Magenta
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    French = 0,
    German = 1,
    English = 2,
}

impl Language {
    pub fn next(self) -> Self {
        match self {
            Language::French => Language::German,
            Language::German => Language::English,
            Language::English => Language::French,
        }
    }
}

pub struct LocalizedText {
    sections: HashMap<String, Vec<String>>,
}

impl LocalizedText {
    pub fn french() -> Self {
        Self::from_pairs(&[
            ("@main", &["Jouer", "Jeu en reseau", "Statistiques", "Reglages", "Deutsch", "Fin"]),
            ("@settings", &["< Joueur 1", "> Joueur 2", "Nom:", "Controle:", "definir Touches ...", "Son:", "Ok", "Clavier", "Souris", "Comp.(facile)", "Comp.(moyen)", "Comp.(difficile)", "marche", "arret"]),
            ("@keydef", &["Joueur 1 (Host)", "Joueur 2 (invite)", " gauche", " droite", " Saut"]),
            ("@win", &["gagne"]),
            ("@nachfrage", &["Vraiment\nquitter\nle jeu?"]),
            ("@names", &["Adam", "Bert", "Carl"]),
            ("@stats", &["Nom", "Jr.", "ga.", "Points", "²", "³", "Ok", "Jeu contre joueur", "Jeu contre Adam (facile)", "Jeu contre Bert (moyen)", "Jeu contre Carl (difficile)", "Tout les jeux"]),
            ("@mpsettings", &["Options-Multijoueur", "Jouer reseau...", "Jouer INVITE...", "retour"]),
            ("@hostsettings", &["Reglages serveur", "Nom du jeu:", "controles:", "jouer!", "retour"]),
            ("@clientsettings", &["reglages invite", "Host-IP:", "controles:", "Chercher jeux", "retour"]),
            ("@clientgamelist", &["jeux trouves:", "retour", "pas de jeux trouves"]),
            ("@hostwaiting", &["attente invite...", "(Annuler avec ESC)"]),
        ])
    }

    pub fn german() -> Self {
        Self::from_pairs(&[
            ("@main", &["Spiel starten", "Netzwerkspiel", "Statistik", "Einstellungen", "English Please", "Ende"]),
            ("@settings", &["< Spieler 1", "> Spieler 2", "Name:", "Steuerung:", "Tasten definieren...", "Sound:", "Ok", "Tastatur", "Maus", "Comp.(leicht)", "Comp.(mittel)", "Comp.(schwer)", "an", "aus"]),
            ("@keydef", &["Spieler 1 (Host)", "Spieler 2 (Gast)", " links", " rechts", " Sprung"]),
            ("@win", &["gewinnt"]),
            ("@nachfrage", &["Spiel\nwirklich\nbeenden?"]),
            ("@names", &["Adam", "Bert", "Carl"]),
            ("@stats", &["Name", "Sp.", "gew.", "Punkte", "²", "³", "Ok", "Spiele gegen Menschen", "Spiele gegen Adam (leicht)", "Spiele gegen Bert (mittel)", "Spiele gegen Carl (schwer)", "alle Spiele"]),
            ("@mpsettings", &["Multiplayer-Optionen", "Ein Spiel hosten...", "Als Gast spielen...", "zurück"]),
            ("@hostsettings", &["Host-Einstellungen", "Name des Spiels:", "Steuerung:", "Spiel beginnen!", "Zurück"]),
            ("@clientsettings", &["Gast-Einstellungen", "Host-IP:", "Steuerung:", "Spiele suchen", "Zurück"]),
            ("@clientgamelist", &["Gefundene Spiele:", "zurück", "keine Spiele gefunden"]),
            ("@hostwaiting", &["Warte auf einen Gast...", "(Abbruch mit ESC)"]),
        ])
    }

    pub fn english() -> Self {
        Self::from_pairs(&[
            ("@main", &["Start Game", "Network Match", "Statistics", "Settings", "Francais", "Quit"]),
            ("@settings", &["< Player 1", "> Player 2", "Name:", "Control:", "Define keys...", "Sound:", "Ok", "Keyboard", "Mouse", "Comp.(easy)", "Comp.(avg.)", "Comp.(hard)", "on", "off"]),
            ("@keydef", &["Player 1 (host)", "Player 2 (client)", " left", " right", " jump"]),
            ("@win", &["wins"]),
            ("@nachfrage", &["Really quit?"]),
            ("@names", &["Adam", "Bert", "Carl"]),
            ("@stats", &["name", "gms", "won", "score", "²", "³", "ok", "matches against humans", "matches against Adam (easy)", "matches against Bert (avg.)", "matches against Carl (hard)", "all matches"]),
            ("@mpsettings", &["Multiplayer Options", "Host a game...", "Join a game...", "Back"]),
            ("@hostsettings", &["Host Settings", "Game name:", "Control:", "Start Hosting!", "Back"]),
            ("@clientsettings", &["Client Settings", "Host-IP:", "Control:", "Find Sessions", "Back"]),
            ("@clientgamelist", &["sessions found:", "back", "no sessions found."]),
            ("@hostwaiting", &["Waiting for Client... ", "(press ESC to cancel)"]),
        ])
    }

    fn from_pairs(pairs: &[(&str, &[&str])]) -> Self {
        let mut sections = HashMap::new();
        for &(k, vals) in pairs {
            sections.insert(
                k.to_string(),
                vals.iter().map(|&s| s.to_string()).collect(),
            );
        }
        Self { sections }
    }

    pub fn get(&self, section: &str, idx: usize) -> &str {
        self.sections
            .get(section)
            .and_then(|v| v.get(idx))
            .map(|s| s.as_str())
            .unwrap_or("")
    }
}

pub struct GameAssets {
    pub bg: Vec<u32>,                       // 800x600 top-down (matches Framebuffer)
    pub bg_dark: Vec<u32>,                  // 800x600 top-down (1/8 brightness for menus)
    pub ball_frames: Vec<Sprite>,         // 16 frames of 64x64
    pub blobby_base: Vec<Sprite>,         // 5 frames of 148x101 (grayscale base)
    pub blobby_colored: [Vec<Sprite>; 2], // 5 frames per player
    pub font_glyphs: Vec<Sprite>,         // 55 glyphs of 24x24
    pub font_icon_base: Sprite,           // base glyph 51 (0x33) for player color icons
    pub blobby_shadows: Vec<Sprite>,      // 10 frames of 129x25 (actually 5 mirrored/frames)
    pub ball_shadow: Sprite,              // 69x17
    pub cursor: Sprite,                   // 48x48
    pub title_logo: Sprite,               // entry 89 (410x230)
    pub ending_splash: Sprite,            // entry 90 (400x300)
    pub sounds: [Arc<[u8]>; 2],             // 0: whistle, 1: hit/menu click
    pub texts: [LocalizedText; 3],          // 0: FR, 1: DE, 2: EN
}

pub fn find_settings_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("bv_settings.txt").exists() {
        return cwd;
    }
    let orig = cwd.join("original");
    if orig.join("bv_settings.txt").exists() {
        return orig;
    }
    let manifest_orig = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("original");
    if manifest_orig.join("bv_settings.txt").exists() {
        return manifest_orig;
    }
    cwd
}

pub fn tint_pixel(color: u32, pixel: u32) -> u32 {
    let b = (pixel & 0xFF) as i32;
    let g = ((pixel >> 8) & 0xFF) as i32;
    let r = ((pixel >> 16) & 0xFF) as i32;
    let highlight = (b * 5 - 0x4B2).max(0);
    let cb = (color & 0xFF) as i32;
    let cg = ((color >> 8) & 0xFF) as i32;
    let cr = ((color >> 16) & 0xFF) as i32;
    let out_r = (((r * cr) >> 8) + highlight).min(255) as u32;
    let out_g = (((g * cg) >> 8) + highlight).min(255) as u32;
    let out_b = (((b * cb) >> 8) + highlight).min(255) as u32;
    (out_r << 16) | (out_g << 8) | out_b
}

impl GameAssets {
    /// Generates all game graphics, audio, and localization tables procedurally
    /// with zero runtime dependency on external `.pak` files.
    pub fn generate() -> Self {
        let bg = generate_background();
        let bg_dark: Vec<u32> = bg.iter().map(|&p| (p & 0x00F8F8F8) >> 3).collect();
        let ball_frames = generate_ball_frames();
        let blobby_base = generate_blobby_frames();
        let font_glyphs = generate_font_glyphs(&blobby_base[0]);
        let font_icon_base = font_glyphs[0x33].clone();
        let blobby_shadows = generate_blobby_shadows(&blobby_base);
        let ball_shadow = generate_ball_shadow();
        let cursor = generate_cursor();
        let title_logo = generate_title_logo();
        let ending_splash = generate_ending_splash();

        let sounds = [
            Arc::from(generate_whistle_wav().into_boxed_slice()),
            Arc::from(generate_hit_wav().into_boxed_slice()),
        ];

        let texts = [
            LocalizedText::french(),
            LocalizedText::german(),
            LocalizedText::english(),
        ];

        let mut assets = Self {
            bg,
            bg_dark,
            ball_frames,
            blobby_base,
            blobby_colored: [Vec::new(), Vec::new()],
            font_glyphs,
            font_icon_base,
            blobby_shadows,
            ball_shadow,
            cursor,
            title_logo,
            ending_splash,
            sounds,
            texts,
        };
        assets.recolor_player(0, 0);
        assets.recolor_player(1, 3);
        assets
    }

    pub fn recolor_player(&mut self, player_idx: usize, color_idx: usize) {
        let color = BLOBBY_COLORS[color_idx % BLOBBY_COLORS.len()];
        let mut colored_frames = Vec::with_capacity(5);
        for base in &self.blobby_base {
            let mut img = base.clone();
            for p in &mut img.pixels {
                let sum = (*p & 0xFF) + ((*p >> 8) & 0xFF) + ((*p >> 16) & 0xFF);
                if sum < 0x28 {
                    *p = 0;
                } else {
                    *p = tint_pixel(color, *p);
                    if *p == 0 {
                        *p = 1;
                    }
                }
            }
            colored_frames.push(img);
        }
        self.blobby_colored[player_idx] = colored_frames;

        // Also tint the player's little blobby icon in font_glyphs[0x33 + player_idx]
        let mut icon = self.font_icon_base.clone();
        for p in &mut icon.pixels {
            if *p != 0 {
                *p = tint_pixel(color, *p);
            }
        }
        self.font_glyphs[0x33 + player_idx] = icon;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_text_tables_have_expected_entries() {
        let fr = LocalizedText::french();
        let de = LocalizedText::german();
        let en = LocalizedText::english();
        assert_eq!(fr.get("@main", 0), "Jouer");
        assert_eq!(de.get("@main", 0), "Spiel starten");
        assert_eq!(en.get("@main", 0), "Start Game");
        assert_eq!(en.get("@names", 0), "Adam");
        assert_eq!(en.get("@missing", 0), "");
        assert!(!fr.get("@nachfrage", 0).contains("\\n"));
        assert!(!de.get("@nachfrage", 0).contains("\\n"));
        assert_eq!(fr.get("@nachfrage", 0).lines().count(), 3);
        assert_eq!(de.get("@nachfrage", 0).lines().count(), 3);
    }

    #[test]
    fn game_assets_generate_is_deterministic_and_valid() {
        let a = GameAssets::generate();
        let b = GameAssets::generate();
        assert_eq!(a.bg.len(), 800 * 600);
        assert_eq!(a.bg, b.bg);
        assert_eq!(a.ball_frames.len(), 16);
        assert_eq!((a.ball_frames[0].width, a.ball_frames[0].height), (64, 64));
        assert_eq!(a.ball_frames, b.ball_frames);
        assert_eq!(a.blobby_base.len(), 5);
        assert_eq!((a.blobby_base[0].width, a.blobby_base[0].height), (148, 101));
        assert_eq!(a.blobby_shadows.len(), 10);
        assert_eq!((a.ball_shadow.width, a.ball_shadow.height), (69, 17));
        assert_eq!(a.font_glyphs.len(), 55);
        assert_eq!((a.cursor.width, a.cursor.height), (48, 48));
        assert_eq!((a.title_logo.width, a.title_logo.height), (410, 230));
        assert_eq!((a.ending_splash.width, a.ending_splash.height), (400, 300));
        assert_eq!(&a.sounds[0][0..4], b"RIFF");
        assert_eq!(&a.sounds[1][0..4], b"RIFF");
    }

    #[test]
    fn tint_pixel_preserves_black_and_adds_highlight() {
        assert_eq!(tint_pixel(0xFF0000, 0x000000), 0);
        // White pixel (255,255,255) with red color gets highlight on all channels
        let tinted = tint_pixel(0xFF0000, 0xFFFFFF);
        assert_eq!(tinted >> 16, 255);
        assert!(tinted & 0xFF > 0);
    }
}
