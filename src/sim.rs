#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerControl {
    Keyboard = 0,
    Mouse = 1,
    AiEasy = 2,
    AiMedium = 3,
    AiHard = 4,
    Network = 50,
}

impl PlayerControl {
    pub fn is_ai(self) -> bool {
        matches!(
            self,
            PlayerControl::AiEasy | PlayerControl::AiMedium | PlayerControl::AiHard
        )
    }

    pub fn next_in_settings(self, other: PlayerControl) -> PlayerControl {
        let mut v = ((self as usize) + 1) % 5;
        if v == 1 && other == PlayerControl::Mouse {
            v = 2;
        }
        Self::from_u8(v as u8)
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => PlayerControl::Keyboard,
            1 => PlayerControl::Mouse,
            2 => PlayerControl::AiEasy,
            3 => PlayerControl::AiMedium,
            4 => PlayerControl::AiHard,
            50 => PlayerControl::Network,
            _ => PlayerControl::Keyboard,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerInput {
    pub left: bool,
    pub right: bool,
    pub jump: bool,
}

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub x: i32,
    pub y: i32,
    pub vy: i32,
    pub y_frac: i32,
    pub anim_frame: i32,
    pub anim_dir: i32,
    pub airborne: bool,
    pub hit_this_frame: bool,
    pub hit_prev_frame: bool,
    pub hit_count: i32,
    pub stuck_frames: i32,
    pub score: i32,
    pub ai_stun_frames: i32,
    pub input: PlayerInput,
}

impl PlayerState {
    pub fn new(x: i32) -> Self {
        Self {
            x,
            y: 100,
            vy: 0,
            y_frac: 0,
            anim_frame: 0,
            anim_dir: 0,
            airborne: false,
            hit_this_frame: false,
            hit_prev_frame: false,
            hit_count: 0,
            stuck_frames: 0,
            score: 0,
            ai_stun_frames: 0,
            input: PlayerInput::default(),
        }
    }

    pub fn reset_for_rally(&mut self) {
        self.hit_count = 0;
        self.hit_this_frame = false;
        self.hit_prev_frame = false;
        self.input = PlayerInput::default();
        self.ai_stun_frames = 0;
    }
}

#[derive(Clone, Debug)]
pub struct BallState {
    pub active: bool,
    pub x: i32,      // fixed-point * 32
    pub y: i32,      // fixed-point * 32
    pub vx: i32,     // fixed-point * 32 per substep
    pub vy: i32,     // fixed-point * 32 per substep
    pub prev_x: i32, // for net-top crossing detection
    pub prev_y: i32,
    pub spin_phase: u32,
}

impl BallState {
    pub fn new_serve(server: usize) -> Self {
        let x = (server as i32 * 400 + 200) * 32;
        let y = 0x2580; // 300 * 32
        Self {
            active: false,
            x,
            y,
            vx: 0,
            vy: 0,
            prev_x: x,
            prev_y: y,
            spin_phase: 0,
        }
    }
}

/// Events emitted by a single simulation tick so the audio/UI layer stays decoupled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimEvents {
    pub ball_hit: bool,
    pub whistle: bool,
}

/// Deterministic xorshift32 PRNG replacing Delphi's `Random(n)` for reproducible simulation.
#[derive(Clone, Debug)]
pub struct SimpleRng(pub u32);

impl SimpleRng {
    pub fn next_i32(&mut self, bound: i32) -> i32 {
        if bound <= 1 {
            return 0;
        }
        let mut x = self.0;
        if x == 0 {
            x = 0x1234_5678;
        }
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x % (bound as u32)) as i32
    }
}

// Constants from volley.exe: 0x0044daf4, 0x0044db1c (head/body Y offsets from top of 148x101 sprite)
const HEAD_CY: [i32; 5] = [36, 41, 45, 50, 54];
const BODY_CY: [i32; 5] = [69, 71, 74, 76, 79];
const DOT_THRESHOLD: f64 = -0.7; // 0x004440cc

#[derive(Clone, Debug)]
pub struct RallySim {
    pub players: [PlayerState; 2],
    pub controls: [PlayerControl; 2],
    pub ball: BallState,
    pub server: usize,
    pub is_first_rally: bool,
    pub rally_over: bool,
    pub point_for_server: bool,
    pub rally_over_ticks: u32,
    pub ticks_since_start: u32,
    pub rng: SimpleRng,
}

