"""给隔离详细核心增加HP写点空间上下文；仅改变观察输出，不修改vendor或现有二进制。"""
from pathlib import Path

POLICY = 'hp-write-spatial-context-v1'

WINDOW = r'''
#[derive(Clone,Debug,Serialize,serde::Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SpatialWindow {
    pub from_frame:i32,pub to_frame:i32,
    pub victim_side:Option<usize>,pub victim_slots:Option<Vec<usize>>,
    pub attacker_side:Option<usize>,pub attacker_slots:Option<Vec<usize>>,
}
impl SpatialWindow {
    pub fn validate(&self,max_frames:i32)->std::result::Result<(),String> {
        if self.from_frame<0 || self.to_frame<self.from_frame || self.to_frame>max_frames {return Err("空间上下文帧区间越界".into());}
        for side in [self.victim_side,self.attacker_side].into_iter().flatten() {if side>1 {return Err("空间上下文side只能为0或1".into());}}
        for slots in [&self.victim_slots,&self.attacker_slots].into_iter().flatten() {if slots.is_empty() || slots.iter().any(|n|*n>10000) {return Err("空间上下文槽位为空或越界".into());}}
        Ok(())
    }
    fn matches(&self,frame:i32,victim:Option<&Object>,attacker:Option<&Object>)->bool {
        let identity=|object:Option<&Object>,side:Option<usize>,slots:&Option<Vec<usize>>| {
            side.is_none_or(|s|object.and_then(|o|o.side)==Some(s)) &&
            slots.as_ref().is_none_or(|ss|object.and_then(|o|o.slot).is_some_and(|s|ss.contains(&s)))
        };
        frame>=self.from_frame && frame<=self.to_frame && identity(victim,self.victim_side,&self.victim_slots) && identity(attacker,self.attacker_side,&self.attacker_slots)
    }
}
'''

HELPERS = r'''
    fn trace_spatial_entity(&self,id:i32)->serde_json::Value {
        if id<0 {return serde_json::Value::Null;}
        let Some(e)=self.elements.get(id as usize).and_then(|n|n.as_ref()) else {return serde_json::Value::Null;};
        let (x,y)=e.xy();
        let mut value=json!({"entityId":id,"kindCode":e.kind_code(),"x":x,"y":y});
        if let Element::Ball(b)=e {
            value["side"]=json!(b.side);value["typeId"]=json!(b.type_);value["hp"]=json!(b.hp);
            value["jumpFlag"]=json!(b.jump_flg);value["onSide"]=json!(b.on_side);
            value["velocityX"]=json!(b.xs);value["velocityY"]=json!(b.ys);
            value["angle"]=json!(b.rot);value["counter"]=json!(b.cnt);
        }
        value
    }
    fn trace_battle_context(&self,target:i32,side:usize,x:f32,y:f32,owner:Option<i32>,root:Option<i32>,source:i32)->serde_json::Value {
        let core_ids=self.detail.as_ref().map(|t|t.borrow().core_ids).unwrap_or([-1,-1]);
        let sides=(0..2).map(|s| {
            let base=self.elements.get(self.base_id[s].max(0) as usize).and_then(|n|n.as_ref()).and_then(|e|match e {
                Element::Base(b) if self.base_id[s]>=0=>Some(json!({"x":b.x,"y":b.y,"velocityX":b.xs,"velocityY":b.ys,"acceleration":b.axl})),_=>None});
            let true_core=if target==core_ids[s] {Some(json!({"x":x,"y":y,"positionSource":"current_hp_write"}))}
                else {self.elements.get(core_ids[s].max(0) as usize).and_then(|n|n.as_ref()).and_then(|e|match e {
                    Element::Core(c) if core_ids[s]>=0=>Some(json!({"x":c.x,"y":c.y,"positionSource":"live_core_element"})),_=>None})};
            json!({"side":s,"base":base,"trueCore":true_core,"coreHp":self.hp[s],
                "aimTarget":{"x":self.core_x[s],"y":self.core_y[s]}})
        }).collect::<Vec<_>>();
        json!({"policy":"hp-write-spatial-context-v1","phase":"hp_write","coordinateSpace":"battle-world",
            "victim":{"entityId":target,"side":side,"x":x,"y":y},
            "attacker":owner.map(|id|self.trace_spatial_entity(id)),
            "attackerRoot":root.map(|id|self.trace_spatial_entity(id)),
            "sourceObject":self.trace_spatial_entity(source),"sides":sides,
            "availability":"null means no live element available at this update phase; never substitute aimTarget for trueCore"})
    }
'''

