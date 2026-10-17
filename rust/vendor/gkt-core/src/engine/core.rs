//! 核心（Java `Core`/`BossCore`/`BossCore2` 的公共行为）。
//!
//! `BossCore2` 依赖 Bullet 机器（BossMissileBullet/HitsBomb），下一步落地；
//! 此处保留分支并在被使用时显式报错，避免静默偏离。

use crate::engine::GameTask;
use crate::gen_literals::jf32;

#[derive(Clone, Debug)]
pub enum CoreKind {
    Normal,
    Boss {
        cnt: i32,
    },
    Boss2 {
        cnt: i32,
        cnt2: i32,
        next_start_target_index: i32,
    },
}

#[derive(Clone, Debug)]
pub struct Core {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub unit_x: f32,
    pub unit_y: f32,
    pub kind: CoreKind,
}

impl Core {
    pub fn new(id: i32, x: f32, y: f32, side: usize, kind: CoreKind) -> Self {
        Self {
            id,
            side,
            x,
            y,
            unit_x: x,
            unit_y: y,
            kind,
        }
    }

    /// Java `Core.step()` + 子类增量
    pub fn step(&mut self, game: &mut GameTask) {
        let (bx, by) = {
            let b = game.base(self.side);
            (b.x, b.y)
        };
        let dx = bx + self.unit_x - self.x;
        let dy = by + self.unit_y - self.y;
        self.x += dx;
        self.y += dy;

        let opp = 1 - self.side;
        let hit = game.atk_hit_core13(opp, self.x, self.y);
        if hit {
            game.hp[self.side] -= 1;
        }
        if game.hp[self.side] < 100 && game.repair[self.side].hit_test(game, self.x, self.y) {
            game.hp[self.side] += 1;
        }

        match &mut self.kind {
            CoreKind::Normal => {}
            CoreKind::Boss { cnt } => {
                *cnt += 1;
                if *cnt < 2761 && *cnt % 900 == 60 {
                    game.spawn_boss_laser(self.side, self.x, self.y);
                }
            }
            CoreKind::Boss2 {
                cnt,
                cnt2,
                next_start_target_index,
            } => {
                *cnt += 1;
                if *cnt < 961 && *cnt % 300 == 60 {
                    game.spawn_boss_laser2(self.side, self.x, self.y);
                }
                *cnt2 += 1;
                if *cnt2 > 300 && *cnt2 % 3 == 0 && game.hp0_flg[1] == 0 {
                    let target = game.pick_target_id_core2(1 - self.side, next_start_target_index);
                    let r1 = crate::rng::next(&mut game.rng_seed);
                    let r2 = crate::rng::next(&mut game.rng_seed);
                    let r3 = crate::rng::next(&mut game.rng_seed);
                    let r4 = crate::rng::next(&mut game.rng_seed);
                    let r5 = crate::rng::next(&mut game.rng_seed);
                    let (x, y) = (self.x, self.y);
                    game.spawn_boss_missile(self.side, x, y, target, r1, r2, r3, r4, r5);
                    if *cnt2 >= 360 {
                        *cnt2 = 0;
                    }
                }
            }
        }
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        let mut dx = x - self.x;
        let mut dy = y - self.y;
        if dx < -53.5 || dx > 53.5 || dy < -53.5 || dy > 53.5 {
            return false;
        }
        dx = dx.abs();
        dy = dy.abs();
        let limit = jf32!(9.221316);
        if dy - dx <= limit && dy - dx >= -limit && dx < 40.0 {
            return true;
        }
        if dx * dx + dy * dy <= 1122.25 {
            return true;
        }
        dx -= 40.0;
        dy -= 40.0;
        dx * dx + dy * dy <= 182.25
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.core_id[self.side] = -1;
        game.kill_element(self.id);
        game.wall_remove(self.side, self.id);
    }
}