impl RallySim {
    pub fn new(
        controls: [PlayerControl; 2],
        scores: [i32; 2],
        server: usize,
        is_first_rally: bool,
        seed: u32,
    ) -> Self {
        let mut p0 = PlayerState::new(200);
        let mut p1 = PlayerState::new(600);
        p0.score = scores[0];
        p1.score = scores[1];
        Self {
            players: [p0, p1],
            controls,
            ball: BallState::new_serve(server),
            server,
            is_first_rally,
            rally_over: false,
            point_for_server: false,
            rally_over_ticks: 0,
            ticks_since_start: 0,
            rng: SimpleRng(seed),
        }
    }

    pub fn reset_rally(&mut self, server: usize, is_first_rally: bool) {
        self.server = server;
        self.is_first_rally = is_first_rally;
        self.rally_over = false;
        self.point_for_server = false;
        self.rally_over_ticks = 0;
        self.ticks_since_start = 0;
        self.ball = BallState::new_serve(server);
        for p in &mut self.players {
            p.reset_for_rally();
        }
    }

    /// Returns `Some(point_for_server)` once the rally has finished and the 2-second whistle
    /// cooldown + both players on ground condition is satisfied.
    pub fn step(
        &mut self,
        human_inputs: [PlayerInput; 2],
        mouse_x: i32,
        mouse_clicked: bool,
    ) -> (SimEvents, Option<bool>) {
        let mut events = SimEvents::default();
        self.ticks_since_start += 1;

        // 1. Apply human / mouse inputs
        for (i, &human_input) in human_inputs.iter().enumerate() {
            match self.controls[i] {
                PlayerControl::Keyboard => {
                    self.players[i].input = human_input;
                }
                PlayerControl::Mouse => {
                    let target_x = (mouse_x >> 1) + (i as i32) * 400;
                    self.players[i].input.right = self.players[i].x + 9 < target_x;
                    self.players[i].input.left = target_x < self.players[i].x - 9;
                    self.players[i].input.jump = mouse_clicked;
                }
                _ => {}
            }
        }

        // 2. AI inputs (when rally is still active)
        if !self.rally_over {
            for i in 0..2 {
                if self.controls[i].is_ai() {
                    self.update_ai(i);
                }
            }
        }

        // 3. Clear per-frame hit flags & advance ball spin
        self.players[0].hit_this_frame = false;
        self.players[1].hit_this_frame = false;
        self.ball.spin_phase = (self.ball.spin_phase + 3) & 0x7F;

        // 4. First physics pass (before player movement)
        if !self.rally_over {
            self.collide_ball_entities();
        }
        self.step_ball_physics();

        // 5. Net-crossing hit count reset
        if self.ball.x < 0x3340 {
            if self.ball.x < 0x30C1 {
                self.players[1].hit_count = 0;
            }
        } else {
            self.players[0].hit_count = 0;
        }

        // 6. Update players (movement, jump, animation)
        for (i, p) in self.players.iter_mut().enumerate() {
            if p.input.right {
                p.x += 12;
                if i == 0 && p.x > 360 {
                    p.x = 360;
                }
                if p.x > 800 {
                    p.x = 800;
                }
                if !p.airborne && p.anim_frame == 0 && p.anim_dir == 0 {
                    p.anim_dir = 1;
                }
            }
            if p.input.left {
                p.x -= 12;
                if i == 1 && p.x < 440 {
                    p.x = 440;
                }
                if p.x < 0 {
                    p.x = 0;
                }
                if !p.airborne && p.anim_frame == 0 && p.anim_dir == 0 {
                    p.anim_dir = 1;
                }
            }
            if p.input.jump && !p.airborne {
                p.airborne = true;
                p.vy = 10000;
                p.y_frac = 0;
                if p.anim_frame != 4 {
                    p.anim_dir = 1;
                }
            }
            if p.airborne {
                p.y_frac += p.vy;
                p.y += p.y_frac / 256;
                p.y_frac %= 256;
                p.vy -= 800;
                if !p.input.jump {
                    p.vy -= 800;
                }
                if p.y < 101 {
                    p.airborne = false;
                    p.y = 100;
                    p.anim_frame = 0;
                    p.anim_dir = 1;
                }
            }
            if p.anim_frame == 0 && p.anim_dir == -1 {
                p.anim_dir = 0;
            } else {
                p.anim_frame += p.anim_dir;
                if p.anim_frame == 4 {
                    p.anim_dir = -p.anim_dir;
                }
            }
        }

        // 7. Second physics pass (after player movement)
        if !self.rally_over {
            self.collide_ball_entities();
        }
        self.step_ball_physics();

        // 8. Hit counting & 4-hit / 40-frame stuck fault rules
        let mut fault_occurred = false;
        for i in 0..2 {
            let hit_now = self.players[i].hit_this_frame;
            let hit_prev = self.players[i].hit_prev_frame;
            if hit_now {
                if !hit_prev {
                    self.players[i].hit_count += 1;
                    events.ball_hit = true;
                    if self.players[i].hit_count > 3 {
                        fault_occurred = true;
                        self.point_for_server = i != self.server;
                    }
                    self.players[i].stuck_frames = 0;
                } else {
                    self.players[i].stuck_frames += 1;
                    if self.players[i].stuck_frames > 40 {
                        fault_occurred = true;
                        self.point_for_server = i != self.server;
                    }
                }
            }
            self.players[i].hit_prev_frame = hit_now;
        }
        if fault_occurred {
            self.rally_over = true;
            self.players[0].hit_count = 0;
            self.players[1].hit_count = 0;
            self.ball.vy /= 2;
            self.ball.vx /= 2;
        }

        // 9. Whistle on rally end & 2-second cooldown (approx 60 ticks at 30 Hz)
        if self.rally_over {
            if self.rally_over_ticks == 0 {
                events.whistle = true;
            }
            self.rally_over_ticks += 1;
            if self.rally_over_ticks > 60 && self.players[0].y == 100 && self.players[1].y == 100 {
                return (events, Some(self.point_for_server));
            }
        }

        (events, None)
    }

