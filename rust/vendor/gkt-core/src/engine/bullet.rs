//! 子弹基类（Java `Bullet`）与 Wave C1 子类（Gun/Cannon/Shotgun/Retsu）以及 `ShaBullet` 射线。
//!
//! `Bullet.id` 在 Java 里遮蔽 `Shape.id`；本移植用自己的 id 字段并对拍用 `itemKeys`。

use crate::engine::{GameTask, ParentList};
use crate::fdlibm;
use crate::gen_literals::jf32;
use crate::jmath;
use crate::trig;

pub const BULLET_RADIUS: f32 = 15.5;

#[derive(Clone, Debug)]
pub enum BulletKind {
    Plain,
    Gun,
    Cannon {
        hp: i32,
    },
    Shotgun {
        cnt: i32,
    },
    Retsu,
    Bomb,
    Con {
        speed: f32,
        hp: i32,
        cnt: i32,
        next_target_index: i32,
        rot: i32,
        cos_rot: f32,
        sin_rot: f32,
    },
    ConMissile {
        speed: f32,
        cnt: i32,
        target_id: i32,
        desired_rot: f32,
        rot: f32,
    },
    Geki {
        speed: f32,
        cos_rot: f32,
        sin_rot: f32,
    },
    Hanabi {
        cnt: i32,
        rot: i32,
    },
    Hane {
        bounce: i32,
    },
    Heri {
        rot: f32,
        speed: f32,
        hp: i32,
        cnt: i32,
        xl1: i32,
        xl2: i32,
        yrot: i32,
    },
    Hinoko {
        cnt: i32,
    },
    Kaku {
        speed: f32,
        hp: i32,
        cos_rot: f32,
        sin_rot: f32,
    },
    Kan,
    Missile {
        cnt: i32,
        speed: f32,
        rot: f32,
    },
    Nin {
        bounce: i32,
    },
    Push,
    Sen {
        hp: i32,
        cnt: i32,
        drop_flg: bool,
    },
    SenTama {
        hp: i32,
    },
    Tobi {
        rot: f32,
        speed: f32,
        hp: i32,
        cnt: i32,
        xl1: i32,
        xl2: i32,
    },
    TobiBomb,
    Ton {
        hp: i32,
        bounce: i32,
    },
    Tui {
        hp: i32,
        cnt: i32,
        drop_flg: bool,
    },
    TuiMissile {
        speed: f32,
        cos_rot: f32,
        sin_rot: f32,
    },
    BossMissile {
        cnt: i32,
        target_rot: f32,
        target_id: i32,
        rot: f32,
        target_rot_delta: f32,
    },
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub gei_flg: i32,
    pub xs: f32,
    pub ys: f32,
    pub gravity: f32,
    pub power: f32,
    pub kind: BulletKind,
}

#[inline]
fn cos32(v: f32) -> f32 {
    fdlibm::cos(v as f64) as f32
}

#[inline]
fn sin32(v: f32) -> f32 {
    fdlibm::sin(v as f64) as f32
}

impl Bullet {
    pub fn new(id: i32, x: f32, y: f32, side: usize, kind: BulletKind) -> Self {
        Self {
            id,
            side,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: BULLET_RADIUS,
            gei_flg: 1,
            xs: 0.0,
            ys: 0.0,
            gravity: 0.0,
            power: 0.0,
            kind,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        match self.kind {
            BulletKind::Plain => self.step_plain(game),
            BulletKind::Gun => self.step_gun(game),
            BulletKind::Cannon { .. } => self.step_cannon(game),
            BulletKind::Shotgun { .. } => self.step_shotgun(game),
            BulletKind::Retsu => self.step_retsu(game),
            BulletKind::Bomb => self.step_bomb(game),
            BulletKind::Con { .. } => self.step_con(game),
            BulletKind::ConMissile { .. } => self.step_con_missile(game),
            BulletKind::Geki { .. } => self.step_geki(game),
            BulletKind::Hanabi { .. } => self.step_hanabi(game),
            BulletKind::Hane { .. } => self.step_hane(game),
            BulletKind::Heri { .. } => self.step_heri(game),
            BulletKind::Hinoko { .. } => self.step_hinoko(game),
            BulletKind::Kaku { .. } => self.step_kaku(game),
            BulletKind::Kan => self.step_kan(game),
            BulletKind::Missile { .. } => self.step_missile(game),
            BulletKind::Nin { .. } => self.step_nin(game),
            BulletKind::Push => self.step_push(game),
            BulletKind::Sen { .. } => self.step_sen(game),
            BulletKind::SenTama { .. } => self.step_sen_tama(game),
            BulletKind::Tobi { .. } => self.step_tobi(game),
            BulletKind::TobiBomb => self.step_tobi_bomb(game),
            BulletKind::Ton { .. } => self.step_ton(game),
            BulletKind::Tui { .. } => self.step_tui(game),
            BulletKind::TuiMissile { .. } => self.step_tui_missile(game),
            BulletKind::BossMissile { .. } => self.step_boss_missile(game),
        }
    }

    /// Java `Bullet.step()`（嵌套 if 与 Java 一致）
    #[allow(clippy::collapsible_if)]
    fn step_plain(&mut self, game: &mut GameTask) {
        if self.y > 570.0 {
            self.hit_ground(game);
            return;
        }
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            if !self.hit(game) {
                return;
            }
        }
        self.move_bullet();
    }

