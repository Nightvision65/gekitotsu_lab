//! 帧骨架：`GameTask` + `init`/`main_setup`/`base_move`/`judge`/`update` 与传感器容器。
//!
//! Tier-0 语义（AGENTS §4）：
//! - 元素 step 顺序 = ID 插入顺序；本帧新建元素不 step（`to = next_id` 帧首快照）。
//! - 传感器列表保序；墙/队等 CompositeShape 只是 OR 组合，命中顺序不影响布尔结果。
//! - `IntShapeMap.compact` 只影响内存布局（相对顺序不变），本实现用 ID 稠密数组天然无序化，
//!   故不实现 compact；`deadCount`/`size` 的触发条件没有可观测语义。

// 元素 spawn 方法参数与 Java 构造器对齐，保持长参数表。
#![allow(clippy::too_many_arguments)]

mod ball;
mod base;
mod batch;
mod bullet;
mod core;
mod hits;
mod kekkai;
mod laser;
#[cfg(feature = "phase-timing")]
pub mod prof;
mod special;
mod telemetry;
mod wall;

pub use ball::{Ball, BallInit, BallKind};
pub use base::Base;
pub use batch::{BatchJob, BatchOutcome, run_batch, run_batch_with};
pub use bullet::{Bullet, BulletKind, SearchShot, ShaRay, UkiShot};
pub use core::{Core, CoreKind};
pub use hits::{Hit, HitKind, HitYari, HitsBomb, HitsDrop, HitsDropFrames, HitsHeal, HitsJump};
pub use kekkai::KekkaiField;
pub use laser::{Laser, LaserKind};
pub use special::{Creature, CreatureKind, GeiLink, HolyGhost, StarPrepareLaser, SyouShield};
pub use telemetry::{
    BattleEvent, BattleEventType, BattleFrameSnapshot, BattleObserver, BattleRecorder,
    BattleSummary, BattleTelemetry, TelemetryMode,
};
pub use wall::{Wall, WallKind};

use telemetry::FrameCallback;

use crate::decode::{CompiledFort, core_spawn, seed_factor, unit_spawn};
use crate::gen_literals::jf32;
use crate::jmath;
use crate::rng;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Result {
    pub status: i32,
    pub winner_hp: i32,
    pub frame_passed: i32,
}

impl Result {
    pub fn is_timeout(&self) -> bool {
        self.status == -1
    }
}

/// Java `CompositeShape`：保序 ID 列表，命中为子元素 OR
#[derive(Clone, Debug, Default)]
pub struct Sensor {
    pub ids: Vec<i32>,
}

impl Sensor {
    pub fn add(&mut self, id: i32) {
        self.ids.push(id);
    }

    /// Java `List.remove(Object)`：身份删除第一个匹配
    pub fn remove(&mut self, id: i32) {
        if let Some(p) = self.ids.iter().position(|&v| v == id) {
            self.ids.remove(p);
        }
    }

    pub fn hit_test(&self, game: &GameTask, x: f32, y: f32) -> bool {
        self.hit_test_skip(game, x, y, -1)
    }

    /// 跳过 `skip` 元素（step 期间该元素被临时借出；调用方以自身命中补回，OR 等价）
    pub fn hit_test_skip(&self, game: &GameTask, x: f32, y: f32, skip: i32) -> bool {
        #[cfg(feature = "phase-timing")]
        {
            prof::inc(&prof::SENSOR_CALLS[0]);
            prof::add(&prof::SENSOR_ENTRIES[0], self.ids.len() as u64);
        }
        self.ids
            .iter()
            .any(|&id| id != skip && game.raw(id).hit_test_point(x, y))
    }

    /// 同 `hit_test`，但返回第一个命中的元素 id（无命中 -1）；供 land 记录支撑墙
    pub fn hit_test_id(&self, game: &GameTask, x: f32, y: f32) -> i32 {
        for &id in &self.ids {
            if game.raw(id).hit_test_point(x, y) {
                return id;
            }
        }
        -1
    }
}

/// 侧别 wall 列表整体 AABB（相对该侧 Base，仅无 BoxBall/Elevator 的对局启用）。
///
/// 依据：无 Box 时 wall 列表只减不增；无 Elevator 时所有成员相对 Base 的坐标固定，
/// 帧内唯一误差是"未 step 的墙滞后 d = base 本帧位移"，查询时用实时 |d| 扩边严格覆盖。
#[derive(Clone, Copy, Debug)]
pub struct WallAabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// 元素包围圆（保守上界，用于传感器扫描粗筛）。
///
/// 不变式：`bounds[id]` 与元素当前 (x, y, 尺寸) 一致——元素自身 step 结束后、
/// 任何跨元素写坐标的辅助方法内、以及 spawn 时都会刷新。`r2 = +inf` 表示放弃粗筛。
#[derive(Clone, Copy, Debug)]
pub struct Bound {
    pub x: f32,
    pub y: f32,
    pub r2: f32,
}

#[inline]
fn circle_bound(x: f32, y: f32, r: f32) -> Bound {
    Bound { x, y, r2: r * r }
}

const BOUND_SKIP: Bound = Bound {
    x: 0.0,
    y: 0.0,
    r2: f32::INFINITY,
};

/// 命中形状的保守外接圆。宁可放大（多走精确判定），绝不能缩小。
fn element_bound(e: &Element) -> Bound {
    match e {
        // 图形尺寸见 Base/Core/Wall 的 hitTestPoint：半对角/半宽
        Element::Base(b) => circle_bound(b.x, b.y, 200.0),
        Element::Core(c) => circle_bound(c.x, c.y, 76.0),
        Element::Wall(w) => circle_bound(w.x, w.y, 24.75),
        Element::HitsJump(h) => circle_bound(h.x, h.y, 24.75),
        Element::HitsDrop(h) => circle_bound(h.x, h.y, h.r),
        Element::HitsDropFrames(h) => circle_bound(h.x, h.y, h.r),
        Element::HitsHeal(h) => circle_bound(h.x, h.y, h.r),
        Element::Ball(b) => {
            // Tate/Knight 为旋转矩形，取 3r 保守圆
            if matches!(b.kind, BallKind::Tate { .. } | BallKind::Knight { .. }) {
                circle_bound(b.x, b.y, b.r * 3.0)
            } else {
                circle_bound(b.x, b.y, b.r)
            }
        }
        Element::Bullet(b) => circle_bound(b.x, b.y, b.r),
        Element::HitsBomb(h) => circle_bound(h.x, h.y, h.r),
        // 近战命中体：Nagi 旋转空间半径 63 + 偏心 18.4；Kabe 多边形随帧缩放；其余取点空间半径 r
        Element::Hit(h) => match h.kind {
            HitKind::Nagi => circle_bound(h.x, h.y, 90.0),
            HitKind::Kabe => circle_bound(h.x, h.y, 300.0),
            _ => circle_bound(h.x, h.y, h.r + 10.0),
        },
        Element::Creature(c) => circle_bound(c.x, c.y, 60.0),
        Element::HolyGhost(h) => circle_bound(h.x, h.y, 30.0),
        Element::SyouShield(s) => circle_bound(s.x, s.y, 60.0),
        // 长条/射线类：粗筛无意义，始终走精确判定
        Element::KekkaiField(_)
        | Element::Laser(_)
        | Element::StarPrepare(_)
        | Element::HitYari(_)
        | Element::ShaRay(_)
        | Element::UkiShot(_)
        | Element::SearchShot(_)
        | Element::GeiLink(_) => BOUND_SKIP,
    }
}

/// Java `UnitHitSystem`
#[derive(Clone, Debug, Default)]
pub struct UnitSensor {
    pub ids: Vec<i32>,
    pub common: Vec<i32>,
    pub left: Vec<i32>,
    pub right: Vec<i32>,
}

impl UnitSensor {
    pub fn add(&mut self, id: i32) {
        self.ids.push(id);
        self.common.push(id);
    }

    pub fn remove(&mut self, id: i32) {
        for list in [
            &mut self.ids,
            &mut self.common,
            &mut self.left,
            &mut self.right,
        ] {
            if let Some(p) = list.iter().position(|&v| v == id) {
                list.remove(p);
            }
        }
    }

    pub fn resign(&mut self, game: &GameTask) {
        self.common.clear();
        self.left.clear();
        self.right.clear();
        for &id in &self.ids {
            if game.is_ball(id) {
                let (bx, _) = game.raw(id).xy();
                if bx < (game.mid + 42) as f32 {
                    self.left.push(id);
                }
                if bx > (game.mid - 42) as f32 {
                    self.right.push(id);
                }
            } else {
                self.common.push(id);
            }
        }
    }

    pub fn hit_test(&self, game: &GameTask, x: f32, y: f32) -> bool {
        self.hit_test_skip(game, x, y, -1)
    }

    pub fn hit_test_skip(&self, game: &GameTask, x: f32, y: f32, skip: i32) -> bool {
        #[cfg(feature = "phase-timing")]
        {
            prof::inc(&prof::SENSOR_CALLS[1]);
            let side_len = if x <= game.mid as f32 {
                self.left.len()
            } else {
                self.right.len()
            };
            prof::add(
                &prof::SENSOR_ENTRIES[1],
                (self.common.len() + side_len) as u64,
            );
        }
        for &id in &self.common {
            if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y) {
                return true;
            }
        }
        if x <= game.mid as f32 {
            for &id in &self.left {
                if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y) {
                    return true;
                }
            }
            false
        } else {
            for &id in &self.right {
                if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y) {
                    return true;
                }
            }
            false
        }
    }
}

/// Java `AtkHitSystem`（hitTestPoint 继承 HitSystem 的上/下分区逻辑）
#[derive(Clone, Debug, Default)]
pub struct AtkSensor {
    pub ids: Vec<i32>,
    pub upper_left: Vec<i32>,
    pub upper_right: Vec<i32>,
    pub left: Vec<i32>,
    pub right: Vec<i32>,
    pub common: Vec<i32>,
}

impl AtkSensor {
    pub fn add(&mut self, id: i32) {
        self.ids.push(id);
        self.common.push(id);
    }

    pub fn remove(&mut self, id: i32) {
        for list in [
            &mut self.ids,
            &mut self.upper_left,
            &mut self.upper_right,
            &mut self.left,
            &mut self.right,
            &mut self.common,
        ] {
            if let Some(p) = list.iter().position(|&v| v == id) {
                list.remove(p);
            }
        }
    }

