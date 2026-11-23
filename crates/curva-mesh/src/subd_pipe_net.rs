//! MultiPipe round a network of straight segments, as Rhino 8 measured (23 November 2026).
//! Segments meeting end to end join into runs; where three or more meet, a block joins them (see
//! `subd_pipe_node`). Each connected group of segments gives one SubD.
//!
//! A run leaving a junction starts at its line's face of the block: its rings start square to
//! the face (turned to the average of the face's corners), Struts rings sit evenly between the
//! farthest corner of the face and the next corner or end, and rings turn with the line round
//! bends as in a lone pipe. A run joining two junctions turns its rings by the least turn that
//! lines them up with the second junction's face, spread evenly over its last segment's rings
//! (Rhino: 7.5°, 15°, 22.5° for a 30° twist with Struts=3; none of the turn before a bend). An
//! H of three lines gives 46 points with Struts=1, a T 29.

use crate::subd_pipe::{
    PipeOptions, add, cross, dot, mitred, rings, scale, sub, transported, unit,
};
use crate::subd_pipe_node::{Node, node, quarter_wrapped};
use crate::{Mesh, SmoothError, SubD};
use std::collections::BTreeMap;

type Point = [f64; 3];

/// `v` (square to `d`) turned by `angle` round `d`.
fn rotated(v: Point, d: Point, angle: f64) -> Point {
    add(scale(v, angle.cos()), scale(cross(d, v), angle.sin()))
}

/// The angle round `d` from `u` to the nearest of the four ring directions of `f`.
fn twist(u: Point, f: Point, d: Point) -> f64 {
    quarter_wrapped(dot(d, cross(u, f)).atan2(dot(u, f)))
}

/// Where a run starts or ends at a junction.
#[derive(Clone, Copy)]
struct Joint {
    reach: f64,
    frame: Point,
}

/// The rings of an open run that starts at a junction (and may end at one), and the first ring
/// direction at its end.
fn joined_rings(
    points: &[Point],
    options: PipeOptions,
    start: Joint,
    end: Option<Joint>,
) -> Result<(Vec<[Point; 4]>, Point), SmoothError> {
    let bad = || SmoothError("a pipe needs segments of some length".to_owned());
    let m = points.len();
    let dirs: Vec<Point> = points
        .windows(2)
        .map(|w| unit(sub(w[1], w[0])).ok_or_else(bad))
        .collect::<Result<_, _>>()?;
    let last = m - 2;
    let mut frames = vec![start.frame];
    for s in 1..=last {
        let next = transported(frames[s - 1], dirs[s - 1], dirs[s]);
        frames.push(unit(next).unwrap_or(frames[s - 1]));
    }
    let turn = end.map_or(0.0, |e| twist(frames[last], e.frame, dirs[last]));
    let size = 1.5 * options.radius;
    let clear = |s: usize| {
        let c = dot(dirs[s - 1], dirs[s]).clamp(-1.0, 1.0);
        size * ((1.0 - c) / (1.0 + c).max(1e-12)).sqrt()
    };
    let ring_at = |centre: Point, u1: Point, d: Point| -> [Point; 4] {
        let u2 = cross(u1, d);
        [u1, u2, scale(u1, -1.0), scale(u2, -1.0)].map(|o| add(centre, scale(o, size)))
    };
    let mut out = Vec::new();
    for s in 0..=last {
        let length = dot(sub(points[s + 1], points[s]), dirs[s]);
        let from = if s == 0 { start.reach } else { clear(s) };
        let to = length
            - if s == last {
                end.map_or(0.0, |e| e.reach)
            } else {
                clear(s + 1)
            };
        #[allow(clippy::cast_precision_loss)]
        for k in 1..=options.struts {
            let part = k as f64 / (options.struts + 1) as f64;
            let u = if s == last {
                rotated(frames[s], dirs[s], turn * part)
            } else {
                frames[s]
            };
            out.push(ring_at(
                add(points[s], scale(dirs[s], from + (to - from) * part)),
                u,
                dirs[s],
            ));
        }
        if s < last {
            out.push(mitred(
                points[s + 1],
                frames[s],
                [dirs[s], dirs[s + 1]],
                size,
            ));
        }
    }
    let end_frame = rotated(frames[last], dirs[last], turn);
    if end.is_none() {
        out.push(ring_at(points[m - 1], end_frame, dirs[last]));
    }
    Ok((out, end_frame))
}

