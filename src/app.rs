use std::path::PathBuf;
use std::time::{Duration, Instant};

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window};

use crate::assets::GameAssets;
use crate::audio::SoundEngine;
use crate::net::{ClientStatePacket, NetClient, NetHost};
use crate::render::{Framebuffer, CHAR_ADVANCE, HEIGHT, LINE_HEIGHT, WIDTH};
use crate::settings::{key_display_name, GameSettings};
use crate::sim::{MatchState, PlayerControl, PlayerInput};

const TICK_DURATION: Duration = Duration::from_millis(33);

const MAIN_MENU_Y: [i32; 6] = [310, 265, 220, 175, 130, 85];
const SETTINGS_Y: [i32; 7] = [480, 440, 320, 280, 180, 140, 40];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    Settings,
    KeyDef,
    Statistics,
    MultiplayerMenu,
    HostSettings,
    HostWaiting,
    ClientSettings,
    ClientGameList,
    Playing,
    ConfirmQuit,
    MatchWin,
    EndingSplash,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextTarget {
    PlayerName(usize),
    HostGameName,
    ClientHostIp,
}

struct TextEditState {
    target: TextTarget,
    buffer: String,
    max_len: usize,
    x: i32,
    y: i32,
}

pub struct App {
    assets: GameAssets,
    asset_dir: PathBuf,
    settings: GameSettings,
    sound: SoundEngine,
    fb: Framebuffer,
    screen: Screen,

    // Menu cursors
    main_sel: usize,
    settings_sel: usize,
    mp_sel: usize,
    host_sel: usize,
    client_sel: usize,
    list_sel: usize,

    // Text editing
    editing: Option<TextEditState>,

    // Key definition
    keydef_step: usize,
    keydef_captured: Vec<Key>,

    // Statistics view
    stats_bucket: usize,
    stats_sort_col: usize, // 0=name, 1=gms, 2=won, 3=score
    stats_offset: usize,

    // Gameplay
    match_state: Option<MatchState>,
    accumulator: Duration,
    last_frame: Instant,
    start_instant: Instant,
    prev_mouse_down: bool,

    // Match win animation
    win_player: usize,
    win_y: f64,
    win_vy: f64,
    win_step: usize,

    // Ending splash
    splash_start: Option<Instant>,

    // Networking
    net_host: Option<NetHost>,
    net_client: Option<NetClient>,
    is_net_client: bool,
    client_remote_match: Option<MatchState>,

    pub should_quit: bool,
}

impl App {
    pub fn new(assets: GameAssets, asset_dir: PathBuf) -> Self {
        let settings = GameSettings::load_from_dir(&asset_dir);
        let sound = SoundEngine::new(settings.sound_enabled);
        let mut app = Self {
            assets,
            asset_dir,
            settings,
            sound,
            fb: Framebuffer::new(),
            screen: Screen::MainMenu,
            main_sel: 0,
            settings_sel: 6,
            mp_sel: 0,
            host_sel: 0,
            client_sel: 0,
            list_sel: 0,
            editing: None,
            keydef_step: 0,
            keydef_captured: Vec::new(),
            stats_bucket: 4,
            stats_sort_col: 3,
            stats_offset: 0,
            match_state: None,
            accumulator: Duration::ZERO,
            last_frame: Instant::now(),
            start_instant: Instant::now(),
            prev_mouse_down: false,
            win_player: 0,
            win_y: 0.0,
            win_vy: 0.0,
            win_step: 0,
            splash_start: None,
            net_host: None,
            net_client: None,
            is_net_client: false,
            client_remote_match: None,
            should_quit: false,
        };
        app.assets.recolor_player(0, app.settings.player_colors[0]);
        app.assets.recolor_player(1, app.settings.player_colors[1]);
        app
    }

    fn lang(&self) -> usize {
        self.settings.language as usize
    }

    fn t(&self, section: &str, idx: usize) -> &str {
        self.assets.texts[self.lang()].get(section, idx)
    }

    fn play_click(&self) {
        self.sound.play_wav(&self.assets.sounds[1]);
    }

    fn play_whistle(&self) {
        self.sound.play_wav(&self.assets.sounds[0]);
    }

