//! 百科计时验证：仅隔离有效计数与分支，不模拟实际支撑、互保和来弹。
use gkt_core::engine::{Ball, BallKind, Element, GameTask};
fn block(kind: char, x: u32, y: u32) -> String {
    let alphabet=b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut v=x*1000+y; let mut digits=[b'0';5];
    for i in (0..5).rev() {digits[i]=alphabet[(v%61) as usize]; v/=61;}
    format!("{}{}",kind,std::str::from_utf8(&digits).unwrap())
}
fn decoded(x:u32, kind:char) -> (GameTask, Vec<Ball>) {
    let code=format!("{}{}",block('0',152,158),block(kind,x+16,120));
    let fort=gkt_core::decode::compile_fort("百科计时", &code);
    let (game,_)=GameTask::run_single(&fort,&fort,0,Some(1));
    let balls=game.elements.iter().filter_map(|e| match e {Some(Element::Ball(b))=>Some(b.clone()),_=>None}).collect();
    (game,balls)
}
fn active(b:&Ball)->bool {matches!(b.kind,BallKind::Kabe{shooting:true,..})}
fn wall_states(x:u32, side:usize, hurt:bool)->Vec<bool> {
    let (mut game,balls)=decoded(x,'j'); let mut b=balls[side].clone();
    if hurt {b.hurt(false); assert!(!active(&b));}
    (0..1200).map(|_|{b.cnt+=1;b.step_ex(&mut game);active(&b)}).collect()
}
#[test]
fn wall_initial_and_hurt_recovery() {
    for side in 0..2 {
        for (x,initial,recovery) in [(54,70,60),(114,70,60),(174,70,60),(234,70,60),(294,70,60),(44,60,0)] {
            assert_eq!(wall_states(x,side,false).iter().position(|v|*v).unwrap()+1,initial);
            // x44 mirrored recovery differs; x54+60n is fastest on both sides.
            if x!=44 {assert_eq!(wall_states(x,side,true).iter().position(|v|*v).unwrap()+1,recovery);}
        }
        for (a,b,initial_gap,hurt_gap) in [(43,55,12,12),(53,55,58,2)] {
            for (hurt,gap) in [(false,initial_gap),(true,hurt_gap)] {
                let aa=wall_states(a,side,hurt);let bb=wall_states(b,side,hurt);
                assert_eq!((500..861).filter(|i|!aa[*i]&&!bb[*i]).count(),gap);
            }
        }
    }
}
fn launches(x:u32,kind:char)->Vec<usize> {
    let (mut game,balls)=decoded(x,kind);let mut b=balls[0].clone();let mut out=Vec::new();
    for t in 1..=600 {let before=game.elements.len();b.cnt+=1;b.step_ex(&mut game);if game.elements.len()>before {out.push(t);}}
    out
}
#[test]
fn cannon_and_crossbow_cycles() {
    let a=launches(54,'9');let b=launches(114,'9');
    assert!(a.windows(2).all(|w|w[1]-w[0]==120));
    let mut merged:Vec<_>=a.into_iter().chain(b).collect();merged.sort();
    assert!(merged.windows(2).all(|w|w[1]-w[0]==60));
    assert_eq!(launches(42,'7')[0],64);assert_eq!(launches(44,'7')[0],36);
    assert!(launches(42,'7').windows(2).all(|w|w[1]-w[0]==36));
}
