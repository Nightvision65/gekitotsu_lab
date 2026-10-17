//! 两种模式共享的帧末生命周期观察，不改变 RNG 或对战状态。
use super::engine::decode::compile_fort;
use super::engine::engine::{BattleObserver, Element, GameTask, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize,Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    pub p1: String,
    pub p2: String,
    #[serde(default = "frames")]
    pub max_frames: i32,
    pub seed: Option<i32>,
    #[serde(default = "interval")]
    pub snapshot_interval: i32,
    #[serde(default = "health_window")]
    pub health_recovery_window_frames: i32,
    #[serde(default = "enabled")]
    pub record_half_hp:bool,
    #[serde(default="basic_mode")]
    pub mode:String,
    #[serde(default="flight_interval")]
    pub flight_sample_interval:i32,
    #[serde(default="detail_limit")]
    pub max_detailed_events:usize,
    #[serde(default)]
    pub plain: bool,
}
fn frames() -> i32 { 65536 }
fn interval() -> i32 { 120 }
fn health_window() -> i32 { 30 }
fn enabled()->bool {true}
fn basic_mode()->String {"basic".into()}
fn flight_interval()->i32 {6}
fn detail_limit()->usize {500000}

#[derive(Clone)]
struct State { id: i32, side: usize, type_id: i32, hp: i32, max_hp: i32,
    x: f32, y: f32, jump: i32, on_side: usize, alive: bool, category: &'static str,
    cnt: Option<i32>, speed: Option<i32> }

