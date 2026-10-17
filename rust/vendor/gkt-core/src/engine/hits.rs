//! 命中体：`HitsJump`（白线跳跃判定）与 `HitsDropFrames`（黑线/箱体判定），
//! 以及 Wave B 近战命中体（Ken/Nagi/Bone/Kabe/Knight/Yari/Heal）。
// 与 Java 构造器/帧逻辑形状保持一致：参数多、嵌套 match、下标循环、`% + %` 归一化均原样保留。
#![allow(
    clippy::too_many_arguments,
    clippy::collapsible_match,
    clippy::needless_range_loop,
    clippy::manual_rem_euclid
)]

use crate::engine::{GameTask, ParentList, wall::wall_shape_hit};
use crate::gen_literals::jf32;
use crate::geom::Round;
use crate::jmath;

/// Java `HitsJump extends CompositeShape`：形状同 Wall，但 dx 有 +0.3F 偏移
#[derive(Clone, Debug)]
pub struct HitsJump {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub parent: ParentList,
}

impl HitsJump {
    pub fn new(id: i32, x: f32, y: f32, parent: ParentList) -> Self {
        Self { id, x, y, parent }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        game.list_remove(self.parent, self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        wall_shape_hit(self.x, self.y, x, y, jf32!(0.3))
    }
}

/// Java `HitsDrop extends Round`：半径 15.5F，xySync 后注册到 `unit[side]`，下一帧移除自身
#[derive(Clone, Debug)]
pub struct HitsDrop {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub parent: ParentList,
}

impl HitsDrop {
    pub fn new(id: i32, x: f32, y: f32, parent: ParentList) -> Self {
        Self {
            id,
            x,
            y,
            r: jf32!(15.5),
            parent,
        }
    }

    pub fn quantize(&mut self) {
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
    }