    /// Java `GunBullet.step()`
    fn step_gun(&mut self, game: &mut GameTask) {
        if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            let _ = self.hit(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `ShotgunBullet.step()`
    fn step_shotgun(&mut self, game: &mut GameTask) {
        let BulletKind::Shotgun { cnt } = &mut self.kind else {
            unreachable!()
        };
        *cnt += 1;
        let cnt_v = *cnt;
        if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 || cnt_v > 7 {
            let _ = self.hit(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `CannonBullet.step()`
    fn step_cannon(&mut self, game: &mut GameTask) {
        let mut hp = match self.kind {
            BulletKind::Cannon { hp } => hp,
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y > 570.0 {
            hp = 0;
        } else if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y)
            {
                hp = 0;
            } else {
                hp -= 1;
            }
        }
        self.move_bullet();
        if hp <= 0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb(x, y, side, jf32!(1.2));
            self.kill(game);
        } else if let BulletKind::Cannon { hp: h } = &mut self.kind {
            *h = hp;
        }
    }

    /// Java `RetsuBullet.step()`
    fn step_retsu(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) || self.ys > 7.0 || self.gei_flg == 2 {
            let angle = fdlibm::atan2(self.ys as f64, self.xs as f64);
            let split_rot = fdlibm::to_degrees(angle) as f32;
            for i in 0..7 {
                let atk_rot = split_rot - 30.0 + 10.0 * i as f32;
                let rad = fdlibm::to_radians(atk_rot as f64) as f32;
                let vx = cos32(rad);
                let vy = sin32(rad);
                let id = game.spawn_bullet(self.x, self.y, self.side, BulletKind::Gun);
                game.bullet_set_vec_mult(id, vx, vy, 20.0);
                game.bullet_move_now(id);
            }
            self.kill(game);
            return;
        }
        if self.y > 570.0 {
            self.kill(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `Bullet.hit_ground()`
    pub fn hit_ground(&mut self, game: &mut GameTask) {
        self.kill(game);
    }

    /// Java `Bullet.move()`
    pub fn move_bullet(&mut self) {
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.ys += self.gravity;
    }

    /// Java `Bullet.hit()`
    #[must_use]
    pub fn hit(&mut self, game: &mut GameTask) -> bool {
        let (x, y, side) = (self.x, self.y, self.side);
        game.spawn_hits_drop(x, y, ParentList::Atk(side));
        self.kill(game);
        false
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.atk[self.side].remove(self.id);
        game.kill_element(self.id);
    }

    /// Java `Bullet.betray()`：换边并改写 atk 列表
    pub fn betray(&mut self, game: &mut GameTask) {
        game.atk[self.side].remove(self.id);
        self.side = 1 - self.side;
        game.atk[self.side].add(self.id);
    }

    /// Java `Bullet.reflect(int)`
    pub fn reflect(&mut self, game: &mut GameTask, from_rot: i32) {
        self.betray(game);
        self.xs = trig::cos(from_rot) * self.power;
        self.ys = trig::sin(from_rot) * self.power;
    }
}

/// Java `ShaBullet` / `SniperBullet`：一次性瞄准射线（不注册传感器）
#[derive(Clone, Debug)]
pub struct ShaRay {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub xs: f32,
    pub ys: f32,
    pub frames: i32,
}

impl ShaRay {
    pub fn new(id: i32, x: f32, y: f32, side: usize, xs: f32, ys: f32, frames: i32) -> Self {
        Self {
            id,
            side,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: BULLET_RADIUS,
            xs,
            ys,
            frames,
        }
    }

    /// Java `ShaBullet.step()`
    pub fn step(&mut self, game: &mut GameTask) {
        let mut hit = false;
        let opp = 1 - self.side;
        for _ in 0..1000 {
            self.x += self.xs;
            self.y += self.ys;
            if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                break;
            }
            if game.fort_hit(opp, self.x, self.y)
                || game.shield[opp].hit_test(game, self.x, self.y)
                || self.y > 570.0
            {
                hit = true;
                break;
            }
        }
        if hit {
            game.spawn_hits_drop_frames(
                self.x,
                self.y,
                ParentList::Atk(self.side),
                self.frames,
                jf32!(1.3),
            );
        }
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        crate::geom::Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}

impl Bullet {
    /// Java `BombBullet.step()`
    fn step_bomb(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        if self.y > 570.0 || game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `ConBullet.step()`
    fn step_con(&mut self, game: &mut GameTask) {
        let (mut speed, mut hp, mut cnt, rot, cos_rot, sin_rot, mut next_target_index) =
            match self.kind {
                BulletKind::Con {
                    speed,
                    hp,
                    cnt,
                    next_target_index,
                    rot,
                    cos_rot,
                    sin_rot,
                } => (speed, hp, cnt, rot, cos_rot, sin_rot, next_target_index),
                _ => unreachable!(),
            };
        cnt += 1;
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            hp -= 1;
        }
        if cnt == 150 || cnt == 152 || cnt == 154 || cnt == 156 {
            let (x, y) = (self.x, self.y);
            let t1 = game.pick_target_id(opp, &mut next_target_index);
            let id = game.spawn_bullet(
                x,
                y,
                self.side,
                BulletKind::ConMissile {
                    speed: 12.0,
                    cnt: 0,
                    target_id: t1,
                    desired_rot: (rot - 90) as f32,
                    rot: (rot - 90) as f32,
                },
            );
            game.bullet_set_radius(id, BULLET_RADIUS * jf32!(0.6));
            game.bullet_update_velocity_torad(id, 12.0);
            let t2 = game.pick_target_id(opp, &mut next_target_index);
            let id = game.spawn_bullet(
                x,
                y,
                self.side,
                BulletKind::ConMissile {
                    speed: 12.0,
                    cnt: 0,
                    target_id: t2,
                    desired_rot: (rot + 90) as f32,
                    rot: (rot + 90) as f32,
                },
            );
            game.bullet_set_radius(id, BULLET_RADIUS * jf32!(0.6));
            game.bullet_update_velocity_torad(id, 12.0);
            if cnt == 156 {
                hp = 0;
            }
        }
        if hp <= 0 || self.y > 570.0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        if speed < 5.0 {
            speed += jf32!(0.1);
        }
        self.xs = cos_rot * speed;
        self.ys = sin_rot * speed;
        if let BulletKind::Con {
            speed: sp,
            hp: h,
            cnt: c,
            next_target_index: n,
            cos_rot: cr,
            sin_rot: sr,
            ..
        } = &mut self.kind
        {
            *sp = speed;
            *h = hp;
            *c = cnt;
            *n = next_target_index;
            *cr = cos_rot;
            *sr = sin_rot;
        }
        self.move_bullet();
    }

    /// Java `ConMissileBullet.step()`
    fn step_con_missile(&mut self, game: &mut GameTask) {
        let (mut speed, mut cnt, target_id, mut desired_rot, mut rot) = match self.kind {
            BulletKind::ConMissile {
                speed,
                cnt,
                target_id,
                desired_rot,
                rot,
            } => (speed, cnt, target_id, desired_rot, rot),
            _ => unreachable!(),
        };
        cnt += 1;
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || self.y > 570.0 || self.gei_flg == 2 || cnt > 200 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
            self.kill(game);
            return;
        }
        rot = normalize_rotation(rot);
        if target_id != -1 {
            if let Some(t) = game.get(target_id) {
                let (tx, ty) = t.xy();
                desired_rot = jmath::round_f32(fdlibm::to_degrees(fdlibm::atan2(
                    (ty - self.y) as f64,
                    (tx - self.x) as f64,
                )) as f32) as f32;
            }
        } else {
            desired_rot = jmath::round_f32(fdlibm::to_degrees(fdlibm::atan2(
                (game.core_y[opp] - self.y) as f64,
                (game.core_x[opp] - self.x) as f64,
            )) as f32) as f32;
        }
        if rot - 180.0 > desired_rot {
            desired_rot += 360.0;
        }
        if rot + 180.0 < desired_rot {
            desired_rot -= 360.0;
        }
        if speed < 15.0 {
            speed += jf32!(0.3);
        }
        if speed > 14.0 {
            rot = rot + (desired_rot - rot) / 4.0;
            rot = normalize_rotation(rot);
        }
        let rad = fdlibm::to_radians(rot as f64) as f32;
        self.xs = cos32(rad) * speed;
        self.ys = sin32(rad) * speed;
        if let BulletKind::ConMissile {
            speed: sp,
            cnt: c,
            desired_rot: dr,
            rot: rr,
            ..
        } = &mut self.kind
        {
            *sp = speed;
            *c = cnt;
            *dr = desired_rot;
            *rr = rot;
        }
        self.move_bullet();
    }

    /// Java `GekiBullet.step()`
    fn step_geki(&mut self, game: &mut GameTask) {
        let (mut speed, cos_rot, sin_rot) = match self.kind {
            BulletKind::Geki {
                speed,
                cos_rot,
                sin_rot,
            } => (speed, cos_rot, sin_rot),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || self.y > 570.0 || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        if speed < 25.0 {
            speed += 1.0;
        }
        self.xs = cos_rot * speed;
        self.ys = sin_rot * speed;
        if let BulletKind::Geki { speed: sp, .. } = &mut self.kind {
            *sp = speed;
        }
        self.move_bullet();
    }

    /// Java `HanabiBullet.step()`
    fn step_hanabi(&mut self, game: &mut GameTask) {
        let (cnt, rot) = match &mut self.kind {
            BulletKind::Hanabi { cnt, rot } => {
                *cnt += 1;
                (*cnt, *rot)
            }
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        if self.y > 570.0 || game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 || cnt > 60 {
            for i in 0..20 {
                let r = rot + 18 * i;
                let hx = self.x + trig::cos(r) * 14.0;
                let hy = self.y + trig::sin(r) * 14.0;
                let id = game.spawn_bullet(hx, hy, self.side, BulletKind::Hinoko { cnt: 0 });
                game.bullet_set_vec_r(id, r, 30.0);
            }
            let _ = self.hit(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `HaneBullet.step()`
    fn step_hane(&mut self, game: &mut GameTask) {
        let bounce = match &mut self.kind {
            BulletKind::Hane { bounce } => {
                let b = bounce;
                if self.y > 570.0 || self.y < -600.0 {
                    self.ys = -self.ys;
                    *b += 1;
                }
                if self.x > 1920.0 || self.x < 0.0 {
                    self.xs = -self.xs;
                    *b += 1;
                }
                *b
            }
            _ => unreachable!(),
        };
        if bounce > 4 {
            self.kill(game);
            return;
        }
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
            self.kill(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `HeriBullet.step()`
    fn step_heri(&mut self, game: &mut GameTask) {
        let (mut rot, mut speed, mut hp, mut cnt, xl1, xl2, mut yrot) = match self.kind {
            BulletKind::Heri {
                rot,
                speed,
                hp,
                cnt,
                xl1,
                xl2,
                yrot,
            } => (rot, speed, hp, cnt, xl1, xl2, yrot),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -600.0 || self.x > xl2 as f32 || self.x < xl1 as f32 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) {
            hp -= 1;
        }
        if hp <= 0 || self.y > 570.0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        let mut t_rot = 180.0 * self.side as f32;
        if rot - 180.0 > t_rot {
            t_rot += 360.0;
        }
        if rot + 180.0 < t_rot {
            t_rot -= 360.0;
        }
        rot = rot + (t_rot - rot) / 16.0;
        let rad = fdlibm::to_radians(rot as f64) as f32;
        self.xs = cos32(rad) * speed;
        self.ys = sin32(rad) * speed;
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        yrot += 5;
        if yrot > 360 {
            yrot -= 360;
        }
        self.y += trig::sin(yrot) * 3.0;
        self.y = jmath::xy_sync(self.y);
        if speed < jf32!(3.01) {
            speed += jf32!(0.4);
        }
        cnt += 1;
        let power = 30;
        if cnt == power || cnt == power + 6 || cnt == power + 12 {
            if cnt == power + 12 {
                cnt = 0;
            }
            let scale = if rot < 90.0 || rot > 270.0 { 1 } else { -1 };
            let atk_rot = jmath::round_f32(rot + 45.0 * scale as f32);
            let id = game.spawn_bullet(self.x, self.y, self.side, BulletKind::Gun);
            game.bullet_set_vec_r(id, atk_rot, 20.0);
            game.bullet_move_now(id);
        }
        if let BulletKind::Heri {
            rot: r,
            speed: sp,
            hp: h,
            cnt: c,
            yrot: yr,
            ..
        } = &mut self.kind
        {
            *r = rot;
            *sp = speed;
            *h = hp;
            *c = cnt;
            *yr = yrot;
        }
    }

    /// Java `HinokoBullet.step()`
    fn step_hinoko(&mut self, game: &mut GameTask) {
        let cnt = match &mut self.kind {
            BulletKind::Hinoko { cnt } => {
                *cnt += 1;
                *cnt
            }
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || cnt > 20 || self.gei_flg == 2 {
            let _ = self.hit(game);
            return;
        }
        self.xs *= jf32!(0.8);
        self.ys *= jf32!(0.8);
        self.move_bullet();
    }

    /// Java `KakuBullet.step()`
    fn step_kaku(&mut self, game: &mut GameTask) {
        let (mut speed, mut hp, cos_rot, sin_rot) = match self.kind {
            BulletKind::Kaku {
                speed,
                hp,
                cos_rot,
                sin_rot,
            } => (speed, hp, cos_rot, sin_rot),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.atk[opp].hit_test(game, self.x, self.y) || self.gei_flg == 2 {
            hp -= 1;
        }
        if hp <= 0 || self.y > 570.0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y) {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb(x, y, side, 8.0);
            self.kill(game);
            return;
        }
        if speed < 3.0 {
            speed += jf32!(0.01);
        }
        self.xs = cos_rot * speed;
        self.ys = sin_rot * speed;
        if let BulletKind::Kaku {
            speed: sp, hp: h, ..
        } = &mut self.kind
        {
            *sp = speed;
            *h = hp;
        }
        self.move_bullet();
    }

    /// Java `KanBullet.step()`
    fn step_kan(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        if game.atk[opp].hit_test(game, self.x, self.y) || self.gei_flg == 2 {
            let _ = self.hit(game);
            return;
        }
        if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y) {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
        }
        self.move_bullet();
    }

    /// Java `MissileBullet.step()`
    fn step_missile(&mut self, game: &mut GameTask) {
        let (mut cnt, mut speed, mut rot) = match self.kind {
            BulletKind::Missile { cnt, speed, rot } => (cnt, speed, rot),
            _ => unreachable!(),
        };
        cnt += 1;
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || self.y > 570.0 || self.gei_flg == 2 || cnt > 200 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        let mut target_rot = jmath::round_f32(fdlibm::to_degrees(fdlibm::atan2(
            (game.core_y[opp] - self.y) as f64,
            (game.core_x[opp] - self.x) as f64,
        )) as f32) as f32;
        if rot - 180.0 > target_rot {
            target_rot += 360.0;
        }
        if rot + 180.0 < target_rot {
            target_rot -= 360.0;
        }
        if speed < 15.0 {
            speed += jf32!(0.3);
        }
        if speed > 14.0 {
            rot = rot + (target_rot - rot) / 8.0;
        }
        let rad = fdlibm::to_radians(rot as f64) as f32;
        self.xs = cos32(rad) * speed;
        self.ys = sin32(rad) * speed;
        if let BulletKind::Missile {
            cnt: c,
            speed: sp,
            rot: r,
        } = &mut self.kind
        {
            *c = cnt;
            *sp = speed;
            *r = rot;
        }
        self.move_bullet();
    }

    /// Java `NinBullet.step()`
    fn step_nin(&mut self, game: &mut GameTask) {
        let bounce = match &mut self.kind {
            BulletKind::Nin { bounce } => {
                let b = bounce;
                if self.x > 1920.0 {
                    if self.side != 1 {
                        *b = 99;
                    } else {
                        self.x = 0.0;
                        *b += 1;
                    }
                } else if self.x < 0.0 {
                    if self.side != 0 {
                        *b = 99;
                    } else {
                        self.x = 1920.0;
                        *b += 1;
                    }
                }
                *b
            }
            _ => unreachable!(),
        };
        if bounce > 1 {
            self.kill(game);
            return;
        }
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
            self.kill(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `PushBullet.step()`
    fn step_push(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
            let ids = game.unit[opp].ids.clone();
            for tid in ids {
                let (tx, ty) = game.raw(tid).xy();
                let dx = self.x - tx;
                let dy = self.y - ty;
                if dx * dx + dy * dy <= 900.0 {
                    let nx = tx + self.xs * 2.0;
                    let ny = ty + self.ys * 2.0;
                    game.element_set_xy(tid, nx, ny);
                    game.element_xy_sync(tid);
                    let (nx2, ny2) = game.raw(tid).xy();
                    if ny2 >= 566.0 {
                        game.element_set_xy(tid, nx2, 566.0);
                    }
                    let (nx3, _) = game.raw(tid).xy();
                    if nx3 < 0.0 || nx3 > 1920.0 {
                        // Java 用 `target.id`（SyouBullet 遮蔽为 0）从 elements 删除
                        let rem_key = game.shape_id_for_ops(tid);
                        game.unit[opp].remove(tid);
                        game.kill_element(rem_key);
                        self.kill(game);
                        return;
                    }
                }
            }
            self.kill(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `SenBullet.step()`
    fn step_sen(&mut self, game: &mut GameTask) {
        let (mut hp, mut cnt, mut drop_flg) = match self.kind {
            BulletKind::Sen { hp, cnt, drop_flg } => (hp, cnt, drop_flg),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) {
            hp -= 1;
        }
        if hp <= 0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.ys += jf32!(0.32);
        if self.y > 566.0 {
            drop_flg = true;
            self.y = 566.0;
            self.ys *= -jf32!(0.125);
            self.xs = (2 - 4 * self.side as i32) as f32;
        }
        if drop_flg {
            cnt += 1;
        }
        if drop_flg && cnt == 40 {
            cnt = 0;
            let cos = if self.side == 1 {
                -jf32!(0.7071067811865475)
            } else {
                jf32!(0.7071067811865475)
            };
            let id = game.spawn_bullet(self.x, self.y, self.side, BulletKind::SenTama { hp: 3 });
            game.bullet_set_vec_mult(id, cos, -jf32!(0.7071067811865475), 20.0);
            game.bullet_move_now(id);
        }
        if let BulletKind::Sen {
            hp: h,
            cnt: c,
            drop_flg: d,
        } = &mut self.kind
        {
            *h = hp;
            *c = cnt;
            *d = drop_flg;
        }
    }

    /// Java `SenTamaBullet.step()`
    fn step_sen_tama(&mut self, game: &mut GameTask) {
        let mut hp = match self.kind {
            BulletKind::SenTama { hp } => hp,
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y > 570.0 {
            hp = 0;
        } else if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y)
            {
                hp = 0;
            } else {
                hp -= 1;
            }
        }
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.ys += jf32!(0.32);
        if hp <= 0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb(x, y, side, jf32!(0.5));
            self.kill(game);
        } else if let BulletKind::SenTama { hp: h } = &mut self.kind {
            *h = hp;
        }
    }

    /// Java `TobiBullet.step()`
    fn step_tobi(&mut self, game: &mut GameTask) {
        let (mut rot, mut speed, mut hp, mut cnt, xl1, xl2) = match self.kind {
            BulletKind::Tobi {
                rot,
                speed,
                hp,
                cnt,
                xl1,
                xl2,
            } => (rot, speed, hp, cnt, xl1, xl2),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -600.0 || self.x > xl2 as f32 || self.x < xl1 as f32 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) {
            hp -= 1;
        }
        if hp <= 0 || self.y > 570.0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        let mut t_rot = 180.0 * self.side as f32;
        if rot - 180.0 > t_rot {
            t_rot += 360.0;
        }
        if rot + 180.0 < t_rot {
            t_rot -= 360.0;
        }
        rot = rot + (t_rot - rot) / 32.0;
        let rad = fdlibm::to_radians(rot as f64) as f32;
        self.xs = cos32(rad) * speed;
        self.ys = sin32(rad) * speed;
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        if speed < 4.0 {
            speed += jf32!(0.5);
        }
        cnt += 1;
        if cnt == 20 {
            cnt = 0;
            let id = game.spawn_bullet(self.x, self.y + 10.0, self.side, BulletKind::TobiBomb);
            game.bullet_set_power(id, 5.0);
            game.bullet_set_gravity(id, jf32!(0.32));
        }
        if let BulletKind::Tobi {
            rot: r,
            speed: sp,
            hp: h,
            cnt: c,
            ..
        } = &mut self.kind
        {
            *r = rot;
            *sp = speed;
            *h = hp;
            *c = cnt;
        }
    }

    /// Java `TobiBombBullet.step()`
    fn step_tobi_bomb(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        if self.y > 570.0 || game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb(x, y, side, jf32!(0.5));
            self.kill(game);
            return;
        }
        self.move_bullet();
    }

    /// Java `TonBullet.step()`
    fn step_ton(&mut self, game: &mut GameTask) {
        let (mut hp, mut bounce) = match self.kind {
            BulletKind::Ton { hp, bounce } => (hp, bounce),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
            self.kill(game);
            return;
        }
        if self.y > 570.0 {
            self.y = 570.0;
            self.ys = -self.ys;
            bounce += 1;
            if bounce >= 5 {
                hp = 0;
            }
        } else if game.team_hit(opp, self.x, self.y) || self.gei_flg == 2 {
            hp -= 1;
            if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y)
            {
                hp = 0;
            }
        }
        if hp <= 0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_drop(x, y, ParentList::Atk(side));
            self.kill(game);
            return;
        }
        if let BulletKind::Ton { hp: h, bounce: b } = &mut self.kind {
            *h = hp;
            *b = bounce;
        }
        self.move_bullet();
    }

    /// Java `TuiBullet.step()`
    fn step_tui(&mut self, game: &mut GameTask) {
        let (mut hp, mut cnt, mut drop_flg) = match self.kind {
            BulletKind::Tui { hp, cnt, drop_flg } => (hp, cnt, drop_flg),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) {
            hp -= 1;
        }
        if hp <= 0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb_plain(x, y, side);
            self.kill(game);
            return;
        }
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.ys += jf32!(0.32);
        if self.y > 566.0 {
            drop_flg = true;
            self.y = 566.0;
            self.ys *= -jf32!(0.125);
            self.xs = (3 - 6 * self.side as i32) as f32;
        }
        if drop_flg {
            cnt += 1;
        }
        if cnt == 40 || cnt == 44 || cnt == 60 || cnt == 64 || cnt == 80 || cnt == 84 {
            if cnt == 84 {
                cnt = 0;
            }
            let cos_rot = if self.side == 1 {
                -jf32!(0.5)
            } else {
                jf32!(0.5)
            };
            let id = game.spawn_bullet(
                self.x,
                self.y - 20.0,
                self.side,
                BulletKind::TuiMissile {
                    speed: 15.0,
                    cos_rot,
                    sin_rot: -jf32!(0.8660254037844386),
                },
            );
            game.bullet_update_velocity_torad(id, 15.0);
        }
        if let BulletKind::Tui {
            hp: h,
            cnt: c,
            drop_flg: d,
        } = &mut self.kind
        {
            *h = hp;
            *c = cnt;
            *d = drop_flg;
        }
    }

    /// Java `BossMissileBullet.step()`
    fn step_boss_missile(&mut self, game: &mut GameTask) {
        let (mut cnt, mut target_rot, target_id, mut rot, target_rot_delta) = match self.kind {
            BulletKind::BossMissile {
                cnt,
                target_rot,
                target_id,
                rot,
                target_rot_delta,
            } => (cnt, target_rot, target_id, rot, target_rot_delta),
            _ => unreachable!(),
        };
        cnt += 1;
        let opp = 1 - self.side;
        if cnt < 60 {
            rot += target_rot_delta;
            rot = ((rot % 360.0) + 360.0) % 360.0;
            self.move_bullet();
            if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
                self.kill(game);
                return;
            }
            if game.team_hit(opp, self.x, self.y) || self.y > 570.0 || self.gei_flg == 2 {
                let (x, y, side) = (self.x, self.y, self.side);
                game.spawn_hits_bomb_plain(x, y, side);
                self.kill(game);
            }
        } else {
            self.gravity = 0.0;
            if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
                self.kill(game);
                return;
            }
            if game.team_hit(opp, self.x, self.y)
                || self.y > 570.0
                || self.gei_flg == 2
                || cnt > 260
            {
                let (x, y, side) = (self.x, self.y, self.side);
                game.spawn_hits_bomb_plain(x, y, side);
                self.kill(game);
                return;
            }
            if target_id != -1 {
                if let Some(t) = game.get(target_id) {
                    let (tx, ty) = t.xy();
                    target_rot = jmath::round_f32(
                        (fdlibm::atan2((ty - self.y) as f64, (tx - self.x) as f64) * 180.0
                            / std::f64::consts::PI) as f32,
                    ) as f32;
                }
            } else {
                target_rot = jmath::round_f32(
                    (fdlibm::atan2(
                        (game.core_y[opp] - self.y) as f64,
                        (game.core_x[opp] - self.x) as f64,
                    ) * 180.0
                        / std::f64::consts::PI) as f32,
                ) as f32;
            }
            if rot - 180.0 > target_rot {
                target_rot += 360.0;
            }
            if rot + 180.0 < target_rot {
                target_rot -= 360.0;
            }
            rot = rot * jf32!(0.875) + target_rot * jf32!(0.125);
            let rad = fdlibm::to_radians(target_rot as f64) as f32;
            self.xs = cos32(rad) * 20.0;
            self.ys = sin32(rad) * 20.0;
            self.x += self.xs;
            self.y += self.ys;
            self.x = jmath::xy_sync(self.x);
            self.y = jmath::xy_sync(self.y);
        }
        if let BulletKind::BossMissile {
            cnt: c,
            target_rot: tr,
            rot: r,
            ..
        } = &mut self.kind
        {
            *c = cnt;
            *tr = target_rot;
            *r = rot;
        }
    }

    /// Java `TuiMissileBullet.step()`
    fn step_tui_missile(&mut self, game: &mut GameTask) {
        let (mut speed, cos_rot, sin_rot) = match self.kind {
            BulletKind::TuiMissile {
                speed,
                cos_rot,
                sin_rot,
            } => (speed, cos_rot, sin_rot),
            _ => unreachable!(),
        };
        let opp = 1 - self.side;
        if self.y < -1200.0 || self.x > 2560.0 || self.x < -640.0 {
            self.kill(game);
            return;
        }
        if game.team_hit(opp, self.x, self.y) || self.y > 570.0 || self.gei_flg == 2 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb(x, y, side, jf32!(0.5));
            self.kill(game);
            return;
        }
        if speed < 20.0 {
            speed += 1.0;
        }
        self.xs = cos_rot * speed;
        self.ys = sin_rot * speed;
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        if let BulletKind::TuiMissile { speed: sp, .. } = &mut self.kind {
            *sp = speed;
        }
    }
}

fn normalize_rotation(rotation: f32) -> f32 {
    let mut normalized = rotation % 360.0;
    if normalized > 180.0 {
        normalized -= 360.0;
    } else if normalized < -180.0 {
        normalized += 360.0;
    }
    normalized
}

const K32: f32 = crate::fdlibm_consts::MATH_DUMP_DEG_TO_RAD_F32;

/// Java `UkiBullet`：浮在 unit[side] 列表里的漂浮子弹（可被敌方击中）
#[derive(Clone, Debug)]
pub struct UkiShot {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub hp: i32,
    pub xs: f32,
    pub ys: f32,
    pub yrot: i32,
    pub ysin: f32,
    pub ycos: f32,
}

impl UkiShot {
    pub fn new(id: i32, x: f32, y: f32, side: usize, xs: f32, ys: f32) -> Self {
        Self {
            id,
            side,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: BULLET_RADIUS,
            hp: 1,
            xs,
            ys,
            yrot: 0,
            ysin: 0.0,
            ycos: 0.0,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        self.yrot += 10;
        if self.yrot > 360 {
            self.yrot -= 360;
        }
        self.ysin = trig::sin(self.yrot);
        self.ycos = trig::cos(self.yrot);
        let opp = 1 - self.side;
        if self.y > 570.0 {
            self.hp = 0;
        } else if game.team_hit(opp, self.x, self.y) {
            if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y)
            {
                self.hp = 0;
            } else {
                self.hp -= 1;
            }
        }
        self.x += self.xs;
        self.y += self.ys;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.x += self.ycos;
        self.y += self.ysin;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
            self.kill(game);
            return;
        }
        if self.hp <= 0 {
            let (x, y, side) = (self.x, self.y, self.side);
            game.spawn_hits_bomb(x, y, side, jf32!(1.2));
            self.kill(game);
        }
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.unit[self.side].remove(self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        crate::geom::Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}

/// Java `SearchBullet`（Round 系追踪弹，位于 atk 列表）
#[derive(Clone, Debug)]
pub struct SearchShot {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub xs: f32,
    pub ys: f32,
    pub dir_x: f32,
    pub dir_y: f32,
}

impl SearchShot {
    pub fn new(id: i32, x: f32, y: f32, side: usize, cos_rot: f32, sin_rot: f32) -> Self {
        Self {
            id,
            side,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: BULLET_RADIUS,
            xs: cos_rot * 15.0,
            ys: sin_rot * 15.0,
            dir_x: cos_rot,
            dir_y: sin_rot,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        let ids = game.atk[opp].ids.clone();
        let mut i = 0;
        let mut rot_changed = true;
        while i < 5 {
            let mut t_dist2 = 4_000_000.0f32;
            let mut t_dist_est = 2000i32;
            for &bid in &ids {
                if !game.is_bullet_gei1(bid) {
                    continue;
                }
                let (bx, by) = game.raw(bid).xy();
                let dx = bx - self.x;
                if dx > t_dist_est as f32 || dx < -(t_dist_est as f32) {
                    continue;
                }
                let dy = by - self.y;
                if dy > t_dist_est as f32 || dy < -(t_dist_est as f32) {
                    continue;
                }
                let dist2 = dx * dx + dy * dy;
                if dist2 > t_dist2 {
                    continue;
                }
                let dot = self.dir_x * dx + self.dir_y * dy;
                if dot > 0.0 && dot * dot >= jf32!(0.5) * dist2 {
                    t_dist2 = dist2;
                    let t_rot = fdlibm::to_degrees(fdlibm::atan2(dy as f64, dx as f64)) as f32;
                    let nlz = (t_dist2 as i32).leading_zeros() as i32;
                    t_dist_est = 1 << (((32 - nlz + 1) >> 1) as u32);
                    t_dist_est += t_dist_est >> 3;
                    let rot = t_rot;
                    rot_changed = true;
                    self.dir_x = cos32(rot * K32);
                    self.dir_y = sin32(rot * K32);
                    if dist2 <= 900.0 {
                        game.set_bullet_gei_flg(bid, 2);
                    }
                }
            }
            if rot_changed {
                self.xs = self.dir_x * 15.0;
                self.ys = self.dir_y * 15.0;
                rot_changed = false;
            }
            self.x += self.xs;
            self.y += self.ys;
            self.x = jmath::xy_sync(self.x);
            self.y = jmath::xy_sync(self.y);
            if self.y > 570.0 || self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                self.kill(game);
                return;
            }
            if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y)
            {
                let (x, y, side) = (self.x, self.y, self.side);
                game.spawn_hits_drop_frames(x, y, ParentList::Atk(side), 3, 1.0);
                self.kill(game);
                return;
            }
            i += 1;
        }
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.atk[self.side].remove(self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        crate::geom::Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}
