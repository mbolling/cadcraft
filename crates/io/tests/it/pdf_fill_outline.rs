//! A solid fill is plotted to PDF as the outline of its triangles (#3459): far less path data,
//! the same filled area, islands still empty.

use cadcraft_doc::{Common, Drawing, EntityKind, Hatch, HatchLoop, Space};
use cadcraft_geom::{PolyVertex, Vec2};
use cadcraft_io::plotter;
use serde_json::json;

/// Vertices of the outer boundary.
const RIM: usize = 400;
/// Radius of the outer boundary.
const R: f64 = 100.0;
/// Half the width of the square island.
const HALF: f64 = 20.0;

fn ring(points: impl Iterator<Item = (f64, f64)>, outer: bool) -> HatchLoop {
    HatchLoop { vertices: points.map(|(x, y)| PolyVertex::new(Vec2::new(x, y))).collect(), outer, boundary_flags: None, sources: Vec::new() }
}

/// A solid hatch: a many-sided outer loop around a square island.
fn drawing() -> Drawing {
    let rim = (0..RIM).map(|i| {
        let a = std::f64::consts::TAU * i as f64 / RIM as f64;
        (R * a.cos(), R * a.sin())
    });
    let island = [(-HALF, -HALF), (HALF, -HALF), (HALF, HALF), (-HALF, HALF)].into_iter();
    let hatch = Hatch {
        pattern: "SOLID".into(),
        solid: true,
        loops: vec![ring(rim, true), ring(island, false)],
        scale: 1.0,
        angle: 0.0,
        associative: false,
        style: 0,
        elevation: 0.0,
        gradient: None,
        origin: Vec2::ZERO,
        background: None,
        pattern_lines: Vec::new(),
    };
    let mut d = Drawing::new_metric();
    d.add(&Space::Model, Common::default(), EntityKind::Hatch(hatch)).unwrap();
    d
}

#[test]
fn a_solid_hatch_plots_as_its_outline_with_the_island_empty() {
    let bytes = plotter::plot(&drawing(), &Space::Model, &json!({"paper": "A4", "compress": false})).unwrap();
    let doc = crate::pdf_read::read(&bytes);
    let drawn = crate::pdf_read::drawn(doc.content());
    let fills = drawn.iter().filter(|o| o.op == "f").count();
    assert_eq!(fills, 1, "one fill for the hatch");
    let path: Vec<&crate::pdf_read::Op> = drawn.iter().filter(|o| matches!(o.op.as_str(), "m" | "l")).collect();
    // The triangles of a fill around an island number at least its boundary vertices; one
    // subpath per triangle would give that many `m` and twice as many `l`.
    let subpaths = path.iter().filter(|o| o.op == "m").count();
    assert!(subpaths < RIM / 10, "{subpaths} subpaths for {RIM} boundary vertices");
    assert!(path.len() < 2 * RIM, "{} path vertices for {RIM} boundary vertices", path.len());

    // Fill the path as a viewer does and compare with the shape.
    let mut pb = tiny_skia::PathBuilder::new();
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for o in &path {
        let (x, y) = (o.nums[0] as f32, o.nums[1] as f32);
        (lo, hi) = ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)));
        if o.op == "m" {
            pb.close();
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    let size = 400u32;
    let k = size as f32 / (hi.0 - lo.0).max(hi.1 - lo.1);
    let mut pm = tiny_skia::Pixmap::new(size, size).unwrap();
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    let to_px = tiny_skia::Transform::from_row(k, 0.0, 0.0, k, -lo.0 * k, -lo.1 * k);
    pm.fill_path(&pb.finish().unwrap(), &paint, tiny_skia::FillRule::Winding, to_px, None);
    let alpha = |fx: f64, fy: f64| pm.pixel((fx * f64::from(size)) as u32, (fy * f64::from(size)) as u32).unwrap().alpha();
    assert_eq!(alpha(0.5, 0.5), 0, "the island is empty");
    assert_eq!(alpha(0.5 + 0.3, 0.5), 255, "the ring between island and rim is filled");
    assert_eq!(alpha(0.03, 0.03), 0, "outside the rim is empty");
    // The filled share of the bounding square matches the ring's.
    let covered: f64 = pm.pixels().iter().map(|p| f64::from(p.alpha()) / 255.0).sum::<f64>() / f64::from(size * size);
    let want = (std::f64::consts::PI * R * R - 4.0 * HALF * HALF) / (4.0 * R * R);
    assert!((covered - want).abs() < 0.01, "covered {covered}, the ring covers {want}");
}