/// The junction face's points in ring order for a run along `d` whose first ring direction is
/// `u1`: the corner nearest `u1` first, then on round as the ring goes (a quarter turn back round
/// `d` each).
fn in_ring_order(
    face: [usize; 4],
    points: &[Point],
    centre: Point,
    d: Point,
    u1: Point,
) -> [usize; 4] {
    let side = cross(d, u1);
    let angle = |p: usize| {
        let v = sub(points[p], centre);
        dot(v, side).atan2(dot(v, u1))
    };
    let first = face
        .iter()
        .copied()
        .min_by(|&a, &b| angle(a).abs().total_cmp(&angle(b).abs()))
        .unwrap_or(face[0]);
    let mut out = face;
    let back = |p: usize| (angle(first) - angle(p)).rem_euclid(std::f64::consts::TAU);
    out.sort_by(|&a, &b| back(a).total_cmp(&back(b)));
    out
}

/// One run: its points, whether it closes, and the junction (node, line) at each end.
struct Run {
    points: Vec<Point>,
    closed: bool,
    start: Option<(usize, usize)>,
    end: Option<(usize, usize)>,
}

/// The network: its places, and at each the segments leaving it as (segment, other place).
struct Network {
    places: Vec<Point>,
    leaving: Vec<Vec<(usize, usize)>>,
}

fn key(p: Point) -> [i64; 3] {
    #[allow(clippy::cast_possible_truncation)]
    p.map(|c| (c * 1e9).round() as i64)
}

fn network(segments: &[[Point; 2]]) -> Network {
    let mut index: BTreeMap<[i64; 3], usize> = BTreeMap::new();
    let mut places = Vec::new();
    let mut place = |p: Point| {
        *index.entry(key(p)).or_insert_with(|| {
            places.push(p);
            places.len() - 1
        })
    };
    let ends: Vec<[usize; 2]> = segments
        .iter()
        .map(|[a, b]| [place(*a), place(*b)])
        .collect();
    let mut leaving = vec![Vec::new(); places.len()];
    for (s, [a, b]) in ends.iter().enumerate() {
        leaving[*a].push((s, *b));
        leaving[*b].push((s, *a));
    }
    Network { places, leaving }
}

/// The runs between junctions and free ends, then the loops left over.
fn runs_of(net: &Network, segments: usize) -> Vec<Run> {
    let degree = |p: usize| net.leaving[p].len();
    let mut used = vec![false; segments];
    let mut out = Vec::new();
    let starts = (0..net.places.len()).filter(|&p| degree(p) != 2);
    let loops = (0..net.places.len()).filter(|&p| degree(p) == 2);
    for (p, ends_open) in starts.map(|p| (p, true)).chain(loops.map(|p| (p, false))) {
        for line in 0..degree(p) {
            let (mut s, mut here) = net.leaving[p][line];
            if used[s] {
                continue;
            }
            used[s] = true;
            let mut points = vec![net.places[p], net.places[here]];
            while degree(here) == 2 && here != p {
                let Some(&(n, next)) = net.leaving[here].iter().find(|&&(n, _)| n != s) else {
                    break;
                };
                used[n] = true;
                (s, here) = (n, next);
                points.push(net.places[here]);
            }
            let at = |q: usize, s: usize| {
                (degree(q) > 2).then(|| {
                    (
                        q,
                        net.leaving[q]
                            .iter()
                            .position(|&(n, _)| n == s)
                            .unwrap_or(0),
                    )
                })
            };
            let closed = !ends_open;
            if closed {
                points.pop();
            }
            let start = if ends_open {
                at(p, net.leaving[p][line].0)
            } else {
                None
            };
            let end = if ends_open { at(here, s) } else { None };
            out.push(Run {
                points,
                closed,
                start,
                end,
            });
        }
    }
    out
}

