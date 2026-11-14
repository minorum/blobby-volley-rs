use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use minifb::Key;

use crate::assets::Language;
use crate::sim::PlayerControl;

#[derive(Clone, Debug)]
pub struct StatEntry {
    pub name: String,
    pub gms: [i16; 5], // 0: vs humans, 1: vs Adam, 2: vs Bert, 3: vs Carl, 4: all
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
            let name = &self.player_names[i];
            let pos = self
                .stats
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| {
                    self.stats.push(StatEntry::new(name.clone()));
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
                            let mut parts = v.split('|');
                            if let Some(name) = parts.next() {
                                let mut nums = [0i16; 15];
                                let mut count = 0;
                                for p in parts {
                                    if count < 15 {
                                        nums[count] = p.parse().unwrap_or(0);
                                    }
                                    count += 1;
                                }
                                if count == 15 {
                                    let mut st = StatEntry::new(name.to_string());
                                    st.gms.copy_from_slice(&nums[0..5]);
                                    st.won.copy_from_slice(&nums[5..10]);
                                    st.score.copy_from_slice(&nums[10..15]);
                                    s.stats.push(st);
                                }
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
        let mut out = String::with_capacity(256);
        let _ = writeln!(out, "lang={}", self.language as u8);
        let _ = writeln!(out, "c0={}", self.controls[0] as u8);
        let _ = writeln!(out, "c1={}", self.controls[1] as u8);
        let _ = writeln!(out, "sound={}", u8::from(self.sound_enabled));
        let _ = writeln!(out, "n0={}", self.player_names[0]);
        let _ = writeln!(out, "n1={}", self.player_names[1]);
        let _ = writeln!(out, "col0={}", self.player_colors[0]);
        let _ = writeln!(out, "col1={}", self.player_colors[1]);
        for st in &self.stats {
            let _ = write!(out, "stat={}", st.name);
            for v in st.gms.iter().chain(st.won.iter()).chain(st.score.iter()) {
                let _ = write!(out, "|{v}");
            }
            out.push('\n');
        }
        let _ = fs::write(dir.join("bv_settings.txt"), out);
    }
}

pub fn key_display_name(key: Key) -> &'static str {
    match key {
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::Key0 => "0",
        Key::Key1 => "1",
        Key::Key2 => "2",
        Key::Key3 => "3",
        Key::Key4 => "4",
        Key::Key5 => "5",
        Key::Key6 => "6",
        Key::Key7 => "7",
        Key::Key8 => "8",
        Key::Key9 => "9",
        Key::Left => "left",
        Key::Right => "right",
        Key::Up => "up",
        Key::Down => "down",
        Key::Space => "Space",
        Key::LeftShift | Key::RightShift => "Shift",
        Key::LeftCtrl | Key::RightCtrl => "Ctrl",
        Key::Enter => "Enter",
        Key::Backspace => "backspc",
        _ => "key",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_match_updates_human_and_all_buckets_only() {
        let mut s = GameSettings::default();
        s.player_names[0] = "Alice".to_string();
        s.controls = [PlayerControl::Keyboard, PlayerControl::AiHard];
        s.record_match([15, 10]);

        assert_eq!(s.stats.len(), 1);
        let st = &s.stats[0];
        assert_eq!(st.name, "Alice");
        // Bucket 3 = vs Carl (AiHard), Bucket 4 = all
        assert_eq!(st.gms[3], 1);
        assert_eq!(st.won[3], 1);
        assert_eq!(st.score[3], 5);
        assert_eq!(st.gms[4], 1);
        assert_eq!(st.won[4], 1);
        assert_eq!(st.score[4], 5);
        assert_eq!(st.gms[0], 0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("blobby_volley_rs_settings_test");
        let _ = fs::create_dir_all(&dir);
        let mut s = GameSettings::default();
        s.language = Language::German;
        s.sound_enabled = false;
        s.player_names = ["Max".to_string(), "Moritz".to_string()];
        s.player_colors = [2, 5];
        s.record_match([15, 12]);
        s.save_to_dir(&dir);

        let loaded = GameSettings::load_from_dir(&dir);
        assert_eq!(loaded.language, Language::German);
        assert!(!loaded.sound_enabled);
        assert_eq!(loaded.player_names, ["Max", "Moritz"]);
        assert_eq!(loaded.player_colors, [2, 5]);
        assert_eq!(loaded.stats.len(), 1);
        assert_eq!(loaded.stats[0].score[4], 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
