//! Static-support opening gravity. Calls the engine's actual Ball::land, never step().
use gkt_core::decode::compile_fort;
use gkt_core::engine::{Element, GameTask};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const POLICY: &str = "static-support-gravity-v3";
const EPS: f32 = 0.001;
const SUPPORT_PUSH_INITIAL_Y: i32 = 0;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Formation { #[serde(default)] name: String, code: String }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    formations: Vec<Formation>,
    #[serde(default = "max_frames")] max_frames: usize,
    #[serde(default = "stable_frames")] stable_frames: usize,
}
fn max_frames() -> usize { 240 }
fn stable_frames() -> usize { 12 }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Unit { slot: usize, type_id: i32, x: i32, y: i32, r: i32 }

fn decode(code: &str) -> Result<Vec<Unit>, String> {
    let bytes = code.as_bytes();
    if bytes.len() < 6 || bytes.len() % 6 != 0 || bytes.len() > 3072 || !b"012".contains(&bytes[0]) {
        return Err("阵码长度或slot0核心非法".into());
    }
    let mut out = Vec::new();
    for (slot, c) in bytes.chunks_exact(6).enumerate() {
        let id = ALPHABET.iter().position(|&v| v == c[0]).ok_or("单位ID非法")? as i32;
        if slot > 0 && !(1..=60).contains(&id) { return Err("单位ID非法".into()); }
        let mut n = 0i32;
        for &b in &c[1..] {
            let d = ALPHABET[..61].iter().position(|&v| v == b).ok_or("坐标必须base61，不能含Z")?;
            n = n * 61 + d as i32;
        }
        let (dx, dy, xmax, ymax) = if slot == 0 { (52,58,276,276) } else { (16,20,348,349) };
        let (x,y,r) = (n/1000%1000-dx, n%1000-dy, n/1_000_000);
        if !(0..=xmax).contains(&x) || !(0..=ymax).contains(&y) || !(0..360).contains(&r) {
            return Err(format!("slot{slot}坐标或角度越界"));
        }
        out.push(Unit {slot,type_id:if slot==0 {bytes[0] as i32-b'0' as i32} else {id},x,y,r});
    }
    Ok(out)
}

fn encode(units: &[Unit]) -> String {
    let mut out = String::new();
    for u in units {
        out.push(ALPHABET[u.type_id as usize] as char);
        let (dx,dy) = if u.slot==0 {(52,58)} else {(16,20)};
        let mut n = u.r*1_000_000+(u.x+dx)*1000+u.y+dy;
        let mut digits = [b'0';5];
        for b in digits.iter_mut().rev() {*b=ALPHABET[(n%61) as usize];n/=61;}
        out.push_str(std::str::from_utf8(&digits).unwrap());
    }
    out
}

