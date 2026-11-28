//! The outline of a set of filled triangles: closed loops that fill, under the nonzero rule,
//! exactly what the triangles fill as one path of one subpath each.
//!
//! A filled path's nonzero winding number at a point is the sum, over its directed edges, of
//! the crossings of a ray from the point. So it depends only on how many times each directed
//! edge occurs, less the times it occurs reversed. An edge shared by two neighbouring
//! triangles in opposite directions cancels, and so does a zero-length edge. What is left
//! enters each vertex as often as it leaves it (each triangle does, and a cancelled pair takes
//! one entry and one exit from both its ends), so it splits into closed loops, and any such
//! split winds the same as the triangles everywhere off the edges. This holds for overlapping
//! triangles, mixed orientations, zero-area triangles and T-junctions alike: edges that do not
//! pair stay in the outline, which is then larger but still fills the same.
//!
//! Vertices are matched by the exact bits of the `f32` coordinates that are written, never by a
//! tolerance, so two distinct written points are never merged.

/// A written vertex.
pub(super) type Pt = (f32, f32);

/// The bits of a vertex, the key it is matched by.
fn key(p: Pt) -> u64 {
    (u64::from(p.0.to_bits()) << 32) | u64::from(p.1.to_bits())
}

/// The loops (each closed back to its first vertex) whose nonzero fill is that of `tris`, or
/// `None` when they cannot be traced (which the reasoning above rules out; the caller then
/// writes the triangles). Time is `O(n log n)` and memory `O(n)` in the triangle count.
pub(super) fn loops(tris: &[[Pt; 3]]) -> Option<Vec<Vec<Pt>>> {
    let mut keys: Vec<u64> = tris.iter().flatten().map(|p| key(*p)).collect();
    keys.sort_unstable();
    keys.dedup();
    let pts: Vec<Pt> = keys.iter().map(|k| (f32::from_bits((k >> 32) as u32), f32::from_bits(*k as u32))).collect();
    let id = |p: &Pt| keys.binary_search(&key(*p)).ok().and_then(|i| u32::try_from(i).ok());
    // Each edge as (lower id, higher id, +1 when it runs from the lower to the higher).
    let mut undirected: Vec<(u32, u32, i8)> = Vec::with_capacity(tris.len().saturating_mul(3));
    for t in tris {
        let v = [id(&t[0])?, id(&t[1])?, id(&t[2])?];
        for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
            match a.cmp(&b) {
                std::cmp::Ordering::Less => undirected.push((a, b, 1)),
                std::cmp::Ordering::Greater => undirected.push((b, a, -1)),
                std::cmp::Ordering::Equal => {}
            }
        }
    }
    undirected.sort_unstable_by_key(|e| (e.0, e.1));
    // What is left of each edge once opposite directions cancel, by start vertex.
    let mut edges: Vec<(u32, u32)> = Vec::new();
    for run in undirected.chunk_by(|x, y| (x.0, x.1) == (y.0, y.1)) {
        let Some(&(a, b, _)) = run.first() else { continue };
        let net: i64 = run.iter().map(|e| i64::from(e.2)).sum();
        let e = if net > 0 { (a, b) } else { (b, a) };
        edges.extend(std::iter::repeat_n(e, usize::try_from(net.unsigned_abs()).ok()?));
    }
    drop(undirected);
    edges.sort_unstable();
    // `next[v]..end[v]`: the unused edges leaving vertex `v`.
    let mut end = vec![0usize; pts.len()];
    for e in &edges {
        *end.get_mut(e.0 as usize)? += 1;
    }
    let mut next = vec![0usize; pts.len()];
    let mut at = 0usize;
    for (n, e) in next.iter_mut().zip(end.iter_mut()) {
        *n = at;
        at += *e;
        *e = at;
    }
    let mut out = Vec::new();
    for start in 0..pts.len() {
        while next.get(start)? < end.get(start)? {
            // Follow unused edges from `start` until one leads back to it. Each step uses an
            // edge, so this ends; balance means only `start` can be left with no edge out.
            let mut ring = vec![*pts.get(start)?];
            let mut cur = start;
            loop {
                let i = *next.get(cur)?;
                if i >= *end.get(cur)? {
                    return None;
                }
                *next.get_mut(cur)? += 1;
                let to = edges.get(i)?.1 as usize;
                if to == start {
                    break;
                }
                ring.push(*pts.get(to)?);
                cur = to;
            }
            out.push(ring);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fill paths of one subpath per closed ring with the nonzero rule, as a PDF viewer would.
    /// Aliased, so a pixel is filled exactly when the winding at its sample point is nonzero;
    /// with anti-aliasing, coverage sums along the triangles' inner edges round differently by
    /// a level in a few pixels.
    fn raster(rings: &[Vec<Pt>]) -> Vec<u8> {
        let mut pb = tiny_skia::PathBuilder::new();
        for r in rings {
            for (j, (x, y)) in r.iter().enumerate() {
                if j == 0 {
                    pb.move_to(*x, *y);
                } else {
                    pb.line_to(*x, *y);
                }
            }
            pb.close();
        }
        let mut pm = tiny_skia::Pixmap::new(64, 64).unwrap();
        if let Some(path) = pb.finish() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(0, 0, 0, 255);
            paint.anti_alias = false;
            pm.fill_path(&path, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
        }
        pm.data().to_vec()
    }

    fn soup(tris: &[[Pt; 3]]) -> Vec<Vec<Pt>> {
        tris.iter().map(|t| t.to_vec()).collect()
    }

    fn vertices(rings: &[Vec<Pt>]) -> usize {
        rings.iter().map(Vec::len).sum()
    }

    /// A fan of a regular polygon around its centre, `n` triangles.
    fn fan(c: Pt, r: f32, n: usize, ccw: bool) -> Vec<[Pt; 3]> {
        let p = |i: usize| {
            let a = std::f32::consts::TAU * (i % n) as f32 / n as f32;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        };
        (0..n).map(|i| if ccw { [c, p(i), p(i + 1)] } else { [c, p(i + 1), p(i)] }).collect()
    }

    /// The outline fills the same pixels as the triangles, and is no larger.
    fn same_fill(tris: &[[Pt; 3]]) -> Vec<Vec<Pt>> {
        let rings = loops(tris).expect("traced");
        let (a, b) = (raster(&rings), raster(&soup(tris)));
        let differ = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        assert_eq!(differ, 0, "{rings:?}");
        assert!(vertices(&rings) <= 3 * tris.len());
        rings
    }

    #[test]
    fn a_fan_becomes_its_rim() {
        let tris = fan((32.0, 32.0), 25.0, 40, true);
        let rings = same_fill(&tris);
        assert_eq!(rings.len(), 1);
        assert!(vertices(&rings) < tris.len() + 1, "{} vertices for {} triangles", vertices(&rings), tris.len());
    }

    #[test]
    fn a_ring_with_a_hole_keeps_the_hole() {
        // A square annulus: outer 8..56, hole 24..40, as eight triangles.
        let (o0, o1, i0, i1) = (8.0, 56.0, 24.0, 40.0);
        let q = |a: Pt, b: Pt, c: Pt, d: Pt| [[a, b, c], [a, c, d]];
        let mut tris = Vec::new();
        tris.extend(q((o0, o0), (o1, o0), (i1, i0), (i0, i0)));
        tris.extend(q((o1, o0), (o1, o1), (i1, i1), (i1, i0)));
        tris.extend(q((o1, o1), (o0, o1), (i0, i1), (i1, i1)));
        tris.extend(q((o0, o1), (o0, o0), (i0, i0), (i0, i1)));
        let rings = same_fill(&tris);
        assert_eq!(rings.len(), 2, "{rings:?}");
        let centre = raster(&rings)[(32 * 64 + 32) * 4 + 3];
        assert_eq!(centre, 0, "the hole stays empty");
    }

    #[test]
    fn mixed_orientations_overlaps_degenerates_and_t_junctions_fill_the_same() {
        let mut tris = fan((20.0, 20.0), 15.0, 12, true);
        // Overlapping, the other way round: where they overlap the windings cancel.
        tris.extend(fan((34.0, 30.0), 15.0, 9, false));
        // The same triangle twice.
        tris.push([(40.0, 40.0), (60.0, 42.0), (50.0, 60.0)]);
        tris.push([(40.0, 40.0), (60.0, 42.0), (50.0, 60.0)]);
        // A T-junction: one big triangle beside two that split its edge.
        tris.push([(2.0, 50.0), (2.0, 62.0), (14.0, 56.0)]);
        tris.push([(2.0, 50.0), (-4.0, 53.0), (2.0, 56.0)]);
        tris.push([(2.0, 56.0), (-4.0, 59.0), (2.0, 62.0)]);
        // Zero-area triangles: collinear, and with a repeated vertex.
        tris.push([(5.0, 5.0), (10.0, 10.0), (15.0, 15.0)]);
        tris.push([(30.0, 5.0), (30.0, 5.0), (40.0, 8.0)]);
        // Vertices a rounding step apart are not merged.
        tris.push([(50.0, 10.0), (60.0, 10.0), (55.0, 20.0)]);
        tris.push([(60.0, 10.0), (50.0, 10.000_001), (55.0, 2.0)]);
        same_fill(&tris);
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert_eq!(loops(&[]), Some(Vec::new()));
        assert_eq!(loops(&[[(1.0, 1.0); 3]]), Some(Vec::new()));
    }
}