/// The segments, each cut where another one ends partway along it (Rhino joins an H's bars to
/// its middle line there).
fn split_at_ends(segments: &[[Point; 2]]) -> Vec<[Point; 2]> {
    let ends: Vec<Point> = segments.iter().flatten().copied().collect();
    let mut out = Vec::new();
    for &[a, b] in segments {
        let d = sub(b, a);
        let length2 = dot(d, d);
        let mut cuts: Vec<(f64, Point)> = ends
            .iter()
            .filter_map(|&p| {
                let t = dot(sub(p, a), d) / length2;
                let off = sub(p, add(a, scale(d, t)));
                (t > 1e-9 && t < 1.0 - 1e-9 && dot(off, off) < 1e-14 * length2.max(1.0))
                    .then_some((t, p))
            })
            .collect();
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        cuts.dedup_by(|x, y| key(x.1) == key(y.1));
        let mut from = a;
        for (_, p) in cuts {
            out.push([from, p]);
            from = p;
        }
        out.push([from, b]);
    }
    out
}

/// The groups of places joined by segments, each with its places in order.
fn groups(net: &Network) -> Vec<Vec<usize>> {
    let mut group = vec![usize::MAX; net.places.len()];
    let mut out: Vec<Vec<usize>> = Vec::new();
    for p in 0..net.places.len() {
        if group[p] != usize::MAX {
            continue;
        }
        let mut todo = vec![p];
        group[p] = out.len();
        let mut members = Vec::new();
        while let Some(q) = todo.pop() {
            members.push(q);
            for &(_, r) in &net.leaving[q] {
                if group[r] == usize::MAX {
                    group[r] = out.len();
                    todo.push(r);
                }
            }
        }
        members.sort_unstable();
        out.push(members);
    }
    out
}

/// A cage being built: its points and faces.
#[derive(Default)]
struct Cage {
    points: Vec<Point>,
    faces: Vec<Vec<u32>>,
}

impl Cage {
    fn station(&mut self, ring: [Point; 4]) -> [u32; 4] {
        let first = self.points.len();
        self.points.extend(ring);
        std::array::from_fn(|k| u32::try_from(first + k).unwrap_or(u32::MAX))
    }

    /// Tube faces between each station and the next (and round, when closed).
    fn tube(&mut self, stations: &[[u32; 4]], closed: bool) {
        let n = stations.len();
        let joins = if closed { n } else { n - 1 };
        for r in 0..joins {
            let (a, b) = (stations[r], stations[(r + 1) % n]);
            for k in 0..4 {
                self.faces
                    .push(vec![a[k], b[k], b[(k + 1) % 4], a[(k + 1) % 4]]);
            }
        }
    }
}

/// The direction of line `line` leaving the junction at place `p`.
fn arm(net: &Network, p: usize, line: usize) -> Point {
    let (_, q) = net.leaving[p][line];
    unit(sub(net.places[q], net.places[p])).unwrap_or([1.0, 0.0, 0.0])
}

impl SubD {
    /// Pipes round a network of straight segments, as Rhino's MultiPipe: one SubD for each group
    /// of segments joined end to end (see `subd_pipe_net` and `subd_pipe_node`).
    ///
    /// # Errors
    /// When a segment has no length, or a junction cannot be joined (two lines on top of each
    /// other, or a line with more than four neighbours round it).
    pub fn pipes(segments: &[[Point; 2]], options: PipeOptions) -> Result<Vec<Self>, SmoothError> {
        if segments.iter().any(|[a, b]| key(*a) == key(*b)) {
            return Err(SmoothError(
                "a pipe needs segments of some length".to_owned(),
            ));
        }
        let segments = split_at_ends(segments);
        let net = network(&segments);
        let size = 1.5 * options.radius;
        let mut nodes: BTreeMap<usize, Node> = BTreeMap::new();
        for p in 0..net.places.len() {
            if net.leaving[p].len() > 2 {
                let arms: Vec<Point> = (0..net.leaving[p].len()).map(|l| arm(&net, p, l)).collect();
                nodes.insert(p, node(net.places[p], &arms, size)?);
            }
        }
        let mut runs = runs_of(&net, segments.len());
        for run in &mut runs {
            if run.start.is_none() && run.end.is_some() {
                run.points.reverse();
                std::mem::swap(&mut run.start, &mut run.end);
            }
        }
        let mut out = Vec::new();
        for members in groups(&net) {
            let mut cage = Cage::default();
            let mut first: BTreeMap<usize, usize> = BTreeMap::new();
            for &p in &members {
                if let Some(node) = nodes.get(&p) {
                    first.insert(p, cage.points.len());
                    cage.points.extend(&node.points);
                }
            }
            let home = |run: &Run| key(run.points[0]);
            for run in runs
                .iter()
                .filter(|r| members.iter().any(|&p| key(net.places[p]) == home(r)))
            {
                add_run(&mut cage, run, &net, &nodes, &first, options)?;
            }
            let mesh =
                Mesh::new(cage.points, &cage.faces).map_err(|e| SmoothError(e.to_string()))?;
            out.push(Self::from_cage(mesh));
        }
        Ok(out)
    }
}