def augment(path):
    path=Path(path)
    code=path.read_text(encoding='utf-8')
    if POLICY in code:raise ValueError('空间上下文补丁已经存在，拒绝重复应用')
    edits=[
        ('pub struct DetailTrace {','pub struct DetailTrace {\n    pub spatial_context:Option<SpatialWindow>,'),
        ('Self {actor:-1,last_hit:-1,core_ids:', 'Self {spatial_context:None,actor:-1,last_hit:-1,core_ids:'),
        ('#[derive(Clone,Debug)]\npub struct DetailTrace', WINDOW+'\n#[derive(Clone,Debug)]\npub struct DetailTrace'),
        ('frame:i32,overlaps:Vec<i32>) {','frame:i32,overlaps:Vec<i32>,context:Option<Value>) {'),
        ('        event.amount=amount;event.effective_hp_loss=effective;',
         '        event.detail=context.map(|c|json!({"battleContext":c}));\n        event.amount=amount;event.effective_hp_loss=effective;'),
        ('"eventPolicy":"shield-as-ability-v2",','"eventPolicy":"shield-as-ability-v2","contextPolicy":"hp-write-spatial-context-v1","contextWindow":self.spatial_context,"contextEventsStored":self.events.iter().filter(|e|e.detail.as_ref().is_some_and(|d|d.get("battleContext").is_some())).count(),'),
        ('impl GameTask {','impl GameTask {'+HELPERS),
        ('            trace.borrow_mut().hp(id,side,before,after,max,x,y,cause,source,self.time,overlaps);',
         '''            let (owner,root,selected)={let t=trace.borrow();let object=t.object(source);let owner=object.and_then(|o|o.owner_unit_id);
                (owner,object.and_then(|o|o.root_unit_id),before!=after && t.spatial_context.as_ref().is_some_and(|w|w.matches(self.time,t.object(id),owner.and_then(|n|t.object(n)))))};
            let context=selected.then(||self.trace_battle_context(id,side,x,y,owner,root,source));
            trace.borrow_mut().hp(id,side,before,after,max,x,y,cause,source,self.time,overlaps,context);''')
    ]
    for old,new in edits:
        if code.count(old)!=1:raise ValueError(f'详细核心上下文不匹配：{old[:70]}')
        code=code.replace(old,new)
    path.write_text(code,encoding='utf-8',newline='\n')

def configure_workspace(workspace):
    """在prepare_core之后配置隔离workspace；默认不生成任何空间上下文。"""
    workspace=Path(workspace)
    edits={
        'rust/observer/src/recording.rs':[
            ('    pub p1: String,','    #[serde(default)]\n    pub spatial_context: Option<Value>,\n    pub p1: String,'),
            ('    validate(&req.p1)?;', '    super::validate_spatial(&req)?;\n    validate(&req.p1)?;')],
        'rust/observer/src/bin/gkt-detail.rs':[
            ('fn supports_mode', '''fn validate_spatial(req:&recording::Request)->std::result::Result<(),Box<dyn std::error::Error>> {
    if let Some(value)=&req.spatial_context {let w:engine::engine::detail::SpatialWindow=serde_json::from_value(value.clone())?;w.validate(req.max_frames)?;} Ok(())
}
fn supports_mode'''),
            ('Some(rec),req.max_detailed_events)', 'Some(rec),req.max_detailed_events,req.spatial_context.clone().map(|v|serde_json::from_value(v).expect("空间窗口已验证")))')],
        'rust/diagnostic-core/src/engine/mod.rs':[
            ('observer,None)', 'observer,None,None)'),
            ('limit:usize)->(Self,Result)', 'limit:usize,spatial_context:Option<detail::SpatialWindow>)->(Self,Result)'),
            ('observer,Some(limit))', 'observer,Some(limit),spatial_context)'),
            ('limit:Option<usize>)->(Self,Result)', 'limit:Option<usize>,spatial_context:Option<detail::SpatialWindow>)->(Self,Result)'),
            ('g.detail=limit.map(|n|std::cell::RefCell::new(detail::DetailTrace::new(n)));', 'g.detail=limit.map(|n|{let mut t=detail::DetailTrace::new(n);t.spatial_context=spatial_context;std::cell::RefCell::new(t)});')]
    }
    for relative,changes in edits.items():
        path=workspace/relative;code=path.read_text(encoding='utf-8')
        for old,new in changes:
            if code.count(old)!=1:raise ValueError(f'隔离入口上下文不匹配：{relative}: {old[:70]}')
            code=code.replace(old,new)
        path.write_text(code,encoding='utf-8',newline='\n')

if __name__=='__main__':
    import argparse,os
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':raise SystemExit('请先激活conda opencode')
    parser=argparse.ArgumentParser();parser.add_argument('--overlay',required=True);args=parser.parse_args()
    augment(args.overlay)
    print('已为指定隔离overlay增加HP写点空间上下文')
