//! 精确HP写点与对象来源链。只读钩子；基础程序链接原始core，不含本模块。
use super::{Element,GameTask,CoreKind,ParentList};
use serde::Serialize;
use serde_json::{Value,json};
use std::collections::BTreeMap;

#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Object {
    pub entity_id:i32,pub kind_code:u8,pub side:Option<usize>,pub slot:Option<usize>,pub type_id:Option<i32>,
    pub parent_entity_id:Option<i32>,pub owner_unit_id:Option<i32>,pub original_owner_unit_id:Option<i32>,pub root_unit_id:Option<i32>,
    pub birth_frame:i32,pub death_frame:Option<i32>,pub initial_hp:Option<i32>,pub max_hp:Option<i32>,
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Event {
    pub sequence:u64,pub frame:i32,pub kind:&'static str,pub entity_id:i32,pub side:Option<usize>,pub slot:Option<usize>,pub type_id:Option<i32>,
    pub source_object_id:Option<i32>,pub attacker_entity_id:Option<i32>,pub attacker_root_entity_id:Option<i32>,
    pub cause:&'static str,pub amount:i32,pub effective_hp_loss:i32,pub hp_before:Option<i32>,pub hp_after:Option<i32>,pub max_hp:Option<i32>,
    pub x:f32,pub y:f32,pub overlapping_object_ids:Vec<i32>,pub restored_full_hp:bool,
    #[serde(skip_serializing_if="Option::is_none")]
    pub detail:Option<Value>,
}
#[derive(Clone,Debug,Default,Serialize)]
#[serde(rename_all="camelCase")]
struct UnitStats {
    entity_id:i32,side:usize,slot:Option<usize>,type_id:i32,initial_hp:i32,max_hp:i32,
    raw_damage_taken:i64,effective_damage_taken:i64,healing_received:i64,hp_adjustment:i64,
    attack_actions:u32,attack_objects:u32,raw_damage_dealt:i64,effective_damage_dealt:i64,healing_given:i64,
    first_damage_frame:Option<i32>,last_damage_frame:Option<i32>,
    launches_in_step:u32,landings_in_step:u32,ground_landings:u32,own_platform_landings:u32,enemy_platform_landings:u32,
    #[serde(skip)] last_attack_frame:Option<i32>,
}
#[derive(Clone,Debug,Default)]
struct SourceStats {raw:i64,effective:i64,events:u32,ambiguous:u32}