    pub fn resign(&mut self, game: &GameTask) {
        self.common.clear();
        self.upper_left.clear();
        self.upper_right.clear();
        self.left.clear();
        self.right.clear();
        for &id in &self.ids {
            if game.is_bullet_or_nagi(id) {
                let (sx, sy) = game.raw(id).xy();
                if sy < 84.0 {
                    if sx < (game.mid + 84) as f32 {
                        self.upper_left.push(id);
                    }
                    if sx > (game.mid - 84) as f32 {
                        self.upper_right.push(id);
                    }
                }
                if sy > -84.0 {
                    if sx < (game.mid + 84) as f32 {
                        self.left.push(id);
                    }
                    if sx > (game.mid - 84) as f32 {
                        self.right.push(id);
                    }
                }
            } else {
                self.common.push(id);
            }
        }
    }

    pub fn hit_test(&self, game: &GameTask, x: f32, y: f32) -> bool {
        self.hit_test_skip(game, x, y, -1)
    }

    pub fn hit_test_skip(&self, game: &GameTask, x: f32, y: f32, skip: i32) -> bool {
        #[cfg(feature = "phase-timing")]
        {
            prof::inc(&prof::SENSOR_CALLS[2]);
            let part_len = if y < 0.0 {
                if x <= game.mid as f32 {
                    self.upper_left.len()
                } else {
                    self.upper_right.len()
                }
            } else if x <= game.mid as f32 {
                self.left.len()
            } else {
                self.right.len()
            };
            prof::add(
                &prof::SENSOR_ENTRIES[2],
                (self.common.len() + part_len) as u64,
            );
        }
        for &id in &self.common {
            if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y) {
                return true;
            }
        }
        if y < 0.0 {
            if x <= game.mid as f32 {
                for &id in &self.upper_left {
                    if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y)
                    {
                        return true;
                    }
                }
                false
            } else {
                for &id in &self.upper_right {
                    if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y)
                    {
                        return true;
                    }
                }
                false
            }
        } else if x <= game.mid as f32 {
            for &id in &self.left {
                if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y) {
                    return true;
                }
            }
            false
        } else {
            for &id in &self.right {
                if id != skip && game.bound_pass(id, x, y) && game.raw(id).hit_test_point(x, y) {
                    return true;
                }
            }
            false
        }
    }
}

/// 命中体挂载的传感器（Java 传入的 `CompositeShape parent`）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentList {
    Wall(usize),
    Unit(usize),
    Atk(usize),
    Shield(usize),
    Heal(usize),
    Repair(usize),
    JumpU(usize),
    JumpF(usize),
}

#[derive(Clone, Debug)]
pub enum Element {
    Base(Base),
    Core(Core),
    KekkaiField(KekkaiField),
    Laser(Laser),
    StarPrepare(StarPrepareLaser),
    Creature(Creature),
    Wall(Wall),
    HitsJump(HitsJump),
    HitsDrop(HitsDrop),
    HitsDropFrames(HitsDropFrames),
    Ball(Ball),
    Hit(Hit),
    HitYari(HitYari),
    HitsHeal(HitsHeal),
    Bullet(Bullet),
    ShaRay(ShaRay),
    HitsBomb(HitsBomb),
    UkiShot(UkiShot),
    SearchShot(SearchShot),
    GeiLink(GeiLink),
    HolyGhost(HolyGhost),
    SyouShield(SyouShield),
}

impl Element {
    pub fn id(&self) -> i32 {
        match self {
            Element::Base(e) => e.id,
            Element::Core(e) => e.id,
            Element::KekkaiField(e) => e.id,
            Element::Laser(e) => e.id,
            Element::StarPrepare(e) => e.id,
            Element::Creature(e) => e.id,
            Element::Wall(e) => e.id,
            Element::HitsJump(e) => e.id,
            Element::HitsDrop(e) => e.id,
            Element::HitsDropFrames(e) => e.id,
            Element::Ball(e) => e.id,
            Element::Hit(e) => e.id,
            Element::HitYari(e) => e.id,
            Element::HitsHeal(e) => e.id,
            Element::Bullet(e) => e.id,
            Element::ShaRay(e) => e.id,
            Element::HitsBomb(e) => e.id,
            Element::UkiShot(e) => e.id,
            Element::SearchShot(e) => e.id,
            Element::GeiLink(e) => e.id,
            Element::HolyGhost(e) => e.id,
            Element::SyouShield(e) => e.id,
        }
    }

    /// 帧 dump 的元素类型码（与 `GameDump.kindOf` 对齐）
    pub fn kind_code(&self) -> u8 {
        match self {
            Element::Base(_) => 0,
            Element::Core(c) => match c.kind {
                CoreKind::Boss { .. } => 1,
                CoreKind::Boss2 { .. } => 2,
                CoreKind::Normal => 3,
            },
            Element::KekkaiField(_) => 4,
            Element::Laser(l) => match l.kind {
                LaserKind::Boss => 5,
                LaserKind::Boss2 => 6,
                LaserKind::Magic => 108,
                LaserKind::Star => 110,
            },
            Element::StarPrepare(_) => 109,
            Element::Creature(c) => match c.kind {
                CreatureKind::Drill => 112,
                CreatureKind::Solid => 113,
                CreatureKind::Shock => 114,
                CreatureKind::Weak => 115,
            },
            Element::Wall(w) => match w.kind {
                WallKind::Wood => 7,
                WallKind::Stone => 8,
                WallKind::Paper => 9,
                WallKind::Iron => 10,
                WallKind::Jet => 11,
                WallKind::Turbo => 12,
                WallKind::Near => 13,
                WallKind::Far { .. } => 14,
                WallKind::Wide { .. } => 15,
                WallKind::Narrow { .. } => 16,
                WallKind::Snipe => 17,
                WallKind::Elevator { .. } => 18,
                WallKind::Box { .. } => 19,
            },
            Element::HitsJump(_) => 20,
            Element::HitsDropFrames(_) => 21,
            Element::HitsDrop(_) => 29,
            Element::Hit(h) => match h.kind {
                HitKind::Ken => 30,
                HitKind::Nagi => 32,
                HitKind::Bone => 33,
                HitKind::Kabe => 34,
                HitKind::Knight => 36,
            },
            Element::HitYari(_) => 31,
            Element::HitsHeal(_) => 35,
            Element::HitsBomb(b) => {
                if b.is_mult {
                    46
                } else {
                    45
                }
            }
            // Java `SniperBullet` 与 `ShaBullet` 是不同类；帧数 3=狙、6=普通
            Element::ShaRay(r) => {
                if r.frames == 3 {
                    48
                } else {
                    47
                }
            }
            Element::Bullet(b) => match b.kind {
                BulletKind::Plain => 49,
                BulletKind::Gun => 50,
                BulletKind::Cannon { .. } => 51,
                BulletKind::Shotgun { .. } => 52,
                BulletKind::Retsu => 53,
                BulletKind::Bomb => 62,
                BulletKind::Con { .. } => 63,
                BulletKind::ConMissile { .. } => 64,
                BulletKind::Geki { .. } => 65,
                BulletKind::Hanabi { .. } => 66,
                BulletKind::Hane { .. } => 67,
                BulletKind::Heri { .. } => 68,
                BulletKind::Hinoko { .. } => 69,
                BulletKind::Kaku { .. } => 70,
                BulletKind::Kan => 71,
                BulletKind::Missile { .. } => 72,
                BulletKind::Nin { .. } => 73,
                BulletKind::Push => 74,
                BulletKind::Sen { .. } => 75,
                BulletKind::SenTama { .. } => 76,
                BulletKind::Tobi { .. } => 77,
                BulletKind::TobiBomb => 78,
                BulletKind::Ton { .. } => 79,
                BulletKind::Tui { .. } => 80,
                BulletKind::TuiMissile { .. } => 81,
                BulletKind::BossMissile { .. } => 111,
            },
            Element::UkiShot(_) => 86,
            Element::SearchShot(_) => 85,
            Element::GeiLink(_) => 82,
            Element::HolyGhost(_) => 83,
            Element::SyouShield(_) => 84,
            Element::Ball(b) => match b.kind {
                BallKind::Sword => 37,
                BallKind::Yari => 38,
                BallKind::Nagi => 39,
                BallKind::Bone => 40,
                BallKind::Kabe { .. } => 41,
                BallKind::Heal => 42,
                BallKind::Repair => 43,
                BallKind::Knight { .. } => 44,
                BallKind::Bow => 54,
                BallKind::Dokyu => 55,
                BallKind::Gun => 56,
                BallKind::Cannon => 57,
                BallKind::Shotgun => 58,
                BallKind::Minigun => 59,
                BallKind::Sniper => 60,
                BallKind::Retsu => 61,
                BallKind::Bomb => 87,
                BallKind::Con => 88,
                BallKind::Gei { .. } => 89,
                BallKind::Geki => 90,
                BallKind::Guide => 91,
                BallKind::Hana => 92,
                BallKind::Hane => 93,
                BallKind::Heri => 94,
                BallKind::Holy => 95,
                BallKind::Kaku => 96,
                BallKind::Kan => 97,
                BallKind::Nin => 98,
                BallKind::Push => 99,
                BallKind::Search => 100,
                BallKind::Sen => 101,
                BallKind::Sha => 102,
                BallKind::Syou => 103,
                BallKind::Tobi => 104,
                BallKind::Ton => 105,
                BallKind::Tui => 106,
                BallKind::Uki => 107,
                BallKind::Magic { .. } => 116,
                BallKind::Hen => 117,
                BallKind::Box => 118,
                BallKind::Dark => 119,
                BallKind::Star { .. } => 120,
                BallKind::Plain => 22,
                BallKind::Target => 23,
                BallKind::Nie { .. } => 24,
                BallKind::Sai => 25,
                BallKind::Tate { .. } => 26,
                BallKind::Han { .. } => 27,
                BallKind::Kekkai { .. } => 28,
            },
        }
    }