fn simulate(formation: &Formation, req: &Request) -> Result<Value, String> {
    let units = decode(&formation.code)?;
    if units.iter().skip(1).any(|u| !(1..=60).contains(&u.type_id)) {return Err("单位类型非法".into());}
    let fort = compile_fort(&formation.name, &formation.code);
    // Zero-frame construction preserves real types/geometry/slot order. No battle advances.
    let (mut game, _) = GameTask::run_single(&fort, &fort, 0, Some(1));
    // Core constructors store base-relative coordinates until their first step. Materialize
    // world coordinates before any Ball::land query, exactly as the battle update does.
    for side in 0..2 {
        let base = game.base(side);
        let (bx,by) = (base.x,base.y);
        let cid = game.core_id[side];
        if let Some(Element::Core(c)) = game.elements[cid as usize].as_mut() {
            c.x=bx+c.unit_x;c.y=by+c.unit_y;
        }
        game.refresh_bound(cid);
    }
    // Every placed unit spawns one Ball or Wall. Match by construction order, not ID math.
    let mut ids = vec![game.core_id[0]];
    ids.extend(game.elements.iter().filter_map(|e| match e.as_ref() {
        Some(Element::Ball(b)) if b.side==0 => Some(b.id),
        Some(Element::Wall(w)) if w.side==0 => Some(w.id), _=>None,
    }));
    if ids.len()!=units.len() {return Err("引擎槽位映射不完整".into());}
    let mut last_change = vec![0usize;units.len()];
    let mut max_up = vec![0f32;units.len()];let mut max_down = max_up.clone();
    let mut max_step_down = max_up.clone();
    let mut quiet=0;let mut elapsed=0;let mut converged=false;
    for frame in 1..=req.max_frames {
        let mut still=true;
        for (slot,&id) in ids.iter().enumerate().skip(1) {
            let Some(Element::Ball(_))=game.elements[id as usize].as_ref() else {continue};
            let Some(Element::Ball(mut ball))=game.elements[id as usize].take() else {unreachable!()};
            let old_y=ball.y;
            // All special effects disabled: no wall step, launch, snipe, turn, attack,
            // summon, healing, propulsion or collision. Ordinary gravity/support only.
            if ball.jump_flg!=2 {ball.land(&mut game);}
            max_step_down[slot]=max_step_down[slot].max(ball.y-old_y);
            if (ball.y-old_y).abs()>EPS {last_change[slot]=frame;still=false;}
            if ball.jump_flg!=2 && ball.ys.abs()>EPS {still=false;}
            let delta=ball.y-152.0-units[slot].y as f32;
            max_up[slot]=max_up[slot].max(-delta);max_down[slot]=max_down[slot].max(delta);
            game.elements[id as usize]=Some(Element::Ball(ball));game.refresh_bound(id);
        }
        elapsed=frame;
        quiet=if still {quiet+1} else {0};
        if quiet>=req.stable_frames {converged=true;break;}
    }
    let mut rows=Vec::new();let mut changed=0;let mut encodable=converged;
    let mut accepted_pushes=0;let mut accepted_falls=0;let mut rejected_movements=0;
    let mut applied=units.clone();let mut unsupported=Vec::new();
    for (slot,&id) in ids.iter().enumerate() {
        let u=&units[slot];
        let (x,y,category,state,vy) = match game.raw(id) {
            Element::Ball(b)=>(b.x-66.0,b.y-152.0,"ball",b.jump_flg,b.ys),
            Element::Wall(_)=>(u.x as f32,u.y as f32,"wall",0,0.0),
            Element::Core(_)=>(u.x as f32,u.y as f32,"core",0,0.0),_=>unreachable!(),
        };
        let (dx,dy)=(x-u.x as f32,y-u.y as f32);
        let moved=dx.abs()>EPS || dy.abs()>EPS;if moved {changed+=1;}
        let fits = slot==0 || ((x-x.round()).abs()<=EPS && (y-y.round()).abs()<=EPS
            && (0.0..=348.0).contains(&x) && (0.0..=349.0).contains(&y));
        if !fits {encodable=false;unsupported.push(slot);}
        applied[slot].x=x.round() as i32;applied[slot].y=y.round() as i32;
        // Final support includes static walls, the core and base; ordinary balls are absent.
        let support = if category=="ball" && state!=2 {
            let support_id=game.wall_hit_own_id(0,x+66.0,y+152.0+16.0);
            if support_id>=0 {Some(json!({"entityId":support_id,"slot":ids.iter().position(|&v|v==support_id),
                "kind":match game.raw(support_id) {Element::Base(_)=>"base",Element::Core(_)=>"core",_=>"wall"}}))} else {None}
        } else {None};
        // Ball::land only pushes upward when resolving existing support geometry.
        // Only balls encoded at the editor's top boundary may be pushed above it.
        let upward_allowed=max_up[slot]<=EPS
            || (category=="ball" && u.y==SUPPORT_PUSH_INITIAL_Y);
        let no_fall=max_down[slot]<=EPS;
        let support_push = category=="ball" && dx.abs()<=EPS && max_up[slot]>EPS
            && upward_allowed && no_fall;
        if support_push {accepted_pushes+=1;}
        // Target balls may deliberately start high to divert the opening ninja shot.
        let target_fall = u.type_id==42 && category=="ball" && dx.abs()<=EPS
            && max_down[slot]>EPS && upward_allowed;
        if target_fall {accepted_falls+=1;}
        if dx.abs()>EPS || !upward_allowed || (!no_fall && u.type_id!=42) {rejected_movements+=1;}
        rows.push(json!({"slot":slot,"typeId":u.type_id,"category":category,"r":u.r,
            "initial":{"x":u.x,"y":u.y},"final":{"x":x,"y":y},"delta":{"x":dx,"y":dy},
            "changed":moved,"lastChangeFrame":last_change[slot],"maxUp":max_up[slot],"maxDown":max_down[slot],
            "maxStepDown":max_step_down[slot],"acceptedSupportPush":support_push,
            "acceptedTargetFall":target_fall,
            "movementState":state,"verticalSpeed":vy,"support":support,"encodable":fits}));
    }
    let applied_code=if encodable {let c=encode(&applied);decode(&c)?;Some(c)} else {None};
    Ok(json!({"name":formation.name,"code":formation.code,"converged":converged,
        "framesSimulated":elapsed,"stableSinceFrame":if converged {Some(elapsed-req.stable_frames+1)} else {None},
        "changedUnits":changed,"acceptedSupportPushUnits":accepted_pushes,"rejectedMovementUnits":rejected_movements,
        "acceptedTargetFallUnits":accepted_falls,
        "auditPassed":rejected_movements==0,"encodable":encodable,
        "unencodableSlots":unsupported,"appliedCode":applied_code,"units":rows}))
}

