//! 兵玉基类（Java `Ball`）与 Wave A 的 6 种子类：Target/Nie/Sai/Tate/Han/Kekkai。
//!
//! `Ball` 以 `BallKind` 表达子类行为；后续波次继续扩展 enum 与 dispatch。
//! step 顺序、判空、RNG 调用点逐行对应 oracle。

use crate::engine::{BulletKind, GameTask, ParentList};
use crate::fdlibm;
use crate::gen_literals::jf32;
use crate::jmath;
use crate::rng;
use crate::trig;

pub const BALL_RADIUS: f32 = 23.25;

#[derive(Clone, Debug)]
pub enum BallKind {
    Plain,
    Target,
    Nie { alarm: i32 },
    Sai,
    Tate { flipped: bool, hurt_time: i32 },
    Han { t_id: i32 },
    Kekkai { hurt_time: i32 },
    Sword,
    Yari,
    Nagi,
    Bone,
    Kabe { shooting: bool, unit_x: i32 },
    Heal,
    Repair,
    Knight { flipped: bool, hurt_time: i32 },
    Bow,
    Dokyu,
    Gun,
    Cannon,
    Shotgun,
    Minigun,
    Sniper,
    Retsu,
    Bomb,
    Con,
    Gei { t_id: i32 },
    Geki,
    Guide,
    Hana,
    Hane,
    Heri,
    Holy,
    Kaku,
    Kan,
    Nin,
    Push,
    Search,
    Sen,
    Sha,
    Syou,
    Tobi,
    Ton,
    Tui,
    Uki,
    Magic { shooting: bool },
    Hen,
    Box,
    Dark,
    Star { shooting: bool },
}

#[derive(Clone, Debug)]
pub struct Ball {
    pub id: i32,
    pub side: usize,
    pub type_: i32,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub cnt: i32,
    pub jump_flg: i32,
    pub speed: i32,
    pub on_side: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub rot: i32,
    pub drop_y: f32,
    pub xs: f32,
    pub ys: f32,
    pub cos_rot: f32,
    pub sin_rot: f32,
    /// land 的上一支撑墙 id（-1 = 无）：仅作扫描优先提示，精确判定自证，无语义作用
    pub land_wall: i32,
    pub kind: BallKind,
}

impl Ball {
    /// Java `Ball` 基类构造器（子类属性由调用方在返回后设置，与 Java super 后赋值一致）
    pub fn new(
        id: i32,
        x: f32,
        y: f32,
        r_deg: i32,
        side: usize,
        type_: i32,
        kind: BallKind,
    ) -> Self {
        let x = jmath::xy_sync(x);
        let y = jmath::xy_sync(y);
        Self {
            id,
            side,
            type_,
            x,
            y,
            r: BALL_RADIUS,
            cnt: 0,
            jump_flg: 0,
            speed: 0,
            on_side: side,
            hp: 10,
            max_hp: 10,
            rot: r_deg,
            drop_y: 0.0,
            xs: 0.0,
            ys: 0.0,
            cos_rot: trig::cos(r_deg),
            sin_rot: trig::sin(r_deg),
            land_wall: -1,
            kind,
        }
    }

    pub fn hurt_time(&self) -> i32 {
        match &self.kind {
            BallKind::Tate { hurt_time, .. }
            | BallKind::Knight { hurt_time, .. }
            | BallKind::Kekkai { hurt_time } => *hurt_time,
            _ => 0,
        }
    }

    /// Java `Ball.step()`
    pub fn step(&mut self, game: &mut GameTask) {
        if self.jump_flg != 1 {
            if self.jump_flg != 2 {
                if !self.land(game) {
                    return;
                }
            } else if !self.ground(game) {
                return;
            }
        } else if !self.jump(game) {
            return;
        }
        if game.saihai_cnt[self.side] > 0
            && ((self.jump_flg == 0 && self.side != self.on_side) || self.jump_flg == 2)
        {
            self.jump_flg = 1;
            self.xs = trig::cos(game.saihai_rot[self.side]) * 4.0;
            self.ys = -2.0 + trig::sin(game.saihai_rot[self.side]) * 4.0;
            if self.ys > -2.0 {
                self.ys = -2.0;
            }
        }
        if self.jump_flg == 0 && game.jump_u[self.side].hit_test(game, self.x, self.y) {
            self.jump_flg = 1;
            self.xs = 4.0 - 8.0 * self.on_side as f32;
            self.ys = -8.0;
            self.on_side = 1 - self.on_side;
        } else if self.jump_flg == 0 && game.jump_f[self.side].hit_test(game, self.x, self.y) {
            self.jump_flg = 1;
            self.xs = 15.0 - 30.0 * self.on_side as f32;
            self.ys = -4.0;
            self.on_side = 1 - self.on_side;
        } else if game.snipe[self.side].hit_test(game, self.x, self.y + 16.0) {
            self.snipe(game);
        } else if game.turn_ccw[self.side].hit_test(game, self.x, self.y + 16.0) {
            self.turn(-2);
        } else if game.turn_cw[self.side].hit_test(game, self.x, self.y + 16.0) {
            self.turn(2);
        }
        let opp = 1 - self.side;
        if game.atk_hit_4corner(opp, self.x, self.y, 8.0)
            || (self.jump_flg == 0 && game.dokkan_flg[self.on_side])
        {
            let crash = game.dokkan_flg[self.on_side];
            self.hurt(crash);
            self.hp -= 1;
        }
        if self.hp <= 0 || (game.hp0_flg[self.on_side] > 0 && self.jump_flg == 0) {
            game.spawn_hits_drop(self.x, self.y, ParentList::Unit(self.side));
            game.dead_last[self.side] = self.type_;
            self.kill(game);
            return;
        }
        if self.hp < self.max_hp && game.heal[self.side].hit_test(game, self.x, self.y) {
            self.hp += 1;
        }
        self.step_ex(game);
    }

