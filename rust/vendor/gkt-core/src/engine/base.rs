//! 车板（Java `org.example.elements.Base`）。
//!
//! Base 在 Java 里是 `CompositeShape` 但没有子形状，故本移植不含子形状列表；
//! `step`/`hitTestPoint`/边界弹飞逻辑逐行对应。

use crate::engine::GameTask;
use crate::gen_literals::jf32;
use crate::jmath;

#[derive(Clone, Debug)]
pub struct Base {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub axl: i32,
    pub wrk: f32,
    pub xs: f32,
    pub ys: f32,
    pub xx: f32,
    pub yy: f32,
    pub old_x: f32,
    pub old_y: f32,
    pub base_move_x: f32,
    pub base_move_y: f32,
}

impl Base {
    pub fn new(id: i32, x: f32, y: f32, side: usize) -> Self {
        Self {
            id,
            side,
            x,
            y,
            axl: 1,
            wrk: 0.0,
            xs: 0.0,
            ys: 0.0,
            xx: x,
            yy: y,
            old_x: x,
            old_y: y,
            base_move_x: 0.0,
            base_move_y: 0.0,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        self.xs += self.axl as f32 / 1000.0;
        if game.hp0_flg[self.side] >= 3 {
            self.xs = 0.0;
        }
        self.ys += jf32!(0.05);
        self.xx += self.xs - 2.0 * self.xs * self.side as f32;
        self.yy += self.ys;
        if self.yy > 532.0 {
            self.yy = 532.0;
            self.ys *= -0.5;
        }
        self.x = jmath::round_f32(self.xx) as f32;
        self.y = jmath::round_f32(self.yy) as f32;
        self.base_move_x = self.x - self.old_x;
        self.base_move_y = self.y - self.old_y;
        self.old_x = self.x;
        self.old_y = self.y;

        let mut tobasare_flg = false;
        if self.x < 190.0 {
            self.wrk = (jmath::round_f32(-self.xs * 5.0) + 1) as f32;
            self.xx = 191.0;
            self.xs = -self.xs / 2.0;
            tobasare_flg = true;
        } else if self.x > 1730.0 {
            self.wrk = (jmath::round_f32(-self.xs * 5.0) + 1) as f32;
            self.xx = 1729.0;
            self.xs = -self.xs / 2.0;
            tobasare_flg = true;
        }
        if tobasare_flg && game.hp0_flg[self.side] == 0 {
            let nie_flg = game.nie_ball_alarm(self.side);
            if !nie_flg {
                game.dokkan_flg[self.side] = true;
                self.wrk = jmath::round_f32(self.wrk) as f32;
                if self.wrk < 0.0 {
                    self.wrk = 0.0;
                }
                game.hp[self.side] -= self.wrk as i32;
            }
        }
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        let mut dx = x - self.x;
        let mut dy = y - self.y;
        if dx < -191.5 || dx > 191.5 || dy < -15.5 || dy > 51.5 {
            return false;
        }
        if dy > -4.0 {
            if dy > 17.0 {
                if dy > 27.5 {
                    dx = dx.abs() - jf32!(108.7);
                    dy -= 20.0;
                    return dx * dx + dy * dy <= 992.25;
                }
                dy -= 17.0;
                dy *= jf32!(1.095238095238);
            } else {
                return true;
            }
        } else {
            dy += 4.0;
        }
        if dx > -180.0 {
            if dx > 180.0 {
                dx -= 180.0;
            } else {
                return true;
            }
        } else {
            dx += 180.0;
        }
        dx * dx + dy * dy <= 132.25
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.base_id[self.side] = -1;
        game.kill_element(self.id);
    }
}