/// The junction joint at (place, line): how far its face reaches and its ring direction.
fn joint<'a>(
    net: &Network,
    nodes: &'a BTreeMap<usize, Node>,
    (p, line): (usize, usize),
) -> (Joint, &'a Node) {
    let node = &nodes[&p];
    let a = arm(net, p, line);
    let centre = net.places[p];
    (
        Joint {
            reach: node.reach(centre, a, line),
            frame: node.frame(centre, a, line),
        },
        node,
    )
}

fn add_run(
    cage: &mut Cage,
    run: &Run,
    net: &Network,
    nodes: &BTreeMap<usize, Node>,
    first: &BTreeMap<usize, usize>,
    options: PipeOptions,
) -> Result<(), SmoothError> {
    let Some(start) = run.start else {
        let rings = rings(&run.points, run.closed, options)?;
        let stations: Vec<[u32; 4]> = rings.into_iter().map(|r| cage.station(r)).collect();
        cage.tube(&stations, run.closed);
        if options.cap && !run.closed {
            let (a, b) = (stations[0], stations[stations.len() - 1]);
            cage.faces.push(a.to_vec());
            cage.faces.push(vec![b[3], b[2], b[1], b[0]]);
        }
        return Ok(());
    };
    let (from, from_node) = joint(net, nodes, start);
    let end = run.end.map(|e| joint(net, nodes, e));
    let (rings, end_frame) = joined_rings(&run.points, options, from, end.map(|(j, _)| j))?;
    let m = run.points.len();
    let global =
        |p: usize, face: [usize; 4]| face.map(|i| u32::try_from(first[&p] + i).unwrap_or(u32::MAX));
    let d0 = unit(sub(run.points[1], run.points[0])).unwrap_or([1.0, 0.0, 0.0]);
    let face = in_ring_order(
        from_node.faces[start.1],
        &from_node.points,
        run.points[0],
        d0,
        from.frame,
    );
    let mut stations = vec![global(start.0, face)];
    stations.extend(rings.into_iter().map(|r| cage.station(r)));
    if let (Some((p, line)), Some((_, end_node))) = (run.end, end) {
        let d = unit(sub(run.points[m - 1], run.points[m - 2])).unwrap_or(d0);
        let face = in_ring_order(
            end_node.faces[line],
            &end_node.points,
            run.points[m - 1],
            d,
            end_frame,
        );
        stations.push(global(p, face));
    } else if options.cap {
        let b = stations[stations.len() - 1];
        cage.faces.push(vec![b[3], b[2], b[1], b[0]]);
    }
    cage.tube(&stations, false);
    Ok(())
}

#[cfg(test)]
#[path = "subd_pipe_net_data.rs"]
mod rhino;

#[cfg(test)]
mod tests {
    use super::rhino;
    use super::*;

    fn rounded(subd: &SubD) -> Vec<f64> {
        let mut out: Vec<Point> = subd
            .cage()
            .vertices()
            .iter()
            .map(|p| p.map(|c| (c * 1e4).round() / 1e4 + 0.0))
            .collect();
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out.concat()
    }

    fn struts(n: usize) -> PipeOptions {
        PipeOptions {
            struts: n,
            ..PipeOptions::default()
        }
    }

    fn arms(dirs: &[Point]) -> Vec<[Point; 2]> {
        dirs.iter()
            .map(|d| [[0.0; 3], scale(unit(*d).unwrap(), 10.0)])
            .collect()
    }

    /// The pipe matches Rhino's points within rounding, and has `faces` faces.
    fn as_in_rhino(segments: &[[Point; 2]], options: PipeOptions, rhino: &[f64], faces: usize) {
        let made = SubD::pipes(segments, options).unwrap();
        assert_eq!(made.len(), 1, "one SubD");
        let ours = rounded(&made[0]);
        assert_eq!(
            (ours.len(), made[0].cage().face_count()),
            (rhino.len(), faces)
        );
        for (k, (a, b)) in ours.iter().zip(rhino).enumerate() {
            assert!(
                (a - b).abs() < 2e-4,
                "point {} of {}: {a} against Rhino's {b}",
                k / 3,
                rhino.len() / 3
            );
        }
    }