    /// Java `Ball.land()`
    pub fn land(&mut self, game: &mut GameTask) -> bool {
        #[cfg(feature = "phase-timing")]
        let saved = crate::engine::prof::set_caller(0);
        let result = self.land_inner(game);
        #[cfg(feature = "phase-timing")]
        crate::engine::prof::set_caller(saved);
        result
    }

    #[inline]
    fn land_inner(&mut self, game: &mut GameTask) -> bool {
        if self.jump_flg == 0 && self.side != self.on_side && game.hp0_flg[self.on_side] > 0 {
            self.jump_flg = 1;
            self.xs = rng::next(&mut game.rng_seed) * 7.0 - 3.0;
            self.ys = rng::next(&mut game.rng_seed) * 4.0 - 10.0;
            self.on_side = 1 - self.on_side;
        }
        self.cnt += 1;
        if game.base_id[self.on_side] >= 0 {
            let (bx, by) = {
                let b = game.base(self.on_side);
                (b.base_move_x, b.base_move_y)
            };
            self.x += bx;
            self.y += by;
        }
        self.ys += 1.0;
        self.drop_y = self.y + self.ys;
        if self.drop_y >= 566.0 {
            self.drop_y = 566.0;
            self.jump_flg = 2;
        } else {
            let enemy = self.on_side != self.side;
            let mut first = true;
            loop {
                let qy = self.drop_y + 15.0;
                // 上一支撑墙优先：提示由精确判定 + alive/side 校验自证（死墙已离列表、
                // 跳边后 side 不符，均自动落回全扫描），不带任何键/纪元，base 移动时
                // 球与墙同步平移依然有效；布尔结果与纯列表扫描严格等价（提示命中 ⟹
                // 该元素在列表中且命中 ⟹ 扫描 OR 为真）。
                let mut hit_id = -1;
                if first {
                    first = false;
                    let h = self.land_wall;
                    #[cfg(feature = "phase-timing")]
                    crate::engine::prof::inc(&crate::engine::prof::LMC);
                    if h >= 0 && game.wall_hint_hit(self.on_side, h, self.x, qy) {
                        #[cfg(feature = "phase-timing")]
                        crate::engine::prof::inc(&crate::engine::prof::LMC_HIT);
                        hit_id = h;
                    }
                }
                if hit_id < 0 {
                    hit_id = if enemy {
                        game.wall_hit_id(self.on_side, self.x, qy)
                    } else {
                        game.wall_hit_own_id(self.on_side, self.x, qy)
                    };
                }
                if hit_id < 0 {
                    break;
                }
                self.land_wall = hit_id;
                self.jump_flg = 0;
                self.drop_y -= 1.0;
                self.ys = 0.0;
            }
        }
        self.y = self.drop_y;
        self.y = jmath::xy_sync(self.y);
        true
    }
    /// Java `Ball.ground()`
    pub fn ground(&mut self, game: &mut GameTask) -> bool {
        #[cfg(feature = "phase-timing")]
        let saved = crate::engine::prof::set_caller(2);
        let result = self.ground_inner(game);
        #[cfg(feature = "phase-timing")]
        crate::engine::prof::set_caller(saved);
        result
    }

    #[inline]
    fn ground_inner(&mut self, game: &mut GameTask) -> bool {
        self.cnt += 1;
        if game.wall_hit(1 - self.side, self.x, self.y) {
            game.dead_last[self.side] = self.type_;
            self.kill(game);
            return false;
        }
        true
    }

    /// Java `Ball.jump()`
    pub fn jump(&mut self, game: &mut GameTask) -> bool {
        #[cfg(feature = "phase-timing")]
        let saved = crate::engine::prof::set_caller(1);
        let result = self.jump_inner(game);
        #[cfg(feature = "phase-timing")]
        crate::engine::prof::set_caller(saved);
        result
    }

    #[inline]
    fn jump_inner(&mut self, game: &mut GameTask) -> bool {
        self.ys += jf32!(0.32);
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        if self.y >= 566.0 {
            self.y = 566.0;
            self.xs = 0.0;
            self.jump_flg = 2;
        } else if game.wall_hit_own(self.on_side, self.x, self.y + 15.0) {
            self.jump_flg = 0;
        } else {
            let side_x = if self.xs < 0.0 { -16.0 } else { 16.0 };
            if game.wall_hit_own(self.on_side, self.x + side_x, self.y) {
                self.xs = 0.0;
            }
        }
        if self.x < 0.0 || self.x > 1920.0 {
            self.kill(game);
            return false;
        }
        true
    }

