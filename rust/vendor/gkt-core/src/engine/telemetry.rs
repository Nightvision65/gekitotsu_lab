//! 遥测（Java `org.example.training.*`）：事件流 + 快照 + 聚合统计。
//!
//! 语义唯一规格：oracle `BattleTraceRecorder.java`（含 `BattleEvent`/`BattleFrameSnapshot`/
//! `BattleSummary`/`BattleTelemetry`/`TelemetryMode`）。只读观测，绝不修改引擎状态。
//! 事件顺序、HP 差分口径、死亡帧仍记最后一次 HP 变化等边界逐行对应。

use std::collections::HashSet;

use super::{Ball, Element, GameTask, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryMode {
    Off,
    Events,
    EventsAndSnapshots,
}

/// Java `BattleEventType` 的 ordinal 必须一致（金样按 ordinal 落盘）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BattleEventType {
    CoreDamage = 0,
    CoreRepair = 1,
    FortCollision = 2,
    UnitDamage = 3,
    UnitHeal = 4,
    UnitSpawn = 5,
    UnitDeath = 6,
    UnitLaunch = 7,
    UnitLand = 8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BattleEvent {
    pub frame: i32,
    pub event_type: BattleEventType,
    pub side: i32,
    pub related_side: i32,
    pub entity_id: i32,
    pub unit_type: i32,
    pub x: f32,
    pub y: f32,
    pub amount: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BattleFrameSnapshot {
    pub frame: i32,
    pub p1_hp: i32,
    pub p2_hp: i32,
    pub p1_alive_units: i32,
    pub p2_alive_units: i32,
    pub p1_projectiles: i32,
    pub p2_projectiles: i32,
    pub p1_base_alive: bool,
    pub p2_base_alive: bool,
    pub p1_base_x: f32,
    pub p2_base_x: f32,
    pub p1_base_speed: f32,
    pub p2_base_speed: f32,
    pub p1_core_x: f32,
    pub p1_core_y: f32,
    pub p2_core_x: f32,
    pub p2_core_y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BattleSummary {
    pub first_p1_core_damage_frame: i32,
    pub first_p2_core_damage_frame: i32,
    pub first_collision_frame: i32,
    pub collision_count: i32,
    pub p1_core_damage_taken: i32,
    pub p2_core_damage_taken: i32,
    pub p1_core_repair: i32,
    pub p2_core_repair: i32,
    pub p1_unit_damage_taken: i32,
    pub p2_unit_damage_taken: i32,
    pub p1_unit_healing: i32,
    pub p2_unit_healing: i32,
    pub p1_unit_spawns: i32,
    pub p2_unit_spawns: i32,
    pub p1_unit_deaths: i32,
    pub p2_unit_deaths: i32,
    pub p1_launches: i32,
    pub p2_launches: i32,
}

impl BattleSummary {
    /// 字段顺序与 Java record 一致（金样逐项对拍用）
    pub fn as_array(&self) -> [i32; 18] {
        [
            self.first_p1_core_damage_frame,
            self.first_p2_core_damage_frame,
            self.first_collision_frame,
            self.collision_count,
            self.p1_core_damage_taken,
            self.p2_core_damage_taken,
            self.p1_core_repair,
            self.p2_core_repair,
            self.p1_unit_damage_taken,
            self.p2_unit_damage_taken,
            self.p1_unit_healing,
            self.p2_unit_healing,
            self.p1_unit_spawns,
            self.p2_unit_spawns,
            self.p1_unit_deaths,
            self.p2_unit_deaths,
            self.p1_launches,
            self.p2_launches,
        ]
    }
}

#[derive(Clone, Debug, Default)]
pub struct BattleTelemetry {
    pub summary: BattleSummary,
    pub events: Vec<BattleEvent>,
    pub snapshots: Vec<BattleFrameSnapshot>,
}

/// Java `BattleObserver`：回调发生在初始化+种子写入后、每帧完整更新后、结果生成后
pub trait BattleObserver {
    fn on_battle_started(&mut self, _game: &GameTask) {}
    fn on_frame_completed(&mut self, _game: &GameTask, _frame: i32) {}
    fn on_battle_finished(&mut self, _game: &GameTask, _result: &Result) {}
}

/// 把只读帧回调适配成 observer（L3 对拍）
pub(crate) struct FrameCallback<F>(pub F);

impl<F: FnMut(&GameTask)> BattleObserver for FrameCallback<F> {
    fn on_frame_completed(&mut self, game: &GameTask, _frame: i32) {
        (self.0)(game)
    }
}

struct TrackedUnit {
    id: i32,
    side: i32,
    hp: i32,
    jump_flag: i32,
    alive: bool,
}

/// Java `BattleTraceRecorder`：只读帧末状态，不介入任何碰撞或单位逻辑
pub struct BattleRecorder {
    mode: TelemetryMode,
    snapshot_interval: i32,
    events: Vec<BattleEvent>,
    snapshots: Vec<BattleFrameSnapshot>,
    units: Vec<TrackedUnit>,
    seen: HashSet<i32>,

    previous_p1_hp: i32,
    previous_p2_hp: i32,
    first_p1_core_damage_frame: i32,
    first_p2_core_damage_frame: i32,
    first_collision_frame: i32,
    collision_count: i32,
    p1_core_damage_taken: i32,
    p2_core_damage_taken: i32,
    p1_core_repair: i32,
    p2_core_repair: i32,
    p1_unit_damage_taken: i32,
    p2_unit_damage_taken: i32,
    p1_unit_healing: i32,
    p2_unit_healing: i32,
    p1_unit_spawns: i32,
    p2_unit_spawns: i32,
    p1_unit_deaths: i32,
    p2_unit_deaths: i32,
    p1_launches: i32,
    p2_launches: i32,
}

impl BattleRecorder {
    pub fn new(mode: TelemetryMode, snapshot_interval: i32) -> Self {
        assert!(
            mode != TelemetryMode::EventsAndSnapshots || snapshot_interval > 0,
            "启用快照时 snapshotInterval 必须大于 0"
        );
        Self {
            mode,
            snapshot_interval,
            events: Vec::new(),
            snapshots: Vec::new(),
            units: Vec::new(),
            seen: HashSet::new(),
            previous_p1_hp: 0,
            previous_p2_hp: 0,
            first_p1_core_damage_frame: -1,
            first_p2_core_damage_frame: -1,
            first_collision_frame: -1,
            collision_count: 0,
            p1_core_damage_taken: 0,
            p2_core_damage_taken: 0,
            p1_core_repair: 0,
            p2_core_repair: 0,
            p1_unit_damage_taken: 0,
            p2_unit_damage_taken: 0,
            p1_unit_healing: 0,
            p2_unit_healing: 0,
            p1_unit_spawns: 0,
            p2_unit_spawns: 0,
            p1_unit_deaths: 0,
            p2_unit_deaths: 0,
            p1_launches: 0,
            p2_launches: 0,
        }
    }

    pub fn mode(&self) -> TelemetryMode {
        self.mode
    }

    pub fn telemetry(&self) -> BattleTelemetry {
        BattleTelemetry {
            summary: self.summary(),
            events: self.events.clone(),
            snapshots: self.snapshots.clone(),
        }
    }

    pub fn into_telemetry(self) -> BattleTelemetry {
        BattleTelemetry {
            summary: self.summary(),
            events: self.events,
            snapshots: self.snapshots,
        }
    }

    pub fn summary(&self) -> BattleSummary {
        BattleSummary {
            first_p1_core_damage_frame: self.first_p1_core_damage_frame,
            first_p2_core_damage_frame: self.first_p2_core_damage_frame,
            first_collision_frame: self.first_collision_frame,
            collision_count: self.collision_count,
            p1_core_damage_taken: self.p1_core_damage_taken,
            p2_core_damage_taken: self.p2_core_damage_taken,
            p1_core_repair: self.p1_core_repair,
            p2_core_repair: self.p2_core_repair,
            p1_unit_damage_taken: self.p1_unit_damage_taken,
            p2_unit_damage_taken: self.p2_unit_damage_taken,
            p1_unit_healing: self.p1_unit_healing,
            p2_unit_healing: self.p2_unit_healing,
            p1_unit_spawns: self.p1_unit_spawns,
            p2_unit_spawns: self.p2_unit_spawns,
            p1_unit_deaths: self.p1_unit_deaths,
            p2_unit_deaths: self.p2_unit_deaths,
            p1_launches: self.p1_launches,
            p2_launches: self.p2_launches,
        }
    }

    fn record_core_hp_changes(&mut self, game: &GameTask, frame: i32) {
        let previous = [self.previous_p1_hp, self.previous_p2_hp];
        for (side, &prev) in previous.iter().enumerate() {
            let delta = game.hp[side] - prev;
            if delta < 0 {
                let amount = -delta;
                if side == 0 {
                    self.p1_core_damage_taken += amount;
                    if self.first_p1_core_damage_frame < 0 {
                        self.first_p1_core_damage_frame = frame;
                    }
                } else {
                    self.p2_core_damage_taken += amount;
                    if self.first_p2_core_damage_frame < 0 {
                        self.first_p2_core_damage_frame = frame;
                    }
                }
                let event = core_event(game, frame, BattleEventType::CoreDamage, side, amount);
                self.events.push(event);
            } else if delta > 0 {
                if side == 0 {
                    self.p1_core_repair += delta;
                } else {
                    self.p2_core_repair += delta;
                }
                let event = core_event(game, frame, BattleEventType::CoreRepair, side, delta);
                self.events.push(event);
            }
        }
        self.previous_p1_hp = game.hp[0];
        self.previous_p2_hp = game.hp[1];
    }

    fn discover_units(&mut self, game: &GameTask, frame: i32, record_spawn: bool) {
        for side in 0..2 {
            for &id in &game.unit[side].ids {
                // Java 遍历单位传感器持有的对象引用（不查 elements map）
                let Some(Element::Ball(ball)) = game.elements[id as usize].as_ref() else {
                    continue;
                };
                if !self.seen.insert(ball.id) {
                    continue;
                }
                self.units.push(TrackedUnit {
                    id: ball.id,
                    side: ball.side as i32,
                    hp: ball.hp,
                    jump_flag: ball.jump_flg,
                    alive: true,
                });
                if record_spawn {
                    if ball.side == 0 {
                        self.p1_unit_spawns += 1;
                    } else {
                        self.p2_unit_spawns += 1;
                    }
                    let event = unit_event(frame, BattleEventType::UnitSpawn, ball, 0);
                    self.events.push(event);
                }
            }
        }
    }

    fn record_unit_changes(&mut self, game: &GameTask, frame: i32) {
        for index in 0..self.units.len() {
            let id = self.units[index].id;
            // Java `game.elements.get(ball.id) == ball`：id 单调不复用，存活标记即等价
            let alive = game.get(id).is_some();
            let Some(Element::Ball(ball)) = game.elements[id as usize].as_ref() else {
                continue;
            };
            if alive {
                self.record_unit_hp_change(index, ball, frame);
                if self.units[index].jump_flag != 1 && ball.jump_flg == 1 {
                    if ball.side == 0 {
                        self.p1_launches += 1;
                    } else {
                        self.p2_launches += 1;
                    }
                    let event = unit_event(frame, BattleEventType::UnitLaunch, ball, 0);
                    self.events.push(event);
                } else if self.units[index].jump_flag == 1 && ball.jump_flg != 1 {
                    let event = unit_event(frame, BattleEventType::UnitLand, ball, 0);
                    self.events.push(event);
                }
                self.units[index].hp = ball.hp;
                self.units[index].jump_flag = ball.jump_flg;
            } else if self.units[index].alive {
                // 单位可能在本帧受到致命伤后立即从 elements 中移除，仍需记录最后一次 HP 变化
                self.record_unit_hp_change(index, ball, frame);
                self.units[index].hp = ball.hp;
                self.units[index].jump_flag = ball.jump_flg;
                self.units[index].alive = false;
                if ball.side == 0 {
                    self.p1_unit_deaths += 1;
                } else {
                    self.p2_unit_deaths += 1;
                }
                let event = unit_event(frame, BattleEventType::UnitDeath, ball, 0);
                self.events.push(event);
            }
        }
    }

    fn record_unit_hp_change(&mut self, index: usize, ball: &Ball, frame: i32) {
        let hp_delta = ball.hp - self.units[index].hp;
        if hp_delta < 0 {
            let amount = -hp_delta;
            if ball.side == 0 {
                self.p1_unit_damage_taken += amount;
            } else {
                self.p2_unit_damage_taken += amount;
            }
            let event = unit_event(frame, BattleEventType::UnitDamage, ball, amount);
            self.events.push(event);
        } else if hp_delta > 0 {
            if ball.side == 0 {
                self.p1_unit_healing += hp_delta;
            } else {
                self.p2_unit_healing += hp_delta;
            }
            let event = unit_event(frame, BattleEventType::UnitHeal, ball, hp_delta);
            self.events.push(event);
        }
    }

    fn snapshot(&self, game: &GameTask, frame: i32) -> BattleFrameSnapshot {
        let mut p1_alive = 0;
        let mut p2_alive = 0;
        for tracked in &self.units {
            if tracked.alive {
                if tracked.side == 0 {
                    p1_alive += 1;
                } else {
                    p2_alive += 1;
                }
            }
        }
        let p1_base = game.base_or_none(0);
        let p2_base = game.base_or_none(1);
        BattleFrameSnapshot {
            frame,
            p1_hp: game.hp[0],
            p2_hp: game.hp[1],
            p1_alive_units: p1_alive,
            p2_alive_units: p2_alive,
            p1_projectiles: game.atk[0].ids.len() as i32,
            p2_projectiles: game.atk[1].ids.len() as i32,
            p1_base_alive: p1_base.is_some(),
            p2_base_alive: p2_base.is_some(),
            p1_base_x: p1_base.map_or(0.0, |b| b.x),
            p2_base_x: p2_base.map_or(0.0, |b| b.x),
            p1_base_speed: p1_base.map_or(0.0, |b| b.xs),
            p2_base_speed: p2_base.map_or(0.0, |b| b.xs),
            p1_core_x: game.core_x[0],
            p1_core_y: game.core_y[0],
            p2_core_x: game.core_x[1],
            p2_core_y: game.core_y[1],
        }
    }
}

impl BattleObserver for BattleRecorder {
    fn on_battle_started(&mut self, game: &GameTask) {
        if self.mode == TelemetryMode::Off {
            return;
        }
        self.previous_p1_hp = game.hp[0];
        self.previous_p2_hp = game.hp[1];
        self.discover_units(game, 0, false);
        if self.mode == TelemetryMode::EventsAndSnapshots {
            let snapshot = self.snapshot(game, 0);
            self.snapshots.push(snapshot);
        }
    }

    fn on_frame_completed(&mut self, game: &GameTask, frame: i32) {
        if self.mode == TelemetryMode::Off {
            return;
        }
        self.record_core_hp_changes(game, frame);
        if game.norikomi_flg {
            if self.first_collision_frame < 0 {
                self.first_collision_frame = frame;
            }
            self.collision_count += 1;
            self.events.push(BattleEvent {
                frame,
                event_type: BattleEventType::FortCollision,
                side: -1,
                related_side: -1,
                entity_id: -1,
                unit_type: -1,
                x: (game.core_x[0] + game.core_x[1]) * 0.5,
                y: (game.core_y[0] + game.core_y[1]) * 0.5,
                amount: 0,
            });
        }
        self.discover_units(game, frame, true);
        self.record_unit_changes(game, frame);
        if self.mode == TelemetryMode::EventsAndSnapshots && frame % self.snapshot_interval == 0 {
            let snapshot = self.snapshot(game, frame);
            self.snapshots.push(snapshot);
        }
    }

    fn on_battle_finished(&mut self, game: &GameTask, result: &Result) {
        if self.mode != TelemetryMode::EventsAndSnapshots {
            return;
        }
        if self.snapshots.is_empty() || self.snapshots.last().unwrap().frame != result.frame_passed
        {
            let snapshot = self.snapshot(game, result.frame_passed);
            self.snapshots.push(snapshot);
        }
    }
}

fn core_event(
    game: &GameTask,
    frame: i32,
    event_type: BattleEventType,
    side: usize,
    amount: i32,
) -> BattleEvent {
    let id = if game.core_id[side] >= 0 {
        game.core_id[side]
    } else {
        -1
    };
    BattleEvent {
        frame,
        event_type,
        side: side as i32,
        related_side: side as i32,
        entity_id: id,
        unit_type: -1,
        x: game.core_x[side],
        y: game.core_y[side],
        amount,
    }
}

fn unit_event(frame: i32, event_type: BattleEventType, ball: &Ball, amount: i32) -> BattleEvent {
    BattleEvent {
        frame,
        event_type,
        side: ball.side as i32,
        related_side: ball.on_side as i32,
        entity_id: ball.id,
        unit_type: ball.type_,
        x: ball.x,
        y: ball.y,
        amount,
    }
}