fn run() -> Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().collect();
    if args.len()!=3 {return Err("用法：gkt-gravity 请求.json 输出.json".into());}
    let req:Request=serde_json::from_slice(&fs::read(&args[1])?)?;
    if req.formations.is_empty() || req.formations.len()>20000 || req.max_frames==0 || req.max_frames>2000
        || req.stable_frames<2 || req.stable_frames>req.max_frames {return Err("阵数或帧数设置非法".into());}
    let results:Result<Vec<_>,_>=req.formations.iter().map(|f|simulate(f,&req)).collect();
    let out=json!({"schema":"gkt-gravity-v1","policy":POLICY,"coordinateSpace":"kit-workbench",
        "frozenSpecialEffects":true,"maxFrames":req.max_frames,"stableFrames":req.stable_frames,
        "epsilon":EPS,"supportPushInitialY":SUPPORT_PUSH_INITIAL_Y,"formations":results?});
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(Path::new(&args[2]))?;
    serde_json::to_writer(&mut file,&out)?;file.flush()?;Ok(())
}
fn main() {if let Err(e)=run() {eprintln!("重力审核失败：{e}");std::process::exit(2);}}

#[cfg(test)]
mod tests {
    use super::*;
    fn formation(units:&[(i32,i32,i32,i32)]) -> Formation {
        Formation{name:"测试".into(),code:encode(&units.iter().enumerate().map(|(slot,&(type_id,x,y,r))|
            Unit{slot,type_id,x,y,r}).collect::<Vec<_>>())}
    }
    fn settle(f:&Formation)->Value {simulate(f,&Request{formations:vec![],max_frames:240,stable_frames:12}).unwrap()}
    #[test] fn fall_and_apply_idempotent() {
        let f=formation(&[(0,0,0,0),(7,250,0,280)]);let a=settle(&f);
        assert!(a["converged"].as_bool().unwrap());assert!(a["units"][1]["delta"]["y"].as_f64().unwrap()>300.0);
        assert_eq!(a["units"][1]["support"]["kind"],"base");
        let b=settle(&Formation{name:f.name,code:a["appliedCode"].as_str().unwrap().into()});
        assert_eq!(b["auditPassed"],true);assert_eq!(b["appliedCode"],a["appliedCode"]);
    }
    #[test] fn wall_push_up_and_core_shape() {
        let f=formation(&[(0,0,130,0),(25,100,100,0),(10,100,100,330)]);let a=settle(&f);
        assert!(a["units"][2]["delta"]["y"].as_f64().unwrap()<0.0);
        assert_eq!(a["units"][2]["support"]["slot"],1);
        let a=settle(&formation(&[(0,100,130,0),(7,136,160,280)]));
        assert_eq!(a["units"][1]["support"]["kind"],"core");
    }
    #[test] fn balls_do_not_support_each_other() {
        let a=settle(&formation(&[(0,0,0,0),(7,250,0,280),(9,250,100,338)]));
        assert_eq!(a["units"][1]["final"],a["units"][2]["final"]);
    }
    #[test] fn all_wall_effects_frozen() {
        // Wall IDs 55..60 include timed launch/rotation/snipe/elevator. Geometry only.
        for id in [25,29,30,55,56,57,58,59,60] {
            let a=settle(&formation(&[(0,0,0,0),(id,200,150,0),(7,200,145,280)]));
            for axis in ["x","y"] {assert_eq!(a["units"][1]["initial"][axis].as_f64(),a["units"][1]["final"][axis].as_f64());}
            assert_eq!(a["units"][2]["final"]["y"],117.0);assert_eq!(a["units"][2]["r"],280);
        }
    }
    #[test] fn no_clamping_or_false_convergence() {
        let f=formation(&[(0,0,0,0),(25,200,0,0),(7,200,0,280)]);let a=settle(&f);
        assert_eq!(a["converged"],true);assert_eq!(a["encodable"],false);assert!(a["appliedCode"].is_null());
        assert_eq!(a["auditPassed"],true);assert_eq!(a["units"][2]["acceptedSupportPush"],true);
        let f=formation(&[(0,0,0,0),(7,250,0,280)]);
        let a=simulate(&f,&Request{formations:vec![],max_frames:2,stable_frames:2}).unwrap();
        assert_eq!(a["converged"],false);assert_eq!(a["auditPassed"],false);assert!(a["appliedCode"].is_null());
    }
    #[test] fn support_push_only_allowed_from_y_zero() {
        for y in [0,1,100] {
            let a=settle(&formation(&[(0,0,130,0),(25,200,y,0),(7,200,y,280)]));
            assert_eq!(a["auditPassed"],y==0);
            assert_eq!(a["acceptedSupportPushUnits"],if y==0 {1} else {0});
            assert_eq!(a["units"][2]["maxUp"],33.0);
        }
        // The second overlapping wall pushes the same ball another wall upward.
        let a=settle(&formation(&[(0,0,130,0),(25,200,100,0),(25,200,66,0),(7,200,100,280)]));
        assert!(a["units"][3]["maxUp"].as_f64().unwrap()>35.0);
        assert_eq!(a["auditPassed"],false);
        let a=settle(&formation(&[(0,100,130,0),(7,136,160,280)]));
        assert!(a["units"][1]["maxUp"].as_f64().unwrap()>35.0);
        assert_eq!(a["auditPassed"],false);
    }
    #[test] fn y_zero_core_support_push_passes_without_encoding_negative_y() {
        for type_id in [7,42] {
            let a=settle(&formation(&[(0,100,0,0),(type_id,136,0,0)]));
            assert!(a["units"][1]["maxUp"].as_f64().unwrap()>EPS as f64);
            assert_eq!(a["auditPassed"],true);
            assert_eq!(a["units"][1]["acceptedSupportPush"],true);
            assert_eq!(a["encodable"],false);assert!(a["appliedCode"].is_null());
        }
    }
    #[test] fn target_fall_exemption_does_not_require_convergence_or_exempt_nonzero_push() {
        let f=formation(&[(0,0,0,0),(42,250,0,0)]);let a=settle(&f);
        assert_eq!(a["auditPassed"],true);assert_eq!(a["acceptedTargetFallUnits"],1);
        assert!(a["units"][1]["maxDown"].as_f64().unwrap()>300.0);
        let a=simulate(&f,&Request{formations:vec![],max_frames:2,stable_frames:2}).unwrap();
        assert_eq!(a["converged"],false);assert_eq!(a["auditPassed"],true);
        let a=settle(&formation(&[(0,0,130,0),(25,200,1,0),(42,200,1,0)]));
        assert_eq!(a["units"][2]["maxUp"],33.0);
        assert_eq!(a["auditPassed"],false);
    }
    #[test] fn reject_malformed() {for code in ["000000","0Z0000","中国阵","0abcde/"] {assert!(decode(code).is_err());}}
}