    fn predict_ball_x(&self, target_y: i32) -> i32 {
        let f1 = (-(self.ball.vy) as f32) / -3.5;
        let f3 = ((target_y << 6) as f32) / -3.5 + (f1 * f1 - ((self.ball.y * 2) as f32) / -3.5);
        if f3 >= 0.0 {
            let sq = f3.sqrt();
            let mut t = f1 + sq;
            if t < 0.0 {
                t = f1 - sq;
            }
            if t > 0.0 {
                let pred = (self.ball.x as f32 + (self.ball.vx as f32) * t) / 32.0;
                return pred.round() as i32;
            }
        }
        -10000
    }

    fn update_ai(&mut self, idx: usize) {
        let ctrl = self.controls[idx];
        let dir: i32 = if idx == 0 { -1 } else { 1 };
        let bx = self.ball.x / 32;
        let by = self.ball.y / 32;
        let p_x = self.players[idx].x;
        let p_y = self.players[idx].y;
        let p_hits = self.players[idx].hit_count;

        let mut target_x = self.predict_ball_x(200);
        let mut carl_spike = false;

        if target_x > -9000 {
            let orig_pred = target_x;
            if ctrl == PlayerControl::AiHard {
                carl_spike = (self.ball.vx.abs() * 4) < (bx - 400) * -dir;
                if carl_spike {
                    target_x = 400;
                }
                if (target_x - 400).abs() > 100 {
                    let spike_pred = self.predict_ball_x(350);
                    if spike_pred < -9000 || by < 300 || p_hits > 1 {
                        target_x = orig_pred;
                    } else {
                        target_x = spike_pred;
                    }
                }
            }

            if idx == 1 {
                if target_x > 768 {
                    target_x = 1536 - target_x;
                }
                if target_x < 437 {
                    target_x = 874 - target_x;
                }
                if target_x < 500 && p_hits < 2 {
                    target_x -= 32;
                }
            } else {
                if target_x < 32 {
                    target_x = 64 - target_x;
                }
                if target_x > 363 {
                    target_x = 726 - target_x;
                }
                if target_x > 300 && p_hits < 2 {
                    target_x += 32;
                }
            }

            target_x += dir * 21;

            if ctrl == PlayerControl::AiEasy {
                if self.players[idx].ai_stun_frames > 0 {
                    target_x = dir * 200 + 400;
                    self.players[idx].ai_stun_frames -= 1;
                }
                if self.ball.vy > 0 {
                    self.players[idx].ai_stun_frames = self.rng.next_i32(7) + 7;
                }
            }

            self.players[idx].input.right = p_x + 7 <= target_x;
            self.players[idx].input.left = target_x <= p_x - 7;
        }

        let mut jump = false;
        if self.ball.vy < 11
            && dir * (p_x - bx) < 21
            && (bx - p_x) * dir < 7
            && (bx - p_x).abs() < 120
            && self.ball.vy.abs() < 65
            && by > 200
            && by - p_y > 70
            && (by < 400 || self.ball.vy == 0)
            && self.ball.vx.abs() < 20
        {
            jump = true;
        }

        if self.rng.next_i32(300) == 0 {
            jump = true;
        }

        if target_x > -9000 {
            if (target_x - p_x).abs() > 75
                && (bx - p_x).abs() < 65
                && self.ball.vy < 0
                && ctrl != PlayerControl::AiHard
            {
                jump = true;
            }
            if carl_spike
                && (target_x + dir * 5 - p_x).abs() < 45
                && by > 250
                && (p_x - bx).abs() < self.ball.vx.abs()
                && by < 600
            {
                jump = true;
            }
        }

        if self.players[idx].hit_this_frame {
            self.players[idx].input.left = false;
            self.players[idx].input.right = false;
        }

        if !self.ball.active {
            let ms = self.ticks_since_start * 33;
            if ms < 500 {
                jump = false;
            }
            if ms > 1000 && idx == self.server {
                jump = true;
            }
        }

        self.players[idx].input.jump = jump;
    }

