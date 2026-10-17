//! catalog-hp-v1：解码生成与修理上限跨三个Rust核心保持一致。
fn block(kind: char, x: u32, y: u32) -> String {
    let alphabet = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut value = x * 1000 + y;
    let mut digits = [b'0'; 5];
    for i in (0..5).rev() { digits[i] = alphabet[(value % 61) as usize]; value /= 61; }
    format!("{}{}", kind, std::str::from_utf8(&digits).unwrap())
}
macro_rules! hp_contract {
    ($name:ident, $core:ident) => {
        #[test]
        fn $name() {
            use $core::engine::{Element, GameTask, ParentList, Wall, WallKind};
            let core_code = block('0', 152, 158);
            let code = format!("{}{}{}{}{}", core_code, block('u',116,120), block('Y',156,120), block('p',196,120), block('t',236,120));
            let p1 = $core::decode::compile_fort("HP修正规则", &code);
            let p2 = $core::decode::compile_fort("对侧核心", &core_code);
            let (mut game, _) = GameTask::run_single(&p1, &p2, 0, Some(1));
            let walls: Vec<_> = game.elements.iter().filter_map(|e| match e {
                Some(Element::Wall(w)) if w.side == 0 => Some((w.type_, w.hp, w.max_hp)),
                _ => None,
            }).collect();
            assert_eq!(walls, vec![(30,100,100),(60,75,75),(25,35,35),(29,15,15)]);
            game.spawn_hits_heal(600.0, 200.0, ParentList::Repair(0), 100.0);
            for (kind, type_, cap) in [(WallKind::Turbo,30,100),(WallKind::Elevator { ys:1 },60,75)] {
                let mut w=Wall::new(-1,600.0,200.0,0,type_,kind);
                w.hp=cap-1;
                w.step(&mut game);
                assert_eq!((w.hp,w.max_hp),(cap,cap));
                w.step(&mut game);
                assert_eq!((w.hp,w.max_hp),(cap,cap));
            }
        }
    }
}
hp_contract!(plain_hp_and_repair_cap, gkt_core);
hp_contract!(basic_hp_and_repair_cap, gkt_basic_core);
hp_contract!(detailed_hp_and_repair_cap, gkt_diagnostic_core);