#[derive(Clone,Debug)]
pub struct DetailTrace {
    pub actor:i32,pub last_hit:i32,pub core_ids:[i32;2],
    objects:Vec<Object>,events:Vec<Event>,stats:BTreeMap<i32,UnitStats>,
    damage:BTreeMap<(i32,Option<i32>,&'static str),SourceStats>,healing:BTreeMap<(i32,Option<i32>,&'static str),SourceStats>,
    damage_roots:BTreeMap<(i32,Option<i32>,&'static str),SourceStats>,
    slots:[usize;2],limit:usize,sequence:u64,dropped:u64,unknown_attack_events:u64,
    rally_ids:[i32;2],
}
impl DetailTrace {
    pub fn new(limit:usize)->Self {Self {actor:-1,last_hit:-1,core_ids:[-1,-1],objects:Vec::new(),events:Vec::new(),stats:BTreeMap::new(),
        damage:BTreeMap::new(),healing:BTreeMap::new(),damage_roots:BTreeMap::new(),slots:[0,0],limit,sequence:0,dropped:0,unknown_attack_events:0,rally_ids:[-1,-1]}}
    fn object(&self,id:i32)->Option<&Object> {if id<0 {None} else {self.objects.get(id as usize)}}
    fn emit(&mut self,mut event:Event) {
        event.sequence=self.sequence;self.sequence+=1;
        if self.events.len()<self.limit {self.events.push(event);} else {self.dropped+=1;}
    }
    fn event(&self,id:i32,frame:i32,kind:&'static str,cause:&'static str,source:i32,x:f32,y:f32)->Event {
        let target=self.object(id);let origin=self.object(source);
        Event {sequence:0,frame,kind,entity_id:id,side:target.and_then(|n|n.side),slot:target.and_then(|n|n.slot),type_id:target.and_then(|n|n.type_id),
            source_object_id:if source>=0 {Some(source)} else {None},attacker_entity_id:origin.and_then(|n|n.owner_unit_id),
            attacker_root_entity_id:origin.and_then(|n|n.root_unit_id),cause,amount:0,effective_hp_loss:0,hp_before:None,hp_after:None,max_hp:None,
            x,y,overlapping_object_ids:Vec::new(),restored_full_hp:false,detail:None}
    }
    pub fn spawn(&mut self,e:&Element,frame:i32) {
        let id=e.id();assert_eq!(self.objects.len(),id as usize,"对象来源链漏登记");
        let parent=self.object(self.actor).cloned();
        let unit=match e {
            Element::Ball(b)=>Some((b.side,b.type_,b.hp,b.max_hp)),
            Element::Wall(w)=>Some((w.side,w.type_,w.hp,w.max_hp)),
            Element::Core(c)=>{self.core_ids[c.side]=id;Some((c.side,match c.kind {CoreKind::Normal=>0,CoreKind::Boss {..}=>61,CoreKind::Boss2 {..}=>62},100,100))},
            _=>None,
        };
        let side=unit.map(|x|x.0).or_else(||match e {
            Element::Base(b)=>Some(b.side),Element::Bullet(b)=>Some(b.side),Element::Laser(l)=>Some(l.side),
            Element::Hit(h)=>Some(h.side),Element::HitYari(h)=>Some(h.side),Element::HitsBomb(h)=>Some(h.side),
            Element::Creature(c)=>Some(c.side),Element::HolyGhost(h)=>Some(h.t_side),
            Element::HitsHeal(h)=>match h.parent {ParentList::Heal(s)|ParentList::Repair(s)=>Some(s),_=>None},
            _=>parent.as_ref().and_then(|n|n.side),
        });
        let slot=unit.and_then(|u|if frame==0 {let k=self.slots[u.0];self.slots[u.0]+=1;Some(k)} else {None});
        let inherited=parent.as_ref().and_then(|n|n.owner_unit_id);
        let root=parent.as_ref().and_then(|n|n.root_unit_id).or(if unit.is_some(){Some(id)}else{None});
        self.objects.push(Object {entity_id:id,kind_code:e.kind_code(),side,slot,type_id:unit.map(|u|u.1),
            parent_entity_id:if self.actor>=0 {Some(self.actor)} else {None},owner_unit_id:if unit.is_some(){Some(id)}else{inherited},
            original_owner_unit_id:if unit.is_some(){Some(id)}else{inherited},root_unit_id:root,
            birth_frame:frame,death_frame:None,initial_hp:unit.map(|u|u.2),max_hp:unit.map(|u|u.3)});
        if let Some((side,type_id,hp,max_hp))=unit {
            self.stats.insert(id,UnitStats {entity_id:id,side,slot,type_id,initial_hp:hp,max_hp,..Default::default()});
        }
        if frame==0 {return;}
        let (x,y)=e.xy();let kind=e.kind_code();
        let offensive=matches!(kind,5|6|30..=33|36|45..=81|85|86|108..=115);
        let actor_unit=parent.as_ref().is_some_and(|n|n.type_id.is_some());
        let event_kind=if unit.is_some(){"unit_spawn"}else if offensive {if actor_unit{"attack"}else{"effect_spawn"}}
            else if matches!(kind,34|35|82|83|84) {"ability"}else{return;};
        if offensive && actor_unit {
            if let Some(s)=self.stats.get_mut(&self.actor) {
                s.attack_objects+=1;if s.last_attack_frame!=Some(frame) {s.attack_actions+=1;s.last_attack_frame=Some(frame);}
            }
        }
        let mut event=self.event(id,frame,event_kind,"object_created",self.actor,x,y);
        // 攻击事件按攻击单位查询；新弹体ID单独保留在sourceObjectId。
        if event_kind=="attack"||event_kind=="ability" {
            let source=self.actor;event=self.event(source,frame,event_kind,"object_created",id,x,y);
            event.attacker_entity_id=Some(source);
        }
        self.emit(event);
    }
    pub fn removed(&mut self,id:i32,frame:i32) {if let Some(n)=self.objects.get_mut(id as usize) {n.death_frame.get_or_insert(frame);}}
    fn hp(&mut self,target:i32,side:usize,before:i32,after:i32,max:i32,x:f32,y:f32,cause:&'static str,source:i32,frame:i32,overlaps:Vec<i32>) {
        if before==after {return;}
        let healing=after>before && cause!="termination_clamp";
        let adjustment=cause=="termination_clamp";
        let kind=if adjustment {"hp_adjustment"}else if healing {"heal"}else{"damage"};
        let amount=(after-before).abs();let effective=if after<before {amount.min(before.max(0))} else {0};
        let mut event=self.event(target,frame,kind,cause,source,x,y);
        event.amount=amount;event.effective_hp_loss=effective;event.hp_before=Some(before);event.hp_after=Some(after);event.max_hp=Some(max);
        event.restored_full_hp=healing&&before<max&&after>=max;event.overlapping_object_ids=overlaps;
        let owner=event.attacker_entity_id;
        let s=self.stats.get_mut(&target).expect("HP目标未登记");assert_eq!(s.side,side);
        if adjustment {s.hp_adjustment+=(after-before) as i64;}
        else if healing {s.healing_received+=amount as i64;}
        else {s.raw_damage_taken+=amount as i64;s.effective_damage_taken+=effective as i64;s.first_damage_frame.get_or_insert(frame);s.last_damage_frame=Some(frame);}
        if !adjustment {
            let table=if healing {&mut self.healing} else {&mut self.damage};
            let pair=table.entry((target,owner,cause)).or_default();pair.raw+=amount as i64;pair.effective+=effective as i64;pair.events+=1;
            pair.ambiguous+=u32::from(event.overlapping_object_ids.len()>1);
            if !healing {
                let root=self.damage_roots.entry((target,event.attacker_root_entity_id,cause)).or_default();
                root.raw+=amount as i64;root.effective+=effective as i64;root.events+=1;root.ambiguous+=u32::from(event.overlapping_object_ids.len()>1);
            }
            if let Some(attacker)=owner.and_then(|id|self.stats.get_mut(&id)) {
                if healing {attacker.healing_given+=amount as i64;}
                else {attacker.raw_damage_dealt+=amount as i64;attacker.effective_damage_dealt+=effective as i64;}
            }
            if cause=="attack" && owner.is_none() {self.unknown_attack_events+=1;}
        }
        self.emit(event);
    }
    pub fn append_output(&self,out:&mut Value) {
        let identities=|id:i32|self.object(id).map(|n|json!({"entityId":id,"side":n.side,"slot":n.slot,"typeId":n.type_id}));
        let sources=|table:&BTreeMap<(i32,Option<i32>,&'static str),SourceStats>,heal:bool|table.iter().map(|(&(target,attacker,cause),s)|{
            let unit=&self.stats[&target];let total=if heal {unit.healing_received}else{unit.effective_damage_taken};
            json!({"entityId":target,"side":unit.side,"slot":unit.slot,"typeId":unit.type_id,"attackerEntityId":attacker,
                "attacker":attacker.and_then(identities),"cause":cause,"rawAmount":s.raw,"effectiveAmount":if heal{s.raw}else{s.effective},
                "percentage":if total==0 {Value::Null}else{json!(100.0*(if heal{s.raw}else{s.effective}) as f64/total as f64)},
                "events":s.events,"ambiguousHitEvents":s.ambiguous})
        }).collect::<Vec<_>>();
        let mut mismatches=Vec::new();let mut stats=Vec::new();
        for (&id,s) in &self.stats {
            let mut row=serde_json::to_value(s).unwrap();
            let final_hp=out["units"].as_array().unwrap().iter().find(|u|u["entityId"].as_i64()==Some(id as i64)).and_then(|u|u["finalHp"].as_i64());
            let calculated=s.initial_hp as i64-s.raw_damage_taken+s.healing_received+s.hp_adjustment;
            row["finalHp"]=json!(final_hp);row["calculatedFinalHp"]=json!(calculated);
            if final_hp!=Some(calculated) {mismatches.push(id);}
            stats.push(row);
        }
        out["combatEvents"]=serde_json::to_value(&self.events).unwrap();out["objects"]=serde_json::to_value(&self.objects).unwrap();
        out["unitStats"]=json!(stats);out["damageSources"]=json!(sources(&self.damage,false));out["healingSources"]=json!(sources(&self.healing,true));
        let mut roots=sources(&self.damage_roots,false);
        for row in &mut roots {let object=row.as_object_mut().unwrap();if let Some(value)=object.remove("attackerEntityId") {object.insert("attackerRootEntityId".into(),value);}}
        out["damageRootSources"]=json!(roots);
        out["detailSummary"]=json!({"policy":"engine-first-match-v1","eventPolicy":"shield-as-ability-v2","eventsObserved":self.sequence,"eventsStored":self.events.len(),"droppedEvents":self.dropped,
            "complete":self.dropped==0&&mismatches.is_empty(),"statisticsComplete":mismatches.is_empty(),"hpBalanceMismatches":mismatches,
            "unknownAttackEvents":self.unknown_attack_events,"attributionComplete":self.unknown_attack_events==0,
            "damageDenominator":"effective HP lost; overkill stored separately; overlapping attacks assigned once to the engine first-match witness"});
        if self.dropped>0 || !out["detailSummary"]["statisticsComplete"].as_bool().unwrap() {out["summary"]["complete"]=json!(false);}
    }
}