    pub fn update(&mut self, window: &Window) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_frame);
        self.last_frame = now;

        let (mx_f, my_f) = window.get_mouse_pos(MouseMode::Clamp).unwrap_or((400.0, 300.0));
        let mx = (mx_f as i32).clamp(0, WIDTH as i32 - 1);
        let my_bu = (HEIGHT as i32 - 1 - (my_f as i32)).clamp(0, HEIGHT as i32 - 1);
        let mouse_down = window.get_mouse_down(MouseButton::Left);
        let mouse_clicked = mouse_down && !self.prev_mouse_down;
        self.prev_mouse_down = mouse_down;

        if self.editing.is_some() {
            self.update_text_edit(window);
            return;
        }

        match self.screen {
            Screen::MainMenu => self.update_main_menu(window, mx, my_bu, mouse_clicked),
            Screen::Settings => self.update_settings(window, mx, my_bu, mouse_clicked),
            Screen::KeyDef => self.update_keydef(window),
            Screen::Statistics => self.update_statistics(window, mx, my_bu, mouse_clicked),
            Screen::MultiplayerMenu => self.update_mp_menu(window, mx, my_bu, mouse_clicked),
            Screen::HostSettings => self.update_host_settings(window, mx, my_bu, mouse_clicked),
            Screen::HostWaiting => self.update_host_waiting(window),
            Screen::ClientSettings => self.update_client_settings(window, mx, my_bu, mouse_clicked),
            Screen::ClientGameList => self.update_client_gamelist(window, mx, my_bu, mouse_clicked),
            Screen::Playing => self.update_playing(window, dt, mx, mouse_down),
            Screen::ConfirmQuit => self.update_confirm_quit(window),
            Screen::MatchWin => self.update_match_win(window, mouse_clicked),
            Screen::EndingSplash => self.update_ending_splash(window, mouse_clicked),
        }
    }

    pub fn framebuffer(&self) -> &[u32] {
        &self.fb.pixels
    }

    fn update_main_menu(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        for (i, &y) in MAIN_MENU_Y.iter().enumerate() {
            if my_bu > y && my_bu < y + LINE_HEIGHT {
                self.main_sel = i;
            }
        }
        let mut activate = clicked;
        if window.is_key_pressed(Key::Up, KeyRepeat::No) {
            self.main_sel = (self.main_sel + 5) % 6;
            self.play_click();
        }
        if window.is_key_pressed(Key::Down, KeyRepeat::No) {
            self.main_sel = (self.main_sel + 1) % 6;
            self.play_click();
        }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            activate = true;
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.screen = Screen::EndingSplash;
            self.splash_start = Some(Instant::now());
            return;
        }

        if activate {
            match self.main_sel {
                0 => {
                    self.play_whistle();
                    self.start_local_match();
                }
                1 => {
                    self.play_click();
                    self.mp_sel = 0;
                    self.screen = Screen::MultiplayerMenu;
                }
                2 => {
                    self.play_click();
                    self.stats_offset = 0;
                    self.screen = Screen::Statistics;
                }
                3 => {
                    self.play_click();
                    self.settings_sel = 6;
                    self.screen = Screen::Settings;
                }
                4 => {
                    self.play_click();
                    self.settings.language = self.settings.language.next();
                    self.settings.save_to_dir(&self.asset_dir);
                }
                5 => {
                    self.play_whistle();
                    self.screen = Screen::EndingSplash;
                    self.splash_start = Some(Instant::now());
                }
                _ => {}
            }
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 0);
        for i in 0..6 {
            let label = self.t("@main", i).to_string();
            let half_w = (label.chars().count() as i32 * CHAR_ADVANCE) / 2;
            self.fb.draw_text(
                &self.assets,
                500 - half_w,
                MAIN_MENU_Y[i],
                false,
                i == self.main_sel,
                &label,
            );
        }
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn start_local_match(&mut self) {
        let seed = self.start_instant.elapsed().subsec_nanos().max(1);
        self.match_state = Some(MatchState::new(self.settings.controls, seed));
        self.is_net_client = false;
        self.accumulator = Duration::ZERO;
        self.screen = Screen::Playing;
    }

    fn update_settings(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        for (i, &y) in SETTINGS_Y.iter().enumerate() {
            if my_bu >= y && my_bu < y + 30 {
                self.settings_sel = i;
            }
        }
        let mut activate = clicked;
        if window.is_key_pressed(Key::Up, KeyRepeat::No) {
            self.settings_sel = (self.settings_sel + 6) % 7;
            self.play_click();
        }
        if window.is_key_pressed(Key::Down, KeyRepeat::No) {
            self.settings_sel = (self.settings_sel + 1) % 7;
            self.play_click();
        }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            activate = true;
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.settings.save_to_dir(&self.asset_dir);
            self.screen = Screen::MainMenu;
            return;
        }

        if activate {
            // Color icons at y = 520 (P1) and y = 360 (P2)
            if clicked && my_bu >= 520 && my_bu < 544 {
                self.play_click();
                self.settings.player_colors[0] = (self.settings.player_colors[0] + 1) % 8;
                self.assets.recolor_player(0, self.settings.player_colors[0]);
            } else if clicked && my_bu >= 360 && my_bu < 384 {
                self.play_click();
                self.settings.player_colors[1] = (self.settings.player_colors[1] + 1) % 8;
                self.assets.recolor_player(1, self.settings.player_colors[1]);
            } else {
                self.play_click();
                match self.settings_sel {
                    0 => {
                        if self.settings.controls[0].is_ai() {
                            while self.settings.controls[0].is_ai() {
                                self.settings.controls[0] =
                                    self.settings.controls[0].next_in_settings(self.settings.controls[1]);
                            }
                        }
                        let prefix_len = self.t("@settings", 2).chars().count() as i32;
                        self.editing = Some(TextEditState {
                            target: TextTarget::PlayerName(0),
                            buffer: self.settings.player_names[0].clone(),
                            max_len: 12,
                            x: 220 + prefix_len * CHAR_ADVANCE,
                            y: SETTINGS_Y[0],
                        });
                    }
                    1 => {
                        self.settings.controls[0] =
                            self.settings.controls[0].next_in_settings(self.settings.controls[1]);
                    }
                    2 => {
                        if self.settings.controls[1].is_ai() {
                            while self.settings.controls[1].is_ai() {
                                self.settings.controls[1] =
                                    self.settings.controls[1].next_in_settings(self.settings.controls[0]);
                            }
                        }
                        let prefix_len = self.t("@settings", 2).chars().count() as i32;
                        self.editing = Some(TextEditState {
                            target: TextTarget::PlayerName(1),
                            buffer: self.settings.player_names[1].clone(),
                            max_len: 12,
                            x: 220 + prefix_len * CHAR_ADVANCE,
                            y: SETTINGS_Y[2],
                        });
                    }
                    3 => {
                        self.settings.controls[1] =
                            self.settings.controls[1].next_in_settings(self.settings.controls[0]);
                    }
                    4 => {
                        self.keydef_step = 0;
                        self.keydef_captured.clear();
                        self.screen = Screen::KeyDef;
                        return;
                    }
                    5 => {
                        self.settings.sound_enabled = !self.settings.sound_enabled;
                        self.sound.enabled = self.settings.sound_enabled;
                    }
                    6 => {
                        self.settings.save_to_dir(&self.asset_dir);
                        self.screen = Screen::MainMenu;
                        return;
                    }
                    _ => {}
                }
            }
        }

        self.render_settings();
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_settings(&mut self) {
        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);

        let p1_hdr = self.t("@settings", 0).to_string();
        let p2_hdr = self.t("@settings", 1).to_string();
        self.fb.draw_text(&self.assets, 220, 520, false, false, &p1_hdr);
        self.fb.draw_text(&self.assets, 220, 360, false, false, &p2_hdr);

        let name_lbl = self.t("@settings", 2).to_string();
        let ctrl_lbl = self.t("@settings", 3).to_string();
        let def_lbl = self.t("@settings", 4).to_string();
        let snd_lbl = self.t("@settings", 5).to_string();
        let ok_lbl = self.t("@settings", 6).to_string();

        let p1_name = self.player_display_name(0);
        let p2_name = self.player_display_name(1);
        let c1_name = self.t("@settings", 7 + self.settings.controls[0] as usize).to_string();
        let c2_name = self.t("@settings", 7 + self.settings.controls[1] as usize).to_string();
        let snd_val = self.t("@settings", if self.settings.sound_enabled { 12 } else { 13 }).to_string();

        let lines = [
            format!("{name_lbl}{p1_name}"),
            format!("{ctrl_lbl}{c1_name}"),
            format!("{name_lbl}{p2_name}"),
            format!("{ctrl_lbl}{c2_name}"),
            def_lbl,
            format!("{snd_lbl}{snd_val}"),
            ok_lbl,
        ];
        for (i, text) in lines.iter().enumerate() {
            self.fb.draw_text(
                &self.assets,
                220,
                SETTINGS_Y[i],
                false,
                i == self.settings_sel,
                text,
            );
        }
    }

    fn player_display_name(&self, idx: usize) -> String {
        match self.settings.controls[idx] {
            PlayerControl::AiEasy => self.t("@names", 0).to_string(),
            PlayerControl::AiMedium => self.t("@names", 1).to_string(),
            PlayerControl::AiHard => self.t("@names", 2).to_string(),
            _ => self.settings.player_names[idx].clone(),
        }
    }

    fn update_text_edit(&mut self, window: &Window) {
        let Some(mut ed) = self.editing.take() else { return };
        let mut done = false;
        let mut cancelled = false;

        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            cancelled = true;
            done = true;
        } else if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            done = true;
        } else if window.is_key_pressed(Key::Backspace, KeyRepeat::Yes) {
            ed.buffer.pop();
        } else if ed.buffer.chars().count() < ed.max_len {
            let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
            for k in window.get_keys_pressed(KeyRepeat::Yes) {
                if let Some(ch) = key_to_char(k, shift) {
                    ed.buffer.push(ch);
                    break;
                }
            }
        }

        // Render underlying screen + live edit buffer with blinking cursor
        match self.screen {
            Screen::Settings => self.render_settings(),
            Screen::HostSettings => self.render_host_settings(),
            Screen::ClientSettings => self.render_client_settings(),
            _ => {}
        }
        let blink = (self.start_instant.elapsed().as_millis() & 0x100) == 0;
        let display = if blink { format!("{}_", ed.buffer) } else { ed.buffer.clone() };
        self.fb.darken_rect_bu(ed.x, ed.y, 780, ed.y + 26, 2);
        self.fb.draw_text(&self.assets, ed.x, ed.y, false, true, &display);

        if done {
            if !cancelled {
                match ed.target {
                    TextTarget::PlayerName(i) => {
                        if !ed.buffer.trim().is_empty() {
                            self.settings.player_names[i] = ed.buffer;
                        }
                    }
                    TextTarget::HostGameName => self.settings.host_game_name = ed.buffer,
                    TextTarget::ClientHostIp => self.settings.client_host_ip = ed.buffer,
                }
            }
        } else {
            self.editing = Some(ed);
        }
    }

    fn update_keydef(&mut self, window: &Window) {
        for k in window.get_keys_pressed(KeyRepeat::No) {
            if k == Key::Escape {
                self.screen = Screen::Settings;
                return;
            }
            if !self.keydef_captured.contains(&k) {
                self.keydef_captured.push(k);
                self.play_click();
                self.keydef_step += 1;
                if self.keydef_step == 6 {
                    self.settings.keys.p1_left = self.keydef_captured[0];
                    self.settings.keys.p1_right = self.keydef_captured[1];
                    self.settings.keys.p1_jump = self.keydef_captured[2];
                    self.settings.keys.p2_left = self.keydef_captured[3];
                    self.settings.keys.p2_right = self.keydef_captured[4];
                    self.settings.keys.p2_jump = self.keydef_captured[5];
                    self.settings.save_to_dir(&self.asset_dir);
                    self.screen = Screen::Settings;
                    return;
                }
                break;
            }
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        for p in 0..2 {
            let hdr = self.t("@keydef", p).to_string();
            self.fb.draw_text(&self.assets, 100, 400 - (p as i32) * 150, false, false, &hdr);
            for s in 0..3 {
                let step = p * 3 + s;
                if step <= self.keydef_step {
                    let y = 400 - ((s as i32 + 1) * 32) - (p as i32) * 150;
                    let lbl = self.t("@keydef", 2 + s).to_string();
                    self.fb.draw_text(&self.assets, 100, y, false, step == self.keydef_step, &lbl);
                    if step < self.keydef_captured.len() {
                        let kname = key_display_name(self.keydef_captured[step]);
                        self.fb.draw_text(&self.assets, 500, y, false, false, kname);
                    }
                }
            }
        }
    }

    fn update_statistics(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        if window.is_key_pressed(Key::Escape, KeyRepeat::No)
            || window.is_key_pressed(Key::Enter, KeyRepeat::No)
        {
            self.screen = Screen::MainMenu;
            return;
        }
        if clicked {
            // Headline at y=530 cycles opponent category
            if my_bu >= 520 && my_bu < 560 {
                self.stats_bucket = (self.stats_bucket + 1) % 5;
                self.stats_offset = 0;
            } else if my_bu >= 450 && my_bu < 490 {
                // Column headers
                if mx < 340 {
                    self.stats_sort_col = 0;
                } else if mx < 460 {
                    self.stats_sort_col = 1;
                } else if mx < 580 {
                    self.stats_sort_col = 2;
                } else {
                    self.stats_sort_col = 3;
                }
            } else if my_bu < 90 {
                if mx < 260 && self.stats_offset >= 9 {
                    self.stats_offset -= 9;
                } else if mx > 540 && self.stats_offset + 9 < self.settings.stats.len() {
                    self.stats_offset += 9;
                } else {
                    self.screen = Screen::MainMenu;
                    return;
                }
            }
        }

        let b = self.stats_bucket;
        let mut sorted = self.settings.stats.clone();
        sorted.sort_by(|a, b_e| match self.stats_sort_col {
            0 => a.name.cmp(&b_e.name),
            1 => b_e.gms[b].cmp(&a.gms[b]),
            2 => b_e.won[b].cmp(&a.won[b]),
            _ => b_e.score[b].cmp(&a.score[b]),
        });

        self.fb.copy_bg(&self.assets.bg_dark);
        let headline = self.t("@stats", 7 + b).to_string();
        let hw = (headline.chars().count() as i32 * CHAR_ADVANCE) / 2;
        self.fb.draw_text(&self.assets, 400 - hw, 530, false, true, &headline);

        let h_name = self.t("@stats", 0).to_string();
        let h_gms = self.t("@stats", 1).to_string();
        let h_won = self.t("@stats", 2).to_string();
        let h_score = self.t("@stats", 3).to_string();
        self.fb.draw_text(&self.assets, 80, 460, false, self.stats_sort_col == 0, &h_name);
        self.fb.draw_text(&self.assets, 360, 460, false, self.stats_sort_col == 1, &h_gms);
        self.fb.draw_text(&self.assets, 480, 460, false, self.stats_sort_col == 2, &h_won);
        self.fb.draw_text(&self.assets, 600, 460, false, self.stats_sort_col == 3, &h_score);

        for (row_idx, entry) in sorted.iter().skip(self.stats_offset).take(9).enumerate() {
            let y = 410 - (row_idx as i32) * 34;
            self.fb.draw_text(&self.assets, 80, y, false, false, &entry.name);
            self.fb.draw_text(&self.assets, 360, y, false, false, &entry.gms[b].to_string());
            self.fb.draw_text(&self.assets, 480, y, false, false, &entry.won[b].to_string());
            self.fb.draw_text(&self.assets, 600, y, false, false, &entry.score[b].to_string());
        }

        let ok_lbl = self.t("@stats", 6).to_string();
        self.fb.draw_text(&self.assets, 380, 50, false, true, &ok_lbl);
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn update_mp_menu(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        let ys = [320, 260, 180];
        for (i, &y) in ys.iter().enumerate() {
            if my_bu >= y && my_bu < y + LINE_HEIGHT {
                self.mp_sel = i;
            }
        }
        let mut activate = clicked;
        if window.is_key_pressed(Key::Up, KeyRepeat::No) {
            self.mp_sel = (self.mp_sel + 2) % 3;
            self.play_click();
        }
        if window.is_key_pressed(Key::Down, KeyRepeat::No) {
            self.mp_sel = (self.mp_sel + 1) % 3;
            self.play_click();
        }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            activate = true;
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.screen = Screen::MainMenu;
            return;
        }
        if activate {
            self.play_click();
            match self.mp_sel {
                0 => {
                    self.host_sel = 0;
                    self.screen = Screen::HostSettings;
                }
                1 => {
                    self.client_sel = 0;
                    self.screen = Screen::ClientSettings;
                }
                2 => self.screen = Screen::MainMenu,
                _ => {}
            }
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        let title = self.t("@mpsettings", 0).to_string();
        self.fb.draw_text(&self.assets, 220, 420, false, false, &title);
        for (i, &y) in ys.iter().enumerate() {
            let lbl = self.t("@mpsettings", i + 1).to_string();
            self.fb.draw_text(&self.assets, 220, y, false, i == self.mp_sel, &lbl);
        }
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn update_host_settings(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        let ys = [360, 310, 220, 170];
        for (i, &y) in ys.iter().enumerate() {
            if my_bu >= y && my_bu < y + LINE_HEIGHT {
                self.host_sel = i;
            }
        }
        let mut activate = clicked;
        if window.is_key_pressed(Key::Up, KeyRepeat::No) {
            self.host_sel = (self.host_sel + 3) % 4;
            self.play_click();
        }
        if window.is_key_pressed(Key::Down, KeyRepeat::No) {
            self.host_sel = (self.host_sel + 1) % 4;
            self.play_click();
        }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            activate = true;
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.screen = Screen::MultiplayerMenu;
            return;
        }
        if activate {
            self.play_click();
            match self.host_sel {
                0 => {
                    let prefix_len = self.t("@hostsettings", 1).chars().count() as i32;
                    self.editing = Some(TextEditState {
                        target: TextTarget::HostGameName,
                        buffer: self.settings.host_game_name.clone(),
                        max_len: 12,
                        x: 200 + prefix_len * CHAR_ADVANCE,
                        y: ys[0],
                    });
                }
                1 => {
                    self.settings.host_control = if self.settings.host_control == PlayerControl::Keyboard {
                        PlayerControl::Mouse
                    } else {
                        PlayerControl::Keyboard
                    };
                }
                2 => {
                    if let Ok(host) = NetHost::start(
                        self.settings.host_game_name.clone(),
                        &self.settings.player_names[0],
                        self.settings.player_colors[0],
                    ) {
                        self.net_host = Some(host);
                        self.screen = Screen::HostWaiting;
                        return;
                    }
                }
                3 => {
                    self.screen = Screen::MultiplayerMenu;
                    return;
                }
                _ => {}
            }
        }
        self.render_host_settings();
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_host_settings(&mut self) {
        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        let title = self.t("@hostsettings", 0).to_string();
        self.fb.draw_text(&self.assets, 200, 440, false, false, &title);
        let ys = [360, 310, 220, 170];
        let ctrl_name = self.t("@settings", 7 + self.settings.host_control as usize).to_string();
        let lines = [
            format!("{}{}", self.t("@hostsettings", 1), self.settings.host_game_name),
            format!("{}{}", self.t("@hostsettings", 2), ctrl_name),
            self.t("@hostsettings", 3).to_string(),
            self.t("@hostsettings", 4).to_string(),
        ];
        for (i, line) in lines.iter().enumerate() {
            self.fb.draw_text(&self.assets, 200, ys[i], false, i == self.host_sel, line);
        }
    }

    fn update_host_waiting(&mut self, window: &Window) {
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.net_host = None;
            self.screen = Screen::HostSettings;
            return;
        }
        let joined = if let Some(host) = &mut self.net_host {
            host.poll_lobby(&self.settings.player_names[0], self.settings.player_colors[0])
        } else {
            false
        };
        if joined {
            if let Some(host) = &self.net_host {
                self.assets.recolor_player(1, host.peer_color);
            }
            self.play_whistle();
            let seed = self.start_instant.elapsed().subsec_nanos().max(1);
            self.match_state = Some(MatchState::new(
                [self.settings.host_control, PlayerControl::Network],
                seed,
            ));
            self.is_net_client = false;
            self.accumulator = Duration::ZERO;
            self.screen = Screen::Playing;
            return;
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        let l0 = self.t("@hostwaiting", 0).to_string();
        let l1 = self.t("@hostwaiting", 1).to_string();
        self.fb.draw_text(&self.assets, 200, 320, false, false, &l0);
        self.fb.draw_text(&self.assets, 200, 270, false, false, &l1);
    }

    fn update_client_settings(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        let ys = [360, 310, 220, 170];
        for (i, &y) in ys.iter().enumerate() {
            if my_bu >= y && my_bu < y + LINE_HEIGHT {
                self.client_sel = i;
            }
        }
        let mut activate = clicked;
        if window.is_key_pressed(Key::Up, KeyRepeat::No) {
            self.client_sel = (self.client_sel + 3) % 4;
            self.play_click();
        }
        if window.is_key_pressed(Key::Down, KeyRepeat::No) {
            self.client_sel = (self.client_sel + 1) % 4;
            self.play_click();
        }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            activate = true;
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.screen = Screen::MultiplayerMenu;
            return;
        }
        if activate {
            self.play_click();
            match self.client_sel {
                0 => {
                    let prefix_len = self.t("@clientsettings", 1).chars().count() as i32;
                    self.editing = Some(TextEditState {
                        target: TextTarget::ClientHostIp,
                        buffer: self.settings.client_host_ip.clone(),
                        max_len: 16,
                        x: 200 + prefix_len * CHAR_ADVANCE,
                        y: ys[0],
                    });
                }
                1 => {
                    self.settings.client_control =
                        if self.settings.client_control == PlayerControl::Keyboard {
                            PlayerControl::Mouse
                        } else {
                            PlayerControl::Keyboard
                        };
                }
                2 => {
                    if let Ok(mut client) = NetClient::new() {
                        client.send_discovery(&self.settings.client_host_ip);
                        self.net_client = Some(client);
                        self.list_sel = 0;
                        self.screen = Screen::ClientGameList;
                        return;
                    }
                }
                3 => {
                    self.screen = Screen::MultiplayerMenu;
                    return;
                }
                _ => {}
            }
        }
        self.render_client_settings();
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_client_settings(&mut self) {
        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        let title = self.t("@clientsettings", 0).to_string();
        self.fb.draw_text(&self.assets, 200, 440, false, false, &title);
        let ys = [360, 310, 220, 170];
        let ctrl_name = self.t("@settings", 7 + self.settings.client_control as usize).to_string();
        let lines = [
            format!("{}{}", self.t("@clientsettings", 1), self.settings.client_host_ip),
            format!("{}{}", self.t("@clientsettings", 2), ctrl_name),
            self.t("@clientsettings", 3).to_string(),
            self.t("@clientsettings", 4).to_string(),
        ];
        for (i, line) in lines.iter().enumerate() {
            self.fb.draw_text(&self.assets, 200, ys[i], false, i == self.client_sel, line);
        }
    }

    fn update_client_gamelist(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        if let Some(client) = &mut self.net_client {
            client.poll_discovery();
        }
        let count = self.net_client.as_ref().map(|c| c.sessions.len()).unwrap_or(0).min(10);
        let total = count + 1;

        for i in 0..total {
            let y = 400 - (i as i32) * 32;
            if my_bu > y && my_bu < y + LINE_HEIGHT {
                self.list_sel = i;
            }
        }
        let mut activate = clicked;
        if window.is_key_pressed(Key::Up, KeyRepeat::No) {
            self.list_sel = (self.list_sel + total - 1) % total;
            self.play_click();
        }
        if window.is_key_pressed(Key::Down, KeyRepeat::No) {
            self.list_sel = (self.list_sel + 1) % total;
            self.play_click();
        }
        if window.is_key_pressed(Key::Enter, KeyRepeat::No) {
            activate = true;
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.net_client = None;
            self.screen = Screen::ClientSettings;
            return;
        }

        if activate {
            if self.list_sel >= count {
                self.play_click();
                self.net_client = None;
                self.screen = Screen::ClientSettings;
                return;
            } else if let Some(client) = &mut self.net_client {
                let addr = client.sessions[self.list_sel].addr;
                if client.join(addr, &self.settings.player_names[1], self.settings.player_colors[1]) {
                    self.assets.recolor_player(0, client.host_color);
                    self.play_whistle();
                    self.client_remote_match = Some(MatchState::new(
                        [PlayerControl::Network, self.settings.client_control],
                        1,
                    ));
                    self.is_net_client = true;
                    self.screen = Screen::Playing;
                    return;
                }
            }
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        let hdr = self.t("@clientgamelist", 0).to_string();
        let hw = (hdr.chars().count() as i32 * CHAR_ADVANCE) / 2;
        self.fb.draw_text(&self.assets, 400 - hw, 500, false, false, &hdr);
        if count == 0 {
            let none = self.t("@clientgamelist", 2).to_string();
            let nw = (none.chars().count() as i32 * CHAR_ADVANCE) / 2;
            self.fb.draw_text(&self.assets, 400 - nw, 450, false, false, &none);
        }
        for i in 0..count {
            let name = self.net_client.as_ref().unwrap().sessions[i].name.clone();
            self.fb.draw_text(&self.assets, 300, 400 - (i as i32) * 32, false, i == self.list_sel, &name);
        }
        let back = self.t("@clientgamelist", 1).to_string();
        self.fb.draw_text(&self.assets, 300, 400 - (count as i32) * 32, false, self.list_sel == count, &back);
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn update_playing(&mut self, window: &Window, dt: Duration, mx: i32, mouse_down: bool) {
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.screen = Screen::ConfirmQuit;
            return;
        }

        let k = &self.settings.keys;
        let inp0 = PlayerInput {
            left: window.is_key_down(k.p1_left),
            right: window.is_key_down(k.p1_right),
            jump: window.is_key_down(k.p1_jump),
        };
        let inp1 = PlayerInput {
            left: window.is_key_down(k.p2_left),
            right: window.is_key_down(k.p2_right),
            jump: window.is_key_down(k.p2_jump),
        };

        if self.is_net_client {
            if let Some(client) = &self.net_client {
                let local_inp = if self.settings.client_control == PlayerControl::Mouse {
                    PlayerInput {
                        left: mx < 600, // simplified mouse relative or send keyboard
                        right: mx > 620,
                        jump: mouse_down,
                    }
                } else {
                    inp1
                };
                client.send_input(local_inp);
                if let Some(pkt) = client.poll_state() {
                    match pkt {
                        ClientStatePacket::Quit => {
                            self.net_client = None;
                            self.screen = Screen::MainMenu;
                            return;
                        }
                        ClientStatePacket::State {
                            ball_x, ball_y, p0_x, p0_y, p0_frame, p1_x, p1_y, p1_frame,
                            spin_frame, scores, server_raw, sound_mask,
                        } => {
                            if (sound_mask & 1) != 0 { self.play_whistle(); }
                            if (sound_mask & 2) != 0 { self.play_click(); }
                            if let Some(m) = &mut self.client_remote_match {
                                m.rally.ball.x = ball_x;
                                m.rally.ball.y = ball_y;
                                m.rally.ball.spin_phase = (spin_frame as u32) << 3;
                                m.rally.players[0].x = p0_x;
                                m.rally.players[0].y = p0_y;
                                m.rally.players[0].anim_frame = p0_frame;
                                m.rally.players[1].x = p1_x;
                                m.rally.players[1].y = p1_y;
                                m.rally.players[1].anim_frame = p1_frame;
                                m.rally.players[0].score = scores[0];
                                m.rally.players[1].score = scores[1];
                                m.rally.is_first_rally = server_raw == 0xFF;
                                if server_raw < 2 {
                                    m.rally.server = server_raw as usize;
                                }
                            }
                        }
                    }
                }
            }
            let tick_ms = self.start_instant.elapsed().as_millis() as u32;
            if let Some(m) = &self.client_remote_match {
                let host_name = self.net_client.as_ref().map(|c| c.host_name.clone()).unwrap_or_default();
                let names = [host_name, self.settings.player_names[1].clone()];
                self.fb.draw_match(&self.assets, m, &names, tick_ms);
            }
            return;
        }

        // Local or Host simulation
        let mut remote_inp = PlayerInput::default();
        if let Some(host) = &mut self.net_host {
            if host.poll_input() == Some(true) {
                self.net_host = None;
                self.match_state = None;
                self.screen = Screen::MainMenu;
                return;
            }
            remote_inp = host.remote_input;
        }

        self.accumulator += dt.min(Duration::from_millis(100));
        while self.accumulator >= TICK_DURATION {
            self.accumulator -= TICK_DURATION;
            let mut sound_mask = 0u8;
            if let Some(m) = &mut self.match_state {
                let inputs = if self.net_host.is_some() {
                    [inp0, remote_inp]
                } else {
                    [inp0, inp1]
                };
                let ev = m.step(inputs, mx, mouse_down);
                if ev.whistle {
                    self.sound.play_wav(&self.assets.sounds[0]);
                    sound_mask |= 1;
                }
                if ev.ball_hit {
                    self.sound.play_wav(&self.assets.sounds[1]);
                    sound_mask |= 2;
                }
                if let Some(host) = &self.net_host {
                    let r = &m.rally;
                    host.send_state(
                        &r.ball,
                        r.players[0].x, r.players[0].y, r.players[0].anim_frame,
                        r.players[1].x, r.players[1].y, r.players[1].anim_frame,
                        [r.players[0].score, r.players[1].score],
                        r.server,
                        r.is_first_rally,
                        sound_mask,
                    );
                }
                if let Some(winner) = m.winner {
                    let scores = [m.rally.players[0].score, m.rally.players[1].score];
                    if self.net_host.is_none() {
                        self.settings.record_match(scores);
                        self.settings.save_to_dir(&self.asset_dir);
                    }
                    self.net_host = None;
                    self.win_player = winner;
                    self.win_y = 0.0;
                    self.win_vy = 12.0;
                    self.win_step = 0;
                    self.screen = Screen::MatchWin;
                    return;
                }
            }
        }

        let tick_ms = self.start_instant.elapsed().as_millis() as u32;
        if let Some(m) = &self.match_state {
            let names = if let Some(host) = &self.net_host {
                [self.settings.player_names[0].clone(), host.peer_name.clone()]
            } else {
                self.settings.player_names.clone()
            };
            self.fb.draw_match(&self.assets, m, &names, tick_ms);
        }
    }

    fn update_confirm_quit(&mut self, window: &Window) {
        let tick_ms = self.start_instant.elapsed().as_millis() as u32;
        if let Some(m) = &self.match_state {
            self.fb.draw_match(&self.assets, m, &self.settings.player_names, tick_ms);
        }
        // Scanline + center box darkening from FUN_0044977c
        for y in (0..HEIGHT).step_by(2) {
            for x in 0..WIDTH {
                self.fb.pixels[y * WIDTH + x] = 0;
            }
        }
        self.fb.darken_rect_bu(150, 200, 650, 400, 2);
        let prompt = self.t("@nachfrage", 0).to_string();
        self.fb.draw_text(&self.assets, 250, 300, false, true, &prompt);

        let keys = window.get_keys_pressed(KeyRepeat::No);
        if !keys.is_empty() {
            let yes = keys.iter().any(|&k| {
                matches!(k, Key::Y | Key::J | Key::O | Key::Enter)
            });
            if yes {
                if let Some(host) = &self.net_host {
                    host.send_quit();
                }
                if let Some(client) = &self.net_client {
                    client.send_quit();
                }
                self.net_host = None;
                self.net_client = None;
                self.match_state = None;
                self.client_remote_match = None;
                self.assets.recolor_player(0, self.settings.player_colors[0]);
                self.assets.recolor_player(1, self.settings.player_colors[1]);
                self.screen = Screen::MainMenu;
            } else {
                self.screen = Screen::Playing;
            }
        }
    }

    fn update_match_win(&mut self, window: &Window, clicked: bool) {
        if clicked || !window.get_keys_pressed(KeyRepeat::No).is_empty() {
            self.match_state = None;
            self.screen = Screen::MainMenu;
            return;
        }
        const WIN_FRAMES: [usize; 8] = [0, 1, 2, 3, 4, 3, 2, 1];
        self.win_y += self.win_vy;
        self.win_vy -= 0.8;
        if self.win_y <= 0.0 {
            self.win_y = 0.0;
            self.win_vy = 0.0;
            self.win_step += 1;
            if self.win_step >= 8 {
                self.win_step = 0;
                self.win_vy = 12.0;
            }
        }

        self.fb.copy_bg(&self.assets.bg);
        self.fb.darken_rect_bu(0, 0, 800, 600, 1);
        self.fb.darken_rect_bu(200, 200, 600, 400, 2);

        let wname = self.player_display_name(self.win_player);
        let wins = self.t("@win", 0).to_string();
        self.fb.draw_text(&self.assets, 300, 320, false, false, &wname);
        self.fb.draw_text(&self.assets, 300, 270, false, false, &wins);

        let frame = WIN_FRAMES[self.win_step];
        self.fb.draw_sprite_bu(
            &self.assets.blobby_colored[self.win_player][frame],
            440,
            25 + self.win_y.round() as i32,
        );
    }

    fn update_ending_splash(&mut self, window: &Window, clicked: bool) {
        let elapsed = self.splash_start.map(|s| s.elapsed()).unwrap_or_default();
        if clicked
            || !window.get_keys_pressed(KeyRepeat::No).is_empty()
            || elapsed > Duration::from_secs(4)
        {
            self.should_quit = true;
            return;
        }
        self.fb.pixels.fill(0);
        self.fb.draw_sprite_bu(&self.assets.ending_splash, 200, 150);
    }
}

fn key_to_char(k: Key, shift: bool) -> Option<char> {
    let ch = match k {
        Key::A => 'a', Key::B => 'b', Key::C => 'c', Key::D => 'd',
        Key::E => 'e', Key::F => 'f', Key::G => 'g', Key::H => 'h',
        Key::I => 'i', Key::J => 'j', Key::K => 'k', Key::L => 'l',
        Key::M => 'm', Key::N => 'n', Key::O => 'o', Key::P => 'p',
        Key::Q => 'q', Key::R => 'r', Key::S => 's', Key::T => 't',
        Key::U => 'u', Key::V => 'v', Key::W => 'w', Key::X => 'x',
        Key::Y => 'y', Key::Z => 'z',
        Key::Key0 => '0', Key::Key1 => '1', Key::Key2 => '2', Key::Key3 => '3',
        Key::Key4 => '4', Key::Key5 => '5', Key::Key6 => '6', Key::Key7 => '7',
        Key::Key8 => '8', Key::Key9 => '9',
        Key::Period => '.',
        Key::Space => ' ',
        Key::Minus => '-',
        _ => return None,
    };
    if shift && ch.is_ascii_lowercase() {
        Some(ch.to_ascii_uppercase())
    } else {
        Some(ch)
    }
}
