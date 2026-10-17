//! Wave C2 特殊元素：`GeiBullet`（迎玉链接）、`HolyBullet`（复活）、`SyouBullet`（障壁）。
//! 三者都只挂 elements（不挂传感器或按需挂），命中测试与 Java 一致。

use crate::engine::{Element, GameTask, ParentList};
use crate::gen_literals::jf32;
use crate::jmath;

/// Java `GeiBullet`：连接迎玉与其锁定的子弹，下一帧把目标 `gei_flg=2`
#[derive(Clone, Debug)]
pub struct GeiLink {
    pub id: i32,
    pub user_id: i32,
    pub target_id: i32,
}

impl GeiLink {
    pub fn new(id: i32, user_id: i32, target_id: i32) -> Self {
        Self {
            id,
            user_id,
            target_id,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        if game.get(self.user_id).is_none() {
            game.kill_element(self.id);
            return;
        }
        if game.get(self.target_id).is_none() {
            game.ball_reset_gei(self.user_id);
            game.kill_element(self.id);
            return;
        }
        game.set_bullet_gei_flg(self.target_id, 2);
        if game.ball_t_id(self.user_id) == -1 {
            game.kill_element(self.id);
        }
    }

    pub fn hit_test_point(&self, _x: f32, _y: f32) -> bool {
        false
    }
}

/// Java `HolyBullet`：复活球，20 帧后在当前位置重建死亡单位
#[derive(Clone, Debug)]
pub struct HolyGhost {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub cnt: i32,
    pub t_type: i32,
    pub t_rot: i32,
    pub t_side: usize,
    pub t_on_side: usize,
    pub t_jump_flg: i32,
}

impl HolyGhost {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i32,
        x: f32,
        y: f32,
        t_type: i32,
        t_rot: i32,
        t_side: usize,
        t_on_side: usize,
        t_jump_flg: i32,
    ) -> Self {
        Self {
            id,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            cnt: 0,
            t_type,
            t_rot,
            t_side,
            t_on_side,
            t_jump_flg,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        if self.t_jump_flg == 0 && game.base_id[self.t_on_side] >= 0 {
            let (bx, by) = {
                let b = game.base(self.t_on_side);
                (b.base_move_x, b.base_move_y)
            };
            self.x += bx;
            self.y += by;
        }
        self.cnt += 1;
        if self.cnt == 20 {
            let nid = game.unit_make(self.t_side, self.x, self.y, self.t_rot, self.t_type);
            if let Some(Element::Ball(b)) = game.elements[nid as usize].as_mut() {
                b.on_side = self.t_on_side;
            }
        } else if self.cnt >= 30 {
            game.kill_element(self.id);
        }
    }

    pub fn hit_test_point(&self, _x: f32, _y: f32) -> bool {
        false
    }
}

/// Java `SyouBullet`：障壁（Rectangle，先平移抛射，停下后激活并被己方 unit 列表登记）
#[derive(Clone, Debug)]
pub struct SyouShield {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub hp: i32,
    pub cnt: i32,
    pub power: f32,
    pub cos_rot: f32,
    pub sin_rot: f32,
    pub active: bool,
}

impl SyouShield {
    pub fn new(id: i32, x: f32, y: f32, side: usize, cos_rot: f32, sin_rot: f32) -> Self {
        Self {
            id,
            side,
            x,
            y,
            hp: 10,
            cnt: -7,
            power: 20.0,
            cos_rot,
            sin_rot,
            active: false,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        if game.team_hit(opp, self.x, self.y) {
            self.hp -= 1;
            if game.fort_hit(opp, self.x, self.y) || game.shield[opp].hit_test(game, self.x, self.y)
            {
                self.hp = 0;
            }
        }
        if self.y > 570.0
            || self.y < -600.0
            || self.x > 1920.0
            || self.x < 0.0
            || self.hp <= 0
            || self.cnt >= 159
        {
            if self.active {
                game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
            }
            self.kill(game);
            return;
        }
        if self.power > 0.0 {
            self.power -= 1.0;
            self.x += self.cos_rot * self.power;
            self.y += self.sin_rot * self.power;
            self.x = jmath::xy_sync(self.x);
            self.y = jmath::xy_sync(self.y);
        } else {
            self.cnt += 1;
            if self.cnt == 0 && !self.active {
                self.active = true;
                game.unit[self.side].add(self.id);
            }
        }
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        if self.active {
            game.unit[self.side].remove(self.id);
        }
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        crate::geom::Rectangle::new(self.x, self.y, -14.0, -14.0, 28.0, 28.0).hit_test_point(x, y)
    }
}

/// Java `StarPrepareLaser`：星玉的预瞄激光（射线扫描后生成 StarLaser）
#[derive(Clone, Debug)]
pub struct StarPrepareLaser {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub rot_deg: f32,
    pub x_speed: f32,
    pub y_speed: f32,
    pub start_x: f32,
    pub start_y: f32,
    pub old_rot_deg: f32,
    pub side: usize,
    pub steps: i32,
    pub cnt: i32,
    pub user: i32,
}

impl StarPrepareLaser {
    pub fn new(id: i32, x: f32, y: f32, r_deg: i32, side: usize, user: i32) -> Self {
        Self {
            id,
            x,
            y,
            rot_deg: r_deg as f32,
            x_speed: 0.0,
            y_speed: 10.0,
            start_x: x,
            start_y: y,
            old_rot_deg: 90.0,
            side,
            steps: -1,
            cnt: 0,
            user,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        use crate::engine::{BallKind, Element};
        self.cnt += 1;
        let info = match game.get(self.user) {
            Some(Element::Ball(b)) if matches!(b.kind, BallKind::Star { .. }) => Some((
                b.x,
                b.y,
                b.rot,
                matches!(b.kind, BallKind::Star { shooting: true }),
            )),
            _ => None,
        };
        let Some((bx, by, rot, shooting)) = info else {
            game.kill_element(self.id);
            return;
        };
        self.rot_deg = rot as f32;
        self.start_x = bx + 28.0 * crate::trig::cos(rot);
        self.start_y = by + 28.0 * crate::trig::sin(rot);
        if self.rot_deg != self.old_rot_deg {
            self.old_rot_deg = self.rot_deg;
            self.x_speed = crate::trig::cos(self.rot_deg as i32) * 10.0;
            self.y_speed = crate::trig::sin(self.rot_deg as i32) * 10.0;
        }
        self.x = self.start_x;
        self.y = self.start_y;
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
        self.steps = 0;
        while self.steps < 1000 {
            self.x += self.x_speed;
            self.y += self.y_speed;
            self.x = jmath::xy_sync(self.x);
            self.y = jmath::xy_sync(self.y);
            if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                self.steps = -1;
                break;
            } else if self.y > 570.0
                || game.wall_hit(1 - self.side, self.x, self.y)
                || game.shield[1 - self.side].hit_test(game, self.x, self.y)
            {
                break;
            }
            self.steps += 1;
        }
        if self.cnt == 30 && self.steps >= 0 {
            game.spawn_star_laser(self.x, self.side);
        }
        if !shooting {
            game.kill_element(self.id);
        }
    }

