//! Java 几何基元逐位复刻（`Shape`/`Round`/`Rectangle`/`Polygon`/`Sector`/`EllipticalSector`/`CompositeShape`）。
//!
//! 语义以 oracle `org.example` 对应类为准：判空、短路顺序、乘法顺序均逐行对应。
//! 传感器所需的身份删除在最终元素模型里用 ID 表达（见 `docs/ISSUES.md` D-005）。
// `!(a < x || a > y)` 是 Java 原写法，保留形状。
#![allow(clippy::manual_range_contains)]

use crate::fdlibm;
use crate::gen_literals::jf32;
use crate::jmath;
use crate::trig;

/// oracle 字面量 `0.017453292519943295F` 的 f32 位模式（`MathDump` 导出）
pub const DEG_TO_RAD: f32 = crate::fdlibm_consts::MATH_DUMP_DEG_TO_RAD_F32;

#[inline(always)]
fn cos_f32(v: f32) -> f32 {
    fdlibm::cos(v as f64) as f32
}

#[inline(always)]
fn sin_f32(v: f32) -> f32 {
    fdlibm::sin(v as f64) as f32
}

/// Java `Round`
#[derive(Clone, Debug)]
pub struct Round {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

impl Round {
    pub fn new(x: f32, y: f32, r: f32) -> Self {
        Self { x, y, r }
    }

    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        if px - self.x > self.r
            || py - self.y > self.r
            || self.x - px > self.r
            || self.y - py > self.r
        {
            return false;
        }
        let dx = self.x - px;
        let dy = self.y - py;
        dx * dx + dy * dy <= self.r * self.r
    }
}

/// Java `Rectangle`（x,y 与 start_x,start_y 的双原点语义逐行保留）
#[derive(Clone, Debug)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub start_x: f32,
    pub start_y: f32,
}

impl Rectangle {
    pub fn new(x: f32, y: f32, start_x: f32, start_y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            start_x,
            start_y,
        }
    }

    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        let px = px - self.start_x;
        let py = py - self.start_y;
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

/// Java `Polygon`：局部顶点 + 构造时预计算的包围盒（注意 Java 默认初值 0.0 参与 min/max）
#[derive(Clone, Debug)]
pub struct Polygon {
    pub x: f32,
    pub y: f32,
    pub local_vertices: Vec<[f32; 2]>,
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl Polygon {
    pub fn new(x: f32, y: f32, local_vertices: Vec<[f32; 2]>) -> Self {
        let mut min_x = 0.0f32;
        let mut max_x = 0.0f32;
        let mut min_y = 0.0f32;
        let mut max_y = 0.0f32;
        for v in &local_vertices {
            if v[0] < min_x {
                min_x = v[0];
            }
            if v[0] > max_x {
                max_x = v[0];
            }
            if v[1] < min_y {
                min_y = v[1];
            }
            if v[1] > max_y {
                max_y = v[1];
            }
        }
        Self {
            x,
            y,
            local_vertices,
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        let px = px - self.x;
        let py = py - self.y;
        if self.max_x < px || self.min_x > px || self.max_y < py || self.min_y > py {
            return false;
        }
        let mut hit = false;
        let n = self.local_vertices.len();
        let mut j = n - 1;
        for i in 0..n {
            let xi = self.local_vertices[i][0];
            let yi = self.local_vertices[i][1];
            let xj = self.local_vertices[j][0];
            let yj = self.local_vertices[j][1];
            if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
                hit = !hit;
            }
            j = i;
        }
        hit
    }
}

/// Java `Sector`
#[derive(Clone, Debug)]
pub struct Sector {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub cos_half_angle: f32,
    pub dir: f32,
    pub a: f32,
}

impl Sector {
    pub fn new(x: f32, y: f32, r: f32, angle: f32, direction: f32) -> Self {
        let a = angle * DEG_TO_RAD;
        let dir = direction * DEG_TO_RAD;
        let dir_x = cos_f32(dir);
        let dir_y = sin_f32(dir);
        let cos_half_angle = cos_f32(angle * 0.5f32);
        Self {
            x,
            y,
            r,
            dir_x,
            dir_y,
            cos_half_angle,
            dir,
            a,
        }
    }

    pub fn update(&mut self) {
        self.dir_x = cos_f32(self.dir);
        self.dir_y = sin_f32(self.dir);
        self.cos_half_angle = cos_f32(self.a * 0.5f32);
    }

    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        if px - self.x > self.r
            || py - self.y > self.r
            || self.x - px > self.r
            || self.y - py > self.r
        {
            return false;
        }
        let dx = px - self.x;
        let dy = py - self.y;
        let dist2 = dx * dx + dy * dy;
        if dist2 > self.r * self.r {
            return false;
        }
        let dot = dx * self.dir_x + dy * self.dir_y;
        let cos2 = self.cos_half_angle * self.cos_half_angle;
        dot > 0.0 && dot * dot >= dist2 * cos2
    }
}