    fn step_ball_physics(&mut self) {
        for substep in 1..=2 {
            self.ball.x += self.ball.vx;
            self.ball.y += self.ball.vy;

            let mut grav = substep + 2;
            if self.ball.vy < 0 {
                grav -= (self.ball.vy.abs() / 3) / 32;
            }
            if self.ball.active {
                self.ball.vy -= grav;
            }

            if self.ball.y < 0x1080 {
                self.ball.vy = (-self.ball.vy * 3) / 4;
                self.ball.vx = (self.ball.vx * 3) / 4;
                self.ball.y = 0x1080;
                if !self.rally_over {
                    self.point_for_server = (self.ball.x > 0x31FF) == (self.server == 0);
                    self.rally_over = true;
                }
            }

            if self.ball.x < 0x400 {
                self.ball.vx = -self.ball.vx;
                self.ball.x = 0x400;
            }
            if self.ball.x > 0x6000 {
                self.ball.vx = -self.ball.vx;
                self.ball.x = 0x6000;
            }

            // Net vertical sides
            let left_edge = (self.ball.x / 32) - 32;
            if left_edge < 0x19A
                && left_edge > 0x17B
                && self.ball.vx < 0
                && (self.ball.y / 32) - 32 < 0x143
            {
                self.ball.x = 0x3760;
                self.ball.vx = -self.ball.vx;
            }
            let right_edge = (self.ball.x / 32) + 32;
            if right_edge < 0x1A5
                && right_edge > 0x186
                && self.ball.vx > 0
                && (self.ball.y / 32) - 32 < 0x143
            {
                self.ball.x = 0x2CA0;
                self.ball.vx = -self.ball.vx;
            }

            // Net top cap crossing guard (FUN_00444100)
            if ((self.ball.x > 0x2D1F && self.ball.x < 0x36E1)
                || ((self.ball.prev_x > 0x2DFF) != (self.ball.x > 0x2DFF) && self.ball.active))
                && self.ball.y < 0x2801
                && self.ball.prev_y < 0x28C0
            {
                if self.ball.prev_x < 0x3201 {
                    self.ball.x = 0x2D00;
                } else {
                    self.ball.x = 0x3700;
                }
                self.ball.y = self.ball.prev_y;
                self.ball.vx = -self.ball.vx;
            }
            self.ball.prev_x = self.ball.x;
            self.ball.prev_y = self.ball.y;
        }
    }