    /// Java `Ball.snipe()`
    pub fn snipe(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        let angle = crate::fdlibm::atan2(
            (game.core_y[opp] - self.y) as f64,
            (game.core_x[opp] - self.x) as f64,
        );
        let mut wrk_target = jmath::round_f64(crate::fdlibm::to_degrees(angle)) as i32;
        if self.rot - 180 > wrk_target {
            wrk_target += 360;
        } else if self.rot + 180 < wrk_target {
            wrk_target -= 360;
        }
        if self.rot > wrk_target {
            self.rot -= 1;
            self.cos_rot = trig::cos(self.rot);
            self.sin_rot = trig::sin(self.rot);
        } else if self.rot < wrk_target {
            self.rot += 1;
            self.cos_rot = trig::cos(self.rot);
            self.sin_rot = trig::sin(self.rot);
        }
        if self.rot > 360 {
            self.rot -= 360;
        } else if self.rot < 0 {
            self.rot += 360;
        }
    }

    /// Java `Ball.turn(int)`
    pub fn turn(&mut self, degree: i32) {
        self.rot += (if self.on_side == 0 { 1 } else { -1 }) * degree;
        self.cos_rot = trig::cos(self.rot);
        self.sin_rot = trig::sin(self.rot);
        if self.rot > 360 {
            self.rot -= 360;
        } else if self.rot < 0 {
            self.rot += 360;
        }
    }

    /// Java `Ball.hurt(boolean)`（子类覆写）
    pub fn hurt(&mut self, is_crash: bool) {
        match &mut self.kind {
            BallKind::Tate { hurt_time, .. } | BallKind::Knight { hurt_time, .. } => {
                if !is_crash {
                    *hurt_time = 3;
                }
            }
            BallKind::Kabe { shooting, unit_x } => {
                *shooting = false;
                self.cnt = -(*unit_x) % self.speed;
            }
            BallKind::Minigun => {
                if !is_crash {
                    self.cnt = 0;
                }
            }
            BallKind::Gei { t_id } => {
                self.cnt = 0;
                *t_id = -1;
            }
            BallKind::Holy => {
                self.cnt = 0;
            }
            BallKind::Magic { shooting } => {
                *shooting = false;
            }
            BallKind::Star { shooting } => {
                *shooting = false;
            }
            BallKind::Kekkai { hurt_time } => {
                let _ = is_crash;
                *hurt_time = 5;
            }
            BallKind::Han { t_id } => {
                self.cnt = 0;
                *t_id = -1;
            }
            _ => {}
        }
    }