/// Java `EllipticalSector`（y 方向按 flatness 缩放后再做扇形判定）
#[derive(Clone, Debug)]
pub struct EllipticalSector {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub cos_half_angle: f32,
    pub dir: f32,
    pub a: f32,
    pub flatness: f32,
}

impl EllipticalSector {
    pub fn new(x: f32, y: f32, r: f32, angle: f32, direction: f32, flatness: f32) -> Self {
        let a = angle * DEG_TO_RAD;
        let dir = direction * DEG_TO_RAD;
        let dir_x = cos_f32(dir);
        let dir_y = sin_f32(dir);
        let cos_half_angle = cos_f32(angle * 0.5f32);
        Self {
            x,
            y,
            r,
            dir_x,
            dir_y,
            cos_half_angle,
            dir,
            a,
            flatness,
        }
    }

    /// Java `EllipticalSector` 的九参构造（显式三角函数值版本）
    #[allow(clippy::too_many_arguments)]
    pub fn with_trig(
        x: f32,
        y: f32,
        r: f32,
        angle: f32,
        direction: f32,
        flatness: f32,
        dir_x: f32,
        dir_y: f32,
        cos_half_angle: f32,
    ) -> Self {
        let mut s = Self::new(x, y, r, angle, direction, flatness);
        s.dir_x = dir_x;
        s.dir_y = dir_y;
        s.cos_half_angle = cos_half_angle;
        s
    }

    pub fn update(&mut self) {
        self.dir_x = cos_f32(self.dir);
        self.dir_y = sin_f32(self.dir);
        self.cos_half_angle = cos_f32(self.a * 0.5f32);
    }

    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        let dx = px - self.x;
        let dy = (py - self.y) * self.flatness;
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
}

/// Java `CompositeShape`
#[derive(Clone, Debug)]
pub struct CompositeShape {
    pub x: f32,
    pub y: f32,
    pub shapes: Vec<Shape>,
}

impl CompositeShape {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            shapes: Vec::new(),
        }
    }

    pub fn add_shape(&mut self, s: Shape) {
        self.shapes.push(s);
    }

    /// Java `List.remove(Object)` 为身份删除；最终元素模型以 ID 表达（此处按索引删除）
    pub fn remove_shape_at(&mut self, index: usize) -> Shape {
        self.shapes.remove(index)
    }

    pub fn move_by(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
        for s in &mut self.shapes {
            s.move_by(dx, dy);
        }
    }

    pub fn move_to(&mut self, nx: f32, ny: f32) {
        let dx = nx - self.x;
        let dy = ny - self.y;
        for s in &mut self.shapes {
            s.move_by(dx, dy);
        }
        self.x = nx;
        self.y = ny;
    }

    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        self.shapes.iter().any(|s| s.hit_test_point(px, py))
    }
}

/// Java `YariBase`：枪系判定单元（先缩放、再平移、再旋转/翻转）
#[derive(Clone, Debug)]
pub struct YariBase {
    pub x: f32,
    pub y: f32,
    pub flip_rad: i32,
    pub internal_scale_x: f32,
    pub internal_x: f32,
    pub internal_y: f32,
    pub flipped: bool,
    pub cos_rot: f32,
    pub sin_rot: f32,
}

impl YariBase {
    pub fn new(x: f32, y: f32, r_deg: i32, sx: f32, ix: f32, iy: f32) -> Self {
        let flipped = !(r_deg < 90 || r_deg > 270);
        let flip_rad = if flipped { (r_deg + 180) % 360 } else { r_deg };
        Self {
            x,
            y,
            flip_rad,
            internal_scale_x: sx,
            internal_x: ix,
            internal_y: iy,
            flipped,
            cos_rot: trig::cos(flip_rad),
            sin_rot: trig::sin(flip_rad),
        }
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        let raw_dx = x - self.x;
        let raw_dy = y - self.y;
        let mut dx = raw_dx * self.cos_rot + raw_dy * self.sin_rot;
        let dy = raw_dy * self.cos_rot - raw_dx * self.sin_rot;
        if self.flipped {
            dx = -dx;
        }
        let dx = (dx - self.internal_x) / self.internal_scale_x;
        let dy = dy - self.internal_y;
        if dx < jf32!(76.0)
            || dx > jf32!(156.0)
            || dy < jf32!(0.2) * (dx - jf32!(156.0))
            || dy > -jf32!(0.2) * (dx - jf32!(156.0))
        {
            return false;
        }
        dx * dx / jf32!(7744.0) + dy * dy / jf32!(1004.89) > 1.0
    }
}

/// 所有几何基元的统一分发（对应 Java `Shape` 多态）
#[derive(Clone, Debug)]
pub enum Shape {
    Round(Round),
    Rectangle(Rectangle),
    Polygon(Polygon),
    Sector(Sector),
    EllipticalSector(EllipticalSector),
    Composite(CompositeShape),
    Yari(YariBase),
}