    fn t() -> Vec<[Point; 2]> {
        vec![
            [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]],
            [[10.0, 0.0, 0.0], [20.0, 0.0, 0.0]],
            [[10.0, 0.0, 0.0], [10.0, 10.0, 0.0]],
        ]
    }

    #[test]
    fn three_lines_in_a_plane_join_as_in_rhino() {
        as_in_rhino(&t(), struts(2), &rhino::T, 36);
        let (c, s) = (-0.5, 0.75_f64.sqrt());
        as_in_rhino(
            &arms(&[[1.0, 0.0, 0.0], [c, s, 0.0], [c, -s, 0.0]]),
            struts(2),
            &rhino::Y,
            36,
        );
    }

    #[test]
    fn three_lines_out_of_a_plane_join_as_in_rhino() {
        let tripod = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        as_in_rhino(&arms(&tripod), struts(2), &rhino::TRIPOD, 36);
        let skew = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 2.0]];
        as_in_rhino(&arms(&skew), struts(2), &rhino::SKEW, 36);
    }

    #[test]
    fn more_lines_join_as_in_rhino() {
        let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let back = |p: Point| scale(p, -1.0);
        as_in_rhino(
            &arms(&[x, back(x), y, back(y)]),
            struts(2),
            &rhino::CROSS,
            48,
        );
        let six = [x, y, z, back(x), back(y), back(z)];
        as_in_rhino(&arms(&six), struts(2), &rhino::SIX, 72);
        let tetra = [
            [1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
        ];
        as_in_rhino(&arms(&tetra), struts(2), &rhino::TETRA, 48);
        as_in_rhino(
            &arms(&[x, back(x), y, back(y), z]),
            struts(2),
            &rhino::FIVE,
            60,
        );
    }

    #[test]
    fn runs_between_junctions_twist_as_in_rhino() {
        let bar = |x: f64, (c, s): (f64, f64)| [[x, -5.0 * c, -5.0 * s], [x, 5.0 * c, 5.0 * s]];
        let mut h = vec![[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], bar(0.0, (1.0, 0.0))];
        h.push(bar(10.0, (1.0, 0.0)));
        as_in_rhino(&h, struts(2), &rhino::H, 60);
        h[2] = bar(10.0, (30_f64.to_radians().cos(), 30_f64.to_radians().sin()));
        as_in_rhino(&h, struts(3), &rhino::TWIST, 80);
        let (c, s) = (40_f64.to_radians().cos(), 40_f64.to_radians().sin());
        let bent = vec![
            [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]],
            [[10.0, 0.0, 0.0], [10.0, 20.0, 0.0]],
            bar(0.0, (1.0, 0.0)),
            [
                [10.0 - 5.0 * c, 20.0, -5.0 * s],
                [10.0 + 5.0 * c, 20.0, 5.0 * s],
            ],
        ];
        as_in_rhino(&bent, struts(2), &rhino::BENT_TWIST, 72);
    }

    #[test]
    fn caps_close_the_free_ends_only() {
        let options = PipeOptions {
            cap: true,
            ..PipeOptions::default()
        };
        as_in_rhino(&t(), options, &rhino::T_CAP, 27);
        let made = SubD::pipes(&t(), options).unwrap();
        assert_eq!(made[0].creases().count(), 0, "nothing creased");
    }

    #[test]
    fn lines_apart_give_a_subd_each_and_a_lone_line_is_a_plain_pipe() {
        let apart = [
            [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]],
            [[0.0, 5.0, 0.0], [10.0, 5.0, 0.0]],
        ];
        let made = SubD::pipes(&apart, PipeOptions::default()).unwrap();
        assert_eq!(made.len(), 2);
        assert_eq!(made[0].cage().vertex_count(), 12);
    }

    #[test]
    fn a_fan_of_six_round_a_seventh_is_refused() {
        let mut dirs: Vec<Point> = (0..6)
            .map(|k| {
                let a = f64::from(k) * std::f64::consts::FRAC_PI_3;
                [a.cos(), a.sin(), 0.0]
            })
            .collect();
        dirs.push([0.0, 0.0, 1.0]);
        assert!(SubD::pipes(&arms(&dirs), PipeOptions::default()).is_err());
    }
}