    fn collide_ball_entities(&mut self) {
        let speed = ((self.ball.vx as f64).powi(2) + (self.ball.vy as f64).powi(2)).sqrt();

        // Head & body circles for both players
        for i in 0..2 {
            let frame = self.players[i].anim_frame.clamp(0, 4) as usize;
            let px = self.players[i].x as f64;
            let py = self.players[i].y as f64;

            // Head circle (radius 25 + ball 32 = 57; 57^2 = 3249, sep 59^2 = 3481)
            let hcy = (py + 101.0) - HEAD_CY[frame] as f64;
            if self.collide_player_circle(i, px, hcy, 3249.0, 3481.0, speed) {
                self.players[1 - i].hit_count = 0;
            }

            // Body circle (radius 32 + ball 32 = 64; 64^2 = 4096, sep 66^2 = 4356)
            let bcy = (py + 101.0) - BODY_CY[frame] as f64;
            if self.collide_player_circle(i, px, bcy, 4096.0, 4356.0, speed) {
                self.players[1 - i].hit_count = 0;
            }
        }

        // Net top circle (400, 329), radius 5 + ball 32 = 37; 37^2 = 1369, sep 39^2 = 1521
        let dx = (self.ball.x as f64) / 32.0 - 400.0;
        let dy = (self.ball.y as f64) / 32.0 - 329.0;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < 1369.0 && dy > 0.0 {
            self.ball.active = true;
            let dist = dist_sq.sqrt().max(1e-6);
            self.ball.vx = ((dx / dist) * 4.0 * 32.0).round() as i32;
            self.ball.vy = ((dy / dist) * 4.0 * 32.0).round() as i32;
            if self.ball.vx == 0 {
                self.ball.vx = self.rng.next_i32(5) - 2;
            }
            separate_ball_from_circle(&mut self.ball, 400.0, 329.0, 1521.0);
        }
    }

    fn collide_player_circle(
        &mut self,
        player_idx: usize,
        cx: f64,
        cy: f64,
        r_sq: f64,
        sep_sq: f64,
        speed: f64,
    ) -> bool {
        let dx = (self.ball.x as f64) / 32.0 - cx;
        let dy = (self.ball.y as f64) / 32.0 - cy;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq >= r_sq {
            return false;
        }
        self.ball.active = true;
        let dist = dist_sq.sqrt().max(1e-6);
        let dot = (self.ball.vy as f64) * dy + (self.ball.vx as f64) * dx;
        let mut new_hit = false;
        if dot <= DOT_THRESHOLD * dist * speed {
            self.players[player_idx].hit_this_frame = true;
            new_hit = true;
        }
        self.ball.vx = ((dx / dist) * 8.0 * 32.0).round() as i32;
        self.ball.vy = ((dy / dist) * 8.0 * 32.0).round() as i32;

        separate_ball_from_circle(&mut self.ball, cx, cy, sep_sq);
        new_hit
    }
}

fn separate_ball_from_circle(ball: &mut BallState, cx: f64, cy: f64, sep_sq: f64) {
    let mut guard = 0;
    loop {
        ball.x += ball.vx / 10;
        ball.y += ball.vy / 10;
        let ndx = (ball.x as f64) / 32.0 - cx;
        let ndy = (ball.y as f64) / 32.0 - cy;
        guard += 1;
        if ndx * ndx + ndy * ndy > sep_sq || guard > 100 {
            break;
        }
    }
}

/// Manages a full Blobby Volley match (15 points, win by 2, side-out scoring, first rally decides serve).
#[derive(Clone, Debug)]
pub struct MatchState {
    pub rally: RallySim,
    pub winner: Option<usize>,
}

impl MatchState {
    pub fn new(controls: [PlayerControl; 2], seed: u32) -> Self {
        let mut rng = SimpleRng(seed);
        let initial_server = rng.next_i32(2) as usize;
        Self {
            rally: RallySim::new(controls, [0, 0], initial_server, true, seed.wrapping_add(1)),
            winner: None,
        }
    }

