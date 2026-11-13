use std::fs;
use std::path::Path;

use minifb::Key;

use crate::assets::Language;
use crate::sim::PlayerControl;

#[derive(Clone, Debug)]
pub struct StatEntry {
    pub name: String,
    pub gms: [i16; 5],   // 0: vs humans, 1: vs Adam, 2: vs Bert, 3: vs Carl, 4: all
    pub won: [i16; 5],
    pub score: [i16; 5],
}

impl StatEntry {
    pub fn new(name: String) -> Self {
        Self {
            name,
            gms: [0; 5],
            won: [0; 5],
            score: [0; 5],
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeyBindings {
    pub p1_left: Key,
    pub p1_right: Key,
    pub p1_jump: Key,
    pub p2_left: Key,
    pub p2_right: Key,
    pub p2_jump: Key,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            p1_left: Key::A,
            p1_right: Key::D,
            p1_jump: Key::W,
            p2_left: Key::Left,
            p2_right: Key::Right,
            p2_jump: Key::Up,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GameSettings {
    pub language: Language,
    pub controls: [PlayerControl; 2],
    pub sound_enabled: bool,
    pub player_names: [String; 2],
    pub player_colors: [usize; 2],
    pub keys: KeyBindings,
    pub host_game_name: String,
    pub host_control: PlayerControl,
    pub client_host_ip: String,
    pub client_control: PlayerControl,
    pub stats: Vec<StatEntry>,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            language: Language::English,
            controls: [PlayerControl::Keyboard, PlayerControl::AiMedium],
            sound_enabled: true,
            player_names: ["Player 1".to_string(), "Player 2".to_string()],
            player_colors: [0, 3],
            keys: KeyBindings::default(),
            host_game_name: "Blobby".to_string(),
            host_control: PlayerControl::Keyboard,
            client_host_ip: String::new(),
            client_control: PlayerControl::Keyboard,
            stats: Vec::new(),
        }
    }
}

impl GameSettings {
    pub fn record_match(&mut self, scores: [i32; 2]) {
        for i in 0..2 {
            if self.controls[i].is_ai() || self.controls[i] == PlayerControl::Network {
                continue;
            }
            let opp_bucket = match self.controls[1 - i] {
                PlayerControl::AiEasy => 1,
                PlayerControl::AiMedium => 2,
                PlayerControl::AiHard => 3,
                _ => 0,
            };
            let name = self.player_names[i].clone();
            let pos = self
                .stats
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(&name))
                .unwrap_or_else(|| {
                    self.stats.push(StatEntry::new(name));
                    self.stats.len() - 1
                });
            let entry = &mut self.stats[pos];
            let diff = (scores[i] - scores[1 - i]) as i16;
            let won = scores[i] > scores[1 - i];

            for bucket in [opp_bucket, 4] {
                entry.gms[bucket] += 1;
                if won {
                    entry.won[bucket] += 1;
                }
                entry.score[bucket] = (entry.score[bucket] + diff).max(0);
            }
        }
    }

    pub fn load_from_dir(dir: &Path) -> Self {
        let mut s = Self::default();
        let path = dir.join("bv_settings.txt");
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match k {
                        "lang" => {
                            s.language = match v {
                                "0" => Language::French,
                                "1" => Language::German,
                                _ => Language::English,
                            };
                        }
                        "c0" => s.controls[0] = PlayerControl::from_u8(v.parse().unwrap_or(0)),
                        "c1" => s.controls[1] = PlayerControl::from_u8(v.parse().unwrap_or(3)),
                        "sound" => s.sound_enabled = v != "0",
                        "n0" => s.player_names[0] = v.chars().take(12).collect(),
                        "n1" => s.player_names[1] = v.chars().take(12).collect(),
                        "col0" => s.player_colors[0] = v.parse::<usize>().unwrap_or(0) % 8,
                        "col1" => s.player_colors[1] = v.parse::<usize>().unwrap_or(3) % 8,
                        "stat" => {
                            let parts: Vec<&str> = v.split('|').collect();
                            if parts.len() == 16 {
                                let mut st = StatEntry::new(parts[0].to_string());
                                for idx in 0..5 {
                                    st.gms[idx] = parts[1 + idx].parse().unwrap_or(0);
                                    st.won[idx] = parts[6 + idx].parse().unwrap_or(0);
                                    st.score[idx] = parts[11 + idx].parse().unwrap_or(0);
                                }
                                s.stats.push(st);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        s
    }

    pub fn save_to_dir(&self, dir: &Path) {
        let mut out = String::new();
        out.push_str(&format!("lang={}\n", self.language as u8));
        out.push_str(&format!("c0={}\n", self.controls[0] as u8));
        out.push_str(&format!("c1={}\n", self.controls[1] as u8));
        out.push_str(&format!("sound={}\n", u8::from(self.sound_enabled)));
        out.push_str(&format!("n0={}\n", self.player_names[0]));
        out.push_str(&format!("n1={}\n", self.player_names[1]));
        out.push_str(&format!("col0={}\n", self.player_colors[0]));
        out.push_str(&format!("col1={}\n", self.player_colors[1]));
        for st in &self.stats {
            let mut row = st.name.clone();
            for v in st.gms.iter().chain(st.won.iter()).chain(st.score.iter()) {
                row.push('|');
                row.push_str(&v.to_string());
            }
            out.push_str(&format!("stat={row}\n"));
        }
        let _ = fs::write(dir.join("bv_settings.txt"), out);
    }
}

pub fn key_display_name(key: Key) -> &'static str {
    match key {
        Key::A => "A", Key::B => "B", Key::C => "C", Key::D => "D",
        Key::E => "E", Key::F => "F", Key::G => "G", Key::H => "H",
        Key::I => "I", Key::J => "J", Key::K => "K", Key::L => "L",
        Key::M => "M", Key::N => "N", Key::O => "O", Key::P => "P",
        Key::Q => "Q", Key::R => "R", Key::S => "S", Key::T => "T",
        Key::U => "U", Key::V => "V", Key::W => "W", Key::X => "X",
        Key::Y => "Y", Key::Z => "Z",
        Key::Key0 => "0", Key::Key1 => "1", Key::Key2 => "2", Key::Key3 => "3",
        Key::Key4 => "4", Key::Key5 => "5", Key::Key6 => "6", Key::Key7 => "7",
        Key::Key8 => "8", Key::Key9 => "9",
        Key::Left => "left", Key::Right => "right", Key::Up => "up", Key::Down => "down",
        Key::Space => "Space", Key::LeftShift | Key::RightShift => "Shift",
        Key::LeftCtrl | Key::RightCtrl => "Ctrl",
        Key::Enter => "Enter", Key::Backspace => "backspc",
        _ => "key",
    }
}