    /// Java `Ball.stepEx()`（子类覆写）
    // 保留 Java 的 match+if 结构（clippy 建议的 guard 会改变绑定形状）
    #[allow(clippy::collapsible_match)]
    pub fn step_ex(&mut self, game: &mut GameTask) {
        match &mut self.kind {
            BallKind::Nie { alarm } => {
                if *alarm > 0 {
                    *alarm -= 1;
                    if *alarm == 0 {
                        self.hp = 0;
                    }
                }
            }
            BallKind::Sai => {
                if self.cnt == self.speed + 5 {
                    self.cnt = 0;
                    game.saihai_cnt[self.side] = 2;
                    game.saihai_rot[self.side] = self.rot;
                }
            }
            BallKind::Tate { hurt_time, .. } => {
                if *hurt_time > 0 {
                    *hurt_time -= 1;
                }
            }
            BallKind::Han { t_id } => {
                if self.cnt > self.speed + 5 && *t_id == -1 && self.jump_flg != 1 {
                    let mut t_dist2 = 4225.0f32;
                    let opp = 1 - self.side;
                    for k in 0..game.atk[opp].ids.len() {
                        let bid = game.atk[opp].ids[k];
                        if !game.is_bullet_gei1(bid) {
                            continue;
                        }
                        let (bx, by) = game.raw(bid).xy();
                        let dx = bx - self.x;
                        let dy = by - self.y;
                        if dx > 65.0 || dy > 65.0 || dx < -65.0 || dy < -65.0 {
                            continue;
                        }
                        let dist2 = dx * dx + dy * dy;
                        let dot = dx * self.cos_rot + dy * self.sin_rot;
                        if dot < 0.0 || dot * dot < dist2 * jf32!(0.8213938048432696) {
                            continue;
                        }
                        if dist2 <= t_dist2 {
                            t_dist2 = dist2;
                            *t_id = bid;
                        }
                    }
                    if *t_id > -1 {
                        game.reflect_bullet(*t_id, self.rot);
                        self.cnt = 0;
                        *t_id = -1;
                    }
                }
            }
            BallKind::Kekkai { hurt_time } => {
                if *hurt_time > 0 {
                    *hurt_time -= 1;
                }
            }
            BallKind::Sword => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    game.spawn_hits_ken(
                        self.x,
                        self.y,
                        self.rot,
                        self.side,
                        self.id,
                        self.cos_rot,
                        self.sin_rot,
                    );
                }
            }
            BallKind::Yari => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    game.spawn_hit_yari(self.x, self.y, self.rot, self.side, self.id);
                }
            }
            BallKind::Nagi => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    game.spawn_hits_nagi(
                        self.x,
                        self.y,
                        self.rot,
                        self.side,
                        self.id,
                        self.cos_rot,
                        self.sin_rot,
                    );
                }
            }
            BallKind::Bone => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    game.spawn_hits_bone(
                        self.x,
                        self.y,
                        self.rot,
                        self.side,
                        self.id,
                        self.cos_rot,
                        self.sin_rot,
                    );
                }
            }
            BallKind::Kabe { shooting, .. } => {
                if self.jump_flg == 1 {
                    *shooting = false;
                } else {
                    if self.cnt == self.speed {
                        *shooting = true;
                        game.spawn_hits_kabe(
                            self.x + 20.0 * self.cos_rot,
                            self.y + 20.0 * self.sin_rot,
                            self.side,
                            self.id,
                            self.cos_rot,
                            self.sin_rot,
                        );
                    }
                    if self.cnt > self.speed + 300 {
                        self.cnt = 0;
                        *shooting = false;
                    }
                }
            }
            BallKind::Heal => {
                if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    game.spawn_hits_heal(
                        self.x + self.cos_rot * 120.0,
                        self.y + self.sin_rot * 120.0,
                        ParentList::Heal(self.side),
                        100.0,
                    );
                }
            }
            BallKind::Repair => {
                if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    game.spawn_hits_heal(
                        self.x + self.cos_rot * 120.0,
                        self.y + self.sin_rot * 120.0,
                        ParentList::Repair(self.side),
                        100.0,
                    );
                }
            }
            BallKind::Knight { .. } => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    game.spawn_hits_knight(
                        self.x,
                        self.y,
                        self.rot,
                        self.side,
                        self.id,
                        self.cos_rot,
                        self.sin_rot,
                    );
                }
            }
            BallKind::Bomb => {
                if self.cnt == self.speed + 1 {
                    self.cnt = 0;
                    let id = game.spawn_bullet(self.x, self.y - 10.0, self.side, BulletKind::Bomb);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 10.0);
                    game.bullet_set_gravity(id, jf32!(0.32));
                }
            }
            BallKind::Con => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 15.0;
                    let spawn_y = self.y + self.sin_rot * 15.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Con {
                            speed: 1.0,
                            hp: 5,
                            cnt: 0,
                            next_target_index: -1,
                            rot: self.rot,
                            cos_rot: self.cos_rot,
                            sin_rot: self.sin_rot,
                        },
                    );
                    game.bullet_set_radius(id, jf32!(12.4));
                    game.bullet_set_vec(id, self.cos_rot, self.sin_rot);
                }
            }
            BallKind::Gei { t_id } => {
                if self.cnt > self.speed + 5 && *t_id == -1 && self.jump_flg != 1 {
                    let mut t_dist2 = 90601.0f32;
                    let opp = 1 - self.side;
                    for k in 0..game.atk[opp].ids.len() {
                        let bid = game.atk[opp].ids[k];
                        if !game.is_bullet_gei1(bid) {
                            continue;
                        }
                        let (bx, by) = game.raw(bid).xy();
                        let dx = bx - self.x;
                        let dy = by - self.y;
                        if dx > 301.0 || dy > 301.0 || dx < -301.0 || dy < -301.0 {
                            continue;
                        }
                        let dist2 = dx * dx + dy * dy;
                        let dot = dx * self.cos_rot + dy * self.sin_rot;
                        if dot < 0.0 || dot * dot < dist2 * jf32!(0.75) {
                            continue;
                        }
                        if dist2 <= t_dist2 {
                            t_dist2 = dist2;
                            *t_id = bid;
                        }
                    }
                    if *t_id > -1 {
                        game.set_bullet_gei_flg(*t_id, 3);
                        game.spawn_gei_link(self.id, *t_id);
                    }
                }
            }
            BallKind::Geki => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 45.0;
                    let spawn_y = self.y + self.sin_rot * 45.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Geki {
                            speed: 5.0,
                            cos_rot: self.cos_rot,
                            sin_rot: self.sin_rot,
                        },
                    );
                    game.bullet_set_vec(id, self.cos_rot * 5.0, self.sin_rot * 5.0);
                }
            }
            BallKind::Guide => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 45.0;
                    let spawn_y = self.y + self.sin_rot * 45.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Missile {
                            cnt: 0,
                            speed: 4.0,
                            rot: self.rot as f32,
                        },
                    );
                    game.bullet_update_velocity_torad(id, 4.0);
                }
            }
            BallKind::Hana => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 45.0;
                    let spawn_y = self.y + self.sin_rot * 45.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Hanabi {
                            cnt: 0,
                            rot: self.rot,
                        },
                    );
                    game.bullet_set_radius(id, jf32!(15.5) * jf32!(0.8));
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 10.0);
                    game.bullet_move_now(id);
                }
            }
            BallKind::Hane => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let id = game.spawn_bullet(
                        self.x,
                        self.y,
                        self.side,
                        BulletKind::Hane { bounce: 0 },
                    );
                    game.bullet_set_radius(id, jf32!(15.5) * jf32!(0.8));
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 18.0);
                    game.bullet_move_now(id);
                }
            }
            BallKind::Heri => {
                if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 25.0;
                    let spawn_y = self.y + self.sin_rot * 25.0;
                    let xl1 = if self.side == 0 { -640 } else { 0 };
                    let xl2 = if self.side != 0 { 2560 } else { 1920 };
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Heri {
                            rot: self.rot as f32,
                            speed: 1.0,
                            hp: 3,
                            cnt: 0,
                            xl1,
                            xl2,
                            yrot: 0,
                        },
                    );
                    game.set_bullet_gei_flg(id, 0);
                    game.bullet_set_vec(id, self.cos_rot, self.sin_rot);
                }
            }
            BallKind::Holy => {
                if self.cnt > self.speed + 5 && game.dead_last[self.side] != 0 && self.jump_flg != 1
                {
                    self.cnt = 0;
                    game.spawn_holy_ghost(
                        self.x + self.cos_rot * 38.0,
                        self.y + self.sin_rot * 38.0,
                        game.dead_last[self.side],
                        self.rot,
                        self.side,
                        self.on_side,
                        self.jump_flg,
                    );
                    game.dead_last[self.side] = 0;
                }
            }
            BallKind::Kaku => {
                if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 45.0;
                    let spawn_y = self.y + self.sin_rot * 45.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Kaku {
                            speed: 1.0,
                            hp: 4,
                            cos_rot: self.cos_rot,
                            sin_rot: self.sin_rot,
                        },
                    );
                    game.bullet_set_vec(id, self.cos_rot, self.sin_rot);
                }
            }
            BallKind::Kan => {
                if self.cnt == self.speed + 7 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 50.0;
                    let spawn_y = self.y + self.sin_rot * 50.0;
                    let id = game.spawn_bullet(spawn_x, spawn_y, self.side, BulletKind::Kan);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 40.0);
                    game.bullet_move_now(id);
                }
            }
            BallKind::Nin => {
                if self.cnt == self.speed + 3 {
                    self.cnt = 0;
                    let id =
                        game.spawn_bullet(self.x, self.y, self.side, BulletKind::Nin { bounce: 0 });
                    game.bullet_set_radius(id, jf32!(12.4));
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 18.0);
                    game.bullet_move_now(id);
                }
            }
            BallKind::Push => {
                if self.cnt == self.speed + 6 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 50.0;
                    let spawn_y = self.y + self.sin_rot * 50.0;
                    let id = game.spawn_bullet(spawn_x, spawn_y, self.side, BulletKind::Push);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 20.0);
                    game.bullet_move_now(id);
                }
            }
            BallKind::Search => {
                if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 35.0;
                    let spawn_y = self.y + self.sin_rot * 35.0;
                    game.spawn_search_shot(spawn_x, spawn_y, self.side, self.cos_rot, self.sin_rot);
                }
            }
            BallKind::Sen => {
                if self.cnt == self.speed && self.jump_flg != 1 {
                    // Java 空分支：此时不发射
                } else if self.cnt == self.speed + 3 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 35.0;
                    let spawn_y = self.y + self.sin_rot * 35.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Sen {
                            hp: 3,
                            cnt: 0,
                            drop_flg: false,
                        },
                    );
                    game.set_bullet_gei_flg(id, 0);
                    let (c, sn) = (crate::trig::cos(self.rot), crate::trig::sin(self.rot));
                    game.bullet_set_vec(id, c * 6.0, sn * 6.0);
                }
            }
            BallKind::Sha => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 70.0;
                    let spawn_y = self.y + self.sin_rot * 70.0;
                    game.spawn_sha_ray(
                        spawn_x,
                        spawn_y,
                        self.side,
                        self.cos_rot * 10.0,
                        self.sin_rot * 10.0,
                        6,
                    );
                }
            }
            BallKind::Syou => {
                if self.cnt == self.speed + 7 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 30.0;
                    let spawn_y = self.y + self.sin_rot * 30.0;
                    game.spawn_syou_shield(spawn_x, spawn_y, self.side, self.cos_rot, self.sin_rot);
                }
            }
            BallKind::Tobi => {
                if self.cnt == self.speed && self.jump_flg != 1 {
                    // Java 空分支
                } else if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 25.0;
                    let spawn_y = self.y + self.sin_rot * 25.0;
                    let xl1 = if self.side == 0 { -640 } else { 0 };
                    let xl2 = if self.side != 0 { 2560 } else { 1920 };
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Tobi {
                            rot: self.rot as f32,
                            speed: 1.0,
                            hp: 3,
                            cnt: 0,
                            xl1,
                            xl2,
                        },
                    );
                    game.set_bullet_gei_flg(id, 0);
                    game.bullet_set_vec(id, self.cos_rot, self.sin_rot);
                }
            }
            BallKind::Ton => {
                if self.cnt == self.speed + 3 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 38.0;
                    let spawn_y = self.y + self.sin_rot * 38.0;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Ton { hp: 3, bounce: 0 },
                    );
                    game.bullet_set_radius(id, jf32!(15.5) * jf32!(1.2));
                    game.bullet_set_gravity(id, 0.0);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 10.0);
                    game.bullet_set_gravity(id, jf32!(0.32));
                }
            }
            BallKind::Tui => {
                if self.cnt == self.speed + 3 {
                    self.cnt = 0;
                    let id = game.spawn_bullet(
                        self.x + self.cos_rot * 35.0,
                        self.y + self.sin_rot * 35.0,
                        self.side,
                        BulletKind::Tui {
                            hp: 3,
                            cnt: 0,
                            drop_flg: false,
                        },
                    );
                    game.set_bullet_gei_flg(id, 0);
                    game.bullet_set_vec(id, self.cos_rot * 6.0, self.sin_rot * 6.0);
                }
            }
            BallKind::Magic { shooting } => {
                if self.jump_flg == 1 {
                    *shooting = false;
                } else {
                    if self.cnt == self.speed + 1 {
                        *shooting = true;
                        let sx = self.x + 38.0 * trig::cos(self.rot);
                        let sy = self.y + 38.0 * trig::sin(self.rot);
                        game.spawn_magic_laser(sx, sy, self.rot, self.side, self.id);
                    }
                    if self.cnt >= self.speed + 38 {
                        self.cnt = 0;
                        *shooting = false;
                    }
                }
            }
            BallKind::Star { shooting } => {
                if self.jump_flg == 1 {
                    *shooting = false;
                } else {
                    if self.cnt == self.speed + 1 {
                        *shooting = true;
                        let sx = self.x + 28.0 * trig::cos(self.rot);
                        let sy = self.y + 28.0 * trig::sin(self.rot);
                        game.spawn_star_prepare(sx, sy, self.rot, self.side, self.id);
                    }
                    if self.cnt >= self.speed + 31 {
                        self.cnt = 0;
                        *shooting = false;
                    }
                }
            }
            BallKind::Hen => {
                if self.cnt == self.speed + 8 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 25.0;
                    let spawn_y = self.y + self.sin_rot * 25.0;
                    let spawn_type = jmath::round_f32(rng::next(&mut game.rng_seed) * 7.0);
                    game.spawn_creature(spawn_x, spawn_y, self.side, self.rot, spawn_type);
                }
            }
            BallKind::Box => {
                let px = self.x + self.cos_rot * 38.0;
                let py = self.y + self.sin_rot * 38.0;
                let blocked = crate::geom::Round::new(self.x, self.y, self.r)
                    .hit_test_point(px, py)
                    || game.fort_hit_skip(self.on_side, px, py, self.id);
                if self.cnt == self.speed && self.jump_flg != 1 && blocked {
                    self.cnt -= 1;
                } else if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    game.spawn_box(
                        self.x + self.cos_rot * 38.0,
                        self.y + self.sin_rot * 38.0,
                        self.on_side,
                        if self.jump_flg == 2 {
                            -1
                        } else {
                            self.on_side as i32
                        },
                    );
                }
            }
            BallKind::Dark => {
                if self.cnt == self.speed + 4 {
                    self.cnt = 0;
                    game.spawn_bone_summon(
                        self.x + self.cos_rot * 38.0,
                        self.y + self.sin_rot * 38.0,
                        self.rot,
                        self.side,
                        1 - self.side,
                    );
                }
            }
            BallKind::Uki => {
                if self.cnt == self.speed + 8 {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 50.0;
                    let spawn_y = self.y + self.sin_rot * 50.0;
                    game.spawn_uki_shot(
                        spawn_x,
                        spawn_y,
                        self.side,
                        self.cos_rot * 3.0,
                        self.sin_rot * 3.0,
                    );
                }
            }
            BallKind::Bow => {
                if self.cnt == self.speed + 6 {
                    self.cnt = 0;
                    let id = game.spawn_bullet(self.x, self.y, self.side, BulletKind::Plain);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 15.0);
                    game.bullet_set_gravity(id, jf32!(0.32));
                }
            }
            BallKind::Dokyu => {
                if self.cnt == self.speed + 6 {
                    self.cnt = 0;
                    let id = game.spawn_bullet(self.x, self.y, self.side, BulletKind::Plain);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 25.0);
                    game.bullet_set_gravity(id, jf32!(0.32));
                }
            }
            BallKind::Gun => {
                if self.cnt == self.speed
                    || self.cnt == self.speed + 2
                    || self.cnt == self.speed + 4
                {
                    if self.cnt == self.speed + 4 {
                        self.cnt = 0;
                    }
                    let id = game.spawn_bullet(self.x, self.y, self.side, BulletKind::Gun);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 20.0);
                    game.bullet_move_now(id);
                }
            }
            BallKind::Cannon => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let flipped = if self.rot >= 90 + self.side as i32
                        && self.rot <= 270 + self.side as i32
                    {
                        -1.0f32
                    } else {
                        1.0f32
                    };
                    let spawn_x = self.x + self.sin_rot * 38.0 * flipped;
                    let spawn_y = self.y - self.cos_rot * 38.0 * flipped;
                    let id = game.spawn_bullet(
                        spawn_x,
                        spawn_y,
                        self.side,
                        BulletKind::Cannon { hp: 3 },
                    );
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 10.0);
                    game.bullet_set_gravity(id, jf32!(0.08));
                }
            }
            BallKind::Shotgun => {
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 40.0;
                    let spawn_y = self.y + self.sin_rot * 40.0;
                    for i in 0..9 {
                        let atk_rot = self.rot - 40 + 10 * i;
                        let speed = 27.0 + rng::next(&mut game.rng_seed) * 7.0;
                        let id = game.spawn_bullet(
                            spawn_x,
                            spawn_y,
                            self.side,
                            BulletKind::Shotgun { cnt: 0 },
                        );
                        game.bullet_set_vec_r(id, atk_rot, speed);
                    }
                }
            }
            BallKind::Minigun => {
                if self.cnt >= self.speed && self.cnt % 4 == 0 && self.jump_flg != 1 {
                    let vx = self.cos_rot * 22.0;
                    let vy = self.sin_rot * 22.0;
                    let id = game.spawn_bullet(
                        self.x + vx * 3.0,
                        self.y + vy * 3.0,
                        self.side,
                        BulletKind::Gun,
                    );
                    game.bullet_set_vec(id, vx, vy);
                    game.bullet_set_power(id, 22.0);
                }
                if self.cnt >= self.speed + 200 {
                    self.cnt = 0;
                }
            }
            BallKind::Sniper => {
                let opp = 1 - self.side;
                let angle = fdlibm::atan2(
                    (game.core_y[opp] - self.y) as f64,
                    (game.core_x[opp] - self.x) as f64,
                );
                let mut t_rot = jmath::round_f32(fdlibm::to_degrees(angle) as f32);
                if self.rot - 180 > t_rot {
                    t_rot += 360;
                }
                if self.rot + 180 < t_rot {
                    t_rot -= 360;
                }
                if self.rot > t_rot {
                    self.rot -= 1;
                }
                if self.rot < t_rot {
                    self.rot += 1;
                }
                if self.rot > 360 {
                    self.rot -= 360;
                } else if self.rot < 0 {
                    self.rot += 360;
                }
                self.cos_rot = trig::cos(self.rot);
                self.sin_rot = trig::sin(self.rot);
                if self.cnt == self.speed {
                    self.cnt = 0;
                    let spawn_x = self.x + self.cos_rot * 40.0;
                    let spawn_y = self.y + self.sin_rot * 40.0;
                    game.spawn_sha_ray(
                        spawn_x,
                        spawn_y,
                        self.side,
                        self.cos_rot * 10.0,
                        self.sin_rot * 10.0,
                        3,
                    );
                }
            }
            BallKind::Retsu => {
                if self.cnt == self.speed + 1 {
                    self.cnt = 0;
                    let id = game.spawn_bullet(self.x, self.y - 10.0, self.side, BulletKind::Retsu);
                    game.bullet_set_vec_mult(id, self.cos_rot, self.sin_rot, 15.0);
                    game.bullet_set_gravity(id, jf32!(0.32));
                }
            }
            _ => {}
        }
    }

    /// Java `Ball.kill()`（KekkaiBall 覆写会先摘掉结界 ID）
    pub fn kill(&mut self, game: &mut GameTask) {
        if matches!(self.kind, BallKind::Kekkai { .. }) {
            let id = self.id;
            game.kekkai_ids[self.side].retain(|&v| v != id);
        }
        game.unit[self.side].remove(self.id);
        game.kill_element(self.id);
    }

    /// Java `TateBall.hitTestPoint`
    pub fn tate_hit_test(&self, x: f32, y: f32) -> bool {
        let (flipped, hurt_time) = match self.kind {
            BallKind::Tate { flipped, hurt_time } | BallKind::Knight { flipped, hurt_time } => {
                (flipped, hurt_time)
            }
            _ => unreachable!(),
        };
        let mut dx = x - self.x;
        let mut dy = y - self.y;
        if dx > 54.0 || dy > 54.0 || dx < -54.0 || dy < -54.0 {
            return false;
        }
        dx = (x - self.x) * self.cos_rot + (y - self.y) * self.sin_rot;
        dy = (y - self.y) * self.cos_rot - (x - self.x) * self.sin_rot;
        if flipped {
            dy = -dy;
        }
        if hurt_time > 0 {
            if dx > jf32!(26.75)
                || (dy as f64) > crate::gen_literals::jf64!(25.45)
                || dx < -jf32!(43.4)
                || dy < -jf32!(44.75)
            {
                return false;
            }
            if dx * dx + dy * dy <= jf32!(647.7025) {
                return true;
            }
            let mut dx2 = dx - jf32!(20.1);
            let mut dy2 = dy + jf32!(22.5);
            if dx2 * dx2 + dy2 * dy2 <= jf32!(43.23) {
                return true;
            }
            if (dx + jf32!(31.25)) * jf32!(0.9) - (dy + jf32!(43.45)) * jf32!(22.55) <= 0.0
                && (dx + jf32!(8.7)) * jf32!(41.65) + (dy + jf32!(42.55)) * jf32!(24.05) <= 0.0
                && -(dx + jf32!(32.75)) * jf32!(18.35) + (dy + jf32!(0.9)) * jf32!(12.35) <= 0.0
                && -(dx + jf32!(45.1)) * jf32!(24.2) - (dy + jf32!(19.25)) * jf32!(13.85) <= 0.0
            {
                return true;
            }
            dx2 = dx + jf32!(20.5);
            dy2 = dy + jf32!(22.5);
            return dx2 * dx2 + dy2 * dy2 <= jf32!(43.23);
        }
        if dx > 55.0
            || (dy as f64) > crate::gen_literals::jf64!(25.2)
            || -dx > self.r
            || dy < -jf32!(23.8)
        {
            return false;
        }
        if dx >= jf32!(16.15) {
            dx -= jf32!(16.15);
            dy -= jf32!(0.7);
            dx * dx * jf32!(600.25) + dy * dy * jf32!(1509.3225) <= jf32!(905970.830625)
        } else {
            dx * dx + dy * dy <= self.r * self.r
        }
    }

    /// `Utils.cos/sin(rot)` 的公共重算（测试与外部使用）
    pub fn refresh_trig(&mut self) {
        self.cos_rot = trig::cos(self.rot);
        self.sin_rot = trig::sin(self.rot);
    }
}

