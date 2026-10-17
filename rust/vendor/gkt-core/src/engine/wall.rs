//! 要塞壁（Java `Wall` + `elements/wall/*`）。
//!
//! 13 种：Wood/Stone/Paper/Iron/Jet/Turbo/Near/Far/Wide/Narrow/Snipe/Elevator/Box。
//! `unit_make` 直接生成的 12 种在 `GameTask::unit_make` 落地；Box 由 BoxBall 生成，随兵玉批次启用。

use crate::engine::{GameTask, ParentList};
use crate::gen_literals::jf32;
use crate::jmath;

#[derive(Clone, Debug)]
pub enum WallKind {
    Wood,
    Stone,
    Paper,
    Iron,
    Jet,
    Turbo,
    Near,
    Far { cnt: i32, speed: i32 },
    Wide { cnt: i32, speed: i32, mode: bool },
    Narrow { cnt: i32, speed: i32, mode: bool },
    Snipe,
    Elevator { ys: i32 },
    Box { on_side: i32, ys: f32 },
}

#[derive(Clone, Debug)]
pub struct Wall {
    pub id: i32,
    pub side: usize,
    pub type_: i32,
    pub x: f32,
    pub y: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub breaking: i32,
    pub kind: WallKind,
}

impl Wall {
    /// Java 各子类构造器
    pub fn new(id: i32, x: f32, y: f32, side: usize, type_: i32, kind: WallKind) -> Self {
        let (hp, max_hp) = match &kind {
            WallKind::Stone => (75, 75),
            WallKind::Paper => (1, 1),
            WallKind::Iron => (150, 150),
            WallKind::Jet => (15, 15),
            WallKind::Turbo => (100, 100),
            WallKind::Snipe => (50, 50),
            WallKind::Elevator { .. } => (75, 75),
            WallKind::Box { .. } => (15, 15),
            _ => (35, 35),
        };
        let mut w = Self {
            id,
            side,
            type_,
            x,
            y,
            hp,
            max_hp,
            breaking: 0,
            kind,
        };
        if matches!(w.kind, WallKind::Box { .. }) {
            w.x = jmath::xy_sync(w.x);
            w.y = jmath::xy_sync(w.y);
        }
        w
    }

    /// Java `Wall.step()` 的动态分派
    pub fn step(&mut self, game: &mut GameTask) {
        match self.kind {
            WallKind::Jet | WallKind::Turbo => self.step_jet_turbo(game),
            WallKind::Box { .. } => self.step_box(game),
            _ => self.step_common(game),
        }
    }

    /// Java `Wall.step()`（Wood/Stone/Paper/Iron/Near/Far/Wide/Narrow/Snipe/Elevator）
    fn step_common(&mut self, game: &mut GameTask) {
        if game.base_id[self.side] < 0 {
            self.kill_common(game);
            return;
        }
        let (dx, dy) = {
            let b = game.base(self.side);
            (b.base_move_x, b.base_move_y)
        };
        self.x += dx;
        self.y += dy;
        self.step_ex(game);
        if self.breaking > 0 || game.hp0_flg[self.side] > 0 {
            self.breaking += 1;
            if self.breaking > 4 || game.hp0_flg[self.side] > 0 {
                self.kill(game);
                return;
            }
        } else {
            let opp = 1 - self.side;
            if game.atk_hit_4corner(opp, self.x, self.y, 6.0) {
                self.hp -= 1;
                if self.hp <= 0 {
                    self.breaking += 1;
                }
            }
        }
        if self.hp < self.max_hp && game.repair[self.side].hit_test(game, self.x, self.y) {
            self.hp += 1;
        }
    }

    /// Java `Jet.step()` / `Turbo.step()`（无 breaking/hp0_flg 逻辑）
    fn step_jet_turbo(&mut self, game: &mut GameTask) {
        if game.base_id[self.side] < 0 {
            self.kill_common(game);
            return;
        }
        let (dx, dy) = {
            let b = game.base(self.side);
            (b.base_move_x, b.base_move_y)
        };
        self.x += dx;
        self.y += dy;
        self.step_ex(game);
        let opp = 1 - self.side;
        if game.atk_hit_4corner(opp, self.x, self.y, 6.0) {
            self.hp -= 1;
            if self.hp <= 0 {
                self.kill(game);
                return;
            }
        }
        if self.hp < self.max_hp && game.repair[self.side].hit_test(game, self.x, self.y) {
            self.hp += 1;
        }
    }

