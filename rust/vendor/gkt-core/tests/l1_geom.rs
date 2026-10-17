//! L1：几何基元对拍 Java 金样（`fixtures/golden/geometry.txt`，由 `org.example.tools.GeoDump shapes` 生成）。
//!
//! 格式：`shape <kind> <args...>`；可选一行 `fields ...`（构造派生字段）；随后若干 `p <px> <py> <0|1>`。
//! 复合形状的子形状行以 `sub` 开头并递归。所有浮点均为 `0x` + f32 原始位。

use gkt_core::geom::{CompositeShape, EllipticalSector, Polygon, Rectangle, Round, Sector, Shape};

fn bits(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok.trim_start_matches("0x"), 16).unwrap())
}

struct Lines<'a> {
    lines: Vec<&'a str>,
    i: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines().filter(|l| !l.is_empty()).collect(),
            i: 0,
        }
    }

    fn next(&mut self) -> &'a str {
        let l = self.lines[self.i];
        self.i += 1;
        l
    }

    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.i).copied()
    }
}

fn parse_shape(lines: &mut Lines) -> Shape {
    let line = lines.next();
    let t: Vec<&str> = line.split(' ').collect();
    let kind = t[1];
    match kind {
        "round" => Shape::Round(Round::new(bits(t[2]), bits(t[3]), bits(t[4]))),
        "rectangle" => Shape::Rectangle(Rectangle::new(
            bits(t[2]),
            bits(t[3]),
            bits(t[4]),
            bits(t[5]),
            bits(t[6]),
            bits(t[7]),
        )),
        "polygon" => {
            let n: usize = t[4].parse().unwrap();
            let mut verts = Vec::with_capacity(n);
            for k in 0..n {
                verts.push([bits(t[5 + 2 * k]), bits(t[6 + 2 * k])]);
            }
            Shape::Polygon(Polygon::new(bits(t[2]), bits(t[3]), verts))
        }
        "sector" => Shape::Sector(Sector::new(
            bits(t[2]),
            bits(t[3]),
            bits(t[4]),
            bits(t[5]),
            bits(t[6]),
        )),
        "esector" => Shape::EllipticalSector(EllipticalSector::new(
            bits(t[2]),
            bits(t[3]),
            bits(t[4]),
            bits(t[5]),
            bits(t[6]),
            bits(t[7]),
        )),
        "composite" => {
            let n: usize = t[4].parse().unwrap();
            let mut c = CompositeShape::new(bits(t[2]), bits(t[3]));
            for _ in 0..n {
                c.add_shape(parse_shape(lines));
            }
            Shape::Composite(c)
        }
        other => panic!("unknown shape kind {other}"),
    }
}

#[test]
fn geometry_matches_java_golden() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/golden/geometry.txt");
    let text = std::fs::read_to_string(path)
        .expect("missing geometry golden; run tools/gensrc/regen_geo.ps1");
    let mut lines = Lines::new(&text);
    let mut shapes = 0usize;
    let mut points = 0usize;

    while let Some(peek) = lines.peek() {
        assert!(peek.starts_with("shape "), "unexpected line: {peek}");
        let shape = parse_shape(&mut lines);

        if let Some(rest) = lines.peek().and_then(|fl| fl.strip_prefix("fields ")) {
            lines.next();
            let f: Vec<&str> = rest.split(' ').collect();
            match &shape {
                Shape::Polygon(p) => {
                    assert_eq!(p.min_x.to_bits(), bits(f[0]).to_bits());
                    assert_eq!(p.max_x.to_bits(), bits(f[1]).to_bits());
                    assert_eq!(p.min_y.to_bits(), bits(f[2]).to_bits());
                    assert_eq!(p.max_y.to_bits(), bits(f[3]).to_bits());
                }
                Shape::Sector(s) => {
                    assert_eq!(s.dir_x.to_bits(), bits(f[0]).to_bits());
                    assert_eq!(s.dir_y.to_bits(), bits(f[1]).to_bits());
                    assert_eq!(s.cos_half_angle.to_bits(), bits(f[2]).to_bits());
                    assert_eq!(s.a.to_bits(), bits(f[3]).to_bits());
                    assert_eq!(s.dir.to_bits(), bits(f[4]).to_bits());
                }
                Shape::EllipticalSector(s) => {
                    assert_eq!(s.dir_x.to_bits(), bits(f[0]).to_bits());
                    assert_eq!(s.dir_y.to_bits(), bits(f[1]).to_bits());
                    assert_eq!(s.cos_half_angle.to_bits(), bits(f[2]).to_bits());
                    assert_eq!(s.a.to_bits(), bits(f[3]).to_bits());
                    assert_eq!(s.dir.to_bits(), bits(f[4]).to_bits());
                    assert_eq!(s.flatness.to_bits(), bits(f[5]).to_bits());
                }
                other => panic!("unexpected fields for {other:?}"),
            }
        }

        while let Some(p) = lines.peek() {
            if !p.starts_with("p ") {
                break;
            }
            lines.next();
            let t: Vec<&str> = p.split(' ').collect();
            let px = bits(t[1]);
            let py = bits(t[2]);
            let expect = t[3] == "1";
            assert_eq!(
                shape.hit_test_point(px, py),
                expect,
                "shape #{} point ({}, {})",
                shapes,
                t[1],
                t[2]
            );
            points += 1;
        }
        shapes += 1;
    }

    assert!(shapes >= 160, "unexpected shape count {shapes}");
    assert!(points >= 5000, "unexpected point count {points}");
}

#[test]
fn move_and_move_to_follow_java_ops() {
    let mut s = Shape::Composite(CompositeShape::new(0.0, 0.0));
    if let Shape::Composite(c) = &mut s {
        c.add_shape(Shape::Round(Round::new(1.5, 2.5, 1.0)));
    }
    s.move_to(10.0, 20.0);
    assert_eq!(s.x(), 10.0);
    assert_eq!(s.y(), 20.0);
    if let Shape::Composite(c) = &s {
        assert_eq!(c.shapes[0].x(), 11.5);
        assert_eq!(c.shapes[0].y(), 22.5);
    }
    s.move_by(-0.25, 0.75);
    assert_eq!(s.x(), 9.75);
    assert_eq!(s.y(), 20.75);
    if let Shape::Composite(c) = &s {
        assert_eq!(c.shapes[0].x(), 11.25);
        assert_eq!(c.shapes[0].y(), 23.25);
    }
    // 普通基元的 move_to 只改自身
    let mut r = Round::new(1.0, 2.0, 3.0);
    let mut sh = Shape::Round(Round::new(1.0, 2.0, 3.0));
    sh.move_to(4.0, 5.0);
    assert_eq!((sh.x(), sh.y()), (4.0, 5.0));
    r.x = 4.0;
    assert_eq!(r.x, 4.0);
}