    pub fn hit_test_point(&self, _x: f32, _y: f32) -> bool {
        false
    }
}

/// Java 变玉召唤物：Drill/Solid/Shock/Weak（Round 系，挂在 unit[side]）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatureKind {
    Drill,
    Solid,
    Shock,
    Weak,
}

#[derive(Clone, Debug)]
pub struct Creature {
    pub id: i32,
    pub side: usize,
    pub kind: CreatureKind,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub hp: i32,
    pub xs: f32,
    pub ys: f32,
    pub rot: f32,
    pub cnt: i32,
    pub yrot: i32,
    pub ysin: f32,
    pub ycos: f32,
    pub xuv: f32,
    pub yuv: f32,
    pub speed: f32,
    pub xs2: f32,
}

impl Creature {
    pub fn new(id: i32, x: f32, y: f32, side: usize, r_deg: i32, kind: CreatureKind) -> Self {
        let (hp, xs, ys) = match kind {
            CreatureKind::Drill => (7, 0.0, 0.0),
            CreatureKind::Solid => (
                20,
                crate::trig::cos(r_deg) * 2.0,
                crate::trig::sin(r_deg) * 2.0,
            ),
            CreatureKind::Shock => (
                10,
                crate::trig::cos(r_deg) * 2.0,
                crate::trig::sin(r_deg) * 2.0,
            ),
            CreatureKind::Weak => (
                3,
                crate::trig::cos(r_deg) * 3.0,
                crate::trig::sin(r_deg) * 3.0,
            ),
        };
        Self {
            id,
            side,
            kind,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: 15.5,
            hp,
            xs,
            ys,
            rot: r_deg as f32,
            cnt: 0,
            yrot: 0,
            ysin: 0.0,
            ycos: 0.0,
            xuv: crate::trig::cos(r_deg),
            yuv: crate::trig::sin(r_deg),
            speed: 0.0,
            xs2: 0.0,
        }
    }