    pub fn step(
        &mut self,
        human_inputs: [PlayerInput; 2],
        mouse_x: i32,
        mouse_clicked: bool,
    ) -> SimEvents {
        if self.winner.is_some() {
            return SimEvents::default();
        }
        let (events, result) = self.rally.step(human_inputs, mouse_x, mouse_clicked);
        if let Some(point_for_server) = result {
            let server = self.rally.server;
            if self.rally.is_first_rally {
                let next_server = if point_for_server { server } else { 1 - server };
                self.rally.reset_rally(next_server, false);
            } else if point_for_server {
                self.rally.players[server].score += 1;
                let s0 = self.rally.players[0].score;
                let s1 = self.rally.players[1].score;
                if (s0 >= 15 || s1 >= 15) && (s0 - s1).abs() >= 2 {
                    self.winner = Some(if s0 > s1 { 0 } else { 1 });
                } else {
                    self.rally.reset_rally(server, false);
                }
            } else {
                self.rally.reset_rally(1 - server, false);
            }
        }
        events
    }

    pub fn is_match_point(&self) -> bool {
        if self.rally.is_first_rally {
            return false;
        }
        let s = self.rally.server;
        self.rally.players[s].score > 13
            && self.rally.players[s].score > self.rally.players[1 - s].score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_floats_until_hit() {
        let mut sim = RallySim::new(
            [PlayerControl::Keyboard, PlayerControl::Keyboard],
            [0, 0],
            0,
            false,
            1,
        );
        for _ in 0..10 {
            sim.step([PlayerInput::default(); 2], 0, false);
        }
        assert!(!sim.ball.active);
        assert_eq!(sim.ball.x, 200 * 32);
        assert_eq!(sim.ball.y, 0x2580);
    }

    #[test]
    fn jump_reaches_peak_and_lands_at_100() {
        let mut sim = RallySim::new(
            [PlayerControl::Keyboard, PlayerControl::Keyboard],
            [0, 0],
            0,
            false,
            1,
        );
        let mut max_y = 100;
        for t in 0..40 {
            let inp = [
                PlayerInput {
                    left: false,
                    right: false,
                    jump: t < 15,
                },
                PlayerInput::default(),
            ];
            sim.step(inp, 0, false);
            max_y = max_y.max(sim.players[0].y);
        }
        assert!(max_y > 300);
        assert_eq!(sim.players[0].y, 100);
        assert!(!sim.players[0].airborne);
    }

    #[test]
    fn four_hits_triggers_fault_and_sideout() {
        let mut m = MatchState::new([PlayerControl::Keyboard, PlayerControl::Keyboard], 42);
        m.rally.is_first_rally = false;
        m.rally.server = 0;
        m.rally.players[0].hit_count = 3;
        m.rally.players[0].hit_this_frame = false;
        m.rally.players[0].hit_prev_frame = false;
        // Position ball right above P0's head moving downward
        m.rally.ball.active = true;
        m.rally.ball.x = 200 * 32;
        m.rally.ball.y = 210 * 32;
        m.rally.ball.vy = -200;

        let mut hit_seen = false;
        for _ in 0..10 {
            let ev = m.step([PlayerInput::default(); 2], 0, false);
            if ev.whistle {
                hit_seen = true;
                break;
            }
        }
        assert!(hit_seen);
        assert!(m.rally.rally_over);
        assert!(!m.rally.point_for_server);
    }

    #[test]
    fn ai_vs_ai_plays_and_scores() {
        let mut m = MatchState::new([PlayerControl::AiHard, PlayerControl::AiMedium], 7);
        for _ in 0..5000 {
            m.step([PlayerInput::default(); 2], 0, false);
            if m.rally.players[0].score > 0 || m.rally.players[1].score > 0 || m.winner.is_some() {
                break;
            }
        }
        assert!(m.rally.players[0].score > 0 || m.rally.players[1].score > 0);
    }

    #[test]
    fn match_requires_two_point_lead_at_deuce() {
        let mut m = MatchState::new([PlayerControl::Keyboard, PlayerControl::Keyboard], 99);
        m.rally.is_first_rally = false;
        m.rally.server = 0;
        m.rally.players[0].score = 14;
        m.rally.players[1].score = 14;
        m.rally.rally_over = true;
        m.rally.point_for_server = true;
        m.rally.rally_over_ticks = 61;
        m.step([PlayerInput::default(); 2], 0, false);
        assert_eq!(m.rally.players[0].score, 15);
        assert_eq!(m.winner, None);

        // Next point to 16-14 wins
        m.rally.rally_over = true;
        m.rally.point_for_server = true;
        m.rally.rally_over_ticks = 61;
        m.step([PlayerInput::default(); 2], 0, false);
        assert_eq!(m.rally.players[0].score, 16);
        assert_eq!(m.winner, Some(0));
    }

    #[test]
    fn ball_bounces_off_walls_and_net() {
        let mut sim = RallySim::new(
            [PlayerControl::Keyboard, PlayerControl::Keyboard],
            [0, 0],
            0,
            false,
            1,
        );
        sim.ball.active = true;
        sim.ball.x = 0x410;
        sim.ball.y = 400 * 32;
        sim.ball.vx = -100;
        sim.ball.vy = 0;
        sim.step([PlayerInput::default(); 2], 0, false);
        assert!(sim.ball.vx > 0);
        assert!(sim.ball.x >= 0x400);
    }

    #[test]
    fn first_rally_decides_serve_without_scoring() {
        let mut m = MatchState::new([PlayerControl::Keyboard, PlayerControl::Keyboard], 5);
        assert!(m.rally.is_first_rally);
        let initial_server = m.rally.server;
        m.rally.rally_over = true;
        m.rally.point_for_server = false;
        m.rally.rally_over_ticks = 61;
        m.step([PlayerInput::default(); 2], 0, false);
        assert!(!m.rally.is_first_rally);
        assert_eq!(m.rally.server, 1 - initial_server);
        assert_eq!(m.rally.players[0].score, 0);
        assert_eq!(m.rally.players[1].score, 0);
    }

    #[test]
    fn match_point_detection() {
        let mut m = MatchState::new([PlayerControl::Keyboard, PlayerControl::Keyboard], 5);
        m.rally.is_first_rally = false;
        m.rally.server = 0;
        m.rally.players[0].score = 14;
        m.rally.players[1].score = 13;
        assert!(m.is_match_point());

        // Deuce 14-14 is not match point
        m.rally.players[1].score = 14;
        assert!(!m.is_match_point());

        // Receiver at 14-13 is not match point
        m.rally.server = 1;
        assert!(!m.is_match_point());
    }

    #[test]
    fn stuck_ball_over_40_frames_triggers_fault() {
        let mut m = MatchState::new([PlayerControl::Keyboard, PlayerControl::Keyboard], 12);
        m.rally.is_first_rally = false;
        m.rally.server = 0;
        m.rally.players[0].hit_prev_frame = true;
        m.rally.players[0].stuck_frames = 40;
        m.rally.ball.active = true;
        m.rally.ball.x = 200 * 32;
        m.rally.ball.y = 200 * 32;

        let ev = m.step([PlayerInput::default(); 2], 0, false);
        assert!(ev.whistle);
        assert!(m.rally.rally_over);
        assert!(!m.rally.point_for_server);
    }

    #[test]
    fn net_side_and_top_cap_bounce() {
        let mut sim = RallySim::new(
            [PlayerControl::Keyboard, PlayerControl::Keyboard],
            [0, 0],
            0,
            false,
            1,
        );
        // Ball approaching net from right side below net top
        sim.ball.active = true;
        sim.ball.x = (0x199 + 32) * 32;
        sim.ball.y = 200 * 32;
        sim.ball.prev_x = sim.ball.x;
        sim.ball.prev_y = sim.ball.y;
        sim.ball.vx = -64;
        sim.ball.vy = 0;
        sim.step([PlayerInput::default(); 2], 0, false);
        assert!(sim.ball.vx > 0);
    }

    #[test]
    fn mouse_control_steers_player_toward_cursor() {
        let mut sim = RallySim::new(
            [PlayerControl::Mouse, PlayerControl::Keyboard],
            [0, 0],
            0,
            false,
            1,
        );
        // mouse_x = 600 -> target_x = 300 > p0.x (200) -> moves right
        sim.step([PlayerInput::default(); 2], 600, false);
        assert!(sim.players[0].x > 200);
        // mouse_x = 100 -> target_x = 50 < p0.x -> moves left
        sim.step([PlayerInput::default(); 2], 100, false);
        assert!(sim.players[0].input.left);
    }
}
