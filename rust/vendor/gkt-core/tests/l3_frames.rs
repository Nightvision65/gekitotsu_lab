//! L3：逐帧状态对拍 Java 金样（`fixtures/golden/frames_*.bin`，由 `GameDump frames` 生成）。
//!
//! 二进制（大端）：magic 'GKTF' u32, version u32, count u32；随后 count 条帧记录；末尾 trailer。
//! 帧记录：time i32, rng i32, mid i32, hp[2] i32, hp0_flg[2] i32, dokkan[2] u8, norikomi u8,
//! base0/1: x,y,xs,ys,xx,yy f32（死亡写 NaN）, core0/1: x,y f32（死亡写 NaN）,
//! alive i32, 然后 alive × (id i32, kind u8, x f32, y f32)。

use gkt_core::decode::compile_fort;
use gkt_core::engine::GameTask;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> u8 {
        let v = self.data[self.pos];
        self.pos += 1;
        v
    }

    fn i32(&mut self) -> i32 {
        let v = i32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        v
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.i32() as u32)
    }
}

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn eq_f32(got: f32, want: f32, frame: usize, field: &str) {
    if got.to_bits() != want.to_bits() {
        panic!(
            "frame {frame} {field}: got 0x{:08x} ({got:?}), want 0x{:08x} ({want:?})",
            got.to_bits(),
            want.to_bits()
        );
    }
}

fn eq_i32(got: i32, want: i32, frame: usize, field: &str) {
    if got != want {
        panic!("frame {frame} {field}: got {got}, want {want}");
    }
}

