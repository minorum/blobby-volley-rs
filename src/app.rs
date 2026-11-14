use std::path::PathBuf;
use std::sync::Arc;
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

enum NetRole {
    None,
    Host(NetHost),
    Client {
        client: NetClient,
        remote_match: Option<MatchState>,
    },
}

enum MenuAction {
    None,
    Activate,
    Back,
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
    net: NetRole,

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
            net: NetRole::None,
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

    fn draw_t(&mut self, x: i32, y: i32, highlight: bool, section: &str, idx: usize) {
        let lang = self.lang();
        self.fb.draw_text(
            &self.assets,
            x,
            y,
            false,
            highlight,
            self.assets.texts[lang].get(section, idx),
        );
    }

    fn play_click(&self) {
        self.sound.play_wav(&self.assets.sounds[1]);
    }

    fn play_whistle(&self) {
        self.sound.play_wav(&self.assets.sounds[0]);
    }

    fn sort_stats(&mut self) {
        let b = self.stats_bucket;
        let col = self.stats_sort_col;
        self.settings.stats.sort_by(|a, b_e| match col {
            0 => a.name.cmp(&b_e.name),
            1 => b_e.gms[b].cmp(&a.gms[b]),
            2 => b_e.won[b].cmp(&a.won[b]),
            _ => b_e.score[b].cmp(&a.score[b]),
        });
    }

    pub fn update(&mut self, window: &Window) {
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last_frame);
        self.last_frame = now;

        let (mx_f, my_f) = window
            .get_mouse_pos(MouseMode::Clamp)
            .unwrap_or((400.0, 300.0));
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
        match step_menu(
            window,
            my_bu,
            clicked,
            &MAIN_MENU_Y,
            LINE_HEIGHT,
            &mut self.main_sel,
            &self.sound,
            &self.assets.sounds[1],
        ) {
            MenuAction::Back => {
                self.screen = Screen::EndingSplash;
                self.splash_start = Some(Instant::now());
                return;
            }
            MenuAction::Activate => match self.main_sel {
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
                    self.sort_stats();
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
            },
            MenuAction::None => {}
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 0);
        let lang = self.lang();
        for (i, &y) in MAIN_MENU_Y.iter().enumerate() {
            let label = self.assets.texts[lang].get("@main", i);
            let half_w = (label.chars().count() as i32 * CHAR_ADVANCE) / 2;
            self.fb.draw_text(
                &self.assets,
                500 - half_w,
                y,
                false,
                i == self.main_sel,
                label,
            );
        }
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn start_local_match(&mut self) {
        let seed = self.start_instant.elapsed().subsec_nanos().max(1);
        self.match_state = Some(MatchState::new(self.settings.controls, seed));
        self.net = NetRole::None;
        self.accumulator = Duration::ZERO;
        self.screen = Screen::Playing;
    }

