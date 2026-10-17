"""从已校验规则源码生成详细观察核心；按精确上下文插入只读钩子，绝不回写vendor。"""
import hashlib
import json
import os
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[2]

def prepare():
    manifest=json.loads((ROOT/'rust/vendor/source-hashes.json').read_text(encoding='utf-8'))
    for name,expected in manifest['files'].items():
        if hashlib.sha256((ROOT/name).read_bytes()).hexdigest()!=expected:
            raise ValueError(f'冻结源码校验失败：{name}')
    upstream=ROOT/'rust/vendor/gkt-core/src'
    sources={str(p.relative_to(upstream)).replace('\\','/'):p.read_text(encoding='utf-8') for p in upstream.rglob('*.rs')}
    edits=[]
    def edit(file,old,new,count=1):
        actual=sources[file].count(old)
        if actual!=count:raise ValueError(f'观察钩子上下文不匹配：{file}，预期{count}处，实际{actual}处')
        sources[file]=sources[file].replace(old,new)
        edits.append({'file':file,'count':count,'oldSha256':hashlib.sha256(old.encode()).hexdigest()})
    mod='engine/mod.rs'
    # 基础模式仅增加满血恢复信号；不包含详细模式的攻击/HP账本/来源链钩子。
    edit(mod,'pub struct GameTask {','pub struct GameTask {\n    pub full_recoveries:Vec<FullRecovery>,\n    pub record_full_recoveries:bool,')
    edit(mod,'impl GameTask {\n    pub fn new()','''#[derive(Clone,Copy,Debug)]
pub struct FullRecovery {pub entity_id:i32,pub side:usize,pub before:i32,pub hp:i32,pub max_hp:i32,pub x:f32,pub y:f32}
impl GameTask {
    pub fn note_full_recovery(&mut self,id:i32,side:usize,before:i32,hp:i32,max_hp:i32,x:f32,y:f32) {
        if self.record_full_recoveries && before<max_hp && hp>=max_hp {
            self.full_recoveries.push(FullRecovery {entity_id:id,side,before,hp,max_hp,x,y});
        }
    }
    pub fn new()''')
    edit(mod,'            next_id: 0,','            next_id: 0,\n            full_recoveries:Vec::new(),record_full_recoveries:false,')
    edit(mod,'        if let Some(o) = observer.as_mut() {\n            o.on_battle_started(&g);','        g.record_full_recoveries=observer.is_some();\n        if let Some(o) = observer.as_mut() {\n            o.on_battle_started(&g);')
    edit(mod,'            g.time += 1;','            g.time += 1;\n            g.full_recoveries.clear();')
    edit('engine/ball.rs','            self.hp += 1;','            self.hp += 1;\n            game.note_full_recovery(self.id,self.side,self.hp-1,self.hp,self.max_hp,self.x,self.y);')
    edit('engine/wall.rs','            self.hp += 1;','            self.hp += 1;\n            game.note_full_recovery(self.id,self.side,self.hp-1,self.hp,self.max_hp,self.x,self.y);',3)
    edit('engine/core.rs','            game.hp[self.side] += 1;','            game.hp[self.side] += 1;\n            game.note_full_recovery(self.id,self.side,game.hp[self.side]-1,game.hp[self.side],100,self.x,self.y);')
    def publish(folder,values,patches):
        target=ROOT/'rust'/folder/'src';target.mkdir(parents=True,exist_ok=True);hashes={}
        for name,code in values.items():
            path=target/name;path.parent.mkdir(parents=True,exist_ok=True);data=code.encode('utf-8');path.write_bytes(data)
            hashes[name]=hashlib.sha256(data).hexdigest()
        (target.parent/'generated-hashes.json').write_text(json.dumps({'sourceCommit':manifest['sourceCommit'],'ruleset':manifest.get('ruleset','upstream-149e365'),'generatorSha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'patches':patches,'files':hashes},ensure_ascii=False,indent=2),encoding='utf-8')
        return len(hashes)
    basic_count=publish('basic-core',dict(sources),list(edits))
    edit(mod,'mod telemetry;','mod telemetry;\npub mod detail;')
    edit(mod,'    pub next_id: i32,','    pub next_id: i32,\n    pub detail: Option<std::cell::RefCell<detail::DetailTrace>>,')
    edit(mod,'            next_id: 0,','            next_id: 0,\n            detail: None,')
    edit(mod,'        let element = make(id);','        let element = make(id);\n        if let Some(trace)=&self.detail {trace.borrow_mut().spawn(&element,self.time);}')
    edit(mod,'    pub fn kill_element(&mut self, id: i32) {','    pub fn kill_element(&mut self, id: i32) {\n        if id>=0 {if let Some(trace)=&self.detail {trace.borrow_mut().removed(id,self.time);}}')
    edit(mod,'                e.step(self);','                if let Some(trace)=&self.detail {trace.borrow_mut().actor=i as i32;}\n                e.step(self);\n                if let Some(trace)=&self.detail {trace.borrow_mut().actor=-1;}')
    edit(mod,'            .any(|&id| id != skip && game.raw(id).hit_test_point(x, y))',
         '            .any(|&id| {let hit=id != skip && game.raw(id).hit_test_point(x, y);if hit {game.trace_witness(id);}hit})')
    # 在原来确实返回true的分支旁记录见证对象，不替换传感器顺序或OR规则。
    for start,end in [('    pub fn atk_hit_4corner','    /// 粗盒判定'),('    pub fn atk_hit_core13','    #[inline]\n    fn core13_box_pass')]:
        a=sources[mod].index(start);b=sources[mod].index(end,a)
        old=sources[mod][a:b]
        if old.count('return true;')!=5:raise ValueError('攻击传感器源码结构变化')
        edit(mod,old,old.replace('return true;','self.trace_witness(id);\n                return true;'))
    header='''    pub fn run_single_observed(
        f1: &CompiledFort,
        f2: &CompiledFort,
        max_frames: i32,
        random_seed: Option<i32>,
        mut observer: Option<&mut dyn BattleObserver>,
    ) -> (Self, Result) {'''
    replacement='''    pub fn run_single_observed(f1:&CompiledFort,f2:&CompiledFort,max_frames:i32,random_seed:Option<i32>,observer:Option<&mut dyn BattleObserver>)->(Self,Result) {
        Self::run_with_detail(f1,f2,max_frames,random_seed,observer,None)
    }
    pub fn run_single_detailed(f1:&CompiledFort,f2:&CompiledFort,max_frames:i32,random_seed:Option<i32>,observer:Option<&mut dyn BattleObserver>,limit:usize)->(Self,Result) {
        Self::run_with_detail(f1,f2,max_frames,random_seed,observer,Some(limit))
    }
    fn run_with_detail(f1:&CompiledFort,f2:&CompiledFort,max_frames:i32,random_seed:Option<i32>,mut observer:Option<&mut dyn BattleObserver>,limit:Option<usize>)->(Self,Result) {'''
    edit(mod,header,replacement)
    edit(mod,'        let mut g = Self::new();\n        g.init();','        let mut g = Self::new();\n        g.detail=limit.map(|n|std::cell::RefCell::new(detail::DetailTrace::new(n)));\n        g.init();')
    edit(mod,'                    self.hp[i] -= wrk as i32;','                    let before=self.hp[i];\n                    self.hp[i] -= wrk as i32;\n                    let (x,y)=self.core_position(i);\n                    self.trace_hp(self.core_id[i],i,before,self.hp[i],100,x,y,"fort_collision",-1);')
    edit('engine/base.rs','                game.hp[self.side] -= self.wrk as i32;','''                let before=game.hp[self.side];game.hp[self.side] -= self.wrk as i32;
                let c=game.core(self.side);let (x,y)=(self.x+c.unit_x,self.y+c.unit_y);
                game.trace_hp(game.core_id[self.side],self.side,before,game.hp[self.side],100,x,y,"boundary_collision",-1);''')
    edit(mod,'                    self.hp[0] = self.hp[0].max(0);\n                    self.hp[1] = self.hp[1].max(0);','''                    for side in 0..2 {
                        let before=self.hp[side];self.hp[side]=before.max(0);
                        let id=self.detail.as_ref().map_or(-1,|t|t.borrow().core_ids[side]);
                        self.trace_hp(id,side,before,self.hp[side],100,self.core_x[side],self.core_y[side],"termination_clamp",-1);
                    }''')
    edit(mod,'            b.reflect(self, rot);','            b.reflect(self, rot);\n            self.trace_reflection(id,b.side);')
    edit(mod,'            b.gei_flg = v;','            b.gei_flg = v;\n            if v==2 || v==3 {self.trace_control(id,"projectile_intercepted");}')
    # 每个真实HP写点旁记录前后值。amount仍由引擎原来的扣血/治疗规则决定。
    ball='engine/ball.rs'
    edit(ball,'    pub fn step(&mut self, game: &mut GameTask) {','    pub fn step(&mut self, game: &mut GameTask) {\n        let previous_jump=self.jump_flg;let previous_on_side=self.on_side;')
    edit(ball,'        if game.saihai_cnt[self.side] > 0','''        game.trace_motion(self.id,self.side,previous_jump,previous_on_side,self.jump_flg,self.on_side,self.x,self.y,self.xs,self.ys,"movement",-1);
        let control_jump=self.jump_flg;let control_on_side=self.on_side;let mut launch_source=-1;
        if game.saihai_cnt[self.side] > 0''')
    edit(ball,'            self.jump_flg = 1;\n            self.xs = trig::cos(game.saihai_rot[self.side]) * 4.0;','            self.jump_flg = 1;\n            launch_source=game.trace_rally_id(self.side);\n            self.xs = trig::cos(game.saihai_rot[self.side]) * 4.0;')
    edit(ball,'            self.ys = -8.0;','            self.ys = -8.0;\n            launch_source=game.trace_hit_id();')
    edit(ball,'            self.ys = -4.0;','            self.ys = -4.0;\n            launch_source=game.trace_hit_id();')
    edit(ball,'        let opp = 1 - self.side;\n        if game.atk_hit_4corner','        game.trace_motion(self.id,self.side,control_jump,control_on_side,self.jump_flg,self.on_side,self.x,self.y,self.xs,self.ys,"control",launch_source);\n        let opp = 1 - self.side;\n        if game.atk_hit_4corner')
    edit(ball,'                    game.saihai_rot[self.side] = self.rot;','                    game.saihai_rot[self.side] = self.rot;\n                    game.trace_rally(self.side,self.id,self.x,self.y);')
    edit(ball,'''        if game.atk_hit_4corner(opp, self.x, self.y, 8.0)
            || (self.jump_flg == 0 && game.dokkan_flg[self.on_side])''',
         '''        let attack_hit=game.atk_hit_4corner(opp,self.x,self.y,8.0);
        if attack_hit || (self.jump_flg == 0 && game.dokkan_flg[self.on_side])''')
    edit(ball,'            self.hp -= 1;','            self.hp -= 1;\n            game.trace_hp(self.id,self.side,self.hp+1,self.hp,self.max_hp,self.x,self.y,if attack_hit {"attack"} else {"fort_collision"},if attack_hit {game.trace_hit_id()} else {-1});')
    edit(ball,'            self.hp += 1;','            self.hp += 1;\n            game.trace_hp(self.id,self.side,self.hp-1,self.hp,self.max_hp,self.x,self.y,"heal",game.trace_hit_id());')
    edit(ball,'                        self.hp = 0;','                        let before=self.hp;self.hp = 0;\n                        game.trace_hp(self.id,self.side,before,0,self.max_hp,self.x,self.y,"self_sacrifice",self.id);')
    # 移动步骤内同帧落地后再起跳，也分别记录；移出边界时的最后一次落地不丢失。
    edit(ball,'        } else if !self.jump(game) {\n            return;','        } else if !self.jump(game) {\n            game.trace_motion(self.id,self.side,previous_jump,previous_on_side,self.jump_flg,self.on_side,self.x,self.y,self.xs,self.ys,"movement",-1);\n            return;')
    wall='engine/wall.rs'
    edit(wall,'self.hp -= 1;','self.hp -= 1;\n                game.trace_hp(self.id,self.side,self.hp+1,self.hp,self.max_hp,self.x,self.y,"attack",game.trace_hit_id());',3)
    edit(wall,'self.hp += 1;','self.hp += 1;\n            game.trace_hp(self.id,self.side,self.hp-1,self.hp,self.max_hp,self.x,self.y,"repair",game.trace_hit_id());',3)
    edit(wall,'            self.hp = 0;','            let before=self.hp;self.hp = 0;\n            game.trace_hp(self.id,self.side,before,0,self.max_hp,self.x,self.y,"wall_collision",game.trace_hit_id());')
    core='engine/core.rs'
    edit(core,'            game.hp[self.side] -= 1;','            game.hp[self.side] -= 1;\n            game.trace_hp(self.id,self.side,game.hp[self.side]+1,game.hp[self.side],100,self.x,self.y,"attack",game.trace_hit_id());')
    edit(core,'            game.hp[self.side] += 1;','            game.hp[self.side] += 1;\n            game.trace_hp(self.id,self.side,game.hp[self.side]-1,game.hp[self.side],100,self.x,self.y,"repair",game.trace_hit_id());')
    overlay=ROOT/'rust/diagnostic-core/overlay'
    for p in overlay.rglob('*.rs'):sources[str(p.relative_to(overlay)).replace('\\','/')]=p.read_text(encoding='utf-8')
    detail_count=publish('diagnostic-core',sources,edits)
    return basic_count+detail_count,len(edits)

if __name__=='__main__':
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':sys.exit('请先conda activate opencode')
    files,edits=prepare()
    print(f'已从冻结源码生成基础/详细观察核心：共{files}个文件，{edits}组只读钩子；原vendor保持不变')