    pub fn xy(&self) -> (f32, f32) {
        match self {
            Element::Base(e) => (e.x, e.y),
            Element::Core(e) => (e.x, e.y),
            Element::KekkaiField(e) => (e.x, e.y),
            Element::Laser(e) => (e.x, e.y),
            Element::StarPrepare(e) => (e.x, e.y),
            Element::Creature(e) => (e.x, e.y),
            Element::Wall(e) => (e.x, e.y),
            Element::HitsJump(e) => (e.x, e.y),
            Element::HitsDrop(e) => (e.x, e.y),
            Element::HitsDropFrames(e) => (e.x, e.y),
            Element::Ball(e) => (e.x, e.y),
            Element::Hit(e) => (e.x, e.y),
            Element::HitYari(e) => (e.inner.x, e.inner.y),
            Element::HitsHeal(e) => (e.x, e.y),
            Element::Bullet(e) => (e.x, e.y),
            Element::ShaRay(e) => (e.x, e.y),
            Element::HitsBomb(e) => (e.x, e.y),
            Element::UkiShot(e) => (e.x, e.y),
            Element::SearchShot(e) => (e.x, e.y),
            Element::GeiLink(_) => (0.0, 0.0),
            Element::HolyGhost(e) => (e.x, e.y),
            Element::SyouShield(e) => (e.x, e.y),
        }
    }

    #[cfg(feature = "phase-timing")]
    fn profile_bucket(&self) -> usize {
        match self {
            Element::Base(_) => 0,
            Element::Core(_) => 1,
            Element::KekkaiField(_) => 2,
            Element::Laser(_) => 3,
            Element::StarPrepare(_) => 4,
            Element::Creature(_) => 5,
            Element::Wall(_) => 6,
            Element::HitsJump(_) => 7,
            Element::HitsDrop(_) | Element::HitsDropFrames(_) => 8,
            Element::Ball(_) => 9,
            Element::Hit(_) | Element::HitYari(_) | Element::HitsHeal(_) => 10,
            Element::Bullet(_)
            | Element::ShaRay(_)
            | Element::HitsBomb(_)
            | Element::UkiShot(_)
            | Element::SearchShot(_) => 11,
            Element::GeiLink(_) | Element::HolyGhost(_) | Element::SyouShield(_) => 12,
        }
    }

    pub fn hit_test_point(&self, x: f32, y: f32) -> bool {
        #[cfg(feature = "phase-timing")]
        {
            prof::inc(&prof::HIT_TEST_CALLS);
            let caller = prof::CURRENT_CALLER.load(std::sync::atomic::Ordering::Relaxed) as usize;
            prof::inc(&prof::HIT_BY_CALLER[caller.min(10)]);
            let bucket = self.profile_bucket();
            prof::inc(&prof::HIT_TARGET_CALLS[bucket]);
            let hit = self.hit_test_point_inner(x, y);
            if hit {
                prof::inc(&prof::HIT_TARGET_HITS[bucket]);
            }
            hit
        }
        #[cfg(not(feature = "phase-timing"))]
        self.hit_test_point_inner(x, y)
    }

    fn hit_test_point_inner(&self, x: f32, y: f32) -> bool {
        match self {
            Element::Base(e) => e.hit_test_point(x, y),
            Element::Core(e) => e.hit_test_point(x, y),
            Element::KekkaiField(e) => e.hit_test_point(x, y),
            Element::Laser(e) => e.hit_test_point(x, y),
            Element::StarPrepare(e) => e.hit_test_point(x, y),
            Element::Creature(e) => e.hit_test_point(x, y),
            Element::Wall(e) => e.hit_test_point(x, y),
            Element::HitsJump(e) => e.hit_test_point(x, y),
            Element::HitsDrop(e) => e.hit_test_point(x, y),
            Element::HitsDropFrames(e) => e.hit_test_point(x, y),
            Element::Ball(e) => {
                if matches!(e.kind, BallKind::Tate { .. } | BallKind::Knight { .. }) {
                    e.tate_hit_test(x, y)
                } else {
                    crate::geom::Round::new(e.x, e.y, e.r).hit_test_point(x, y)
                }
            }
            Element::Hit(e) => e.hit_test_point(x, y),
            Element::HitYari(e) => e.hit_test_point(x, y),
            Element::HitsHeal(e) => e.hit_test_point(x, y),
            Element::Bullet(e) => crate::geom::Round::new(e.x, e.y, e.r).hit_test_point(x, y),
            Element::ShaRay(e) => e.hit_test_point(x, y),
            Element::HitsBomb(e) => e.hit_test_point(x, y),
            Element::UkiShot(e) => e.hit_test_point(x, y),
            Element::SearchShot(e) => e.hit_test_point(x, y),
            Element::GeiLink(e) => e.hit_test_point(x, y),
            Element::HolyGhost(e) => e.hit_test_point(x, y),
            Element::SyouShield(e) => e.hit_test_point(x, y),
        }
    }

    /// Java `Shape.xySync()`（不移动子形状）
    pub fn xy_sync_now(&mut self) {
        let (x, y) = self.xy();
        self.set_xy(jmath::xy_sync(x), jmath::xy_sync(y));
    }