    pub fn step(&mut self, game: &mut GameTask) {
        game.list_remove(self.parent, self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}

/// Java `HitsDropFrames extends Round`：半径 15.5*scale，存活 frames 帧
#[derive(Clone, Debug)]
pub struct HitsDropFrames {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub frames: i32,
    pub parent: ParentList,
}

impl HitsDropFrames {
    pub fn new(id: i32, x: f32, y: f32, parent: ParentList, frames: i32, scale: f32) -> Self {
        Self {
            id,
            x,
            y,
            r: jf32!(15.5) * scale,
            frames,
            parent,
        }
    }

    /// 先 xySync 再注册（与 Java 构造器顺序一致）
    pub fn quantize(&mut self) {
        self.x = jmath::xy_sync(self.x);
        self.y = jmath::xy_sync(self.y);
    }

    pub fn step(&mut self, game: &mut GameTask) {
        self.frames -= 1;
        if self.frames <= 0 {
            game.list_remove(self.parent, self.id);
            game.kill_element(self.id);
        }
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}

/// Java `HitsKen`/`HitsNagi`/`HitsBone`/`HitsKabe`/`HitsKnight` 的统一载体
#[derive(Clone, Debug)]
pub enum HitKind {
    Ken,
    Nagi,
    Bone,
    Kabe,
    Knight,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub side: usize,
    pub frame: i32,
    pub user: i32,
    pub cos_rot: f32,
    pub sin_rot: f32,
    pub flipped: bool,
    pub kind: HitKind,
    pub parent: ParentList,
    pub r: f32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub cos_half_angle: f32,
    pub a: f32,
    pub dir: f32,
    pub flatness: f32,
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

const K32: f32 = crate::fdlibm_consts::MATH_DUMP_DEG_TO_RAD_F32;

#[inline]
fn cos32(v: f32) -> f32 {
    crate::fdlibm::cos(v as f64) as f32
}

#[inline]
fn sin32(v: f32) -> f32 {
    crate::fdlibm::sin(v as f64) as f32
}

fn flipped_of(r_deg: i32, side: usize) -> bool {
    r_deg >= 90 + side as i32 && r_deg <= 270 + side as i32
}

impl Hit {
    fn base(
        id: i32,
        x: f32,
        y: f32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
        flipped: bool,
        kind: HitKind,
        parent: ParentList,
    ) -> Self {
        let (x, y) = (jmath::xy_sync(x), jmath::xy_sync(y));
        Self {
            id,
            x,
            y,
            side,
            frame: 0,
            user,
            cos_rot,
            sin_rot,
            flipped,
            kind,
            parent,
            r: 0.0,
            dir_x: 0.0,
            dir_y: 0.0,
            cos_half_angle: 0.0,
            a: 0.0,
            dir: 0.0,
            flatness: 0.0,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 0.0,
            max_y: 0.0,
        }
    }

    /// Java `HitsKen`（EllipticalSector 九参构造：显式覆盖三角函数初值）
    pub fn new_ken(
        id: i32,
        x: f32,
        y: f32,
        r_deg: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) -> Self {
        let mut h = Self::base(
            id,
            x,
            y,
            side,
            user,
            cos_rot,
            sin_rot,
            flipped_of(r_deg, side),
            HitKind::Ken,
            ParentList::Atk(side),
        );
        h.r = 61.0;
        h.a = 74.0 * K32;
        h.dir = 273.0 * K32;
        h.dir_x = jf32!(0.052335956243);
        h.dir_y = -jf32!(0.998629534754);
        h.cos_half_angle = jf32!(0.79863551);
        h.flatness = jf32!(1.1296296);
        h
    }

    /// Java `HitsBone`（六参构造：dirX/dirY/cosHalfAngle 由 cos/sin 计算）
    pub fn new_bone(
        id: i32,
        x: f32,
        y: f32,
        r_deg: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) -> Self {
        let mut h = Self::base(
            id,
            x,
            y,
            side,
            user,
            cos_rot,
            sin_rot,
            flipped_of(r_deg, side),
            HitKind::Bone,
            ParentList::Atk(side),
        );
        h.r = 36.0;
        h.a = 93.0 * K32;
        h.dir = 273.0 * K32;
        h.dir_x = cos32(h.dir);
        h.dir_y = sin32(h.dir);
        h.cos_half_angle = cos32(93.0 * 0.5);
        h.flatness = jf32!(1.1612903);
        h
    }

    /// Java `HitsKnight`
    pub fn new_knight(
        id: i32,
        x: f32,
        y: f32,
        r_deg: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) -> Self {
        let mut h = Self::base(
            id,
            x,
            y,
            side,
            user,
            cos_rot,
            sin_rot,
            flipped_of(r_deg, side),
            HitKind::Knight,
            ParentList::Atk(side),
        );
        h.r = 77.0;
        h.a = 74.0 * K32;
        h.dir = 273.0 * K32;
        h.dir_x = jf32!(0.052335956243);
        h.dir_y = -jf32!(0.998629534754);
        h.cos_half_angle = jf32!(0.79863551);
        h.flatness = jf32!(1.132);
        h
    }

    /// Java `HitsNagi`（Sector 构造）
    pub fn new_nagi(
        id: i32,
        x: f32,
        y: f32,
        r_deg: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) -> Self {
        let mut h = Self::base(
            id,
            x,
            y,
            side,
            user,
            cos_rot,
            sin_rot,
            flipped_of(r_deg, side),
            HitKind::Nagi,
            ParentList::Atk(side),
        );
        h.r = 63.0;
        h.a = 45.0 * K32;
        h.dir = 292.5 * K32;
        h.dir_x = cos32(h.dir);
        h.dir_y = sin32(h.dir);
        h.cos_half_angle = cos32(45.0 * 0.5);
        h
    }

    /// Java `HitsKabe`（Polygon；cos/sin 取自 KabeBall）
    pub fn new_kabe(
        id: i32,
        x: f32,
        y: f32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) -> Self {
        let verts: Vec<[f32; 2]> = KABE_VERTS.iter().map(|v| [v[0], v[1]]).collect();
        let poly = crate::geom::Polygon::new(0.0, 0.0, verts);
        let mut h = Self::base(
            id,
            x,
            y,
            side,
            user,
            cos_rot,
            sin_rot,
            false,
            HitKind::Kabe,
            ParentList::Shield(side),
        );
        h.min_x = poly.min_x;
        h.max_x = poly.max_x;
        h.min_y = poly.min_y;
        h.max_y = poly.max_y;
        h
    }

    fn update(&mut self) {
        self.dir_x = cos32(self.dir);
        self.dir_y = sin32(self.dir);
        self.cos_half_angle = cos32(self.a * 0.5);
    }

    pub fn step(&mut self, game: &mut GameTask) {
        match self.kind {
            HitKind::Ken | HitKind::Knight => {
                if let Some(e) = game.get(self.user) {
                    let (x, y) = e.xy();
                    self.x = x;
                    self.y = y;
                }
                match self.frame {
                    0 => {
                        self.a = 124.0 * K32;
                        self.dir = 298.0 * K32;
                        self.update();
                    }
                    1 => {
                        self.a = 161.0 * K32;
                        self.dir = 316.5 * K32;
                        self.update();
                    }
                    2 => {
                        self.a = 87.0 * K32;
                        self.dir = 353.5 * K32;
                        self.update();
                    }
                    3 => {
                        self.a = 37.0 * K32;
                        self.dir = 18.5 * K32;
                        self.update();
                    }
                    4 => self.kill(game),
                    _ => {}
                }
                self.frame += 1;
            }
            HitKind::Bone => {
                if let Some(e) = game.get(self.user) {
                    let (x, y) = e.xy();
                    self.x = x;
                    self.y = y;
                }
                match self.frame {
                    0 => {
                        self.a = 141.0 * K32;
                        self.dir = 285.0 * K32;
                        self.update();
                    }
                    1 => {
                        self.a = 188.0 * K32;
                        self.dir = 316.5 * K32;
                        self.update();
                    }
                    2 => {
                        self.a = 96.0 * K32;
                        self.dir = 8.0 * K32;
                        self.update();
                    }
                    3 => {
                        self.a = 48.0 * K32;
                        self.dir = 28.0 * K32;
                        self.update();
                    }
                    4 => self.kill(game),
                    _ => {}
                }
                self.frame += 1;
            }
            HitKind::Nagi => {
                if let Some(e) = game.get(self.user) {
                    let (x, y) = e.xy();
                    self.x = x;
                    self.y = y;
                }
                if self.frame == 9 {
                    self.kill(game);
                }
                self.frame += 1;
            }
            HitKind::Kabe => {
                match game.get(self.user) {
                    Some(crate::engine::Element::Ball(b))
                        if matches!(b.kind, crate::engine::BallKind::Kabe { .. }) =>
                    {
                        let (bx, by, bc, bs) = (b.x, b.y, b.cos_rot, b.sin_rot);
                        let shooting =
                            matches!(b.kind, crate::engine::BallKind::Kabe { shooting: true, .. });
                        self.x = bx + 20.0 * bc;
                        self.y = by + 20.0 * bs;
                        self.cos_rot = bc;
                        self.sin_rot = bs;
                        self.x = jmath::xy_sync(self.x);
                        self.y = jmath::xy_sync(self.y);
                        if !shooting {
                            self.kill(game);
                            return;
                        }
                    }
                    _ => {
                        self.kill(game);
                        return;
                    }
                }
                self.frame += 1;
                if self.frame >= 8 {
                    self.frame = 4;
                }
            }
        }
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.list_remove(self.parent, self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        match self.kind {
            HitKind::Ken | HitKind::Knight | HitKind::Bone => {
                let mut dx = (x - self.x) * self.cos_rot + (y - self.y) * self.sin_rot;
                let mut dy = (y - self.y) * self.cos_rot - (x - self.x) * self.sin_rot;
                dy *= self.flatness;
                if self.flipped {
                    dy = -dy;
                }
                if matches!(self.kind, HitKind::Bone) {
                    dx += 4.0;
                }
                if dx > self.r || dy > self.r || dx < -self.r || dy < -self.r {
                    return false;
                }
                let dist2 = dx * dx + dy * dy;
                if dist2 > self.r * self.r {
                    return false;
                }
                let dot = dx * self.dir_x + dy * self.dir_y;
                let cos2 = self.cos_half_angle * self.cos_half_angle;
                dot > 0.0 && dot * dot >= dist2 * cos2
            }
            HitKind::Nagi => {
                if x - self.x > jf32!(81.4)
                    || y - self.y > jf32!(81.4)
                    || x - self.x < -jf32!(81.4)
                    || y - self.y < -jf32!(81.4)
                {
                    return false;
                }
                let dx = (x - self.x) * self.cos_rot + (y - self.y) * self.sin_rot;
                let mut dy = (y - self.y) * self.cos_rot - (x - self.x) * self.sin_rot;
                if !self.flipped {
                    dy = -dy;
                }
                let dx = dx - jf32!(18.4);
                match self.frame {
                    0 => {
                        if dx * dy < dx * dx {
                            return false;
                        }
                    }
                    1 => {
                        if dx * dy < 0.0 {
                            return false;
                        }
                    }
                    2 | 6 => {
                        if dx * dy < -dx * dx {
                            return false;
                        }
                    }
                    3 | 7 => {
                        if dx * dy > dx * dx {
                            return false;
                        }
                    }
                    4 => {
                        if dx * dy > dy * dy {
                            return false;
                        }
                    }
                    5 => {
                        if dx * dy < -dy * dy {
                            return false;
                        }
                    }
                    8 => {
                        if dx * dy > 0.0 {
                            return false;
                        }
                    }
                    9 => {
                        if dx * dy > -dx * dx {
                            return false;
                        }
                    }
                    _ => {}
                }
                dx * dx + dy * dy <= 3969.0
            }
            HitKind::Kabe => {
                let mut px = x - self.x;
                let mut py = y - self.y;
                let old_x = px;
                px = px * self.cos_rot + py * self.sin_rot;
                py = py * self.cos_rot - old_x * self.sin_rot;
                let scale = KABE_SCALE_INVERSE[self.frame.clamp(0, 7) as usize];
                px *= scale;
                py *= scale;
                if self.max_x < px || self.min_x > px || self.max_y < py || self.min_y > py {
                    return false;
                }
                let mut hit = false;
                let n = KABE_VERTS.len();
                let mut j = n - 1;
                for i in 0..n {
                    let xi = KABE_VERTS[i][0];
                    let yi = KABE_VERTS[i][1];
                    let xj = KABE_VERTS[j][0];
                    let yj = KABE_VERTS[j][1];
                    if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
                        hit = !hit;
                    }
                    j = i;
                }
                hit
            }
        }
    }
}

const KABE_VERTS: [[f32; 2]; 12] = [
    [jf32!(62.8), -jf32!(0.05)],
    [jf32!(125.6), jf32!(72.5)],
    [jf32!(31.35), jf32!(54.35)],
    [0.0, 145.0],
    [-jf32!(31.45), jf32!(54.35)],
    [-jf32!(125.6), jf32!(72.5)],
    [-jf32!(62.85), 0.0],
    [-jf32!(125.6), -jf32!(72.55)],
    [-jf32!(31.45), -jf32!(54.4)],
    [-jf32!(0.05), -145.0],
    [jf32!(31.35), -jf32!(54.4)],
    [jf32!(125.6), -jf32!(72.55)],
];

const KABE_SCALE_INVERSE: [f32; 8] = [
    jf32!(4.027777777777778),
    jf32!(2.013888888888889),
    jf32!(1.342592592592593),
    jf32!(1.006944444444444),
    jf32!(0.805555555555556),
    jf32!(0.833333333333333),
    jf32!(0.863095238095238),
    jf32!(0.833333333333333),
];

/// Java `HitsYari`：CompositeShape + 逐帧重建的 YariBase 子形状
#[derive(Clone, Debug)]
pub struct HitYari {
    pub id: i32,
    pub side: usize,
    pub frame: i32,
    pub user: i32,
    pub rot: i32,
    pub inner: crate::geom::CompositeShape,
}

impl HitYari {
    pub fn new(id: i32, x: f32, y: f32, r_deg: i32, side: usize, user: i32) -> Self {
        use crate::geom::{Shape, YariBase};
        let mut inner = crate::geom::CompositeShape::new(jmath::xy_sync(x), jmath::xy_sync(y));
        let rot = ((r_deg % 360) + 360) % 360;
        // 子形状用未量子化的构造参数（与 Java 构造器一致）
        inner.add_shape(Shape::Yari(YariBase::new(
            x,
            y,
            r_deg,
            jf32!(0.5),
            -jf32!(29.5),
            jf32!(3.4),
        )));
        Self {
            id,
            side,
            frame: 0,
            user,
            rot,
            inner,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        use crate::geom::{Shape, YariBase};
        if let Some(e) = game.get(self.user) {
            let (nx, ny) = e.xy();
            self.inner.move_to(nx, ny);
        }
        let (ix, iy) = (self.inner.x, self.inner.y);
        match self.frame {
            0 => {
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    jf32!(0.625),
                    -jf32!(33.0),
                    jf32!(3.4),
                )));
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    jf32!(0.75),
                    -jf32!(32.6),
                    jf32!(3.5),
                )));
            }
            1 => {
                for _ in 0..3 {
                    self.inner.remove_shape_at(0);
                }
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    jf32!(0.75),
                    -jf32!(26.6),
                    jf32!(3.5),
                )));
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    jf32!(0.875),
                    -jf32!(26.55),
                    jf32!(3.6),
                )));
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    1.0,
                    -22.0,
                    jf32!(3.5),
                )));
            }
            2 => {
                for _ in 0..3 {
                    self.inner.remove_shape_at(0);
                }
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    1.0,
                    -jf32!(21.8),
                    jf32!(3.5),
                )));
                self.inner.add_shape(Shape::Yari(YariBase::new(
                    ix,
                    iy,
                    self.rot,
                    1.0,
                    0.0,
                    jf32!(3.6),
                )));
            }
            3 => {
                self.inner.remove_shape_at(0);
            }
            4 => {
                self.inner.remove_shape_at(0);
                self.kill(game);
            }
            _ => {}
        }
        self.frame += 1;
    }

    pub fn kill(&mut self, game: &mut GameTask) {
        game.atk[self.side].remove(self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        self.inner.hit_test_point(x, y)
    }
}