    fn update_settings(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        // Color icons at y = 520 (P1) and y = 360 (P2)
        if clicked && (520..544).contains(&my_bu) {
            self.play_click();
            self.settings.player_colors[0] = (self.settings.player_colors[0] + 1) % 8;
            self.assets
                .recolor_player(0, self.settings.player_colors[0]);
        } else if clicked && (360..384).contains(&my_bu) {
            self.play_click();
            self.settings.player_colors[1] = (self.settings.player_colors[1] + 1) % 8;
            self.assets
                .recolor_player(1, self.settings.player_colors[1]);
        } else {
            match step_menu(
                window,
                my_bu,
                clicked,
                &SETTINGS_Y,
                30,
                &mut self.settings_sel,
                &self.sound,
                &self.assets.sounds[1],
            ) {
                MenuAction::Back => {
                    self.settings.save_to_dir(&self.asset_dir);
                    self.screen = Screen::MainMenu;
                    return;
                }
                MenuAction::Activate => {
                    self.play_click();
                    match self.settings_sel {
                        0 | 2 => {
                            let idx = usize::from(self.settings_sel == 2);
                            let other = self.settings.controls[1 - idx];
                            while self.settings.controls[idx].is_ai() {
                                self.settings.controls[idx] =
                                    self.settings.controls[idx].next_in_settings(other);
                            }
                            let prefix_len = self.t("@settings", 2).chars().count() as i32;
                            self.editing = Some(TextEditState {
                                target: TextTarget::PlayerName(idx),
                                buffer: self.settings.player_names[idx].clone(),
                                max_len: 12,
                                x: 220 + prefix_len * CHAR_ADVANCE,
                                y: SETTINGS_Y[self.settings_sel],
                            });
                        }
                        1 | 3 => {
                            let idx = usize::from(self.settings_sel == 3);
                            let other = self.settings.controls[1 - idx];
                            self.settings.controls[idx] =
                                self.settings.controls[idx].next_in_settings(other);
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
                MenuAction::None => {}
            }
        }

        self.render_settings();
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_settings(&mut self) {
        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);

        self.draw_t(220, 520, false, "@settings", 0);
        self.draw_t(220, 360, false, "@settings", 1);

        let lang = self.lang();
        let texts = &self.assets.texts[lang];
        let name_lbl = texts.get("@settings", 2);
        let ctrl_lbl = texts.get("@settings", 3);
        let def_lbl = texts.get("@settings", 4);
        let snd_lbl = texts.get("@settings", 5);
        let ok_lbl = texts.get("@settings", 6);
        let snd_val = texts.get(
            "@settings",
            if self.settings.sound_enabled { 12 } else { 13 },
        );

        for (i, &y) in SETTINGS_Y.iter().enumerate() {
            let sel = i == self.settings_sel;
            match i {
                0 | 2 => {
                    let idx = usize::from(i == 2);
                    let pname = self.player_display_name(idx);
                    let line = format!("{name_lbl}{pname}");
                    self.fb.draw_text(&self.assets, 220, y, false, sel, &line);
                }
                1 | 3 => {
                    let idx = usize::from(i == 3);
                    let cname = texts.get("@settings", 7 + self.settings.controls[idx] as usize);
                    let line = format!("{ctrl_lbl}{cname}");
                    self.fb.draw_text(&self.assets, 220, y, false, sel, &line);
                }
                4 => self.fb.draw_text(&self.assets, 220, y, false, sel, def_lbl),
                5 => {
                    let line = format!("{snd_lbl}{snd_val}");
                    self.fb.draw_text(&self.assets, 220, y, false, sel, &line);
                }
                _ => self.fb.draw_text(&self.assets, 220, y, false, sel, ok_lbl),
            }
        }
    }

    fn player_display_name(&self, idx: usize) -> &str {
        match self.settings.controls[idx] {
            PlayerControl::AiEasy => self.t("@names", 0),
            PlayerControl::AiMedium => self.t("@names", 1),
            PlayerControl::AiHard => self.t("@names", 2),
            _ => &self.settings.player_names[idx],
        }
    }

    fn update_text_edit(&mut self, window: &Window) {
        let Some(mut ed) = self.editing.take() else {
            return;
        };
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
        let display = if blink {
            format!("{}_", ed.buffer)
        } else {
            ed.buffer.clone()
        };
        self.fb.darken_rect_bu(ed.x, ed.y, 780, ed.y + 26, 2);
        self.fb
            .draw_text(&self.assets, ed.x, ed.y, false, true, &display);

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
            self.draw_t(100, 400 - (p as i32) * 150, false, "@keydef", p);
            for s in 0..3 {
                let step = p * 3 + s;
                if step <= self.keydef_step {
                    let y = 400 - ((s as i32 + 1) * 32) - (p as i32) * 150;
                    self.draw_t(100, y, step == self.keydef_step, "@keydef", 2 + s);
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
            if (520..560).contains(&my_bu) {
                self.stats_bucket = (self.stats_bucket + 1) % 5;
                self.stats_offset = 0;
                self.sort_stats();
            } else if (450..490).contains(&my_bu) {
                // Column headers
                self.stats_sort_col = if mx < 340 {
                    0
                } else if mx < 460 {
                    1
                } else if mx < 580 {
                    2
                } else {
                    3
                };
                self.sort_stats();
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
        let lang = self.lang();
        self.fb.copy_bg(&self.assets.bg_dark);
        let headline = self.assets.texts[lang].get("@stats", 7 + b);
        let hw = (headline.chars().count() as i32 * CHAR_ADVANCE) / 2;
        self.fb
            .draw_text(&self.assets, 400 - hw, 530, false, true, headline);

        for (col, x) in [(0, 80), (1, 360), (2, 480), (3, 600)] {
            self.draw_t(x, 460, self.stats_sort_col == col, "@stats", col);
        }

        for (row_idx, entry) in self
            .settings
            .stats
            .iter()
            .skip(self.stats_offset)
            .take(9)
            .enumerate()
        {
            let y = 410 - (row_idx as i32) * 34;
            self.fb
                .draw_text(&self.assets, 80, y, false, false, &entry.name);
            self.fb.draw_text(
                &self.assets,
                360,
                y,
                false,
                false,
                &entry.gms[b].to_string(),
            );
            self.fb.draw_text(
                &self.assets,
                480,
                y,
                false,
                false,
                &entry.won[b].to_string(),
            );
            self.fb.draw_text(
                &self.assets,
                600,
                y,
                false,
                false,
                &entry.score[b].to_string(),
            );
        }

        self.draw_t(380, 50, true, "@stats", 6);
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn update_mp_menu(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        let ys = [320, 260, 180];
        match step_menu(
            window,
            my_bu,
            clicked,
            &ys,
            LINE_HEIGHT,
            &mut self.mp_sel,
            &self.sound,
            &self.assets.sounds[1],
        ) {
            MenuAction::Back => {
                self.screen = Screen::MainMenu;
                return;
            }
            MenuAction::Activate => {
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
            MenuAction::None => {}
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        self.draw_t(220, 420, false, "@mpsettings", 0);
        for (i, &y) in ys.iter().enumerate() {
            self.draw_t(220, y, i == self.mp_sel, "@mpsettings", i + 1);
        }
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn update_host_settings(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        let ys = [360, 310, 220, 170];
        match step_menu(
            window,
            my_bu,
            clicked,
            &ys,
            LINE_HEIGHT,
            &mut self.host_sel,
            &self.sound,
            &self.assets.sounds[1],
        ) {
            MenuAction::Back => {
                self.screen = Screen::MultiplayerMenu;
                return;
            }
            MenuAction::Activate => {
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
                        self.settings.host_control =
                            if self.settings.host_control == PlayerControl::Keyboard {
                                PlayerControl::Mouse
                            } else {
                                PlayerControl::Keyboard
                            };
                    }
                    2 => {
                        if let Ok(host) = NetHost::start(self.settings.host_game_name.clone()) {
                            self.net = NetRole::Host(host);
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
            MenuAction::None => {}
        }
        self.render_host_settings();
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_host_settings(&mut self) {
        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        self.draw_t(200, 440, false, "@hostsettings", 0);
        let ys = [360, 310, 220, 170];
        let lang = self.lang();
        let texts = &self.assets.texts[lang];
        let ctrl_name = texts.get("@settings", 7 + self.settings.host_control as usize);
        let l0 = format!(
            "{}{}",
            texts.get("@hostsettings", 1),
            self.settings.host_game_name
        );
        let l1 = format!("{}{}", texts.get("@hostsettings", 2), ctrl_name);
        self.fb
            .draw_text(&self.assets, 200, ys[0], false, self.host_sel == 0, &l0);
        self.fb
            .draw_text(&self.assets, 200, ys[1], false, self.host_sel == 1, &l1);
        self.draw_t(200, ys[2], self.host_sel == 2, "@hostsettings", 3);
        self.draw_t(200, ys[3], self.host_sel == 3, "@hostsettings", 4);
    }

    fn update_host_waiting(&mut self, window: &Window) {
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            self.net = NetRole::None;
            self.screen = Screen::HostSettings;
            return;
        }
        let joined_color = if let NetRole::Host(host) = &mut self.net {
            if host.poll_lobby(
                &self.settings.player_names[0],
                self.settings.player_colors[0],
            ) {
                Some(host.peer_color)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(peer_color) = joined_color {
            self.assets.recolor_player(1, peer_color);
            self.play_whistle();
            let seed = self.start_instant.elapsed().subsec_nanos().max(1);
            self.match_state = Some(MatchState::new(
                [self.settings.host_control, PlayerControl::Network],
                seed,
            ));
            self.accumulator = Duration::ZERO;
            self.screen = Screen::Playing;
            return;
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        self.draw_t(200, 320, false, "@hostwaiting", 0);
        self.draw_t(200, 270, false, "@hostwaiting", 1);
    }

    fn update_client_settings(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        let ys = [360, 310, 220, 170];
        match step_menu(
            window,
            my_bu,
            clicked,
            &ys,
            LINE_HEIGHT,
            &mut self.client_sel,
            &self.sound,
            &self.assets.sounds[1],
        ) {
            MenuAction::Back => {
                self.screen = Screen::MultiplayerMenu;
                return;
            }
            MenuAction::Activate => {
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
                            self.net = NetRole::Client {
                                client,
                                remote_match: None,
                            };
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
            MenuAction::None => {}
        }
        self.render_client_settings();
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_client_settings(&mut self) {
        self.fb.copy_bg(&self.assets.bg_dark);
        self.fb.draw_title_logo(&self.assets.title_logo, 2);
        self.draw_t(200, 440, false, "@clientsettings", 0);
        let ys = [360, 310, 220, 170];
        let lang = self.lang();
        let texts = &self.assets.texts[lang];
        let ctrl_name = texts.get("@settings", 7 + self.settings.client_control as usize);
        let l0 = format!(
            "{}{}",
            texts.get("@clientsettings", 1),
            self.settings.client_host_ip
        );
        let l1 = format!("{}{}", texts.get("@clientsettings", 2), ctrl_name);
        self.fb
            .draw_text(&self.assets, 200, ys[0], false, self.client_sel == 0, &l0);
        self.fb
            .draw_text(&self.assets, 200, ys[1], false, self.client_sel == 1, &l1);
        self.draw_t(200, ys[2], self.client_sel == 2, "@clientsettings", 3);
        self.draw_t(200, ys[3], self.client_sel == 3, "@clientsettings", 4);
    }

    fn update_client_gamelist(&mut self, window: &Window, mx: i32, my_bu: i32, clicked: bool) {
        if let NetRole::Client { client, .. } = &mut self.net {
            client.poll_discovery();
        }
        let count = match &self.net {
            NetRole::Client { client, .. } => client.sessions.len().min(10),
            _ => 0,
        };
        let total = count + 1;
        let mut ys = [0i32; 11];
        for (i, slot) in ys[..total].iter_mut().enumerate() {
            *slot = 400 - (i as i32) * 32;
        }

        match step_menu(
            window,
            my_bu,
            clicked,
            &ys[..total],
            LINE_HEIGHT,
            &mut self.list_sel,
            &self.sound,
            &self.assets.sounds[1],
        ) {
            MenuAction::Back => {
                self.net = NetRole::None;
                self.screen = Screen::ClientSettings;
                return;
            }
            MenuAction::Activate => {
                if self.list_sel >= count {
                    self.play_click();
                    self.net = NetRole::None;
                    self.screen = Screen::ClientSettings;
                    return;
                }
                let joined_color = if let NetRole::Client {
                    client,
                    remote_match,
                } = &mut self.net
                {
                    let addr = client.sessions[self.list_sel].addr;
                    if client.join(
                        addr,
                        &self.settings.player_names[1],
                        self.settings.player_colors[1],
                    ) {
                        *remote_match = Some(MatchState::new(
                            [PlayerControl::Network, self.settings.client_control],
                            1,
                        ));
                        Some(client.host_color)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(host_color) = joined_color {
                    self.assets.recolor_player(0, host_color);
                    self.play_whistle();
                    self.screen = Screen::Playing;
                    return;
                }
            }
            MenuAction::None => {}
        }

        self.fb.copy_bg(&self.assets.bg_dark);
        let lang = self.lang();
        let hdr = self.assets.texts[lang].get("@clientgamelist", 0);
        let hw = (hdr.chars().count() as i32 * CHAR_ADVANCE) / 2;
        self.fb
            .draw_text(&self.assets, 400 - hw, 500, false, false, hdr);
        if count == 0 {
            let none = self.assets.texts[lang].get("@clientgamelist", 2);
            let nw = (none.chars().count() as i32 * CHAR_ADVANCE) / 2;
            self.fb
                .draw_text(&self.assets, 400 - nw, 450, false, false, none);
        }
        if let NetRole::Client { client, .. } = &self.net {
            for (i, &y) in ys.iter().enumerate().take(count) {
                self.fb.draw_text(
                    &self.assets,
                    300,
                    y,
                    false,
                    i == self.list_sel,
                    &client.sessions[i].name,
                );
            }
        }
        self.draw_t(300, ys[count], self.list_sel == count, "@clientgamelist", 1);
        self.fb.draw_cursor(&self.assets.cursor, mx, my_bu);
    }

    fn render_active_match(&mut self) {
        let tick_ms = self.start_instant.elapsed().as_millis() as u32;
        match &self.net {
            NetRole::Client {
                client,
                remote_match: Some(m),
            } => {
                let names = [
                    client.host_name.clone(),
                    self.settings.player_names[1].clone(),
                ];
                self.fb.draw_match(&self.assets, m, &names, tick_ms);
            }
            NetRole::Host(host) => {
                if let Some(m) = &self.match_state {
                    let names = [
                        self.settings.player_names[0].clone(),
                        host.peer_name.clone(),
                    ];
                    self.fb.draw_match(&self.assets, m, &names, tick_ms);
                }
            }
            _ => {
                if let Some(m) = &self.match_state {
                    self.fb
                        .draw_match(&self.assets, m, &self.settings.player_names, tick_ms);
                }
            }
        }
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

        if let NetRole::Client {
            client,
            remote_match,
        } = &mut self.net
        {
            let local_inp = if self.settings.client_control == PlayerControl::Mouse {
                let p1_x = remote_match.as_ref().map_or(600, |m| m.rally.players[1].x);
                let target_x = (mx >> 1) + 400;
                PlayerInput {
                    left: target_x < p1_x - 9,
                    right: p1_x + 9 < target_x,
                    jump: mouse_down,
                }
            } else {
                inp1
            };
            client.send_input(local_inp);
            if let Some(pkt) = client.poll_state() {
                match pkt {
                    ClientStatePacket::Quit => {
                        self.net = NetRole::None;
                        self.screen = Screen::MainMenu;
                        return;
                    }
                    ClientStatePacket::State {
                        ball_x,
                        ball_y,
                        p0_x,
                        p0_y,
                        p0_frame,
                        p1_x,
                        p1_y,
                        p1_frame,
                        spin_frame,
                        scores,
                        server_raw,
                        sound_mask,
                    } => {
                        if (sound_mask & 1) != 0 {
                            self.sound.play_wav(&self.assets.sounds[0]);
                        }
                        if (sound_mask & 2) != 0 {
                            self.sound.play_wav(&self.assets.sounds[1]);
                        }
                        if let Some(m) = remote_match {
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
            self.render_active_match();
            return;
        }

        // Local or Host simulation
        let mut remote_inp = PlayerInput::default();
        if let NetRole::Host(host) = &mut self.net {
            if host.poll_input() == Some(true) {
                self.net = NetRole::None;
                self.match_state = None;
                self.screen = Screen::MainMenu;
                return;
            }
            remote_inp = host.remote_input;
        }
        let is_host = matches!(self.net, NetRole::Host(_));

        self.accumulator += dt.min(Duration::from_millis(100));
        while self.accumulator >= TICK_DURATION {
            self.accumulator -= TICK_DURATION;
            let mut sound_mask = 0u8;
            if let Some(m) = &mut self.match_state {
                let inputs = if is_host {
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
                if let NetRole::Host(host) = &self.net {
                    host.send_state(&m.rally, sound_mask);
                }
                if let Some(winner) = m.winner {
                    let scores = [m.rally.players[0].score, m.rally.players[1].score];
                    if !is_host {
                        self.settings.record_match(scores);
                        self.settings.save_to_dir(&self.asset_dir);
                    }
                    self.net = NetRole::None;
                    self.win_player = winner;
                    self.win_y = 0.0;
                    self.win_vy = 12.0;
                    self.win_step = 0;
                    self.screen = Screen::MatchWin;
                    return;
                }
            }
        }

        self.render_active_match();
    }

    fn update_confirm_quit(&mut self, window: &Window) {
        self.render_active_match();
        // Scanline + center box darkening from FUN_0044977c
        for y in (0..HEIGHT).step_by(2) {
            self.fb.pixels[y * WIDTH..(y + 1) * WIDTH].fill(0);
        }
        self.fb.darken_rect_bu(150, 200, 650, 400, 2);
        self.draw_t(250, 300, true, "@nachfrage", 0);

        let keys = window.get_keys_pressed(KeyRepeat::No);
        if !keys.is_empty() {
            let yes = keys
                .iter()
                .any(|&k| matches!(k, Key::Y | Key::J | Key::O | Key::Enter));
            if yes {
                match &self.net {
                    NetRole::Host(host) => host.send_quit(),
                    NetRole::Client { client, .. } => client.send_quit(),
                    NetRole::None => {}
                }
                self.net = NetRole::None;
                self.match_state = None;
                self.assets
                    .recolor_player(0, self.settings.player_colors[0]);
                self.assets
                    .recolor_player(1, self.settings.player_colors[1]);
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

        let lang = self.lang();
        let wname = match self.settings.controls[self.win_player] {
            PlayerControl::AiEasy => self.assets.texts[lang].get("@names", 0),
            PlayerControl::AiMedium => self.assets.texts[lang].get("@names", 1),
            PlayerControl::AiHard => self.assets.texts[lang].get("@names", 2),
            _ => &self.settings.player_names[self.win_player],
        };
        self.fb
            .draw_text(&self.assets, 300, 320, false, false, wname);
        self.draw_t(300, 270, false, "@win", 0);

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
        Key::A => 'a',
        Key::B => 'b',
        Key::C => 'c',
        Key::D => 'd',
        Key::E => 'e',
        Key::F => 'f',
        Key::G => 'g',
        Key::H => 'h',
        Key::I => 'i',
        Key::J => 'j',
        Key::K => 'k',
        Key::L => 'l',
        Key::M => 'm',
        Key::N => 'n',
        Key::O => 'o',
        Key::P => 'p',
        Key::Q => 'q',
        Key::R => 'r',
        Key::S => 's',
        Key::T => 't',
        Key::U => 'u',
        Key::V => 'v',
        Key::W => 'w',
        Key::X => 'x',
        Key::Y => 'y',
        Key::Z => 'z',
        Key::Key0 => '0',
        Key::Key1 => '1',
        Key::Key2 => '2',
        Key::Key3 => '3',
        Key::Key4 => '4',
        Key::Key5 => '5',
        Key::Key6 => '6',
        Key::Key7 => '7',
        Key::Key8 => '8',
        Key::Key9 => '9',
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

#[allow(clippy::too_many_arguments)]
fn step_menu(
    window: &Window,
    my_bu: i32,
    clicked: bool,
    ys: &[i32],
    row_h: i32,
    sel: &mut usize,
    sound: &SoundEngine,
    click_wav: &Arc<[u8]>,
) -> MenuAction {
    let mut hovered = false;
    for (i, &y) in ys.iter().enumerate() {
        if my_bu >= y && my_bu < y + row_h {
            *sel = i;
            hovered = true;
        }
    }
    let n = ys.len();
    if window.is_key_pressed(Key::Up, KeyRepeat::No) {
        *sel = (*sel + n - 1) % n;
        sound.play_wav(click_wav);
    }
    if window.is_key_pressed(Key::Down, KeyRepeat::No) {
        *sel = (*sel + 1) % n;
        sound.play_wav(click_wav);
    }
    if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
        return MenuAction::Back;
    }
    if window.is_key_pressed(Key::Enter, KeyRepeat::No) || (clicked && hovered) {
        return MenuAction::Activate;
    }
    MenuAction::None
}
