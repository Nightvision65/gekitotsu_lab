//! 激光体系：`LaserBase` 的四种变体（BossLaser/BossLaser2/MagicLaser/StarLaser）。
//! 形态差异只在内建 cos/sin、follow 与命中重写上，公共步进逐行对应 `LaserBase.step`。

use crate::engine::GameTask;
use crate::gen_literals::jf32;
use crate::jmath;
use crate::trig;
use crate::utils;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaserKind {
    Boss,
    Boss2,
    Magic,
    Star,
}

#[derive(Clone, Debug)]
pub struct Laser {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub rot_deg: f32,
    pub thickness: f32,
    pub circle_radius: f32,
    pub x_speed: f32,
    pub y_speed: f32,
    pub start_x: f32,
    pub start_y: f32,
    pub old_rot_deg: f32,
    pub side: usize,
    pub steps: i32,
    pub cnt: i32,
    user: i32,
    pub kind: LaserKind,
}

impl Laser {
    pub fn new_boss(id: i32, x: f32, y: f32, side: usize, kind: LaserKind) -> Self {
        debug_assert!(matches!(kind, LaserKind::Boss | LaserKind::Boss2));
        Self {
            id,
            x,
            y,
            rot_deg: 90.0,
            thickness: 30.0,
            circle_radius: 15.5,
            x_speed: 0.0,
            y_speed: 10.0,
            start_x: x,
            start_y: y,
            old_rot_deg: 90.0,
            side,
            steps: 0,
            cnt: 0,
            user: -1,
            kind,
        }
    }

    pub fn new_magic(id: i32, x: f32, y: f32, r_deg: i32, side: usize, user: i32) -> Self {
        Self {
            id,
            x,
            y,
            rot_deg: r_deg as f32,
            thickness: 20.0,
            circle_radius: 15.5,
            x_speed: 0.0,
            y_speed: 10.0,
            start_x: x,
            start_y: y,
            old_rot_deg: 90.0,
            side,
            steps: 0,
            cnt: 0,
            user,
            kind: LaserKind::Magic,
        }
    }

    /// Java `StarLaser`：`super(X, -600, 90, S, 25.F, 15.5F)`
    pub fn new_star(id: i32, x: f32, side: usize) -> Self {
        Self {
            id,
            x,
            y: -600.0,
            rot_deg: 90.0,
            thickness: 25.0,
            circle_radius: 15.5,
            x_speed: 0.0,
            y_speed: 10.0,
            start_x: x,
            start_y: -600.0,
            old_rot_deg: 90.0,
            side,
            steps: 0,
            cnt: 0,
            user: -1,
            kind: LaserKind::Star,
        }
    }

    #[inline]
    fn internal_cos(&self, r_deg: f32) -> f32 {
        match self.kind {
            LaserKind::Boss | LaserKind::Boss2 => utils::cos_f(r_deg),
            LaserKind::Magic | LaserKind::Star => trig::cos(r_deg as i32),
        }
    }

    #[inline]
    fn internal_sin(&self, r_deg: f32) -> f32 {
        match self.kind {
            LaserKind::Boss | LaserKind::Boss2 => utils::sin_f(r_deg),
            LaserKind::Magic | LaserKind::Star => trig::sin(r_deg as i32),
        }
    }