/// Java `HitsHeal extends Round`：挂在 heal/repair 传感器，下一帧移除
#[derive(Clone, Debug)]
pub struct HitsHeal {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub parent: ParentList,
}

impl HitsHeal {
    pub fn new(id: i32, x: f32, y: f32, parent: ParentList, radius: f32) -> Self {
        Self {
            id,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: radius,
            parent,
        }
    }

    pub fn step(&mut self, game: &mut GameTask) {
        game.list_remove(self.parent, self.id);
        game.kill_element(self.id);
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}

/// Java `HitsBomb` / `HitsBombMult`：逐帧放大的圆形暴风
#[derive(Clone, Debug)]
pub struct HitsBomb {
    pub id: i32,
    pub side: usize,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub frame: i32,
    pub mult: f32,
    pub is_mult: bool,
    pub parent: ParentList,
}

impl HitsBomb {
    pub fn new(id: i32, x: f32, y: f32, side: usize, mult: f32) -> Self {
        Self {
            id,
            side,
            x: jmath::xy_sync(x),
            y: jmath::xy_sync(y),
            r: 30.0 * mult,
            frame: 0,
            mult,
            is_mult: true,
            parent: ParentList::Atk(side),
        }
    }

    /// Java `HitsBomb`（无倍率）
    #[allow(dead_code)]
    pub fn new_plain(id: i32, x: f32, y: f32, side: usize) -> Self {
        let mut b = Self::new(id, x, y, side, 1.0);
        b.is_mult = false;
        b
    }

    pub fn step(&mut self, game: &mut GameTask) {
        if self.frame < 3 {
            let base = if self.frame == 0 {
                60.0
            } else if self.frame == 1 {
                39.0
            } else {
                12.0
            };
            self.r = base * self.mult;
            self.frame += 1;
        } else {
            game.list_remove(self.parent, self.id);
            game.kill_element(self.id);
        }
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        crate::geom::Round::new(self.x, self.y, self.r).hit_test_point(x, y)
    }
}