    /// Java 直接字段写 `target.x = ...; target.y = ...`
    pub fn set_xy(&mut self, x: f32, y: f32) {
        match self {
            Element::Base(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Core(e) => {
                e.x = x;
                e.y = y;
            }
            Element::KekkaiField(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Laser(e) => {
                e.x = x;
                e.y = y;
            }
            Element::StarPrepare(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Creature(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Wall(e) => {
                e.x = x;
                e.y = y;
            }
            Element::HitsJump(e) => {
                e.x = x;
                e.y = y;
            }
            Element::HitsDrop(e) => {
                e.x = x;
                e.y = y;
            }
            Element::HitsDropFrames(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Ball(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Hit(e) => {
                e.x = x;
                e.y = y;
            }
            Element::HitYari(e) => {
                e.inner.x = x;
                e.inner.y = y;
            }
            Element::HitsHeal(e) => {
                e.x = x;
                e.y = y;
            }
            Element::Bullet(e) => {
                e.x = x;
                e.y = y;
            }
            Element::ShaRay(e) => {
                e.x = x;
                e.y = y;
            }
            Element::HitsBomb(e) => {
                e.x = x;
                e.y = y;
            }
            Element::UkiShot(e) => {
                e.x = x;
                e.y = y;
            }
            Element::SearchShot(e) => {
                e.x = x;
                e.y = y;
            }
            Element::GeiLink(_) => {}
            Element::HolyGhost(e) => {
                e.x = x;
                e.y = y;
            }
            Element::SyouShield(e) => {
                e.x = x;
                e.y = y;
            }
        }
    }

    fn step(&mut self, game: &mut GameTask) {
        #[cfg(feature = "phase-timing")]
        {
            let bucket = self.profile_bucket();
            prof::inc(&prof::STEP_BY_KIND[bucket]);
            let default_label: u64 = match bucket {
                3 => 6,  // Laser
                1 => 7,  // Core
                6 => 9,  // Wall
                11 => 8, // Bullet*
                _ => 10,
            };
            let saved = prof::set_caller(default_label);
            let t = std::time::Instant::now();
            self.step_inner(game);
            prof::add(
                &prof::STEP_NS_BY_KIND[bucket],
                t.elapsed().as_nanos() as u64,
            );
            prof::set_caller(saved);
        }
        #[cfg(not(feature = "phase-timing"))]
        self.step_inner(game)
    }

    fn step_inner(&mut self, game: &mut GameTask) {
        match self {
            Element::Base(e) => e.step(game),
            Element::Core(e) => e.step(game),
            Element::KekkaiField(e) => e.step(game),
            Element::Laser(e) => e.step(game),
            Element::StarPrepare(e) => e.step(game),
            Element::Creature(e) => e.step(game),
            Element::Wall(e) => e.step(game),
            Element::HitsJump(e) => e.step(game),
            Element::HitsDrop(e) => e.step(game),
            Element::HitsDropFrames(e) => e.step(game),
            Element::Ball(e) => e.step(game),
            Element::Hit(e) => e.step(game),
            Element::HitYari(e) => e.step(game),
            Element::HitsHeal(e) => e.step(game),
            Element::Bullet(e) => e.step(game),
            Element::ShaRay(e) => e.step(game),
            Element::HitsBomb(e) => e.step(game),
            Element::UkiShot(e) => e.step(game),
            Element::SearchShot(e) => e.step(game),
            Element::GeiLink(e) => e.step(game),
            Element::HolyGhost(e) => e.step(game),
            Element::SyouShield(e) => e.step(game),
        }
    }
}

/// 墙构造期的传感器注册（在元素 id 分配后执行）
enum WallReg {
    None,
    Snipe,
    Turn(bool),
    Axl(i32),
}

pub struct GameTask {
    pub next_id: i32,
    pub bounds: Vec<Bound>,
    /// 侧别 wall 列表整体 AABB（相对 Base；None = 不启用）
    pub wall_aabb: [Option<WallAabb>; 2],

    pub elements: Vec<Option<Element>>,
    /// 存活位图（每 id 1 bit）：update 循环按 64 位字整段跳过死槽，
    /// 避免子弹型对局后期每帧线性扫描数千个死 id（语义同原 `Vec<bool>` 逐项检查）。
    pub alive: Vec<u64>,
    pub end: bool,
    pub time: i32,
    pub mid: i32,
    pub norikomi_flg: bool,
    pub hp: [i32; 2],
    pub hp0_flg: [i32; 2],
    pub dokkan_flg: [bool; 2],
    pub saihai_cnt: [i32; 2],
    pub saihai_rot: [i32; 2],
    pub dead_last: [i32; 2],
    pub core_x: [f32; 2],
    pub core_y: [f32; 2],
    pub rng_seed: i32,
    pub base_id: [i32; 2],
    pub core_id: [i32; 2],
    pub wall: [Sensor; 2],
    pub unit: [UnitSensor; 2],
    pub shield: [Sensor; 2],
    pub atk: [AtkSensor; 2],
    pub heal: [Sensor; 2],
    pub repair: [Sensor; 2],
    pub jump_u: [Sensor; 2],
    pub jump_f: [Sensor; 2],
    pub snipe: [Sensor; 2],
    pub turn_ccw: [Sensor; 2],
    pub turn_cw: [Sensor; 2],
    pub kekkai_ids: [Vec<i32>; 2],
    pub kekkai_fields: [i32; 2],
}

impl Default for GameTask {
    fn default() -> Self {
        Self::new()
    }
}

impl GameTask {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            bounds: Vec::new(),
            wall_aabb: [None, None],
            elements: Vec::new(),
            alive: Vec::new(),
            end: false,
            time: 0,
            mid: 960,
            norikomi_flg: false,
            hp: [100, 100],
            hp0_flg: [0, 0],
            dokkan_flg: [false, false],
            saihai_cnt: [0, 0],
            saihai_rot: [0, 0],
            dead_last: [0, 0],
            core_x: [0.0, 0.0],
            core_y: [0.0, 0.0],
            rng_seed: rng::new_seed(),
            base_id: [-1, -1],
            core_id: [-1, -1],
            wall: std::array::from_fn(|_| Sensor::default()),
            unit: std::array::from_fn(|_| UnitSensor::default()),
            shield: std::array::from_fn(|_| Sensor::default()),
            atk: std::array::from_fn(|_| AtkSensor::default()),
            heal: std::array::from_fn(|_| Sensor::default()),
            repair: std::array::from_fn(|_| Sensor::default()),
            jump_u: std::array::from_fn(|_| Sensor::default()),
            jump_f: std::array::from_fn(|_| Sensor::default()),
            snipe: std::array::from_fn(|_| Sensor::default()),
            turn_ccw: std::array::from_fn(|_| Sensor::default()),
            turn_cw: std::array::from_fn(|_| Sensor::default()),
            kekkai_ids: std::array::from_fn(|_| Vec::new()),
            kekkai_fields: [-1, -1],
        }
    }

    // ===== 元素容器 =====

    fn push_element<F: FnOnce(i32) -> Element>(&mut self, make: F) -> i32 {
        #[cfg(feature = "phase-timing")]
        prof::inc(&prof::SPAWNS);
        let id = self.next_id;
        self.next_id += 1;
        let element = make(id);
        self.bounds.push(element_bound(&element));
        self.elements.push(Some(element));
        let idx = id as usize;
        if idx / 64 == self.alive.len() {
            self.alive.push(1);
        } else {
            self.alive[idx / 64] |= 1u64 << (idx % 64);
        }
        id
    }

    /// 粗筛：`!(d2 > r2)`（NaN 输入返回 true，语义与精确判定路径一致地回退）
    #[inline]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn bound_pass(&self, id: i32, x: f32, y: f32) -> bool {
        let b = &self.bounds[id as usize];
        let dx = x - b.x;
        let dy = y - b.y;
        let pass = !(dx * dx + dy * dy > b.r2);
        #[cfg(feature = "bounds-verify")]
        if !pass {
            if let Some(e) = self.elements[id as usize].as_ref() {
                if e.hit_test_point(x, y) {
                    panic!(
                        "bounds-verify false-negative id={id} bound=({}, {}, {}) query=({x}, {y})",
                        b.x, b.y, b.r2
                    );
                }
            }
        }
        pass
    }

    /// 元素坐标/尺寸变化后刷新包围圆
    #[inline]
    pub fn refresh_bound(&mut self, id: i32) {
        let bound = match self.elements[id as usize].as_ref() {
            Some(e) => element_bound(e),
            None => return,
        };
        self.bounds[id as usize] = bound;
    }

    pub fn kill_element(&mut self, id: i32) {
        if id >= 0 {
            self.alive[id as usize / 64] &= !(1u64 << (id as usize % 64));
        }
    }

    /// Java `elements.get(id) != null` 的存活判定
    #[inline]
    pub fn is_alive(&self, id: i32) -> bool {
        (self.alive[id as usize / 64] >> (id as usize % 64)) & 1 != 0
    }

    /// 原始访问：即使元素已 kill 也返回（Java 传感器列表持有引用，不看 map）
    pub fn raw(&self, id: i32) -> &Element {
        self.elements[id as usize]
            .as_ref()
            .expect("element slot missing")
    }

    /// Java `elements.get(id)`：已 kill 返回 null
    pub fn get(&self, id: i32) -> Option<&Element> {
        if self.is_alive(id) {
            self.elements[id as usize].as_ref()
        } else {
            None
        }
    }

    pub fn is_ball(&self, id: i32) -> bool {
        matches!(self.get(id), Some(Element::Ball(_)))
    }

    /// Java `AtkHitSystem.resign` 的分类：`Bullet || HitsNagi`
    pub fn is_bullet_or_nagi(&self, id: i32) -> bool {
        match self.get(id) {
            Some(Element::Hit(h)) => matches!(h.kind, HitKind::Nagi),
            Some(Element::Bullet(_)) => true,
            _ => false,
        }
    }

    pub fn is_target_ball(&self, id: i32) -> bool {
        matches!(self.get(id), Some(Element::Ball(b)) if matches!(b.kind, BallKind::Target))
    }

    pub fn is_live_kekkai_ball(&self, id: i32) -> bool {
        match self.get(id) {
            Some(Element::Ball(b)) => {
                matches!(b.kind, BallKind::Kekkai { .. })
                    && b.jump_flg == 0
                    && b.side == b.on_side
                    && b.hurt_time() <= 0
            }
            _ => false,
        }
    }

    pub fn kekkai_point(&self, id: i32) -> (f32, f32) {
        let Some(Element::Ball(b)) = self.get(id) else {
            unreachable!("kekkai ball missing")
        };
        (b.x + b.cos_rot * 33.0, b.y + b.sin_rot * 33.0)
    }

    /// HanBall 反制目标筛选：`gei_flg == 1` 的 Bullet
    pub fn is_bullet_gei1(&self, id: i32) -> bool {
        matches!(self.get(id), Some(Element::Bullet(b)) if b.gei_flg == 1)
    }

    /// HanBall 反制：调用目标的 `reflect(rot)`
    pub fn reflect_bullet(&mut self, id: i32, rot: i32) {
        let mut e = self.elements[id as usize].take().expect("bullet missing");
        if let Element::Bullet(b) = &mut e {
            b.reflect(self, rot);
        }
        self.elements[id as usize] = Some(e);
        self.refresh_bound(id);
    }

    // ===== 访问器 =====

    pub fn base(&self, side: usize) -> &Base {
        match self.raw(self.base_id[side]) {
            Element::Base(b) => b,
            _ => unreachable!("base slot is not Base"),
        }
    }

    /// Java `game.bases[side] != null`（`Base.kill` 置 -1）
    pub fn base_or_none(&self, side: usize) -> Option<&Base> {
        let id = self.base_id[side];
        if id < 0 {
            return None;
        }
        match self.elements[id as usize].as_ref() {
            Some(Element::Base(b)) => Some(b),
            _ => None,
        }
    }

    pub fn core(&self, side: usize) -> &Core {
        match self.raw(self.core_id[side]) {
            Element::Core(c) => c,
            _ => unreachable!("core slot is not Core"),
        }
    }

    pub fn core_position(&self, side: usize) -> (f32, f32) {
        let c = self.core(side);
        (c.x, c.y)
    }

    /// Java `fort[i]` = wall + unit（CompositeShape OR）
    pub fn fort_hit(&self, side: usize, x: f32, y: f32) -> bool {
        self.wall_hit(side, x, y) || self.unit[side].hit_test(self, x, y)
    }

    /// 跳过指定元素（step 中的自身）的 `fort` 命中
    pub fn fort_hit_skip(&self, side: usize, x: f32, y: f32, skip: i32) -> bool {
        self.wall_hit_skip(side, x, y, skip) || self.unit[side].hit_test_skip(self, x, y, skip)
    }

    /// 侧别 wall 列表整体 AABB（相对 Base）：`main_setup` 末尾构建一次。
    ///
    /// 仅当双方阵型都不含 BoxBall(44)/Elevator(60) 时启用——前者会新增 wall，
    /// 后者相对 Base 坐标不固定。成员半径取 `bounds` 的保守圆。
    fn build_wall_aabbs(&mut self, f1: &CompiledFort, f2: &CompiledFort) {
        let disabled = [f1, f2]
            .iter()
            .any(|f| f.unit_type.iter().any(|&t| t == 44 || t == 60));
        if disabled {
            return;
        }
        for side in 0..2 {
            self.wall_aabb[side] = self.compute_wall_aabb(side);
        }
    }

    /// 由当前 wall 列表成员（相对 Base）计算整体 AABB；任何成员放弃粗筛则退化为全空间
    fn compute_wall_aabb(&self, side: usize) -> Option<WallAabb> {
        let ids = &self.wall[side].ids;
        if ids.is_empty() {
            return None;
        }
        let base_pos = self.raw(ids[0]).xy();
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &id in ids {
            let bound = self.bounds[id as usize];
            if !bound.r2.is_finite() {
                return Some(WallAabb {
                    min_x: f32::NEG_INFINITY,
                    min_y: f32::NEG_INFINITY,
                    max_x: f32::INFINITY,
                    max_y: f32::INFINITY,
                });
            }
            let r = bound.r2.sqrt();
            let rx = bound.x - base_pos.0;
            let ry = bound.y - base_pos.1;
            min_x = min_x.min(rx - r);
            min_y = min_y.min(ry - r);
            max_x = max_x.max(rx + r);
            max_y = max_y.max(ry + r);
        }
        Some(WallAabb {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// wall 列表移除的唯一入口：删除后按存活成员收紧整体 AABB
    pub fn wall_remove(&mut self, side: usize, id: i32) {
        self.wall[side].remove(id);
        if self.wall_aabb[side].is_some() {
            self.wall_aabb[side] = self.compute_wall_aabb(side);
        }
    }

    /// AABB 门禁：返回 false 表示"整列表必不命中"（严格保守）
    #[inline]
    pub fn wall_aabb_pass(&self, side: usize, x: f32, y: f32) -> bool {
        let Some(aabb) = self.wall_aabb[side] else {
            return true;
        };
        // Base 永在 wall 列表首位；死亡后不再移动（d 视作 0）
        let Some(Element::Base(base)) = self.elements[self.wall[side].ids[0] as usize].as_ref()
        else {
            return true;
        };
        let (dx, dy) = if self.base_id[side] >= 0 {
            (base.base_move_x.abs(), base.base_move_y.abs())
        } else {
            (0.0, 0.0)
        };
        let min_x = base.x + aabb.min_x - dx;
        let max_x = base.x + aabb.max_x + dx;
        let min_y = base.y + aabb.min_y - dy;
        let max_y = base.y + aabb.max_y + dy;
        !(x < min_x || x > max_x || y < min_y || y > max_y)
    }

    /// `wall[side]` 点查询（敌方语义：带整体 AABB 门禁）
    #[inline]
    pub fn wall_hit(&self, side: usize, x: f32, y: f32) -> bool {
        if !self.wall_aabb_pass(side, x, y) {
            #[cfg(feature = "wall-aabb-verify")]
            {
                let exact = self.wall[side].hit_test(self, x, y);
                assert!(
                    !exact,
                    "wall-aabb-verify false reject side={side} query=({x},{y})"
                );
            }
            return false;
        }
        self.wall[side].hit_test(self, x, y)
    }

    /// 自家语义：不做 AABB 门禁（供 `land`/`jump` 的 on_side==side 路径）
    #[inline]
    pub fn wall_hit_own(&self, side: usize, x: f32, y: f32) -> bool {
        self.wall[side].hit_test(self, x, y)
    }

    /// `wall[side]` 点查询，返回第一个命中 id（敌方语义：带整体 AABB 门禁）
    #[inline]
    pub fn wall_hit_id(&self, side: usize, x: f32, y: f32) -> i32 {
        if !self.wall_aabb_pass(side, x, y) {
            return -1;
        }
        self.wall[side].hit_test_id(self, x, y)
    }

    /// 自家语义 + 命中 id（供 `land` 记录支撑墙）
    #[inline]
    pub fn wall_hit_own_id(&self, side: usize, x: f32, y: f32) -> i32 {
        self.wall[side].hit_test_id(self, x, y)
    }

    /// land 支撑墙提示的验证：h 必须存活且属于该侧 wall 列表（Wall/Base/Core 且 side 匹配），
    /// 然后做精确命中。任一条件不成立即返回 false（调用方落回全扫描，布尔结果不变）。
    #[inline]
    pub fn wall_hint_hit(&self, side: usize, h: i32, x: f32, y: f32) -> bool {
        if !self.is_alive(h) {
            return false;
        }
        match self.raw(h) {
            Element::Wall(w) if w.side == side => w.hit_test_point(x, y),
            Element::Base(b) if b.side == side => b.hit_test_point(x, y),
            Element::Core(c) if c.side == side => c.hit_test_point(x, y),
            _ => false,
        }
    }

    /// 跳过指定元素的 `wall[side]` 命中（敌方语义：带门禁）
    #[inline]
    pub fn wall_hit_skip(&self, side: usize, x: f32, y: f32, skip: i32) -> bool {
        if !self.wall_aabb_pass(side, x, y) {
            #[cfg(feature = "wall-aabb-verify")]
            {
                let exact = self.wall[side].hit_test_skip(self, x, y, skip);
                assert!(
                    !exact,
                    "wall-aabb-verify false reject(skip) side={side} query=({x},{y})"
                );
            }
            return false;
        }
        self.wall[side].hit_test_skip(self, x, y, skip)
    }

    /// Java `team[i]` = wall + unit + shield + atk
    pub fn team_hit(&self, side: usize, x: f32, y: f32) -> bool {
        self.wall_hit(side, x, y)
            || self.unit[side].hit_test(self, x, y)
            || self.shield[side].hit_test(self, x, y)
            || self.atk[side].hit_test(self, x, y)
    }

    /// AtkSensor 四角融合扫描：与"四角各调一次 `hit_test` 再 OR"布尔等价
    ///（命中判定是纯函数、OR 可交换；分区并集 = 四次调用会扫描的条目集合）。
    /// 每个条目只做一次粗盒判定：角 (x±k, y±k) 命中半径 r 的包围圆 ⇒
    /// |dx|≤k+r 且 |dy|≤k+r，违反即四角全拒（无假阴性；r2=inf/NaN 落入精确路径）。
    #[inline]
    pub fn atk_hit_4corner(&self, side: usize, x: f32, y: f32, k: f32) -> bool {
        let s = &self.atk[side];
        let mid = self.mid as f32;
        for &id in &s.common {
            if self.atk_box_pass(id, x, y, k) {
                return true;
            }
        }
        if y - k < 0.0 {
            if x - k <= mid {
                for &id in &s.upper_left {
                    if self.atk_box_pass(id, x, y, k) {
                        return true;
                    }
                }
            }
            if x + k > mid {
                for &id in &s.upper_right {
                    if self.atk_box_pass(id, x, y, k) {
                        return true;
                    }
                }
            }
        }
        if y + k >= 0.0 {
            if x - k <= mid {
                for &id in &s.left {
                    if self.atk_box_pass(id, x, y, k) {
                        return true;
                    }
                }
            }
            if x + k > mid {
                for &id in &s.right {
                    if self.atk_box_pass(id, x, y, k) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 粗盒判定 + 通过者的四角精确 OR（跨列表重复条目幂等，无需去重）
    #[inline]
    fn atk_box_pass(&self, id: i32, x: f32, y: f32, k: f32) -> bool {
        let b = &self.bounds[id as usize];
        let ax = (b.x - x).abs() - k;
        let ay = (b.y - y).abs() - k;
        if (ax > 0.0 && ax * ax > b.r2) || (ay > 0.0 && ay * ay > b.r2) {
            return false;
        }
        let e = self.raw(id);
        e.hit_test_point(x - k, y - k)
            || e.hit_test_point(x + k, y - k)
            || e.hit_test_point(x - k, y + k)
            || e.hit_test_point(x + k, y + k)
    }

    /// Core 的 13 点融合扫描（等价于 core.rs 原来的 13 次 `hit_test` OR），粗盒 k=40。
    pub fn atk_hit_core13(&self, side: usize, x: f32, y: f32) -> bool {
        let s = &self.atk[side];
        let mid = self.mid as f32;
        for &id in &s.common {
            if self.core13_box_pass(id, x, y) {
                return true;
            }
        }
        if y - 40.0 < 0.0 {
            if x - 40.0 <= mid {
                for &id in &s.upper_left {
                    if self.core13_box_pass(id, x, y) {
                        return true;
                    }
                }
            }
            if x + 40.0 > mid {
                for &id in &s.upper_right {
                    if self.core13_box_pass(id, x, y) {
                        return true;
                    }
                }
            }
        }
        if y + 40.0 >= 0.0 {
            if x - 40.0 <= mid {
                for &id in &s.left {
                    if self.core13_box_pass(id, x, y) {
                        return true;
                    }
                }
            }
            if x + 40.0 > mid {
                for &id in &s.right {
                    if self.core13_box_pass(id, x, y) {
                        return true;
                    }
                }
            }
        }
        false
    }

    #[inline]
    fn core13_box_pass(&self, id: i32, x: f32, y: f32) -> bool {
        let b = &self.bounds[id as usize];
        let ax = (b.x - x).abs() - 40.0;
        let ay = (b.y - y).abs() - 40.0;
        if (ax > 0.0 && ax * ax > b.r2) || (ay > 0.0 && ay * ay > b.r2) {
            return false;
        }
        let e = self.raw(id);
        e.hit_test_point(x - 40.0, y - 40.0)
            || e.hit_test_point(x + 40.0, y - 40.0)
            || e.hit_test_point(x - 40.0, y + 40.0)
            || e.hit_test_point(x + 40.0, y + 40.0)
            || e.hit_test_point(x - 20.0, y - 20.0)
            || e.hit_test_point(x, y - 20.0)
            || e.hit_test_point(x + 20.0, y - 20.0)
            || e.hit_test_point(x - 20.0, y + 20.0)
            || e.hit_test_point(x, y + 20.0)
            || e.hit_test_point(x + 20.0, y + 20.0)
            || e.hit_test_point(x - 20.0, y)
            || e.hit_test_point(x + 20.0, y)
            || e.hit_test_point(x, y)
    }

    pub fn spawn_boss_laser(&mut self, side: usize, x: f32, y: f32) {
        let id = self
            .push_element(|id| Element::Laser(Laser::new_boss(id, x, y, side, LaserKind::Boss)));
        self.atk[side].add(id);
    }

    pub fn spawn_boss_laser2(&mut self, side: usize, x: f32, y: f32) {
        let id = self
            .push_element(|id| Element::Laser(Laser::new_boss(id, x, y, side, LaserKind::Boss2)));
        self.atk[side].add(id);
    }

    pub fn spawn_magic_laser(&mut self, x: f32, y: f32, r_deg: i32, side: usize, user: i32) {
        let id =
            self.push_element(|id| Element::Laser(Laser::new_magic(id, x, y, r_deg, side, user)));
        self.atk[side].add(id);
    }

    pub fn spawn_star_laser(&mut self, x: f32, side: usize) {
        let id = self.push_element(|id| Element::Laser(Laser::new_star(id, x, side)));
        self.atk[side].add(id);
    }

    pub fn spawn_star_prepare(&mut self, x: f32, y: f32, r_deg: i32, side: usize, user: i32) {
        self.push_element(|id| {
            Element::StarPrepare(StarPrepareLaser::new(id, x, y, r_deg, side, user))
        });
    }

    pub fn spawn_creature(&mut self, x: f32, y: f32, side: usize, r_deg: i32, spawn_type: i32) {
        let kind = match spawn_type {
            2 | 3 => CreatureKind::Drill,
            4 => CreatureKind::Solid,
            5 => CreatureKind::Shock,
            _ => CreatureKind::Weak,
        };
        let id =
            self.push_element(|id| Element::Creature(Creature::new(id, x, y, side, r_deg, kind)));
        self.unit[side].add(id);
    }

    pub fn spawn_box(&mut self, x: f32, y: f32, wall_side: usize, box_on_side: i32) {
        let id = self.push_element(|id| {
            Element::Wall(Wall::new(
                id,
                x,
                y,
                wall_side,
                0,
                crate::engine::wall::box_kind(box_on_side),
            ))
        });
        self.wall[wall_side].add(id);
    }

    /// Java `DarkBall` 召唤 BoneBall 并覆写飞行参数
    pub fn spawn_bone_summon(
        &mut self,
        x: f32,
        y: f32,
        rot: i32,
        side: usize,
        target_on_side: usize,
    ) {
        let id = self.push_element(|id| {
            let mut b = Ball::new(id, x, y, rot, side, 31, BallKind::Bone);
            b.speed = 30;
            Element::Ball(b)
        });
        self.unit[side].add(id);
        let (c, sn) = (crate::trig::cos(rot), crate::trig::sin(rot));
        if let Some(Element::Ball(b)) = self.elements[id as usize].as_mut() {
            b.cnt = 0;
            b.jump_flg = 1;
            b.xs = c * 20.0;
            b.ys = sn * 20.0;
            b.on_side = target_on_side;
        }
    }

    /// `BossCore2.pickTargetId`
    pub fn pick_target_id_core2(&self, side: usize, next_index: &mut i32) -> i32 {
        let ids = &self.unit[side].ids;
        if ids.is_empty() {
            return -1;
        }
        if *next_index < 0 || *next_index >= ids.len() as i32 {
            *next_index = ids.len() as i32 - 1;
        }
        let mut target_index = *next_index;
        while target_index >= 0 {
            let id = ids[target_index as usize];
            let shape_id = self.shape_id_for_ops(id);
            if self.get(shape_id).is_some() {
                *next_index = target_index - 1;
                return shape_id;
            }
            target_index -= 1;
        }
        -1
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_boss_missile(
        &mut self,
        side: usize,
        x: f32,
        y: f32,
        target: i32,
        r1: f32,
        r2: f32,
        r3: f32,
        r4: f32,
        r5: f32,
    ) {
        let target_rot = jmath::round_f32(r1 * 360.0) as f32;
        let id = self.push_element(|id| {
            Element::Bullet(Bullet::new(
                id,
                x,
                y,
                side,
                BulletKind::BossMissile {
                    cnt: 0,
                    target_rot,
                    target_id: target,
                    rot: target_rot,
                    target_rot_delta: 0.0,
                },
            ))
        });
        self.atk[side].add(id);
        let mut delta = jmath::round_f32(r4 * 8.0) + 4;
        if jmath::round_f32(r5 * 2.0) != 0 {
            delta *= 4;
        } else {
            delta = (-delta) * 4;
        }
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.r = jf32!(9.3);
            b.xs = (jmath::round_f32(r2 * 16.0) - 8) as f32;
            b.ys = (jmath::round_f32(r3 * 10.0) - 16) as f32;
            b.gei_flg = 1;
            b.gravity = jf32!(0.2);
            if let BulletKind::BossMissile {
                target_rot_delta, ..
            } = &mut b.kind
            {
                *target_rot_delta = delta as f32;
            }
        }
    }

    /// Java `new HitsJump(game, x, y, parent)`：挂到指定传感器
    pub fn spawn_hits_jump(&mut self, x: f32, y: f32, parent: ParentList) {
        let id = self.push_element(|id| Element::HitsJump(HitsJump::new(id, x, y, parent)));
        self.list_add(parent, id);
    }

    /// Java `new HitsDropFrames(game, x, y, parent, frame, scale)`（构造器内先 xySync）
    pub fn spawn_hits_drop_frames(
        &mut self,
        x: f32,
        y: f32,
        parent: ParentList,
        frames: i32,
        scale: f32,
    ) {
        let id = self.push_element(|id| {
            let mut e = HitsDropFrames::new(id, x, y, parent, frames, scale);
            e.quantize();
            Element::HitsDropFrames(e)
        });
        self.list_add(parent, id);
    }

    pub fn list_add(&mut self, list: ParentList, id: i32) {
        match list {
            ParentList::Wall(s) => self.wall[s].add(id),
            ParentList::Unit(s) => self.unit[s].add(id),
            ParentList::Atk(s) => self.atk[s].add(id),
            ParentList::Shield(s) => self.shield[s].add(id),
            ParentList::Heal(s) => self.heal[s].add(id),
            ParentList::Repair(s) => self.repair[s].add(id),
            ParentList::JumpU(s) => self.jump_u[s].add(id),
            ParentList::JumpF(s) => self.jump_f[s].add(id),
        }
    }

    pub fn list_remove(&mut self, list: ParentList, id: i32) {
        match list {
            ParentList::Wall(s) => self.wall_remove(s, id),
            ParentList::Unit(s) => self.unit[s].remove(id),
            ParentList::Atk(s) => self.atk[s].remove(id),
            ParentList::Shield(s) => self.shield[s].remove(id),
            ParentList::Heal(s) => self.heal[s].remove(id),
            ParentList::Repair(s) => self.repair[s].remove(id),
            ParentList::JumpU(s) => self.jump_u[s].remove(id),
            ParentList::JumpF(s) => self.jump_f[s].remove(id),
        }
    }

    /// Jet/Turbo 构造/销毁时调整车板加速度
    pub fn base_axl_add(&mut self, side: usize, delta: i32) {
        let id = self.base_id[side];
        if id < 0 {
            return;
        }
        if let Some(Element::Base(b)) = self.elements[id as usize].as_mut() {
            b.axl += delta;
        }
    }

    /// 被 Base.step 边界弹飞逻辑调用：找到第一个 NieBall 并设 alarm=6
    pub fn nie_ball_alarm(&mut self, side: usize) -> bool {
        let ids = self.unit[side].ids.clone();
        for id in ids {
            // Java 的 instanceof + 字段写入，保持嵌套形状
            #[allow(clippy::collapsible_if)]
            if let Some(Element::Ball(b)) = self.elements[id as usize].as_mut() {
                if let BallKind::Nie { alarm } = &mut b.kind {
                    *alarm = 6;
                    return true;
                }
            }
        }
        false
    }

    // ===== 帧循环 =====

    pub fn run_single(
        f1: &CompiledFort,
        f2: &CompiledFort,
        max_frames: i32,
        random_seed: Option<i32>,
    ) -> (Self, Result) {
        Self::run_single_observed(f1, f2, max_frames, random_seed, None)
    }

    /// 同 `run_single`，但每帧结束后回调（供 L3 对拍；回调不得修改引擎状态）
    pub fn run_single_with<F: FnMut(&GameTask)>(
        f1: &CompiledFort,
        f2: &CompiledFort,
        max_frames: i32,
        random_seed: Option<i32>,
        on_frame: F,
    ) -> (Self, Result) {
        let mut callback = FrameCallback(on_frame);
        Self::run_single_observed(f1, f2, max_frames, random_seed, Some(&mut callback))
    }

    /// Java `run_single(f1, f2, maxFrames, randomSeed, observer)`：观测器与引擎分离借用
    pub fn run_single_observed(
        f1: &CompiledFort,
        f2: &CompiledFort,
        max_frames: i32,
        random_seed: Option<i32>,
        mut observer: Option<&mut dyn BattleObserver>,
    ) -> (Self, Result) {
        assert!(max_frames >= 0, "maxFrames 不能为负数");
        let mut g = Self::new();
        g.init();
        g.main_setup(f1, f2);
        if let Some(s) = random_seed {
            g.rng_seed = rng::normalize_seed(s as i64);
        }
        if let Some(o) = observer.as_mut() {
            o.on_battle_started(&g);
        }
        while g.time < max_frames && !g.end {
            g.time += 1;
            #[cfg(feature = "phase-timing")]
            let t_base_move = std::time::Instant::now();
            g.base_move();
            #[cfg(feature = "phase-timing")]
            prof::add(&prof::BASE_MOVE_NS, t_base_move.elapsed().as_nanos() as u64);
            #[cfg(feature = "phase-timing")]
            let t_judge = std::time::Instant::now();
            g.judge();
            #[cfg(feature = "phase-timing")]
            prof::add(&prof::JUDGE_NS, t_judge.elapsed().as_nanos() as u64);
            g.update();
            if let Some(o) = observer.as_mut() {
                o.on_frame_completed(&g, g.time);
            }
        }
        #[cfg(feature = "phase-timing")]
        {
            let alive: u64 = g.alive.iter().map(|w| w.count_ones() as u64).sum();
            prof::add(&prof::ALIVE_AT_END, alive);
        }
        let res = g.result();
        if let Some(o) = observer.as_mut() {
            o.on_battle_finished(&g, &res);
        }
        (g, res)
    }

    fn init(&mut self) {
        for i in 0..2 {
            let x = if i == 0 { 240.0 } else { 1680.0 };
            let id = self.push_element(|id| Element::Base(Base::new(id, x, 532.0, i)));
            self.base_id[i] = id;
            self.wall[i].add(id);
        }
    }

    fn main_setup(&mut self, f1: &CompiledFort, f2: &CompiledFort) {
        for (i, f) in [f1, f2].into_iter().enumerate() {
            self.rng_seed = self.rng_seed.wrapping_mul(seed_factor(f));
            let (cx, cy) = core_spawn(f, i);
            let kind = match f.core_type {
                1 => CoreKind::Boss { cnt: 0 },
                2 => CoreKind::Boss2 {
                    cnt: 0,
                    cnt2: 0,
                    next_start_target_index: -1,
                },
                _ => CoreKind::Normal,
            };
            let id =
                self.push_element(|id| Element::Core(Core::new(id, cx as f32, cy as f32, i, kind)));
            self.core_id[i] = id;
            self.core_x[i] = cx as f32;
            self.core_y[i] = cy as f32;
            self.wall[i].add(id);
            for k in 0..f.unit_count {
                let (ux, uy, rr) = unit_spawn(f, i, k);
                self.unit_make(i, ux, uy, rr, f.unit_type[k]);
            }
            let kid = self.push_element(|id| Element::KekkaiField(KekkaiField::new(id, i)));
            self.kekkai_fields[i] = kid;
            self.shield[i].add(kid);
        }
        self.build_wall_aabbs(f1, f2);
    }

    /// Java `new HitsBomb(...)`（无倍率）
    pub fn spawn_hits_bomb_plain(&mut self, x: f32, y: f32, side: usize) {
        let id = self.push_element(|id| Element::HitsBomb(HitsBomb::new_plain(id, x, y, side)));
        self.atk[side].add(id);
    }

    /// Java `ConBullet.pickTargetId`（读 `Shape.id`：UkiBullet 遮蔽后为 0）
    pub fn pick_target_id(&self, side: usize, next_index: &mut i32) -> i32 {
        let ids = &self.unit[side].ids;
        if ids.is_empty() {
            return -1;
        }
        if *next_index < 0 || *next_index >= ids.len() as i32 {
            *next_index = ids.len() as i32 - 1;
        }
        let id = ids[*next_index as usize];
        *next_index -= 1;
        // Java 读 `Shape.id`：SyouBullet 声明了私有 id，遮蔽后恒为 0
        match self.elements[id as usize].as_ref() {
            Some(Element::SyouShield(_)) => 0,
            _ => id,
        }
    }

    pub fn bullet_set_radius(&mut self, id: i32, r: f32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.r = r;
        }
        self.refresh_bound(id);
    }

    /// 按子弹种类里的 rot 字段重算速度（ConMissile/Missile/TuiMissile）
    pub fn bullet_update_velocity_torad(&mut self, id: i32, speed: f32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            let rad = match b.kind {
                BulletKind::ConMissile { rot, .. } | BulletKind::Missile { rot, .. } => {
                    crate::fdlibm::to_radians(rot as f64) as f32
                }
                BulletKind::TuiMissile {
                    cos_rot, sin_rot, ..
                } => {
                    b.xs = cos_rot * speed;
                    b.ys = sin_rot * speed;
                    return;
                }
                _ => return,
            };
            b.xs = (crate::fdlibm::cos(rad as f64) as f32) * speed;
            b.ys = (crate::fdlibm::sin(rad as f64) as f32) * speed;
        }
    }

    pub fn set_bullet_gei_flg(&mut self, id: i32, v: i32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.gei_flg = v;
        }
    }

    /// Java 直接读 `Shape.id` 的操作语义（SyouBullet 为 0，其余为真实 key）
    pub fn shape_id_for_ops(&self, id: i32) -> i32 {
        match self.elements[id as usize].as_ref() {
            Some(Element::SyouShield(_)) => 0,
            _ => id,
        }
    }

    pub fn element_set_xy(&mut self, id: i32, x: f32, y: f32) {
        if let Some(e) = self.elements[id as usize].as_mut() {
            e.set_xy(x, y);
        }
        self.refresh_bound(id);
    }

    pub fn element_xy_sync(&mut self, id: i32) {
        if let Some(e) = self.elements[id as usize].as_mut() {
            e.xy_sync_now();
        }
        self.refresh_bound(id);
    }

    /// Java `GeiBall.hurt`/`GeiBullet` 里对 user 的 `cnt=0; t_id=-1`
    pub fn ball_reset_gei(&mut self, id: i32) {
        if let Some(Element::Ball(b)) = self.elements[id as usize].as_mut() {
            b.cnt = 0;
            if let BallKind::Gei { t_id } = &mut b.kind {
                *t_id = -1;
            }
        }
    }

    pub fn ball_t_id(&self, id: i32) -> i32 {
        match self.elements[id as usize].as_ref() {
            Some(Element::Ball(b)) => match b.kind {
                BallKind::Gei { t_id } => t_id,
                _ => -1,
            },
            _ => -1,
        }
    }

    pub fn spawn_uki_shot(&mut self, x: f32, y: f32, side: usize, xs: f32, ys: f32) -> i32 {
        let id = self.push_element(|id| Element::UkiShot(UkiShot::new(id, x, y, side, xs, ys)));
        self.unit[side].add(id);
        id
    }

    pub fn spawn_search_shot(
        &mut self,
        x: f32,
        y: f32,
        side: usize,
        cos_rot: f32,
        sin_rot: f32,
    ) -> i32 {
        let id = self.push_element(|id| {
            Element::SearchShot(SearchShot::new(id, x, y, side, cos_rot, sin_rot))
        });
        self.atk[side].add(id);
        id
    }

    pub fn spawn_gei_link(&mut self, user_id: i32, target_id: i32) -> i32 {
        self.push_element(|id| Element::GeiLink(GeiLink::new(id, user_id, target_id)))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_holy_ghost(
        &mut self,
        x: f32,
        y: f32,
        t_type: i32,
        t_rot: i32,
        t_side: usize,
        t_on_side: usize,
        t_jump_flg: i32,
    ) -> i32 {
        self.push_element(|id| {
            Element::HolyGhost(HolyGhost::new(
                id, x, y, t_type, t_rot, t_side, t_on_side, t_jump_flg,
            ))
        })
    }

    pub fn spawn_syou_shield(
        &mut self,
        x: f32,
        y: f32,
        side: usize,
        cos_rot: f32,
        sin_rot: f32,
    ) -> i32 {
        self.push_element(|id| {
            Element::SyouShield(SyouShield::new(id, x, y, side, cos_rot, sin_rot))
        })
    }

    /// 子弹生成与链式设置（Wave C）
    pub fn spawn_bullet(&mut self, x: f32, y: f32, side: usize, kind: BulletKind) -> i32 {
        let id = self.push_element(|id| Element::Bullet(Bullet::new(id, x, y, side, kind)));
        self.atk[side].add(id);
        id
    }

    pub fn bullet_set_vec_mult(&mut self, id: i32, vx: f32, vy: f32, mult: f32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.xs = vx * mult;
            b.ys = vy * mult;
            b.power = mult;
        }
    }

    pub fn bullet_set_vec(&mut self, id: i32, vx: f32, vy: f32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.xs = vx;
            b.ys = vy;
        }
    }

    pub fn bullet_set_vec_r(&mut self, id: i32, r: i32, speed: f32) {
        let (cos, sin) = (crate::trig::cos(r), crate::trig::sin(r));
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.xs = cos * speed;
            b.ys = sin * speed;
            b.power = speed;
        }
    }

    pub fn bullet_set_gravity(&mut self, id: i32, g: f32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.gravity = g;
        }
    }

    pub fn bullet_set_power(&mut self, id: i32, p: f32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.power = p;
        }
    }

    /// Java 链式 `.move()`：生成当帧立刻移动一次
    pub fn bullet_move_now(&mut self, id: i32) {
        if let Some(Element::Bullet(b)) = self.elements[id as usize].as_mut() {
            b.move_bullet();
        }
        self.refresh_bound(id);
    }

    pub fn spawn_sha_ray(&mut self, x: f32, y: f32, side: usize, xs: f32, ys: f32, frames: i32) {
        self.push_element(|id| Element::ShaRay(ShaRay::new(id, x, y, side, xs, ys, frames)));
    }

    pub fn spawn_hits_bomb(&mut self, x: f32, y: f32, side: usize, mult: f32) {
        let id = self.push_element(|id| Element::HitsBomb(HitsBomb::new(id, x, y, side, mult)));
        self.atk[side].add(id);
    }

    /// 近战命中体生成（Wave B）
    pub fn spawn_hits_ken(
        &mut self,
        x: f32,
        y: f32,
        rot: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) {
        let id = self.push_element(|id| {
            Element::Hit(Hit::new_ken(id, x, y, rot, side, user, cos_rot, sin_rot))
        });
        self.atk[side].add(id);
    }

    pub fn spawn_hits_nagi(
        &mut self,
        x: f32,
        y: f32,
        rot: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) {
        let id = self.push_element(|id| {
            Element::Hit(Hit::new_nagi(id, x, y, rot, side, user, cos_rot, sin_rot))
        });
        self.atk[side].add(id);
    }

    pub fn spawn_hits_bone(
        &mut self,
        x: f32,
        y: f32,
        rot: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) {
        let id = self.push_element(|id| {
            Element::Hit(Hit::new_bone(id, x, y, rot, side, user, cos_rot, sin_rot))
        });
        self.atk[side].add(id);
    }

    pub fn spawn_hits_knight(
        &mut self,
        x: f32,
        y: f32,
        rot: i32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) {
        let id = self.push_element(|id| {
            Element::Hit(Hit::new_knight(id, x, y, rot, side, user, cos_rot, sin_rot))
        });
        self.atk[side].add(id);
    }

    pub fn spawn_hits_kabe(
        &mut self,
        x: f32,
        y: f32,
        side: usize,
        user: i32,
        cos_rot: f32,
        sin_rot: f32,
    ) {
        let id = self
            .push_element(|id| Element::Hit(Hit::new_kabe(id, x, y, side, user, cos_rot, sin_rot)));
        self.shield[side].add(id);
    }

    pub fn spawn_hit_yari(&mut self, x: f32, y: f32, rot: i32, side: usize, user: i32) {
        let id = self.push_element(|id| Element::HitYari(HitYari::new(id, x, y, rot, side, user)));
        self.atk[side].add(id);
    }

    pub fn spawn_hits_heal(&mut self, x: f32, y: f32, parent: ParentList, radius: f32) {
        let id = self.push_element(|id| Element::HitsHeal(HitsHeal::new(id, x, y, parent, radius)));
        self.list_add(parent, id);
    }

    /// Java `new HitsDrop(game, x, y, parent)`（构造器内先 xySync）
    pub fn spawn_hits_drop(&mut self, x: f32, y: f32, parent: ParentList) {
        let id = self.push_element(|id| {
            let mut e = HitsDrop::new(id, x, y, parent);
            e.quantize();
            Element::HitsDrop(e)
        });
        self.list_add(parent, id);
    }

    pub(crate) fn unit_make(&mut self, side: usize, x: f32, y: f32, r: i32, type_: i32) -> i32 {
        if let Some(kind) = crate::engine::wall::kind_for_type(type_, x, side) {
            let reg = match &kind {
                WallKind::Snipe => WallReg::Snipe,
                WallKind::Wide { mode, .. } | WallKind::Narrow { mode, .. } => WallReg::Turn(*mode),
                WallKind::Jet => WallReg::Axl(1),
                WallKind::Turbo => WallReg::Axl(2),
                _ => WallReg::None,
            };
            let id = self.push_element(|id| Element::Wall(Wall::new(id, x, y, side, type_, kind)));
            self.wall[side].add(id);
            match reg {
                WallReg::Snipe => self.snipe[side].add(id),
                WallReg::Turn(true) => self.turn_cw[side].add(id),
                WallReg::Turn(false) => self.turn_ccw[side].add(id),
                WallReg::Axl(n) => self.base_axl_add(side, n),
                WallReg::None => {}
            }
            return id;
        }
        let ball_kind = match type_ {
            0 => Some(BallKind::Plain),
            4 => Some(BallKind::Tate {
                flipped: false,
                hurt_time: 0,
            }),
            24 => Some(BallKind::Kekkai { hurt_time: 0 }),
            38 => Some(BallKind::Han { t_id: -1 }),
            41 => Some(BallKind::Nie { alarm: -1 }),
            42 => Some(BallKind::Target),
            47 => Some(BallKind::Sai),
            3 => Some(BallKind::Sword),
            8 => Some(BallKind::Yari),
            10 => Some(BallKind::Nagi),
            17 => Some(BallKind::Repair),
            18 => Some(BallKind::Heal),
            19 => Some(BallKind::Kabe {
                shooting: false,
                unit_x: x as i32,
            }),
            1 => Some(BallKind::Bow),
            2 => Some(BallKind::Gun),
            7 => Some(BallKind::Dokyu),
            9 => Some(BallKind::Cannon),
            12 => Some(BallKind::Retsu),
            13 => Some(BallKind::Shotgun),
            14 => Some(BallKind::Sniper),
            22 => Some(BallKind::Minigun),
            5 => Some(BallKind::Bomb),
            11 => Some(BallKind::Hane),
            15 => Some(BallKind::Uki),
            16 => Some(BallKind::Guide),
            20 => Some(BallKind::Tobi),
            21 => Some(BallKind::Sen),
            32 => Some(BallKind::Geki),
            33 => Some(BallKind::Ton),
            34 => Some(BallKind::Holy),
            35 => Some(BallKind::Nin),
            36 => Some(BallKind::Syou),
            37 => Some(BallKind::Hana),
            39 => Some(BallKind::Push),
            40 => Some(BallKind::Gei { t_id: -1 }),
            43 => Some(BallKind::Tui),
            46 => Some(BallKind::Heri),
            49 => Some(BallKind::Kaku),
            50 => Some(BallKind::Sha),
            52 => Some(BallKind::Con),
            53 => Some(BallKind::Kan),
            54 => Some(BallKind::Search),
            6 => Some(BallKind::Magic { shooting: false }),
            23 => Some(BallKind::Hen),
            44 => Some(BallKind::Box),
            45 => Some(BallKind::Dark),
            51 => Some(BallKind::Star { shooting: false }),
            31 => Some(BallKind::Bone),
            48 => Some(BallKind::Knight {
                flipped: false,
                hurt_time: 0,
            }),
            _ => None,
        };
        if let Some(kind) = ball_kind {
            let init = BallInit::for_kind(&kind, r, side);
            let id = self.push_element(|id| {
                let mut b = Ball::new(id, x, y, r, side, type_, kind);
                b.hp = init.hp;
                b.max_hp = init.hp;
                b.speed = init.speed;
                match &mut b.kind {
                    BallKind::Tate { flipped, .. } | BallKind::Knight { flipped, .. } => {
                        *flipped = init.flipped;
                    }
                    _ => {}
                }
                Element::Ball(b)
            });
            self.unit[side].add(id);
            if matches!(self.raw(id), Element::Ball(b) if matches!(b.kind, BallKind::Kekkai { .. }))
            {
                self.kekkai_ids[side].push(id);
            }
            // Java `unit_make` 尾段：Ball 的 cnt 初值（float 取模；speed==0 时为 NaN→0）
            let (bx, bside, bs) = {
                let Element::Ball(b) = self.raw(id) else {
                    unreachable!()
                };
                (b.x, b.side, b.speed)
            };
            let speed_f = bs as f32;
            let cnt = if bside != 0 {
                (-(380.0 - (bx - 1490.0)) % speed_f) as i32
            } else {
                (-(bx - 50.0) % speed_f) as i32
            };
            if let Some(Element::Ball(b)) = self.elements[id as usize].as_mut() {
                b.cnt = cnt;
            }
            return id;
        }
        panic!("unit_make 未实现类型 {type_}（Wave A 落地的兵玉之外仍待后续波次）");
    }

    fn base_move(&mut self) {
        self.dokkan_flg[0] = false;
        self.dokkan_flg[1] = false;
        self.norikomi_flg = false;
        self.saihai_cnt[0] -= 1;
        self.saihai_cnt[1] -= 1;
        for i in 0..2 {
            if self.hp0_flg[i] > 0 {
                continue;
            }
            let (cx, cy) = self.core_position(i);
            self.core_x[i] = cx;
            self.core_y[i] = cy;
            if let Some((tx, ty)) = self.find_first_target_ball(i) {
                self.core_x[i] = tx;
                self.core_y[i] = ty;
            }
        }
        if self.hp0_flg[0] == 0 && self.hp0_flg[1] == 0 {
            let b1x = self.base(1).x;
            let b1xs = self.base(1).xs;
            let b0x = self.base(0).x;
            let b0xs = self.base(0).xs;
            if b1x - b1xs - (b0x + b0xs) <= 380.0 {
                self.norikomi_flg = true;
                for i in 0..2 {
                    if self.nie_ball_alarm(i) {
                        continue;
                    }
                    let opp_xs = self.base(1 - i).xs;
                    let d = ((opp_xs * 5.0) as f64).floor() + 1.0;
                    let mut wrk = d as f32;
                    wrk = jmath::round_f32(wrk) as f32;
                    if wrk < 0.0 {
                        wrk = 0.0;
                    }
                    self.hp[i] -= wrk as i32;
                    self.dokkan_flg[i] = true;
                }
                let wrk = self.base(0).xs;
                let mut b0_new = -self.base(1).xs;
                if b0_new > -1.0 {
                    b0_new = -1.0;
                }
                let mut b1_new = -wrk;
                if b1_new > -1.0 {
                    b1_new = -1.0;
                }
                {
                    let mut b0 = self.take_base(0);
                    if let Element::Base(b) = &mut b0 {
                        b.xs = b0_new;
                        b.ys = -b0_new * 2.0;
                    }
                    self.put_back(b0);
                }
                {
                    let mut b1 = self.take_base(1);
                    if let Element::Base(b) = &mut b1 {
                        b.xs = b1_new;
                        b.ys = -b1_new * 2.0;
                    }
                    self.put_back(b1);
                }
            }
        }
    }

    fn find_first_target_ball(&self, side: usize) -> Option<(f32, f32)> {
        for &id in &self.unit[side].ids {
            if self.is_target_ball(id) {
                return Some(self.raw(id).xy());
            }
        }
        None
    }

    fn take_base(&mut self, side: usize) -> Element {
        let id = self.base_id[side];
        self.elements[id as usize].take().expect("base missing")
    }

    fn put_back(&mut self, e: Element) {
        let id = e.id();
        self.elements[id as usize] = Some(e);
    }

    fn judge(&mut self) {
        for i in 0..2 {
            if self.hp0_flg[i] == 0 && self.hp[i] < 1 {
                self.hp0_flg[i] = 1;
                self.dokkan_flg[i] = true;
            }
            if self.hp0_flg[i] > 0 {
                self.hp0_flg[i] += 1;
                self.dokkan_flg[i] = true;
                if self.hp0_flg[i] == 3 {
                    self.kill_base(i);
                    self.kill_core(i);
                }
                if self.hp0_flg[i] > 120 {
                    self.hp[0] = self.hp[0].max(0);
                    self.hp[1] = self.hp[1].max(0);
                    self.end = true;
                    break;
                }
            }
        }
    }

    fn kill_base(&mut self, side: usize) {
        let id = self.base_id[side];
        if id < 0 {
            return;
        }
        let mut e = self.elements[id as usize].take().expect("base missing");
        if let Element::Base(b) = &mut e {
            b.kill(self);
        }
        self.elements[id as usize] = Some(e);
    }

    fn kill_core(&mut self, side: usize) {
        let id = self.core_id[side];
        if id < 0 {
            return;
        }
        let mut e = self.elements[id as usize].take().expect("core missing");
        if let Element::Core(c) = &mut e {
            c.kill(self);
        }
        self.elements[id as usize] = Some(e);
    }

    fn update(&mut self) {
        if self.hp0_flg[0] == 0 && self.hp0_flg[1] == 0 {
            let v = (self.base(0).x + self.base(0).xs + self.base(1).x - self.base(1).xs) * 0.5;
            self.mid = v as i32;
        }
        #[cfg(feature = "phase-timing")]
        let t_resign = std::time::Instant::now();
        for i in 0..2 {
            let mut u = std::mem::take(&mut self.unit[i]);
            u.resign(self);
            self.unit[i] = u;
            let mut a = std::mem::take(&mut self.atk[i]);
            a.resign(self);
            self.atk[i] = a;
        }
        #[cfg(feature = "phase-timing")]
        prof::add(&prof::RESIGN_NS, t_resign.elapsed().as_nanos() as u64);
        #[cfg(feature = "phase-timing")]
        let t_step = std::time::Instant::now();
        // 帧首快照：本帧新建元素不 step（Tier-0）。位图整字跳过死槽；
        // 单元素处理前仍逐位重查（同帧早些时候被 kill 的元素必须跳过，与原语义一致）。
        let to = self.next_id as usize;
        let last_bits = match to % 64 {
            0 => u64::MAX,
            r => (1u64 << r) - 1,
        };
        for w in 0..to.div_ceil(64) {
            let mut bits = self.alive[w];
            if w == to / 64 {
                bits &= last_bits;
            }
            while bits != 0 {
                let i = w * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if !self.is_alive(i as i32) {
                    continue;
                }
                #[cfg(feature = "phase-timing")]
                prof::inc(&prof::STEP_COUNT);
                let mut e = self.elements[i].take().expect("element missing");
                e.step(self);
                self.elements[i] = Some(e);
                self.refresh_bound(i as i32);
            }
        }
        #[cfg(feature = "phase-timing")]
        prof::add(&prof::STEP_NS, t_step.elapsed().as_nanos() as u64);
    }

    pub fn result(&self) -> Result {
        let mut status = -1;
        let mut winner_hp = -1;
        if self.hp0_flg[0] > 0 || self.hp0_flg[1] > 0 {
            if self.hp0_flg[0] == 0 {
                status = 1;
                winner_hp = self.hp[0];
            } else if self.hp0_flg[1] == 0 {
                status = 2;
                winner_hp = self.hp[1];
            } else {
                status = 0;
            }
        }
        Result {
            status,
            winner_hp,
            frame_passed: self.time,
        }
    }
}