    /// Java `Box.step()`
    fn step_box(&mut self, game: &mut GameTask) {
        let WallKind::Box { on_side, ys } = &mut self.kind else {
            unreachable!()
        };
        if *on_side != -1 {
            if game.base_id[self.side] < 0 {
                self.kill(game);
                return;
            }
            let (dx, dy) = {
                let b = game.base(self.side);
                (b.base_move_x, b.base_move_y)
            };
            self.x += dx;
            self.y += dy;
            *ys += 1.0;
            let mut drop_y = self.y + *ys;
            if drop_y >= 566.0 {
                drop_y = 566.0;
                *on_side = -1;
            } else {
                self.y += 100.0;
                while wall_shape_hit(self.x, self.y, self.x, drop_y + 15.0, 0.0)
                    || game.fort_hit_skip(*on_side as usize, self.x, drop_y + 15.0, self.id)
                {
                    drop_y -= 1.0;
                    *ys = 0.0;
                }
                self.y -= 100.0;
            }
            self.y = drop_y;
        } else if game.wall_hit(1 - self.side, self.x, self.y) {
            self.hp = 0;
        }
        self.step_ex(game);
        let opp = 1 - self.side;
        if game.atk_hit_4corner(opp, self.x, self.y, 6.0) {
            self.hp -= 1;
        }
        if self.hp <= 0 || game.hp0_flg[self.side] > 0 || self.y < -600.0 {
            let id = self.id;
            let side = self.side;
            game.spawn_hits_drop_frames(self.x, self.y, ParentList::Wall(side), 1, jf32!(1.3));
            game.wall_remove(side, id);
            game.kill_element(id);
            return;
        }
        if self.hp < self.max_hp && game.repair[self.side].hit_test(game, self.x, self.y) {
            self.hp += 1;
        }
    }

    /// Java 各子类的 `stepEx()`
    fn step_ex(&mut self, game: &mut GameTask) {
        if matches!(self.kind, WallKind::Elevator { .. }) {
            let side = self.side;
            let self_id = self.id;
            let _x = self.x;
            let _y = self.y;
            self.x += 100.0;
            self.y += 100.0;
            let hit_up = self.hit_test_point(_x, _y + 15.0)
                || game.fort_hit_skip(side, _x, _y + 15.0, self_id);
            let hit_down = self.hit_test_point(_x, _y - 48.0)
                || game.wall_hit_skip(side, _x, _y - 48.0, self_id);
            let below_base = _y <= game.base(side).y - 380.0;
            let WallKind::Elevator { ys } = &mut self.kind else {
                unreachable!()
            };
            let mut block_flg = false;
            if *ys == 1 && hit_up {
                block_flg = true;
                *ys = -1;
            } else if *ys == -1 && (hit_down || below_base) {
                block_flg = true;
                *ys = 1;
            }
            let y_step = *ys;
            if !block_flg {
                self.x += 0.0;
                self.y += y_step as f32;
            }
            self.x -= 100.0;
            self.y -= 100.0;
            return;
        }
        match &mut self.kind {
            WallKind::Near => {
                if game.norikomi_flg {
                    game.spawn_hits_jump(self.x, self.y - 35.0, ParentList::JumpU(self.side));
                }
            }
            WallKind::Far { cnt, speed } => {
                *cnt += 1;
                if *cnt == *speed {
                    *cnt = 0;
                    game.spawn_hits_jump(self.x, self.y - 35.0, ParentList::JumpF(self.side));
                }
            }
            WallKind::Wide { cnt, speed, mode } | WallKind::Narrow { cnt, speed, mode } => {
                *cnt += 1;
                if *cnt == *speed {
                    *cnt = 0;
                    let id = self.id;
                    let side = self.side;
                    if *mode {
                        game.turn_cw[side].remove(id);
                        game.turn_ccw[side].add(id);
                    } else {
                        game.turn_ccw[side].remove(id);
                        game.turn_cw[side].add(id);
                    }
                    *mode = !*mode;
                }
            }
            WallKind::Elevator { .. } => {
                unreachable!("Elevator 在 step_ex 开头处理")
            }
            _ => {}
        }
    }

