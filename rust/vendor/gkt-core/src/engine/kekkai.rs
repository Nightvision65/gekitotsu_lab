//! 界玉结界（Java `KekkaiField`，Gemini 优化版结构逐行对应）。
//!
//! 无界玉时 `activePoints == 0`，`hitTestPoint` 恒 false；结构保留以便下一步接入 KekkaiBall。

use crate::engine::GameTask;

#[derive(Clone, Debug)]
pub struct KekkaiField {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub side: usize,
    points_x: Vec<f32>,
    points_y: Vec<f32>,
    active_points: usize,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

const HALF_WIDTH: f32 = 15.0;
const LIMIT_SQ: f32 = HALF_WIDTH * HALF_WIDTH;

impl KekkaiField {
    pub fn new(id: i32, side: usize) -> Self {
        Self {
            id,
            x: 0.0,
            y: 0.0,
            side,
            points_x: Vec::new(),
            points_y: Vec::new(),
            active_points: 0,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 0.0,
            max_y: 0.0,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        self.rebuild_segments(game);
    }

    fn rebuild_segments(&mut self, game: &mut GameTask) {
        let size = game.kekkai_ids[self.side].len();
        if self.points_x.len() < size {
            self.points_x = vec![0.0; size * 2];
            self.points_y = vec![0.0; size * 2];
        }
        self.active_points = 0;
        let mut t_min_x = f32::MAX;
        let mut t_max_x = -f32::MAX;
        let mut t_min_y = f32::MAX;
        let mut t_max_y = -f32::MAX;

        for i in 0..size {
            let id = game.kekkai_ids[self.side][i];
            if game.get(id).is_none() {
                continue;
            }
            if game.is_live_kekkai_ball(id) {
                let (px, py) = game.kekkai_point(id);
                self.points_x[self.active_points] = px;
                self.points_y[self.active_points] = py;
                self.active_points += 1;
                if px < t_min_x {
                    t_min_x = px;
                }
                if px > t_max_x {
                    t_max_x = px;
                }
                if py < t_min_y {
                    t_min_y = py;
                }
                if py > t_max_y {
                    t_max_y = py;
                }
            }
        }

        self.min_x = t_min_x - HALF_WIDTH;
        self.max_x = t_max_x + HALF_WIDTH;
        self.min_y = t_min_y - HALF_WIDTH;
        self.max_y = t_max_y + HALF_WIDTH;
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        if self.active_points < 2 {
            return false;
        }
        if x < self.min_x || x > self.max_x || y < self.min_y || y > self.max_y {
            return false;
        }
        for i in 0..self.active_points {
            let x1 = self.points_x[i];
            let y1 = self.points_y[i];
            let next_idx = if i + 1 == self.active_points {
                0
            } else {
                i + 1
            };
            let x2 = self.points_x[next_idx];
            let y2 = self.points_y[next_idx];
            let vx = x2 - x1;
            let vy = y2 - y1;
            let wx = x - x1;
            let wy = y - y1;
            let c1 = vx * wx + vy * wy;
            let dist_sq;
            if c1 <= 0.0 {
                let dx = x - x1;
                let dy = y - y1;
                dist_sq = dx * dx + dy * dy;
            } else {
                let c2 = vx * vx + vy * vy;
                if c2 <= c1 {
                    let dx = x - x2;
                    let dy = y - y2;
                    dist_sq = dx * dx + dy * dy;
                } else {
                    let b = c1 / c2;
                    let bx = x1 + b * vx;
                    let by = y1 + b * vy;
                    let dx = x - bx;
                    let dy = y - by;
                    dist_sq = dx * dx + dy * dy;
                }
            }
            if dist_sq <= LIMIT_SQ {
                return true;
            }
        }
        false
    }
}