// 只观察HP区间变化；短暂跌破半血后迅速恢复的区间不输出半血事件。
struct HealthEvent {frame:i32, kind:&'static str, state:State, detail:Value}
#[derive(Default)]
struct HealthTracker {
    below_half:bool, pending_half:Option<(i32,State)>, reported_half:bool,
    half_episodes:u32, suppressed_half_episodes:u32,
}
impl HealthTracker {
    fn confirm_half(&mut self, frame:i32, reason:&str, out:&mut Vec<HealthEvent>) {
        if let Some((start,s))=self.pending_half.take() {
            self.half_episodes+=1; self.reported_half=true;
            out.push(HealthEvent {frame:start,kind:"half_hp",state:s,detail:json!({
                "episode":self.half_episodes,"threshold":"hp < 0.5 * initial maxHp",
                "confirmedFrame":frame,"confirmation":reason,"durationObservedFrames":frame-start})});
        }
    }
    fn observe(&mut self, previous:&State, current:&State, frame:i32, window:i32) -> Vec<HealthEvent> {
        let mut out=Vec::new();
        let below=window>=0 && current.hp*2<current.max_hp;
        if below && !self.below_half {self.pending_half=Some((frame,current.clone()));}
        if !below && self.below_half {
            if let Some((start,_))=self.pending_half.as_ref() {
                if frame-*start<=window && window>0 {
                    self.pending_half=None;self.suppressed_half_episodes+=1;
                } else {self.confirm_half(frame,"recovered_after_window",&mut out);}
            }
            if self.reported_half {
                out.push(HealthEvent {frame,kind:"recovered_half_hp",state:current.clone(),
                    detail:json!({"episode":self.half_episodes,"threshold":"hp >= 0.5 * initial maxHp"})});
                self.reported_half=false;
            }
        }
        if below && self.pending_half.as_ref().is_some_and(|(start,_)|window==0 || frame-*start>window || !current.alive) {
            self.confirm_half(frame,if current.alive {"persisted_beyond_window"} else {"removed_below_half"},&mut out);
        }
        let _=previous;
        self.below_half=below;
        out
    }
    fn finish(&mut self, frame:i32) -> Vec<HealthEvent> {
        let mut out=Vec::new();self.confirm_half(frame,"battle_ended_below_half",&mut out);out
    }
}
fn core_world(g:&GameTask, side:usize) -> (f32,f32) {
    if g.time==0 {let c=g.core(side);let b=g.base(side);(b.x+c.unit_x,b.y+c.unit_y)}
    else {
        let core=if g.core_id[side]>=0 {g.elements[g.core_id[side] as usize].as_ref()}
            else {g.elements.iter().filter_map(|e|e.as_ref()).find(|e|matches!(e,Element::Core(c) if c.side==side))};
        match core {Some(Element::Core(c))=>(c.x,c.y),_=>(g.core_x[side],g.core_y[side])}
    }
}
fn state(g: &GameTask, id: i32) -> Option<State> {
    let (side, type_id, hp, max_hp, x, y, jump, on_side, category, cnt, speed) = match g.elements.get(id as usize)?.as_ref()? {
        Element::Ball(b) => (b.side, b.type_, b.hp, b.max_hp, b.x, b.y, b.jump_flg, b.on_side, "ball", Some(b.cnt), Some(b.speed)),
        Element::Wall(w) => (w.side, w.type_, w.hp, w.max_hp, w.x, w.y, 0, w.side, "wall", None, None),
        Element::Core(c) => (c.side, -1, g.hp[c.side], 100, core_world(g,c.side).0, core_world(g,c.side).1, 0, c.side, "core", None, None),
        _ => return None,
    };
    Some(State {id, side, type_id, hp, max_hp, x, y, jump, on_side,
        alive: g.is_alive(id), category, cnt, speed})
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Unit {
    entity_id: i32, side: usize, slot: Option<usize>, type_id: i32, category: String,
    spawned: bool, birth_frame: i32, initial_hp: i32, max_hp: i32, initial_x: f32,
    initial_y: f32, initial_counter: Option<i32>, attack_cycle_base: Option<i32>,
    first_half_frame: Option<i32>, first_quarter_frame: Option<i32>, recovered_half_frame: Option<i32>,
    first_half_observed_frame: Option<i32>, first_full_recovery_frame: Option<i32>,
    half_hp_episodes:u32, full_hp_recoveries:u32, suppressed_half_hp_episodes:u32,
    death_frame: Option<i32>, first_launch_frame: Option<i32>, first_land_frame: Option<i32>,
    hp_at_death:Option<i32>,final_hp_adjustment:i32,
    first_board_frame: Option<i32>, launches: u32, landings: u32,
    net_damage_observed: i64, net_healing_observed: i64, final_hp: i32, alive: bool,
    final_x: f32, final_y: f32,
    #[serde(skip)] previous: State,
    #[serde(skip)] active_flight: Option<usize>,
    #[serde(skip)] health:HealthTracker,
    #[serde(skip)] last_hp_delta:i32,
}
impl Unit {
    fn new(s: State, slot: Option<usize>, frame: i32, type_id: i32) -> Self {
        Self {entity_id:s.id, side:s.side, slot, type_id, category:s.category.into(), spawned:slot.is_none(),
            birth_frame:frame, initial_hp:s.hp, max_hp:s.max_hp, initial_x:s.x, initial_y:s.y,
            initial_counter:s.cnt, attack_cycle_base:s.speed, first_half_frame:None, first_quarter_frame:None,
            recovered_half_frame:None, death_frame:None, first_launch_frame:None, first_land_frame:None,
            hp_at_death:None,final_hp_adjustment:0,
            first_half_observed_frame:None,first_full_recovery_frame:None,half_hp_episodes:0,full_hp_recoveries:0,suppressed_half_hp_episodes:0,
            first_board_frame:None, launches:0, landings:0, net_damage_observed:0, net_healing_observed:0,
            final_hp:s.hp, alive:s.alive, final_x:s.x, final_y:s.y, previous:s,active_flight:None,health:HealthTracker::default(),last_hp_delta:0}
    }
    fn health_events(&mut self, events:&[HealthEvent]) {
        for event in events {match event.kind {
            "half_hp"=>{self.first_half_frame.get_or_insert(event.frame);},
            "recovered_half_hp"=>{self.recovered_half_frame.get_or_insert(event.frame);},
            "recovered_full_hp"=>{self.first_full_recovery_frame.get_or_insert(event.frame);}, _=>{}
        }}
        self.half_hp_episodes=self.health.half_episodes;
        self.suppressed_half_hp_episodes=self.health.suppressed_half_episodes;
    }
}

pub(crate) struct Recorder {
    units: Vec<Unit>, active_units:Vec<usize>, events: Vec<Value>, snapshots: Vec<Value>, flights: Vec<Value>,
    slots: [Vec<i32>;2], scanned: i32, interval: i32, collisions: u32,
    first_collision: Option<i32>, base_alive: [bool;2], initial_seed: i32, dropped_events: u32,
    base_axl: [i32;2],
    health_window:i32,
    record_half_hp:bool,
    flight_sample_interval:Option<i32>,flight_samples:Vec<Value>,last_flight_sample:std::collections::HashMap<i32,(f32,f32)>,
}
fn surface(s:&State) -> &'static str {
    match s.jump {2=>"ground",1=>"airborne",0=>if s.on_side==s.side {"own_platform"} else {"enemy_platform"},_=>"unknown"}
}
fn flight_start(g:&GameTask, s:&State, slot:Option<usize>, type_id:i32, frame:i32, origin:&str) -> Value {
    json!({"entityId":s.id,"side":s.side,"slot":slot,"typeId":type_id,"launchFrame":frame,"launchX":s.x,"launchY":s.y,
        "originSurface":origin,"landFrame":null,"endFrame":null,"endReason":"in_flight","destinationSurface":null,
        "endX":null,"endY":null,"ownCoreRelativeX":null,"ownCoreRelativeY":null,"enemyCoreRelativeX":null,"enemyCoreRelativeY":null,
        "launchOwnCoreRelativeX":s.x-core_world(g,s.side).0,"launchEnemyCoreRelativeX":s.x-core_world(g,1-s.side).0})
}
fn flight_end(f:&mut Value, g:&GameTask, s:&State, frame:i32, reason:&str, landed:bool) {
    f["landFrame"]=if landed {json!(frame)} else {Value::Null};
    f["endFrame"]=json!(frame);f["durationFrames"]=json!(frame-f["launchFrame"].as_i64().unwrap() as i32);
    f["endReason"]=json!(reason);f["destinationSurface"]=if landed {json!(surface(s))} else {Value::Null};
    f["endX"]=json!(s.x);f["endY"]=json!(s.y);f["aliveAtEnd"]=json!(s.alive);
    f["ownCoreRelativeX"]=json!(s.x-core_world(g,s.side).0);f["ownCoreRelativeY"]=json!(s.y-core_world(g,s.side).1);
    f["enemyCoreRelativeX"]=json!(s.x-core_world(g,1-s.side).0);f["enemyCoreRelativeY"]=json!(s.y-core_world(g,1-s.side).1);
    f["ownBaseRelativeX"]=g.base_or_none(s.side).map_or(Value::Null,|b|json!(s.x-b.x));
    f["enemyBaseRelativeX"]=g.base_or_none(1-s.side).map_or(Value::Null,|b|json!(s.x-b.x));
}
impl Recorder {
    fn event(&mut self, frame:i32, kind:&str, s:&State, slot:Option<usize>, extra:Value) {
        if self.events.len() >= 20000 { self.dropped_events += 1; return; }
        self.events.push(json!({"frame":frame,"kind":kind,"side":s.side,"slot":slot,
            "entityId":s.id,"typeId":slot.map_or(s.type_id,|k|self.slots[s.side][k]),"x":s.x,"y":s.y,"hp":s.hp,"maxHp":s.max_hp,"detail":extra}));
    }
    fn discover(&mut self, g:&GameTask, frame:i32, initial:bool) {
        let mut indices = [0usize;2];
        for id in self.scanned..g.next_id {
            let Some(s) = state(g,id) else { continue; };
            let slot = if initial { let k=indices[s.side]; indices[s.side]+=1; Some(k) } else {None};
            let type_id=slot.map_or(s.type_id, |k| self.slots[s.side][k]);
            if !initial { self.event(frame,"spawn",&s,slot,json!({"typeId":type_id,"category":s.category,"parentEntityId":null})); }
            let mut unit=Unit::new(s.clone(),slot,frame,type_id);
            if s.category=="ball" && s.jump==1 {
                unit.active_flight=Some(self.flights.len());
                self.flights.push(flight_start(g,&s,slot,type_id,frame,"born_airborne"));
            }
            self.active_units.push(self.units.len());self.units.push(unit);
        }
        if initial { assert_eq!(indices,[self.slots[0].len(),self.slots[1].len()],"初始实体槽位映射不完整"); }
        self.scanned=g.next_id;
    }
    fn snapshot(&mut self, g:&GameTask, frame:i32) {
        let sides:Vec<Value>=(0..2).map(|side| {
            let base=g.base_or_none(side);
            json!({"side":side,"coreHp":g.hp[side],"coreX":core_world(g,side).0,"coreY":core_world(g,side).1,
                "corePositionSource":if g.time==0 {"base_plus_core_offset"} else {"entity_world"},
                "coreTargetX":if g.time==0 {core_world(g,side).0} else {g.core_x[side]},"coreTargetY":if g.time==0 {core_world(g,side).1} else {g.core_y[side]},
                "baseAlive":base.is_some(),"baseX":base.map(|b|b.x),"baseY":base.map(|b|b.y),"baseSpeed":base.map(|b|b.xs),"baseAxl":base.map(|b|b.axl),
                "aliveBalls":self.units.iter().filter(|u|u.side==side&&u.alive&&u.category=="ball").count(),
                "aliveWalls":self.units.iter().filter(|u|u.side==side&&u.alive&&u.category=="wall").count(),
                "airborneBalls":self.units.iter().filter(|u|u.side==side&&u.alive&&u.previous.jump==1).count(),
                "activeAttackObjects":g.atk[side].ids.len()})
        }).collect();
        self.snapshots.push(json!({"frame":frame,"sides":sides}));
    }
}
impl BattleObserver for Recorder {
    fn on_battle_started(&mut self,g:&GameTask) {
        self.initial_seed=g.rng_seed;
        self.base_axl=[g.base(0).axl,g.base(1).axl];
        self.discover(g,0,true); self.snapshot(g,0);
    }
    fn on_frame_completed(&mut self,g:&GameTask,frame:i32) {
        self.discover(g,frame,false);
        for active_index in 0..self.active_units.len() {
            let unit_index=self.active_units[active_index];let id=self.units[unit_index].entity_id;
            let Some(s)=state(g,id) else {continue};
            let mut pending:Vec<(&str,Value)>=Vec::new();
            let u=&mut self.units[unit_index];
            if !u.previous.alive && u.death_frame.is_some() {continue;}
            let delta=s.hp-u.previous.hp;
            u.last_hp_delta=delta;
            if delta<0 {u.net_damage_observed-=delta as i64;} else {u.net_healing_observed+=delta as i64;}
            if u.category=="core" && delta<0 && u.net_damage_observed==-(delta as i64) {
                pending.push(("core_first_damage",json!({"amount":-delta})));
            }
            if self.record_half_hp && s.hp*2 < s.max_hp {u.first_half_observed_frame.get_or_insert(frame);}
            let health_events=u.health.observe(&u.previous,&s,frame,if self.record_half_hp {self.health_window} else {-1});
            u.health_events(&health_events);
            if self.record_half_hp && s.hp*4 < s.max_hp && u.first_quarter_frame.is_none() {
                u.first_quarter_frame=Some(frame); pending.push(("quarter_hp",json!({})));
            }
            if u.previous.jump!=1 && s.jump==1 {u.launches+=1; u.first_launch_frame.get_or_insert(frame);
                u.active_flight=Some(self.flights.len());
                self.flights.push(flight_start(g,&s,u.slot,u.type_id,frame,surface(&u.previous)));
                pending.push(("launch",json!({"velocityX":match g.raw(id) {Element::Ball(b)=>b.xs,_=>0.0},
                    "velocityY":match g.raw(id) {Element::Ball(b)=>b.ys,_=>0.0},"destinationSide":s.on_side,"originSurface":surface(&u.previous)})));}
            if u.previous.jump==1 && s.jump!=1 {u.landings+=1; u.first_land_frame.get_or_insert(frame);
                if let Some(index)=u.active_flight.take() {flight_end(&mut self.flights[index],g,&s,frame,if s.alive {"landed"} else {"landed_then_removed_same_frame"},true);}
                pending.push(("land",json!({"onSide":s.on_side,"jumpFlag":s.jump,"surface":surface(&s),
                    "enemyCoreRelativeX":s.x-core_world(g,1-s.side).0,"enemyCoreRelativeY":s.y-core_world(g,1-s.side).1})));}
            if s.category=="ball" && s.on_side!=s.side && s.jump==0 && u.first_board_frame.is_none() && s.alive {
                u.first_board_frame=Some(frame); pending.push(("board_enemy",json!({"onSide":s.on_side})));}
            if !s.alive && u.death_frame.is_none() {u.death_frame=Some(frame);u.hp_at_death=Some(s.hp);
                if let Some(index)=u.active_flight.take() {flight_end(&mut self.flights[index],g,&s,frame,"removed_airborne",false);}
                pending.push(("death",json!({"observation":if s.hp<=0 {"hp_depleted"} else {"removed_with_hp"},"attackerEntityId":null})));}
            let slot=u.slot;
            u.final_hp=s.hp; u.alive=s.alive; u.final_x=s.x; u.final_y=s.y; u.previous=s.clone();
            for e in health_events {self.event(e.frame,e.kind,&e.state,slot,e.detail);}
            for (kind,extra) in pending {self.event(frame,kind,&s,slot,extra);}
            if let Some(interval)=self.flight_sample_interval {
                if s.category=="ball" && s.jump==1 && s.alive && frame%interval==0 {
                    let changed=self.last_flight_sample.get(&id).is_none_or(|&(x,y)|(s.x-x).abs()>=0.05||(s.y-y).abs()>=0.05);
                    if changed {self.flight_samples.push(json!({"frame":frame,"side":s.side,"slot":slot,"entityId":id,"typeId":self.units[unit_index].type_id,
                        "x":s.x,"y":s.y,"surface":"airborne","enemyCoreRelativeX":s.x-core_world(g,1-s.side).0,"enemyCoreRelativeY":s.y-core_world(g,1-s.side).1}));
                        self.last_flight_sample.insert(id,(s.x,s.y));}
                } else if s.jump!=1 {self.last_flight_sample.remove(&id);}
            }
        }
        self.active_units.retain(|&index|self.units[index].alive);
        for recovery in &g.full_recoveries {
            let Some(index)=self.units.iter().position(|u|u.entity_id==recovery.entity_id) else {continue};
            let u=&mut self.units[index];u.full_hp_recoveries+=1;u.first_full_recovery_frame.get_or_insert(frame);
            let slot=u.slot;let count=u.full_hp_recoveries;
            if let Some(mut s)=state(g,recovery.entity_id) {
                s.hp=recovery.hp;s.max_hp=recovery.max_hp;s.x=recovery.x;s.y=recovery.y;
                self.event(frame,"recovered_full_hp",&s,slot,json!({"recovery":count,"fromHp":recovery.before,"healerEntityId":null,
                    "targetCategory":s.category,"observation":"healing-write-point","netHpUnchanged":self.units[index].last_hp_delta==0}));
            }
        }
        if g.norikomi_flg { self.collisions+=1;
            if self.first_collision.is_none() {self.first_collision=Some(frame);
                self.events.push(json!({"frame":frame,"kind":"fort_collision","side":null,"slot":null,"entityId":null}));}}
        for side in 0..2 {
            let alive=g.base_or_none(side).is_some();
            if let Some(base)=g.base_or_none(side) {if base.axl!=self.base_axl[side] {
                self.events.push(json!({"frame":frame,"kind":"base_acceleration_changed","side":side,"slot":null,"entityId":g.base_id[side],"detail":{"fromAxl":self.base_axl[side],"toAxl":base.axl}}));
                self.base_axl[side]=base.axl;
            }}
            if self.base_alive[side] && !alive {self.events.push(json!({"frame":frame,"kind":"base_destroyed","side":side,"slot":null,"entityId":g.base_id[side]}));}
            self.base_alive[side]=alive;
        }
        if frame%self.interval==0 {self.snapshot(g,frame);}
    }
    fn on_battle_finished(&mut self,g:&GameTask,r:&Result) {
        let mut health_events=Vec::new();
        for unit in &mut self.units {
            if unit.category=="core" {unit.final_hp_adjustment=g.hp[unit.side]-unit.final_hp;unit.final_hp=g.hp[unit.side];}
            let events=unit.health.finish(r.frame_passed);unit.health_events(&events);
            for e in events {health_events.push((unit.slot,e));}
            if let Some(index)=unit.active_flight.take() {flight_end(&mut self.flights[index],g,&unit.previous,r.frame_passed,"battle_ended_airborne",false);}
        }
        for (slot,e) in health_events {self.event(e.frame,e.kind,&e.state,slot,e.detail);}
        self.events.sort_by_key(|e|e["frame"].as_i64().unwrap_or(0));
        if self.snapshots.last().and_then(|x|x["frame"].as_i64())!=Some(r.frame_passed as i64) {self.snapshot(g,r.frame_passed);}
    }
}

fn validate(code:&str) -> std::result::Result<(),String> {
    const ALPHABET:&str="0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if code.len()<6 || code.len()%6!=0 || code.len()>6*512 || !matches!(code.as_bytes()[0],b'0'|b'1'|b'2') {
        return Err("阵码长度/核心不合法；只接受纯码，最多512槽位".into());
    }
    for (slot,chunk) in code.as_bytes().chunks(6).enumerate() {
        if !ALPHABET.as_bytes().contains(&chunk[0]) || (slot>0 && !ALPHABET.as_bytes()[1..61].contains(&chunk[0])) {return Err("单位ID不合法".into());}
        if chunk[1..].iter().any(|b|!ALPHABET.as_bytes()[..61].contains(b)) {return Err("坐标必须用base61编码，不能使用Z".into());}
        let v=super::engine::decode::to_xyr(&chunk[1..]);
        let (xmin,xmax,ymin,ymax)=if slot==0 {(52,328,58,334)} else {(16,364,20,369)};
        if v[0]<xmin||v[0]>xmax||v[1]<ymin||v[1]>ymax||v[2]>=360 {return Err(format!("slot {slot} 坐标/角度越界"));}
    }
    Ok(())
}
pub(crate) fn run() -> std::result::Result<(),Box<dyn std::error::Error>> {
    let args:Vec<String>=std::env::args().collect();
    if args.len()!=3 {return Err("用法：gkt-observe request.json output.json".into());}
    let req:Request=serde_json::from_slice(&std::fs::read(&args[1])?)?;
    if !super::supports_mode(&req.mode) {return Err("模式与程序不匹配；basic用gkt-observe，detailed用gkt-detail".into());}
    if req.mode=="detailed" && req.plain {return Err("详细模式不能关闭观察".into());}
    validate(&req.p1)?; validate(&req.p2)?;
    if !(0..=1_000_000).contains(&req.max_frames)|| !(1..=65536).contains(&req.snapshot_interval)|| !(0..=65536).contains(&req.health_recovery_window_frames) {return Err("帧数/快照间隔/治疗过滤窗口越界".into());}
    if !(1..=65536).contains(&req.flight_sample_interval)|| !(100..=5000000).contains(&req.max_detailed_events) {return Err("详细事件上限/轨迹采样间隔越界".into());}
    if req.plain {let out=super::plain_output(&req);return write_output(&out,&args[2]);}
    let f1=compile_fort("1P",&req.p1); let f2=compile_fort("2P",&req.p2);
    let slots=|f:&super::engine::decode::CompiledFort| std::iter::once(match f.core_type {1=>61,2=>62,_=>0}).chain(f.unit_type.iter().copied()).collect();
    let mut rec=Recorder {units:Vec::new(),active_units:Vec::new(),events:Vec::new(),snapshots:Vec::new(),flights:Vec::new(),slots:[slots(&f1),slots(&f2)],
        scanned:0,interval:req.snapshot_interval,collisions:0,first_collision:None,base_alive:[true,true],initial_seed:0,dropped_events:0,base_axl:[1,1],health_window:req.health_recovery_window_frames,record_half_hp:req.record_half_hp,
        flight_sample_interval:if req.mode=="detailed" {Some(req.flight_sample_interval)} else {None},flight_samples:Vec::new(),last_flight_sample:std::collections::HashMap::new()};
    let (game,r)=super::execute(&req,&f1,&f2,&mut rec);
    let mut out=json!({"schemaVersion":"gkt-ai-battle-v1","engine":"vendored-rust-149e365-catalog-hp-v1","coordinateSpace":"battle-world","mode":req.mode,
        "sideConvention":"0=1P,1=2P; slot0=core; spawned slot=null", "observation":"frame-end + exact full-recovery signals",
        "healthEventPolicy":"recovery-episodes-v1",
        "request":{"p1":req.p1,"p2":req.p2,"maxFrames":req.max_frames,"seed":req.seed,"snapshotInterval":req.snapshot_interval,"healthRecoveryWindowFrames":req.health_recovery_window_frames,"recordHalfHp":req.record_half_hp,"plain":req.plain,"mode":req.mode,"flightSampleInterval":req.flight_sample_interval,"maxDetailedEvents":req.max_detailed_events},
        "result":{"status":r.status,"winnerHp":r.winner_hp,"frames":r.frame_passed,"p1Hp":game.hp[0],"p2Hp":game.hp[1],"finalSeed":game.rng_seed},
        "summary":{"initialSeed":if req.plain {None} else {Some(rec.initial_seed)},"unitCount":rec.units.len(),
            "eventCount":rec.events.len(),"snapshotCount":rec.snapshots.len(),"firstCollisionFrame":rec.first_collision,
            "collisionFrames":rec.collisions,"droppedEvents":rec.dropped_events,"complete":rec.dropped_events==0},
        "units":rec.units,"events":rec.events,"timeline":rec.snapshots,"flights":rec.flights,"flightSamples":rec.flight_samples});
    super::decorate(&mut out,&game);
    write_output(&out,&args[2])
}
fn write_output(out:&Value,path:&str)->std::result::Result<(),Box<dyn std::error::Error>> {
    let path=std::path::Path::new(path);
    let file=std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut buffered=std::io::BufWriter::with_capacity(65536,file);
    serde_json::to_writer(&mut buffered,out)?;
    std::io::Write::flush(&mut buffered)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reject_unchecked_input() {assert!(validate("000000").is_err());assert!(validate("0Z0000").is_err());assert!(validate("中国阵").is_err());}
    fn fixture(hp:i32, category:&'static str)->State {State {id:3,side:0,type_id:29,hp,max_hp:100,x:50.0,y:400.0,jump:0,on_side:0,alive:true,category,cnt:None,speed:None}}
    #[test] fn repeated_half_episodes_and_full_recovery() {
        let mut h=HealthTracker::default();let full=fixture(100,"ball");let low=fixture(49,"ball");
        assert!(h.observe(&full,&low,10,30).is_empty());
        let e=h.observe(&low,&low,41,30);assert_eq!(e[0].kind,"half_hp");assert_eq!(e[0].frame,10);
        let e=h.observe(&low,&full,50,30);assert_eq!(e.iter().map(|e|e.kind).collect::<Vec<_>>(),vec!["recovered_half_hp"]);
        h.observe(&full,&low,60,30);let e=h.observe(&low,&low,91,30);
        assert_eq!(e[0].detail["episode"],2);assert_eq!(h.half_episodes,2);
    }
    #[test] fn short_recovery_including_one_second_boundary_is_filtered() {
        for duration in [1,29,30] {let mut h=HealthTracker::default();let full=fixture(100,"wall");let low=fixture(20,"wall");
            h.observe(&full,&low,10,30);let e=h.observe(&low,&full,10+duration,30);
            assert!(e.is_empty());
            assert_eq!(h.half_episodes,0);assert_eq!(h.suppressed_half_episodes,1);
            assert!(h.finish(100).is_empty());
        }
    }
    #[test] fn walls_death_and_unfinished_half_are_preserved() {
        let mut h=HealthTracker::default();let full=fixture(100,"wall");let mut dead=fixture(0,"wall");dead.alive=false;
        let e=h.observe(&full,&dead,5,30);assert_eq!(e[0].kind,"half_hp");assert_eq!(e[0].frame,5);
        assert!(h.finish(6).is_empty());
        let mut h=HealthTracker::default();let low=fixture(49,"wall");h.observe(&full,&low,5,30);
        let e=h.finish(10);assert_eq!(e[0].frame,5);assert_eq!(e[0].detail["confirmation"],"battle_ended_below_half");
    }
    #[test] fn actual_max_hp_and_disable_filter() {
        let mut h=HealthTracker::default();let mut full=fixture(150,"wall");full.max_hp=150;
        let mut half=full.clone();half.hp=75;assert!(h.observe(&full,&half,1,0).is_empty());
        let mut low=half.clone();low.hp=74;let e=h.observe(&half,&low,2,0);assert_eq!(e[0].kind,"half_hp");
        let e=h.observe(&low,&full,3,0);assert_eq!(e[0].kind,"recovered_half_hp");
    }
}