impl GameTask {
    pub fn trace_witness(&self,id:i32) {if let Some(t)=&self.detail {t.borrow_mut().last_hit=id;}}
    pub fn trace_hit_id(&self)->i32 {self.detail.as_ref().map_or(-1,|t|t.borrow().last_hit)}
    pub fn trace_hp(&self,id:i32,side:usize,before:i32,after:i32,max:i32,x:f32,y:f32,cause:&'static str,source:i32) {
        if let Some(trace)=&self.detail {
            let overlaps=if cause=="attack" {
                let is_core=trace.borrow().core_ids[side]==id;
                let is_wall=trace.borrow().object(id).is_some_and(|n|matches!(n.kind_code,7..=19));
                self.atk[1-side].ids.iter().copied().filter(|&object|if is_core {self.core13_box_pass(object,x,y)} else {self.atk_box_pass(object,x,y,if is_wall {6.0}else{8.0})}).collect()
            }else{Vec::new()};
            trace.borrow_mut().hp(id,side,before,after,max,x,y,cause,source,self.time,overlaps);
        }
    }
    pub fn trace_reflection(&self,id:i32,side:usize) {
        if let Some(t)=&self.detail {let mut t=t.borrow_mut();let actor=t.actor;
            if let Some(n)=t.objects.get_mut(id as usize) {n.owner_unit_id=Some(actor);n.side=Some(side);}
            let e=t.event(id,self.time,"projectile_reflected","reflection",actor,0.0,0.0);t.emit(e);
        }
    }
    pub fn trace_control(&self,id:i32,kind:&'static str) {
        if let Some(t)=&self.detail {let mut t=t.borrow_mut();let e=t.event(id,self.time,kind,"interception",t.actor,0.0,0.0);t.emit(e);}
    }
    pub fn trace_rally(&self,side:usize,id:i32,x:f32,y:f32) {
        if let Some(t)=&self.detail {let mut t=t.borrow_mut();t.rally_ids[side]=id;let e=t.event(id,self.time,"ability","rally",id,x,y);t.emit(e);}
    }
    pub fn trace_rally_id(&self,side:usize)->i32 {self.detail.as_ref().map_or(-1,|t|t.borrow().rally_ids[side])}
    pub fn trace_motion(&self,id:i32,side:usize,from:i32,from_side:usize,to:i32,on_side:usize,x:f32,y:f32,xs:f32,ys:f32,phase:&'static str,source:i32) {
        if from==to && from_side==on_side {return;}
        if let Some(t)=&self.detail {
            let mut t=t.borrow_mut();
            let kind=if to==1 && from!=1 {"launch"}else if from==1 && to!=1 {"land"}else{"surface_change"};
            let surface=if to==1 {"airborne"}else if to==2 {"ground"}else if side==on_side {"own_platform"}else{"enemy_platform"};
            let mut e=t.event(id,self.time,kind,phase,source,x,y);
            e.detail=Some(json!({"fromJumpFlag":from,"toJumpFlag":to,"fromOnSide":from_side,"onSide":on_side,"surface":surface,"velocityX":xs,"velocityY":ys,"phase":phase}));
            if let Some(s)=t.stats.get_mut(&id) {
                if kind=="launch" {s.launches_in_step+=1;}
                if kind=="land" {s.landings_in_step+=1;match surface {"ground"=>s.ground_landings+=1,"own_platform"=>s.own_platform_landings+=1,"enemy_platform"=>s.enemy_platform_landings+=1,_=>{}}}
            }
            t.emit(e);
        }
    }
}