    /// Java `Wall.kill()`（动态分派）
    pub fn kill(&mut self, game: &mut GameTask) {
        let axl_delta = match self.kind {
            WallKind::Jet => 1,
            WallKind::Turbo => 2,
            _ => 0,
        };
        let is_snipe = matches!(self.kind, WallKind::Snipe);
        let wide_mode = match &self.kind {
            WallKind::Wide { mode, .. } | WallKind::Narrow { mode, .. } => Some(*mode),
            _ => None,
        };
        self.kill_common(game);
        if axl_delta != 0 {
            game.base_axl_add(self.side, -axl_delta);
        }
        if is_snipe {
            game.snipe[self.side].remove(self.id);
        }
        if let Some(mode) = wide_mode {
            if mode {
                game.turn_cw[self.side].remove(self.id);
            } else {
                game.turn_ccw[self.side].remove(self.id);
            }
        }
    }

    /// `super.kill()`：只从 wall 列表与元素表移除（Jet/Turbo 的 base==null 路径）
    fn kill_common(&mut self, game: &mut GameTask) {
        game.wall_remove(self.side, self.id);
        game.kill_element(self.id);
    }

    /// Java `Wall.hitTestPoint`
    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        wall_shape_hit(self.x, self.y, x, y, 0.0)
    }
}

/// Wall 与 HitsJump 共用的九宫格命中（HitsJump 的 dx 有 +0.3 偏移）
pub(crate) fn wall_shape_hit(cx: f32, cy: f32, x: f32, y: f32, dx_offset: f32) -> bool {
    let mut dx = x - cx + dx_offset;
    let mut dy = y - cy;
    if dx < -jf32!(16.85) || dx > jf32!(17.5) || dy < -jf32!(17.5) || dy > jf32!(17.5) {
        return false;
    }
    if dx > -jf32!(12.85) {
        if dx > jf32!(13.5) {
            dx -= jf32!(13.5);
        } else {
            return true;
        }
    } else {
        dx += jf32!(12.85);
    }
    if dy > -jf32!(13.5) {
        if dy > jf32!(13.5) {
            dy -= jf32!(13.5);
        } else {
            return true;
        }
    } else {
        dy += jf32!(13.5);
    }
    dx * dx + dy * dy <= 16.0
}

/// 供 `GameTask::unit_make` 统一取用：类型 → 壁种
pub fn kind_for_type(type_: i32, x: f32, side: usize) -> Option<WallKind> {
    let kind = match type_ {
        25 => WallKind::Wood,
        26 => WallKind::Stone,
        27 => WallKind::Paper,
        28 => WallKind::Iron,
        29 => WallKind::Jet,
        30 => WallKind::Turbo,
        55 => WallKind::Near,
        56 => {
            let speed = 900;
            let int_x = x as i32;
            let cnt = if side == 0 {
                (int_x - 50) % speed
            } else {
                (380 - (int_x - 1490)) % speed
            };
            WallKind::Far { cnt, speed }
        }
        57 => WallKind::Wide {
            cnt: 20,
            speed: 40,
            mode: false,
        },
        58 => WallKind::Narrow {
            cnt: 8,
            speed: 16,
            mode: true,
        },
        59 => WallKind::Snipe,
        60 => WallKind::Elevator { ys: 1 },
        _ => return None,
    };
    Some(kind)
}

/// Box 专用构造（由 BoxBall 调用，兵玉批次启用）
#[allow(dead_code)]
pub fn box_kind(on_side: i32) -> WallKind {
    WallKind::Box { on_side, ys: 0.0 }
}