/// 便于 `unit_make` 应用子类构造后的属性（Java 的 super 后赋值）
pub struct BallInit {
    pub hp: i32,
    pub speed: i32,
    pub flipped: bool,
}

impl BallInit {
    pub fn for_kind(kind: &BallKind, rot: i32, side: usize) -> Self {
        let flipped = rot >= 90 + side as i32 && rot <= 270 + side as i32;
        match kind {
            BallKind::Target => BallInit {
                hp: 30,
                speed: 0,
                flipped: false,
            },
            BallKind::Nie { .. } => BallInit {
                hp: 30,
                speed: 0,
                flipped: false,
            },
            BallKind::Sai => BallInit {
                hp: 15,
                speed: 200,
                flipped: false,
            },
            BallKind::Tate { .. } => BallInit {
                hp: 30,
                speed: 0,
                flipped,
            },
            BallKind::Han { .. } => BallInit {
                hp: 15,
                speed: 20,
                flipped: false,
            },
            BallKind::Kekkai { .. } => BallInit {
                hp: 20,
                speed: 0,
                flipped: false,
            },
            BallKind::Sword => BallInit {
                hp: 30,
                speed: 10,
                flipped: false,
            },
            BallKind::Yari => BallInit {
                hp: 30,
                speed: 15,
                flipped: false,
            },
            BallKind::Nagi => BallInit {
                hp: 30,
                speed: 20,
                flipped: false,
            },
            BallKind::Bone => BallInit {
                hp: 10,
                speed: 30,
                flipped: false,
            },
            BallKind::Kabe { .. } => BallInit {
                hp: 20,
                speed: 60,
                flipped: false,
            },
            BallKind::Heal => BallInit {
                hp: 10,
                speed: 150,
                flipped: false,
            },
            BallKind::Repair => BallInit {
                hp: 10,
                speed: 150,
                flipped: false,
            },
            BallKind::Knight { .. } => BallInit {
                hp: 50,
                speed: 7,
                flipped,
            },
            BallKind::Plain => BallInit {
                hp: 10,
                speed: 0,
                flipped: false,
            },
            BallKind::Magic { .. } => BallInit {
                hp: 10,
                speed: 150,
                flipped: false,
            },
            BallKind::Hen => BallInit {
                hp: 10,
                speed: 100,
                flipped: false,
            },
            BallKind::Box => BallInit {
                hp: 15,
                speed: 200,
                flipped: false,
            },
            BallKind::Dark => BallInit {
                hp: 10,
                speed: 200,
                flipped: false,
            },
            BallKind::Star { .. } => BallInit {
                hp: 10,
                speed: 250,
                flipped: false,
            },
            BallKind::Bow => BallInit {
                hp: 10,
                speed: 30,
                flipped: false,
            },
            BallKind::Dokyu => BallInit {
                hp: 10,
                speed: 30,
                flipped: false,
            },
            BallKind::Gun => BallInit {
                hp: 10,
                speed: 60,
                flipped: false,
            },
            BallKind::Cannon => BallInit {
                hp: 10,
                speed: 120,
                flipped: false,
            },
            BallKind::Shotgun => BallInit {
                hp: 10,
                speed: 50,
                flipped: false,
            },
            BallKind::Minigun => BallInit {
                hp: 10,
                speed: 200,
                flipped: false,
            },
            BallKind::Sniper => BallInit {
                hp: 10,
                speed: 100,
                flipped: false,
            },
            BallKind::Retsu => BallInit {
                hp: 10,
                speed: 120,
                flipped: false,
            },
            BallKind::Bomb => BallInit {
                hp: 10,
                speed: 80,
                flipped: false,
            },
            BallKind::Con => BallInit {
                hp: 10,
                speed: 200,
                flipped: false,
            },
            BallKind::Gei { .. } => BallInit {
                hp: 15,
                speed: 40,
                flipped: false,
            },
            BallKind::Geki => BallInit {
                hp: 10,
                speed: 120,
                flipped: false,
            },
            BallKind::Guide => BallInit {
                hp: 10,
                speed: 180,
                flipped: false,
            },
            BallKind::Hana => BallInit {
                hp: 10,
                speed: 120,
                flipped: false,
            },
            BallKind::Hane => BallInit {
                hp: 10,
                speed: 50,
                flipped: false,
            },
            BallKind::Heri => BallInit {
                hp: 10,
                speed: 300,
                flipped: false,
            },
            BallKind::Holy => BallInit {
                hp: 10,
                speed: 300,
                flipped: false,
            },
            BallKind::Kaku => BallInit {
                hp: 10,
                speed: 350,
                flipped: false,
            },
            BallKind::Kan => BallInit {
                hp: 10,
                speed: 170,
                flipped: false,
            },
            BallKind::Nin => BallInit {
                hp: 10,
                speed: 70,
                flipped: false,
            },
            BallKind::Push => BallInit {
                hp: 10,
                speed: 80,
                flipped: false,
            },
            BallKind::Search => BallInit {
                hp: 10,
                speed: 100,
                flipped: false,
            },
            BallKind::Sen => BallInit {
                hp: 10,
                speed: 300,
                flipped: false,
            },
            BallKind::Sha => BallInit {
                hp: 10,
                speed: 70,
                flipped: false,
            },
            BallKind::Syou => BallInit {
                hp: 10,
                speed: 160,
                flipped: false,
            },
            BallKind::Tobi => BallInit {
                hp: 10,
                speed: 300,
                flipped: false,
            },
            BallKind::Ton => BallInit {
                hp: 10,
                speed: 70,
                flipped: false,
            },
            BallKind::Tui => BallInit {
                hp: 10,
                speed: 300,
                flipped: false,
            },
            BallKind::Uki => BallInit {
                hp: 10,
                speed: 150,
                flipped: false,
            },
        }
    }
}