impl Shape {
    pub fn hit_test_point(&self, px: f32, py: f32) -> bool {
        match self {
            Shape::Round(s) => s.hit_test_point(px, py),
            Shape::Rectangle(s) => s.hit_test_point(px, py),
            Shape::Polygon(s) => s.hit_test_point(px, py),
            Shape::Sector(s) => s.hit_test_point(px, py),
            Shape::EllipticalSector(s) => s.hit_test_point(px, py),
            Shape::Composite(s) => s.hit_test_point(px, py),
            Shape::Yari(s) => s.hit_test_point(px, py),
        }
    }

    pub fn x(&self) -> f32 {
        match self {
            Shape::Round(s) => s.x,
            Shape::Rectangle(s) => s.x,
            Shape::Polygon(s) => s.x,
            Shape::Sector(s) => s.x,
            Shape::EllipticalSector(s) => s.x,
            Shape::Composite(s) => s.x,
            Shape::Yari(s) => s.x,
        }
    }

    pub fn y(&self) -> f32 {
        match self {
            Shape::Round(s) => s.y,
            Shape::Rectangle(s) => s.y,
            Shape::Polygon(s) => s.y,
            Shape::Sector(s) => s.y,
            Shape::EllipticalSector(s) => s.y,
            Shape::Composite(s) => s.y,
            Shape::Yari(s) => s.y,
        }
    }

    /// Java `Shape.move(X, Y)`（Composite 会同步移动子形状）
    pub fn move_by(&mut self, dx: f32, dy: f32) {
        match self {
            Shape::Round(s) => {
                s.x += dx;
                s.y += dy;
            }
            Shape::Rectangle(s) => {
                s.x += dx;
                s.y += dy;
            }
            Shape::Polygon(s) => {
                s.x += dx;
                s.y += dy;
            }
            Shape::Sector(s) => {
                s.x += dx;
                s.y += dy;
            }
            Shape::EllipticalSector(s) => {
                s.x += dx;
                s.y += dy;
            }
            Shape::Composite(s) => s.move_by(dx, dy),
            Shape::Yari(s) => {
                s.x += dx;
                s.y += dy;
            }
        }
    }

    /// Java `Shape.moveTo(X, Y)`
    pub fn move_to(&mut self, nx: f32, ny: f32) {
        match self {
            Shape::Round(s) => {
                s.x = nx;
                s.y = ny;
            }
            Shape::Rectangle(s) => {
                s.x = nx;
                s.y = ny;
            }
            Shape::Polygon(s) => {
                s.x = nx;
                s.y = ny;
            }
            Shape::Sector(s) => {
                s.x = nx;
                s.y = ny;
            }
            Shape::EllipticalSector(s) => {
                s.x = nx;
                s.y = ny;
            }
            Shape::Composite(s) => s.move_to(nx, ny),
            Shape::Yari(s) => {
                s.x = nx;
                s.y = ny;
            }
        }
    }

    /// Java `Shape.xySync()`
    pub fn xy_sync(&mut self) {
        let (x, y) = (jmath::xy_sync(self.x()), jmath::xy_sync(self.y()));
        match self {
            Shape::Round(s) => {
                s.x = x;
                s.y = y;
            }
            Shape::Rectangle(s) => {
                s.x = x;
                s.y = y;
            }
            Shape::Polygon(s) => {
                s.x = x;
                s.y = y;
            }
            Shape::Sector(s) => {
                s.x = x;
                s.y = y;
            }
            Shape::EllipticalSector(s) => {
                s.x = x;
                s.y = y;
            }
            Shape::Composite(s) => {
                s.x = x;
                s.y = y;
            }
            Shape::Yari(s) => {
                s.x = x;
                s.y = y;
            }
        }
    }

    /// Java `Shape.xSync()`
    pub fn x_sync(&mut self) {
        let x = jmath::xy_sync(self.x());
        match self {
            Shape::Round(s) => s.x = x,
            Shape::Rectangle(s) => s.x = x,
            Shape::Polygon(s) => s.x = x,
            Shape::Sector(s) => s.x = x,
            Shape::EllipticalSector(s) => s.x = x,
            Shape::Composite(s) => s.x = x,
            Shape::Yari(s) => s.x = x,
        }
    }

    /// Java `Shape.ySync()`
    pub fn y_sync(&mut self) {
        let y = jmath::xy_sync(self.y());
        match self {
            Shape::Round(s) => s.y = y,
            Shape::Rectangle(s) => s.y = y,
            Shape::Polygon(s) => s.y = y,
            Shape::Sector(s) => s.y = y,
            Shape::EllipticalSector(s) => s.y = y,
            Shape::Composite(s) => s.y = y,
            Shape::Yari(s) => s.y = y,
        }
    }
}