    // Java 的 `x = x + a + b` 结合顺序保留（不能改为 += 复合赋值）
    #[allow(clippy::assign_op_pattern)]
    pub fn step(&mut self, game: &mut GameTask) {
        let opp = 1 - self.side;
        match self.kind {
            CreatureKind::Drill => {
                if game.team_hit(opp, self.x, self.y) {
                    self.speed = -5.0;
                    self.hp -= 1;
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                }
                if self.speed < 25.0 {
                    self.speed += 1.0;
                }
                self.x += self.xuv * self.speed;
                self.y += self.yuv * self.speed;
                self.x = jmath::xy_sync(self.x);
                self.y = jmath::xy_sync(self.y);
                if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                    self.kill(game);
                    return;
                }
                if self.y > 570.0 || self.hp <= 0 {
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                    self.kill(game);
                }
            }
            CreatureKind::Solid => {
                if game.team_hit(opp, self.x, self.y) {
                    self.hp -= 1;
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                    self.x = self.x - crate::trig::cos(self.rot as i32);
                    self.y = self.y - crate::trig::sin(self.rot as i32);
                    self.x = jmath::xy_sync(self.x);
                    self.y = jmath::xy_sync(self.y);
                }
                self.x += self.xs;
                self.y += self.ys;
                self.x = jmath::xy_sync(self.x);
                self.y = jmath::xy_sync(self.y);
                if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                    self.kill(game);
                    return;
                }
                if self.y > 570.0 {
                    self.y = 570.0;
                    self.ys = -self.ys;
                }
                if self.hp <= 0 {
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                    self.kill(game);
                }
            }
            CreatureKind::Shock => {
                if game.team_hit(opp, self.x, self.y) {
                    self.hp -= 1;
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                    self.x = self.x - 4.0 * crate::trig::cos(self.rot as i32);
                    self.y = self.y - 4.0 * crate::trig::sin(self.rot as i32);
                    self.x = jmath::xy_sync(self.x);
                    self.y = jmath::xy_sync(self.y);
                }
                self.yrot += 10;
                if self.yrot > 360 {
                    self.yrot -= 360;
                }
                self.ysin = crate::trig::sin(self.yrot) * 6.0;
                self.ycos = crate::trig::cos(self.yrot) * 6.0;
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
                if self.y > 570.0 {
                    self.y = 570.0;
                    self.ys = -self.ys;
                }
                self.cnt += 1;
                if self.cnt == 60 {
                    self.cnt = 0;
                    let target_rot =
                        jmath::round_f32(crate::fdlibm::to_degrees(crate::fdlibm::atan2(
                            (game.core_y[opp] - self.y) as f64,
                            (game.core_x[opp] - self.x) as f64,
                        )) as f32);
                    let target_rot = (target_rot % 360 + 360) % 360;
                    let id = game.spawn_bullet(
                        self.x,
                        self.y,
                        self.side,
                        crate::engine::BulletKind::Gun,
                    );
                    let (c, sn) = (crate::trig::cos(target_rot), crate::trig::sin(target_rot));
                    game.bullet_set_vec_mult(id, c, sn, 10.0);
                    game.bullet_move_now(id);
                }
                if self.hp <= 0 {
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                    self.kill(game);
                }
            }
            CreatureKind::Weak => {
                if game.team_hit(opp, self.x, self.y) {
                    self.xs2 = -4.0 + 8.0 * self.side as f32;
                    self.hp -= 1;
                    game.spawn_hits_drop_frames(
                        self.x,
                        self.y,
                        ParentList::Atk(self.side),
                        1,
                        jf32!(1.2),
                    );
                }
                let target_rot = jmath::round_f32(crate::fdlibm::to_degrees(crate::fdlibm::atan2(
                    (game.core_y[opp] - self.y) as f64,
                    (game.core_x[opp] - self.x) as f64,
                )) as f32);
                let target_rot = (target_rot % 360 + 360) % 360;
                self.rot = self.rot * jf32!(0.875) + target_rot as f32 * jf32!(0.125);
                self.xs = crate::utils::cos_f(self.rot) * 3.0;
                self.ys = crate::utils::sin_f(self.rot) * 3.0;
                self.x = self.x + self.xs + self.xs2;
                self.y = self.y + self.ys;
                self.x = jmath::xy_sync(self.x);
                self.y = jmath::xy_sync(self.y);
                self.xs2 *= jf32!(0.9);
                if self.y < -600.0 || self.x > 1920.0 || self.x < 0.0 {
                    self.kill(game);
                    return;
                }
                if self.y > 570.0 {
                    self.y = 570.0;
                    self.ys = -self.ys;
                }
                if self.hp <= 0 {
                    game.spawn_hits_drop(self.x, self.y, ParentList::Atk(self.side));
                    self.kill(game);
                }
            }
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