    /// Java 各子类的 `follow()`
    fn follow(&mut self, game: &mut GameTask) {
        match self.kind {
            LaserKind::Boss | LaserKind::Boss2 => {
                if game.hp0_flg[self.side] > 0 {
                    self.kill(game);
                    return;
                }
                let step = if self.kind == LaserKind::Boss {
                    jf32!(0.1)
                } else {
                    jf32!(0.3)
                };
                self.rot_deg -= if self.side == 0 { step } else { -step };
                self.rot_deg = ((self.rot_deg % 360.0) + 360.0) % 360.0;
                let (cx, cy) = game.core_position(self.side);
                self.start_x = cx + 38.0 * self.internal_cos(self.rot_deg);
                self.start_y = cy + 38.0 * self.internal_sin(self.rot_deg);
            }
            LaserKind::Magic => {
                let user = self.user;
                if let Some(crate::engine::Element::Ball(b)) = game.get(user) {
                    if !matches!(b.kind, crate::engine::BallKind::Magic { shooting: true }) {
                        self.kill(game);
                        return;
                    }
                    self.rot_deg = b.rot as f32;
                    let (bx, by) = (b.x, b.y);
                    self.start_x = bx + 38.0 * self.internal_cos(self.rot_deg);
                    self.start_y = by + 38.0 * self.internal_sin(self.rot_deg);
                } else {
                    self.kill(game);
                }
            }
            LaserKind::Star => {
                self.cnt += 1;
                if self.cnt >= 45 {
                    self.kill(game);
                }
            }
        }
    }

    /// Java `LaserBase.step()`（两种 break 在 Java 中就是两个分支）
    #[allow(clippy::if_same_then_else)]
    pub fn step(&mut self, game: &mut GameTask) {
        self.follow(game);
        if self.rot_deg != self.old_rot_deg {
            self.old_rot_deg = self.rot_deg;
            self.x_speed = self.internal_cos(self.rot_deg) * 10.0;
            self.y_speed = self.internal_sin(self.rot_deg) * 10.0;
        }
        self.x = self.start_x;
        self.y = self.start_y;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.steps = 0;
        #[cfg(feature = "phase-timing")]
        let t_march = std::time::Instant::now();
        while self.steps < 1000 {
            self.x += self.x_speed;
            self.y += self.y_speed;
            self.x = jmath::xy_sync(self.x);
            self.y = jmath::xy_sync(self.y);
            if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                break;
            } else if game.shield[1 - self.side].hit_test(game, self.x, self.y)
                || game.fort_hit(1 - self.side, self.x, self.y)
            {
                break;
            }
            self.steps += 1;
            #[cfg(feature = "phase-timing")]
            crate::engine::prof::inc(&crate::engine::prof::LASER_MARCH_ITERS);
        }
        #[cfg(feature = "phase-timing")]
        crate::engine::prof::add(
            &crate::engine::prof::LASER_MARCH_NS,
            t_march.elapsed().as_nanos() as u64,
        );
    }

    /// Java `LaserBase.hitTestPoint`（BossLaser2 覆写另有分支）
    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        if self.kind == LaserKind::Boss2 {
            return self.hit_test_boss2(x, y);
        }
        let dx = x - self.start_x;
        let dy = y - self.start_y;
        let wrk = dx * self.x_speed + dy * self.y_speed;
        if wrk >= 0.0
            && wrk <= self.steps as f32 * 100.0
            && (dy * self.x_speed - dx * self.y_speed).abs() <= self.thickness * 5.0
        {
            return true;
        }
        let dx = x - self.x;
        let dy = y - self.y;
        dx * dx + dy * dy <= self.circle_radius * self.circle_radius
    }

    /// Java `BossLaser2.hitTestPoint`（`!(a > x) || !(a < y)` 为 Java 原写法）
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn hit_test_boss2(&self, x: f32, y: f32) -> bool {
        let dx = x - self.start_x;
        let dy = y - self.start_y;
        let wrk = dx * self.x_speed + dy * self.y_speed;
        if wrk >= 0.0 && wrk <= self.steps as f32 * 100.0 {
            let distx10 = (dy * self.x_speed - dx * self.y_speed).abs();
            if distx10 <= 175.0 {
                return (!(distx10 > 29.0) || !(distx10 < jf32!(58.5)))
                    && (!(distx10 > jf32!(116.5)) || !(distx10 < 146.0));
            }
        }
        let dx = x - self.x;
        let dy = y - self.y;
        dx * dx + dy * dy <= self.circle_radius * self.circle_radius
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.atk[self.side].remove(self.id);
        game.kill_element(self.id);
    }
}