fn check_frame(r: &mut Reader, game: &GameTask) {
    let frame = game.time as usize;
    eq_i32(r.i32(), game.time, frame, "time");
    eq_i32(r.i32(), game.rng_seed, frame, "rng");
    eq_i32(r.i32(), game.mid, frame, "mid");
    eq_i32(r.i32(), game.hp[0], frame, "hp0");
    eq_i32(r.i32(), game.hp[1], frame, "hp1");
    eq_i32(r.i32(), game.hp0_flg[0], frame, "hp0_flg0");
    eq_i32(r.i32(), game.hp0_flg[1], frame, "hp0_flg1");
    eq_i32(r.u8() as i32, game.dokkan_flg[0] as i32, frame, "dokkan0");
    eq_i32(r.u8() as i32, game.dokkan_flg[1] as i32, frame, "dokkan1");
    eq_i32(r.u8() as i32, game.norikomi_flg as i32, frame, "norikomi");
    for side in 0..2 {
        if game.base_id[side] < 0 {
            for field in ["x", "y", "xs", "ys", "xx", "yy"] {
                let want = r.f32();
                assert!(
                    want.is_nan(),
                    "frame {frame} base{side}.{field}: java NaN expected"
                );
            }
        } else {
            let b = game.base(side);
            eq_f32(r.f32(), b.x, frame, "base.x");
            eq_f32(r.f32(), b.y, frame, "base.y");
            eq_f32(r.f32(), b.xs, frame, "base.xs");
            eq_f32(r.f32(), b.ys, frame, "base.ys");
            eq_f32(r.f32(), b.xx, frame, "base.xx");
            eq_f32(r.f32(), b.yy, frame, "base.yy");
        }
    }
    for side in 0..2 {
        if game.core_id[side] < 0 {
            let want = r.f32();
            assert!(
                want.is_nan(),
                "frame {frame} core{side}.x: java NaN expected"
            );
            let want = r.f32();
            assert!(
                want.is_nan(),
                "frame {frame} core{side}.y: java NaN expected"
            );
        } else {
            let (cx, cy) = game.core_position(side);
            eq_f32(r.f32(), cx, frame, "core.x");
            eq_f32(r.f32(), cy, frame, "core.y");
        }
    }

    let alive_java = r.i32();
    let mut live: Vec<(i32, u8, f32, f32)> = Vec::new();
    for id in 0..game.next_id {
        if game.is_alive(id) {
            let e = game.raw(id);
            let (x, y) = e.xy();
            live.push((id, e.kind_code(), x, y));
        }
    }
    if alive_java as usize != live.len() {
        panic!(
            "frame {frame} alive count: rust {} {:?}, java {}",
            live.len(),
            live,
            alive_java
        );
    }
    for (j, rust) in live.iter().enumerate() {
        let jid = r.i32();
        let jkind = r.u8();
        let jx = r.f32();
        let jy = r.f32();
        eq_i32(jid, rust.0, frame, "elem.id");
        eq_i32(jkind as i32, rust.1 as i32, frame, "elem.kind");
        eq_f32(jx, rust.2, frame, &format!("elem[{jkind}@{}].x", rust.0));
        eq_f32(jy, rust.3, frame, &format!("elem[{jkind}@{}].y", rust.0));
        let e = game.raw(rust.0);
        match jkind {
            5 | 6 => {
                let rot = r.f32();
                let xs = r.f32();
                let ys = r.f32();
                let steps = r.i32();
                let sx = r.f32();
                let sy = r.f32();
                let gkt_core::engine::Element::Laser(l) = e else {
                    panic!("frame {frame} elem {j}: java laser, rust not");
                };
                eq_f32(rot, l.rot_deg, frame, "laser.rot");
                eq_f32(xs, l.x_speed, frame, "laser.xs");
                eq_f32(ys, l.y_speed, frame, "laser.ys");
                eq_i32(steps, l.steps, frame, "laser.steps");
                eq_f32(sx, l.start_x, frame, "laser.startX");
                eq_f32(sy, l.start_y, frame, "laser.startY");
            }
            7..=19 => {
                let hp = r.i32();
                let breaking = r.i32();
                let gkt_core::engine::Element::Wall(w) = e else {
                    panic!("frame {frame} elem {j}: java wall, rust not");
                };
                eq_i32(hp, w.hp, frame, "wall.hp");
                eq_i32(breaking, w.breaking, frame, "wall.breaking");
                use gkt_core::engine::WallKind;
                match jkind {
                    14 => {
                        let cnt = r.i32();
                        let WallKind::Far { cnt: rcnt, .. } = w.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(cnt, rcnt, frame, "far.cnt");
                    }
                    15 | 16 => {
                        let cnt = r.i32();
                        let mode = r.u8();
                        let (rcnt, rmode) = match w.kind {
                            WallKind::Wide { cnt, mode, .. }
                            | WallKind::Narrow { cnt, mode, .. } => (cnt, mode),
                            _ => panic!("frame {frame} elem {j}: kind mismatch"),
                        };
                        eq_i32(cnt, rcnt, frame, "wall.cnt");
                        eq_i32(mode as i32, rmode as i32, frame, "wall.mode");
                    }
                    18 => {
                        let ys = r.i32();
                        let WallKind::Elevator { ys: rys } = w.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(ys, rys, frame, "elevator.ys");
                    }
                    19 => {
                        let on_side = r.i32();
                        let ys = r.f32();
                        let WallKind::Box {
                            on_side: ros,
                            ys: rys,
                        } = w.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(on_side, ros, frame, "box.on_side");
                        eq_f32(ys, rys, frame, "box.ys");
                    }
                    _ => {}
                }
            }
            21 => {
                let frames = r.i32();
                let gkt_core::engine::Element::HitsDropFrames(h) = e else {
                    panic!("frame {frame} elem {j}: java hitsdrop, rust not");
                };
                eq_i32(frames, h.frames, frame, "hitsdrop.frames");
            }
            22..=28 | 37..=44 | 54..=61 | 87..=107 | 116..=120 => {
                let hp = r.i32();
                let max_hp = r.i32();
                let jump_flg = r.i32();
                let on_side = r.i32();
                let cnt = r.i32();
                let rot = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let cos_rot = r.f32();
                let sin_rot = r.f32();
                let drop_y = r.f32();
                let speed = r.i32();
                let gkt_core::engine::Element::Ball(b) = e else {
                    panic!("frame {frame} elem {j}: java ball, rust not");
                };
                eq_i32(hp, b.hp, frame, "ball.hp");
                eq_i32(max_hp, b.max_hp, frame, "ball.max_hp");
                eq_i32(jump_flg, b.jump_flg, frame, "ball.jump_flg");
                eq_i32(on_side, b.on_side as i32, frame, "ball.on_side");
                eq_i32(cnt, b.cnt, frame, "ball.cnt");
                eq_i32(rot, b.rot, frame, "ball.rot");
                eq_f32(xs, b.xs, frame, "ball.xs");
                eq_f32(ys, b.ys, frame, "ball.ys");
                eq_f32(cos_rot, b.cos_rot, frame, "ball.cos_rot");
                eq_f32(sin_rot, b.sin_rot, frame, "ball.sin_rot");
                eq_f32(drop_y, b.drop_y, frame, "ball.drop_y");
                eq_i32(speed, b.speed, frame, "ball.speed");
                use gkt_core::engine::BallKind;
                match jkind {
                    24 => {
                        let alarm = r.i32();
                        let BallKind::Nie { alarm: ralarm } = b.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(alarm, ralarm, frame, "nie.alarm");
                    }
                    26 => {
                        let hurt_time = r.i32();
                        let flipped = r.u8();
                        let BallKind::Tate {
                            flipped: rflipped,
                            hurt_time: rht,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hurt_time, rht, frame, "tate.hurt_time");
                        eq_i32(flipped as i32, rflipped as i32, frame, "tate.flipped");
                    }
                    27 => {
                        let t_id = r.i32();
                        let BallKind::Han { t_id: rtid } = b.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(t_id, rtid, frame, "han.t_id");
                    }
                    28 => {
                        let hurt_time = r.i32();
                        let BallKind::Kekkai { hurt_time: rht } = b.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hurt_time, rht, frame, "kekkai.hurt_time");
                    }
                    41 => {
                        let shooting = r.u8();
                        let unit_x = r.i32();
                        let BallKind::Kabe {
                            shooting: rs,
                            unit_x: rux,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(shooting as i32, rs as i32, frame, "kabe.shooting");
                        eq_i32(unit_x, rux, frame, "kabe.unit_x");
                    }
                    44 => {
                        let hurt_time = r.i32();
                        let flipped = r.u8();
                        let BallKind::Knight {
                            flipped: rf,
                            hurt_time: rht,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hurt_time, rht, frame, "knight.hurt_time");
                        eq_i32(flipped as i32, rf as i32, frame, "knight.flipped");
                    }
                    _ => {}
                }
            }
            30 | 32 | 33 | 36 => {
                let hframe = r.i32();
                let user = r.i32();
                let cos_rot = r.f32();
                let sin_rot = r.f32();
                let flipped = r.u8();
                let gkt_core::engine::Element::Hit(h) = e else {
                    panic!("frame {frame} elem {j}: java hit, rust not");
                };
                eq_i32(hframe, h.frame, frame, "hit.frame");
                eq_i32(user, h.user, frame, "hit.user");
                eq_f32(cos_rot, h.cos_rot, frame, "hit.cos_rot");
                eq_f32(sin_rot, h.sin_rot, frame, "hit.sin_rot");
                eq_i32(flipped as i32, h.flipped as i32, frame, "hit.flipped");
            }
            34 => {
                let hframe = r.i32();
                let user = r.i32();
                let cos_rot = r.f32();
                let sin_rot = r.f32();
                let gkt_core::engine::Element::Hit(h) = e else {
                    panic!("frame {frame} elem {j}: java hit, rust not");
                };
                eq_i32(hframe, h.frame, frame, "kabehit.frame");
                eq_i32(user, h.user, frame, "kabehit.user");
                eq_f32(cos_rot, h.cos_rot, frame, "kabehit.cos_rot");
                eq_f32(sin_rot, h.sin_rot, frame, "kabehit.sin_rot");
            }
            31 => {
                let hframe = r.i32();
                let user = r.i32();
                let rot = r.i32();
                let n = r.i32();
                let gkt_core::engine::Element::HitYari(hy) = e else {
                    panic!("frame {frame} elem {j}: java yari, rust not");
                };
                eq_i32(hframe, hy.frame, frame, "yari.frame");
                eq_i32(user, hy.user, frame, "yari.user");
                eq_i32(rot, hy.rot, frame, "yari.rot");
                assert_eq!(
                    n as usize,
                    hy.inner.shapes.len(),
                    "frame {frame} yari child count"
                );
                for k in 0..n as usize {
                    let cx = r.f32();
                    let cy = r.f32();
                    let flip_rad = r.i32();
                    let gkt_core::geom::Shape::Yari(yb) = &hy.inner.shapes[k] else {
                        panic!("frame {frame} yari child {k} not YariBase");
                    };
                    eq_f32(cx, yb.x, frame, "yari.child.x");
                    eq_f32(cy, yb.y, frame, "yari.child.y");
                    eq_i32(flip_rad, yb.flip_rad, frame, "yari.child.flip_rad");
                }
            }
            35 => {
                let rr = r.f32();
                let gkt_core::engine::Element::HitsHeal(hh) = e else {
                    panic!("frame {frame} elem {j}: java hitsheal, rust not");
                };
                eq_f32(rr, hh.r, frame, "hitsheal.r");
            }
            45 => {
                let fr = r.i32();
                let gkt_core::engine::Element::HitsBomb(hb) = e else {
                    panic!("frame {frame} elem {j}: java hitsbomb, rust not");
                };
                eq_i32(fr, hb.frame, frame, "hitsbomb.frame");
                assert!(!hb.is_mult, "frame {frame}: rust hitsbomb is_mult");
            }
            46 => {
                let fr = r.i32();
                let mult = r.f32();
                let gkt_core::engine::Element::HitsBomb(hb) = e else {
                    panic!("frame {frame} elem {j}: java hitsbomb, rust not");
                };
                eq_i32(fr, hb.frame, frame, "hitsbomb.frame");
                eq_f32(mult, hb.mult, frame, "hitsbomb.mult");
                assert!(hb.is_mult, "frame {frame}: rust hitsbomb is_mult");
            }
            47 | 48 => {
                let xs = r.f32();
                let ys = r.f32();
                let gkt_core::engine::Element::ShaRay(ray) = e else {
                    panic!("frame {frame} elem {j}: java sharay, rust not");
                };
                eq_f32(xs, ray.xs, frame, "sharay.xs");
                eq_f32(ys, ray.ys, frame, "sharay.ys");
            }
            108 | 110 => {
                let rot_deg = r.f32();
                let x_speed = r.f32();
                let y_speed = r.f32();
                let steps = r.i32();
                let start_x = r.f32();
                let start_y = r.f32();
                let gkt_core::engine::Element::Laser(l) = e else {
                    panic!("frame {frame} elem {j}: java laser, rust not");
                };
                eq_f32(rot_deg, l.rot_deg, frame, "laser.rot_deg");
                eq_f32(x_speed, l.x_speed, frame, "laser.x_speed");
                eq_f32(y_speed, l.y_speed, frame, "laser.y_speed");
                eq_i32(steps, l.steps, frame, "laser.steps");
                eq_f32(start_x, l.start_x, frame, "laser.start_x");
                eq_f32(start_y, l.start_y, frame, "laser.start_y");
                if jkind == 110 {
                    let cnt = r.i32();
                    eq_i32(cnt, l.cnt, frame, "starlaser.cnt");
                }
            }
            109 => {
                let rot_deg = r.i32();
                let x_speed = r.f32();
                let y_speed = r.f32();
                let start_x = r.f32();
                let start_y = r.f32();
                let steps = r.i32();
                let cnt = r.i32();
                let old_rot_deg = r.f32();
                let side = r.i32();
                let gkt_core::engine::Element::StarPrepare(sp) = e else {
                    panic!("frame {frame} elem {j}: java starprepare, rust not");
                };
                eq_i32(rot_deg, sp.rot_deg as i32, frame, "sp.rot_deg");
                eq_f32(x_speed, sp.x_speed, frame, "sp.x_speed");
                eq_f32(y_speed, sp.y_speed, frame, "sp.y_speed");
                eq_f32(start_x, sp.start_x, frame, "sp.start_x");
                eq_f32(start_y, sp.start_y, frame, "sp.start_y");
                eq_i32(steps, sp.steps, frame, "sp.steps");
                eq_i32(cnt, sp.cnt, frame, "sp.cnt");
                eq_f32(old_rot_deg, sp.old_rot_deg, frame, "sp.old_rot_deg");
                eq_i32(side, sp.side as i32, frame, "sp.side");
            }
            111 => {
                let gei_flg = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let gravity = r.f32();
                let power = r.f32();
                let side = r.i32();
                let cnt = r.i32();
                let target_rot = r.f32();
                let target_id = r.i32();
                let rot = r.f32();
                let target_rot_delta = r.f32();
                let rr = r.f32();
                let gkt_core::engine::Element::Bullet(b) = e else {
                    panic!("frame {frame} elem {j}: java bossmissile, rust not");
                };
                eq_i32(gei_flg, b.gei_flg, frame, "bm.gei_flg");
                eq_f32(xs, b.xs, frame, "bm.xs");
                eq_f32(ys, b.ys, frame, "bm.ys");
                eq_f32(gravity, b.gravity, frame, "bm.gravity");
                eq_f32(power, b.power, frame, "bm.power");
                eq_i32(side, b.side as i32, frame, "bm.side");
                let gkt_core::engine::BulletKind::BossMissile {
                    cnt: c,
                    target_rot: tr,
                    target_id: tid,
                    rot: ro,
                    target_rot_delta: trd,
                } = b.kind
                else {
                    panic!("frame {frame} elem {j}: kind mismatch");
                };
                eq_i32(cnt, c, frame, "bm.cnt");
                eq_f32(target_rot, tr, frame, "bm.target_rot");
                eq_i32(target_id, tid, frame, "bm.target_id");
                eq_f32(rot, ro, frame, "bm.rot");
                eq_f32(target_rot_delta, trd, frame, "bm.target_rot_delta");
                eq_f32(rr, b.r, frame, "bm.r");
            }
            112 => {
                let hp = r.i32();
                let xuv = r.f32();
                let yuv = r.f32();
                let speed = r.f32();
                let gkt_core::engine::Element::Creature(c) = e else {
                    panic!("frame {frame} elem {j}: java creature, rust not");
                };
                eq_i32(hp, c.hp, frame, "drill.hp");
                eq_f32(xuv, c.xuv, frame, "drill.xuv");
                eq_f32(yuv, c.yuv, frame, "drill.yuv");
                eq_f32(speed, c.speed, frame, "drill.speed");
            }
            113 => {
                let hp = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let rot = r.f32();
                let gkt_core::engine::Element::Creature(c) = e else {
                    panic!("frame {frame} elem {j}: java creature, rust not");
                };
                eq_i32(hp, c.hp, frame, "solid.hp");
                eq_f32(xs, c.xs, frame, "solid.xs");
                eq_f32(ys, c.ys, frame, "solid.ys");
                eq_f32(rot, c.rot, frame, "solid.rot");
            }
            114 => {
                let hp = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let rot = r.f32();
                let cnt = r.i32();
                let yrot = r.i32();
                let ycos = r.f32();
                let ysin = r.f32();
                let gkt_core::engine::Element::Creature(c) = e else {
                    panic!("frame {frame} elem {j}: java creature, rust not");
                };
                eq_i32(hp, c.hp, frame, "shock.hp");
                eq_f32(xs, c.xs, frame, "shock.xs");
                eq_f32(ys, c.ys, frame, "shock.ys");
                eq_f32(rot, c.rot, frame, "shock.rot");
                eq_i32(cnt, c.cnt, frame, "shock.cnt");
                eq_i32(yrot, c.yrot, frame, "shock.yrot");
                eq_f32(ycos, c.ycos, frame, "shock.ycos");
                eq_f32(ysin, c.ysin, frame, "shock.ysin");
            }
            115 => {
                let hp = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let rot = r.f32();
                let xs2 = r.f32();
                let gkt_core::engine::Element::Creature(c) = e else {
                    panic!("frame {frame} elem {j}: java creature, rust not");
                };
                eq_i32(hp, c.hp, frame, "weak.hp");
                eq_f32(xs, c.xs, frame, "weak.xs");
                eq_f32(ys, c.ys, frame, "weak.ys");
                eq_f32(rot, c.rot, frame, "weak.rot");
                eq_f32(xs2, c.xs2, frame, "weak.xs2");
            }
            62..=81 => {
                let gei_flg = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let gravity = r.f32();
                let power = r.f32();
                let side = r.i32();
                let gkt_core::engine::Element::Bullet(b) = e else {
                    panic!("frame {frame} elem {j}: java bullet, rust not");
                };
                eq_i32(gei_flg, b.gei_flg, frame, "bullet.gei_flg");
                eq_f32(xs, b.xs, frame, "bullet.xs");
                eq_f32(ys, b.ys, frame, "bullet.ys");
                eq_f32(gravity, b.gravity, frame, "bullet.gravity");
                eq_f32(power, b.power, frame, "bullet.power");
                eq_i32(side, b.side as i32, frame, "bullet.side");
                use gkt_core::engine::BulletKind;
                match jkind {
                    63 => {
                        let cnt = r.i32();
                        let hp = r.i32();
                        let speed = r.f32();
                        let rot = r.i32();
                        let cos_rot = r.f32();
                        let sin_rot = r.f32();
                        let BulletKind::Con {
                            speed: sp,
                            hp: h,
                            cnt: c,
                            rot: rr,
                            cos_rot: cr,
                            sin_rot: sr,
                            ..
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(cnt, c, frame, "con.cnt");
                        eq_i32(hp, h, frame, "con.hp");
                        eq_f32(speed, sp, frame, "con.speed");
                        eq_i32(rot, rr, frame, "con.rot");
                        eq_f32(cos_rot, cr, frame, "con.cos_rot");
                        eq_f32(sin_rot, sr, frame, "con.sin_rot");
                    }
                    64 => {
                        let cnt = r.i32();
                        let target_id = r.i32();
                        let speed = r.f32();
                        let desired_rot = r.f32();
                        let rot = r.f32();
                        let BulletKind::ConMissile {
                            speed: sp,
                            cnt: c,
                            target_id: t,
                            desired_rot: dr,
                            rot: rr,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(cnt, c, frame, "conm.cnt");
                        eq_i32(target_id, t, frame, "conm.target");
                        eq_f32(speed, sp, frame, "conm.speed");
                        eq_f32(desired_rot, dr, frame, "conm.desired_rot");
                        eq_f32(rot, rr, frame, "conm.rot");
                    }
                    65 => {
                        let speed = r.f32();
                        let cos_rot = r.f32();
                        let sin_rot = r.f32();
                        let BulletKind::Geki {
                            speed: sp,
                            cos_rot: cr,
                            sin_rot: sr,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_f32(speed, sp, frame, "geki.speed");
                        eq_f32(cos_rot, cr, frame, "geki.cos_rot");
                        eq_f32(sin_rot, sr, frame, "geki.sin_rot");
                    }
                    66 | 69 => {
                        let cnt = r.i32();
                        let rc = match b.kind {
                            BulletKind::Hanabi { cnt, .. } | BulletKind::Hinoko { cnt } => cnt,
                            _ => panic!("frame {frame} elem {j}: kind mismatch"),
                        };
                        eq_i32(cnt, rc, frame, "cnt");
                    }
                    67 | 73 => {
                        let bounce = r.i32();
                        let rb = match b.kind {
                            BulletKind::Hane { bounce } | BulletKind::Nin { bounce } => bounce,
                            _ => panic!("frame {frame} elem {j}: kind mismatch"),
                        };
                        eq_i32(bounce, rb, frame, "bounce");
                    }
                    68 => {
                        let hp = r.i32();
                        let cnt = r.i32();
                        let yrot = r.i32();
                        let speed = r.f32();
                        let rot = r.f32();
                        let BulletKind::Heri {
                            rot: rr,
                            speed: sp,
                            hp: h,
                            cnt: c,
                            yrot: yr,
                            ..
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "heri.hp");
                        eq_i32(cnt, c, frame, "heri.cnt");
                        eq_i32(yrot, yr, frame, "heri.yrot");
                        eq_f32(speed, sp, frame, "heri.speed");
                        eq_f32(rot, rr, frame, "heri.rot");
                    }
                    70 => {
                        let hp = r.i32();
                        let speed = r.f32();
                        let cos_rot = r.f32();
                        let sin_rot = r.f32();
                        let BulletKind::Kaku {
                            speed: sp,
                            hp: h,
                            cos_rot: cr,
                            sin_rot: sr,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "kaku.hp");
                        eq_f32(speed, sp, frame, "kaku.speed");
                        eq_f32(cos_rot, cr, frame, "kaku.cos_rot");
                        eq_f32(sin_rot, sr, frame, "kaku.sin_rot");
                    }
                    72 => {
                        let cnt = r.i32();
                        let speed = r.f32();
                        let rot = r.f32();
                        let BulletKind::Missile {
                            cnt: c,
                            speed: sp,
                            rot: rr,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(cnt, c, frame, "missile.cnt");
                        eq_f32(speed, sp, frame, "missile.speed");
                        eq_f32(rot, rr, frame, "missile.rot");
                    }
                    75 => {
                        let hp = r.i32();
                        let cnt = r.i32();
                        let drop_flg = r.u8();
                        let BulletKind::Sen {
                            hp: h,
                            cnt: c,
                            drop_flg: d,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "sen.hp");
                        eq_i32(cnt, c, frame, "sen.cnt");
                        eq_i32(drop_flg as i32, d as i32, frame, "sen.drop_flg");
                    }
                    76 => {
                        let hp = r.i32();
                        let BulletKind::SenTama { hp: h } = b.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "sentama.hp");
                    }
                    77 => {
                        let hp = r.i32();
                        let cnt = r.i32();
                        let speed = r.f32();
                        let rot = r.f32();
                        let BulletKind::Tobi {
                            rot: rr,
                            speed: sp,
                            hp: h,
                            cnt: c,
                            ..
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "tobi.hp");
                        eq_i32(cnt, c, frame, "tobi.cnt");
                        eq_f32(speed, sp, frame, "tobi.speed");
                        eq_f32(rot, rr, frame, "tobi.rot");
                    }
                    79 => {
                        let hp = r.i32();
                        let bounce = r.i32();
                        let BulletKind::Ton { hp: h, bounce: b2 } = b.kind else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "ton.hp");
                        eq_i32(bounce, b2, frame, "ton.bounce");
                    }
                    80 => {
                        let hp = r.i32();
                        let cnt = r.i32();
                        let drop_flg = r.u8();
                        let BulletKind::Tui {
                            hp: h,
                            cnt: c,
                            drop_flg: d,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_i32(hp, h, frame, "tui.hp");
                        eq_i32(cnt, c, frame, "tui.cnt");
                        eq_i32(drop_flg as i32, d as i32, frame, "tui.drop_flg");
                    }
                    81 => {
                        let speed = r.f32();
                        let cos_rot = r.f32();
                        let sin_rot = r.f32();
                        let BulletKind::TuiMissile {
                            speed: sp,
                            cos_rot: cr,
                            sin_rot: sr,
                        } = b.kind
                        else {
                            panic!("frame {frame} elem {j}: kind mismatch");
                        };
                        eq_f32(speed, sp, frame, "tuim.speed");
                        eq_f32(cos_rot, cr, frame, "tuim.cos_rot");
                        eq_f32(sin_rot, sr, frame, "tuim.sin_rot");
                    }
                    _ => {}
                }
            }
            82 => {
                let user_id = r.i32();
                let target_id = r.i32();
                let gkt_core::engine::Element::GeiLink(gl) = e else {
                    panic!("frame {frame} elem {j}: java geilink, rust not");
                };
                eq_i32(user_id, gl.user_id, frame, "geilink.user");
                eq_i32(target_id, gl.target_id, frame, "geilink.target");
            }
            83 => {
                let cnt = r.i32();
                let t_type = r.i32();
                let t_rot = r.i32();
                let t_side = r.i32();
                let t_on_side = r.i32();
                let t_jump_flg = r.i32();
                let gkt_core::engine::Element::HolyGhost(hg) = e else {
                    panic!("frame {frame} elem {j}: java holy, rust not");
                };
                eq_i32(cnt, hg.cnt, frame, "holy.cnt");
                eq_i32(t_type, hg.t_type, frame, "holy.t_type");
                eq_i32(t_rot, hg.t_rot, frame, "holy.t_rot");
                eq_i32(t_side, hg.t_side as i32, frame, "holy.t_side");
                eq_i32(t_on_side, hg.t_on_side as i32, frame, "holy.t_on_side");
                eq_i32(t_jump_flg, hg.t_jump_flg, frame, "holy.t_jump_flg");
            }
            84 => {
                let hp = r.i32();
                let cnt = r.i32();
                let power = r.f32();
                let active = r.u8();
                let gkt_core::engine::Element::SyouShield(sh) = e else {
                    panic!("frame {frame} elem {j}: java syou, rust not");
                };
                eq_i32(hp, sh.hp, frame, "syou.hp");
                eq_i32(cnt, sh.cnt, frame, "syou.cnt");
                eq_f32(power, sh.power, frame, "syou.power");
                eq_i32(active as i32, sh.active as i32, frame, "syou.active");
            }
            85 => {
                let dir_x = r.f32();
                let dir_y = r.f32();
                let xs = r.f32();
                let ys = r.f32();
                let gkt_core::engine::Element::SearchShot(ss) = e else {
                    panic!("frame {frame} elem {j}: java search, rust not");
                };
                eq_f32(dir_x, ss.dir_x, frame, "search.dir_x");
                eq_f32(dir_y, ss.dir_y, frame, "search.dir_y");
                eq_f32(xs, ss.xs, frame, "search.xs");
                eq_f32(ys, ss.ys, frame, "search.ys");
            }
            86 => {
                let hp = r.i32();
                let yrot = r.i32();
                let ysin = r.f32();
                let ycos = r.f32();
                let gkt_core::engine::Element::UkiShot(us) = e else {
                    panic!("frame {frame} elem {j}: java uki, rust not");
                };
                eq_i32(hp, us.hp, frame, "uki.hp");
                eq_i32(yrot, us.yrot, frame, "uki.yrot");
                eq_f32(ysin, us.ysin, frame, "uki.ysin");
                eq_f32(ycos, us.ycos, frame, "uki.ycos");
            }
            49..=53 => {
                let gei_flg = r.i32();
                let xs = r.f32();
                let ys = r.f32();
                let gravity = r.f32();
                let power = r.f32();
                let side = r.i32();
                let gkt_core::engine::Element::Bullet(b) = e else {
                    panic!("frame {frame} elem {j}: java bullet, rust not");
                };
                eq_i32(gei_flg, b.gei_flg, frame, "bullet.gei_flg");
                eq_f32(xs, b.xs, frame, "bullet.xs");
                eq_f32(ys, b.ys, frame, "bullet.ys");
                eq_f32(gravity, b.gravity, frame, "bullet.gravity");
                eq_f32(power, b.power, frame, "bullet.power");
                eq_i32(side, b.side as i32, frame, "bullet.side");
                use gkt_core::engine::BulletKind;
                if jkind == 51 {
                    let hp = r.i32();
                    let BulletKind::Cannon { hp: rhp } = b.kind else {
                        panic!("frame {frame} elem {j}: kind mismatch");
                    };
                    eq_i32(hp, rhp, frame, "cannon.hp");
                }
                if jkind == 52 {
                    let cnt = r.i32();
                    let BulletKind::Shotgun { cnt: rcnt } = b.kind else {
                        panic!("frame {frame} elem {j}: kind mismatch");
                    };
                    eq_i32(cnt, rcnt, frame, "shotgun.cnt");
                }
            }
            _ => {}
        }
    }
}

fn check_case(file: &str, code1: &str, code2: &str, seed: Option<i32>, max_frames: i32) {
    // frames_*.bin 体积较大，未随开源仓库分发；缺失时跳过（可用 oracle 重新生成，见 README）
    let Ok(data) = std::fs::read(root().join("fixtures/golden").join(file)) else {
        eprintln!("skip {file}: golden frames not distributed; regenerate via tools/gensrc/regen_engine.ps1");
        return;
    };
    let mut r = Reader {
        data: &data,
        pos: 0,
    };
    assert_eq!(r.i32(), 0x474B_5446, "{file} magic");
    assert_eq!(r.i32(), 1, "{file} version");
    let count = r.i32();

    let f1 = compile_fort("", code1);
    let f2 = compile_fort("", code2);
    let (task, res) = GameTask::run_single_with(&f1, &f2, max_frames, seed, |g| {
        check_frame(&mut r, g);
    });

    assert_eq!(count, task.time, "{file} frame count");
    eq_i32(r.i32(), task.rng_seed, count as usize, "trailer.rng");
    eq_i32(r.i32(), res.status, count as usize, "trailer.status");
    eq_i32(r.i32(), res.winner_hp, count as usize, "trailer.winnerHp");
    eq_i32(
        r.i32(),
        res.frame_passed,
        count as usize,
        "trailer.framePassed",
    );
    assert_eq!(r.pos, r.data.len(), "{file} trailing bytes");
}

#[test]
fn l3_frames_match_java_golden() {
    check_case("frames_core.bin", "0001dw", "000000", None, 3000);
    check_case("frames_boss.bin", "1001dw", "0001dw", Some(1), 3000);
    check_case("frames_zz.bin", "0zzzzz", "0zzzzz", Some(2), 2500);
    check_case("frames_boss2.bin", "100000", "100000", Some(12345), 2100);
    check_case(
        "frames_walls.bin",
        "0001dwp001dwq001dwr001dws001dwt001dwu001dwT001dwU001dw",
        "0001dw",
        Some(12345),
        1500,
    );
    check_case("frames_far.bin", "0001dwU001dw", "0001dw", Some(7), 1200);
    check_case(
        "frames_jet.bin",
        "0001dwt001dwu001dw",
        "0001dw",
        Some(3),
        2500,
    );
    check_case(
        "frames_balls.bin",
        "0001dw00000z40000zo0000zC0000zF0000zG0000zL0000z",
        "0001dw",
        Some(12345),
        2500,
    );
    check_case(
        "frames_balls2.bin",
        "0001dw00000z4000z0o00z00C0z000F0000zG000z0L00z00",
        "1001dw",
        Some(7),
        3000,
    );
    check_case(
        "frames_melee.bin",
        "0001dw3001dw8001dwa001dwv001dwj001dwi001dwh001dwM001dw",
        "1001dw",
        Some(12345),
        2000,
    );
    check_case(
        "frames_melee2.bin",
        "0001dw3001dw8001dwa001dwv001dwj001dwi001dwh001dwM001dw",
        "0001dw3001dw8001dwa001dwv001dwj001dwi001dwh001dwM001dw",
        Some(7),
        2500,
    );
    check_case(
        "frames_ranged.bin",
        "0001dw1001dw2001dw7001dw9001dwc001dwd001dwe001dwm001dw",
        "1001dw",
        Some(12345),
        2000,
    );
    check_case(
        "frames_ranged2.bin",
        "0001dw1001dw2001dw7001dw9001dwc001dwd001dwe001dwm001dw",
        "0001dw1001dw2001dw7001dw9001dwc001dwd001dwe001dwm001dw",
        Some(7),
        2500,
    );
    check_case(
        "frames_c2.bin",
        "0001dw5001dwB001dwD001dwE001dwH001dwK001dwN001dwO001dwQ001dwR001dwS001dwb001dwf001dwg001dwk001dwl001dww001dwx001dwy001dwz001dw",
        "1001dw",
        Some(12345),
        2500,
    );
    check_case(
        "frames_c2b.bin",
        "0001dw5001dwB001dwD001dwE001dwH001dwK001dwN001dwO001dwQ001dwR001dwS001dwb001dwf001dwg001dwk001dwl001dww001dwx001dwy001dwz001dw",
        "0001dw5001dwB001dwD001dwE001dwH001dwK001dwN001dwO001dwQ001dwR001dwS001dwb001dwf001dwg001dwk001dwl001dww001dwx001dwy001dwz001dw",
        Some(7),
        3000,
    );
    check_case(
        "frames_wd.bin",
        "0001dw6001dwn001dwI001dwJ001dwP001dw",
        "1001dw",
        Some(12345),
        2500,
    );
    check_case("frames_wd2.bin", "2001dw", "2001dw", Some(7), 3000);
}
