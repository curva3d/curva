//! Rhino's `PackFaces` for ToNURBS (issue #494): the subdivided quads' patches joined into as few
//! surfaces as Rhino makes. Measured in Rhino 8: quads join unless a crease, an open border or a
//! line running out from a point where other than four faces meet comes between them; such a line
//! runs straight on through ordinary points. A SubD cube packs into 6 faces, a torus into 1 closed
//! both ways, an open plane into 1.

use crate::Point;
use crate::smooth::{Cage, Patch};
use std::collections::{HashMap, HashSet};

/// Patches joined into one bicubic surface: `net[i][j]` with `i` along `spans[0]` patches and `j`
/// along `spans[1]`, the patches meeting at triple knots.
pub(crate) struct Packed {
    /// The surface's control points.
    pub net: Vec<Vec<Point>>,
    /// How many patches it joins each way.
    pub spans: [usize; 2],
    /// How far its patches stray from the true surface at most.
    pub off: f64,
}

impl Packed {
    /// The knots one way: clamped, with a triple knot where patches meet.
    pub(crate) fn knots(spans: usize) -> Vec<f64> {
        let mut knots = vec![0.0; 4];
        for k in 1..spans {
            #[allow(clippy::cast_precision_loss)]
            knots.extend([k as f64; 3]);
        }
        #[allow(clippy::cast_precision_loss)]
        knots.extend([spans as f64; 4]);
        knots
    }
}

type Net = [[Point; 4]; 4];
/// Patches laid out in rows, each with its turned net.
type Rows = Vec<Vec<(usize, Net)>>;
type Edge = (u32, u32);

fn key(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}

/// The quads' topology: each edge's faces, and the edges no patch may be joined across.
struct Quads<'a> {
    faces: Vec<&'a [u32]>,
    edge_faces: HashMap<Edge, Vec<usize>>,
    apart: HashSet<Edge>,
    from_quads: &'a [bool],
}

impl<'a> Quads<'a> {
    fn new(cage: &'a Cage, from_quads: &'a [bool]) -> Self {
        let mut faces = Vec::with_capacity(cage.face_vertex_counts.len());
        let mut at = 0;
        for &n in &cage.face_vertex_counts {
            faces.push(&cage.face_vertex_indices[at..at + n as usize]);
            at += n as usize;
        }
        let mut edge_faces: HashMap<Edge, Vec<usize>> = HashMap::new();
        for (f, face) in faces.iter().enumerate() {
            for k in 0..face.len() {
                let edge = key(face[k], face[(k + 1) % face.len()]);
                edge_faces.entry(edge).or_default().push(f);
            }
        }
        let creased: HashSet<Edge> = cage
            .edge_vertices
            .iter()
            .zip(&cage.edge_creases)
            .filter(|&(_, &s)| s > 0.0)
            .map(|(&[a, b], _)| key(a, b))
            .collect();
        let apart: HashSet<Edge> = edge_faces
            .iter()
            .filter(|(e, f)| {
                f.len() != 2 || creased.contains(e) || f.iter().any(|&g| !from_quads[g])
            })
            .map(|(&e, _)| e)
            .collect();
        let mut quads = Self {
            faces,
            edge_faces,
            apart,
            from_quads,
        };
        quads.add_lines(cage.vertex_count as usize);
        quads
    }

    /// Marks the lines running out from each point where other than four faces meet (or, on a
    /// border or crease, other than two), straight on through ordinary points. Only quads from
    /// four-sided cage faces count, as in Rhino: a SubD cylinder with triangle caps packs its side
    /// into one face and keeps each triangle's three quads apart.
    fn add_lines(&mut self, points: usize) {
        let mut face_count = vec![0u32; points];
        let mut stops = vec![0u32; points];
        let mut next: Vec<Vec<u32>> = vec![Vec::new(); points];
        for (f, face) in self.faces.iter().enumerate() {
            if !self.from_quads[f] {
                continue;
            }
            for &v in *face {
                face_count[v as usize] += 1;
            }
        }
        for (&(a, b), faces) in &self.edge_faces {
            // Edges inside other faces neither stop nor carry a line.
            if !faces.iter().any(|&f| self.from_quads[f]) {
                continue;
            }
            next[a as usize].push(b);
            next[b as usize].push(a);
            if self.apart.contains(&(a, b)) {
                stops[a as usize] += 1;
                stops[b as usize] += 1;
            }
        }
        let ordinary = |v: usize| stops[v] == 0 && face_count[v] == 4;
        let special = |v: usize| match stops[v] {
            0 => face_count[v] != 4,
            2 => face_count[v] > 2 && face_count[v] != 4,
            _ => face_count[v] > 1,
        };
        let mut lines = HashSet::new();
        for v in (0..points).filter(|&v| special(v)) {
            for &w in &next[v] {
                let (mut from, mut to) = (u32::try_from(v).unwrap_or(u32::MAX), w);
                while !self.apart.contains(&key(from, to)) && lines.insert(key(from, to)) {
                    if !ordinary(to as usize) {
                        break;
                    }
                    let came = &self.edge_faces[&key(from, to)];
                    let Some(&on) = next[to as usize].iter().find(|&&n| {
                        n != from
                            && !self.edge_faces[&key(to, n)]
                                .iter()
                                .any(|f| came.contains(f))
                    }) else {
                        break;
                    };
                    (from, to) = (to, on);
                }
            }
        }
        self.apart.extend(lines);
    }

    /// The faces joined to `f` across an edge.
    fn neighbours(&self, f: usize) -> impl Iterator<Item = usize> + '_ {
        let face = self.faces[f];
        (0..face.len()).filter_map(move |k| {
            let edge = key(face[k], face[(k + 1) % face.len()]);
            if self.apart.contains(&edge) {
                return None;
            }
            self.edge_faces[&edge].iter().copied().find(|&g| g != f)
        })
    }

    /// The regions of faces joined to one another, each a list of faces.
    fn regions(&self) -> Vec<Vec<usize>> {
        let mut region = vec![usize::MAX; self.faces.len()];
        let mut out = Vec::new();
        for start in 0..self.faces.len() {
            if region[start] != usize::MAX {
                continue;
            }
            let mut members = vec![start];
            region[start] = out.len();
            let mut at = 0;
            while at < members.len() {
                for g in self.neighbours(members[at]) {
                    if region[g] == usize::MAX {
                        region[g] = out.len();
                        members.push(g);
                    }
                }
                at += 1;
            }
            out.push(members);
        }
        out
    }
}

/// The net turned by one of the square's eight symmetries.
fn turned(net: &Net, s: u8) -> Net {
    std::array::from_fn(|a| {
        std::array::from_fn(|b| {
            let (x, y) = if s & 4 == 0 { (a, b) } else { (b, a) };
            let i = if s & 1 == 0 { x } else { 3 - x };
            let j = if s & 2 == 0 { y } else { 3 - y };
            net[i][j]
        })
    })
}

/// One side of a net: 0 is the far end along the first index, 1 the near end, 2 and 3 the same
/// along the second.
fn side(net: &Net, which: usize) -> [Point; 4] {
    std::array::from_fn(|k| match which {
        0 => net[3][k],
        1 => net[0][k],
        2 => net[k][3],
        _ => net[k][0],
    })
}

/// A region's patches laid out on a grid.
struct Layout<'a> {
    quads: &'a Quads<'a>,
    patches: &'a [Patch],
    members: HashSet<usize>,
    tolerance: f64,
}

impl Layout<'_> {
    /// The patch beyond `net`'s side `which`, turned to carry on from it.
    fn step(&self, f: usize, net: &Net, which: usize) -> Option<(usize, Net)> {
        let along = side(net, which);
        let close = |p: &[Point; 4]| {
            p.iter().zip(&along).all(|(a, b)| {
                (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>() <= self.tolerance.powi(2)
            })
        };
        self.quads
            .neighbours(f)
            .filter(|g| self.members.contains(g))
            .find_map(|g| {
                (0..8)
                    .map(|s| turned(&self.patches[g].net, s))
                    .find(|t| close(&side(t, which ^ 1)))
                    .map(|t| (g, t))
            })
    }

    /// From a patch, along side `which` as far as it goes: the patches in order, and whether the
    /// run came back to where it started.
    fn run(&self, start: (usize, Net), which: usize) -> (Vec<(usize, Net)>, bool) {
        let mut out = vec![start];
        while out.len() <= self.members.len() {
            let (f, net) = out[out.len() - 1];
            match self.step(f, &net, which) {
                Some((g, _)) if g == start.0 => return (out, true),
                Some(next) => out.push(next),
                None => return (out, false),
            }
        }
        (out, false)
    }

    /// The region's patches as rows, and whether they close up both ways (as on a torus); `None`
    /// when they do not lie on one grid.
    fn grid(&self, seed: usize) -> Option<(Rows, bool)> {
        let seed = (seed, self.patches[seed].net);
        let (back, _) = self.run(seed, 1);
        let left = back[back.len() - 1];
        let (down, _) = self.run(left, 3);
        let corner = down[down.len() - 1];
        let (starts, closed_up) = self.run(corner, 2);
        let rows: Rows = starts.into_iter().map(|s| self.run(s, 0).0).collect();
        let width = rows[0].len();
        let mut seen = HashSet::new();
        let whole = rows.iter().all(|r| r.len() == width)
            && rows.iter().flatten().all(|(f, _)| seen.insert(*f))
            && seen.len() == self.members.len();
        let closed_across = self.run(rows[0][0], 0).1;
        whole.then_some((rows, closed_up && closed_across))
    }
}

/// The patches packed into surfaces, as Rhino's `PackFaces`; a region that does not lie on one
/// grid keeps its patches apart.
pub(crate) fn packed(patches: &[Patch], quads: &Cage, from_quads: &[bool]) -> Vec<Packed> {
    let quads = Quads::new(quads, from_quads);
    let size = patches
        .iter()
        .flat_map(|p| p.net.iter().flatten())
        .fold(0.0_f64, |m, p| {
            m.max(p.iter().fold(0.0_f64, |m, c| m.max(c.abs())))
        });
    let mut out = Vec::new();
    for members in quads.regions() {
        let layout = Layout {
            quads: &quads,
            patches,
            members: members.iter().copied().collect(),
            tolerance: 1e-6 * size.max(1.0),
        };
        match layout.grid(members[0]) {
            // Rhino makes a torus one face closed both ways; the kernel cannot make a solid of
            // that, so it is cut in two, each half closed one way.
            Some((rows, true)) if rows.len() > 1 => {
                let half = rows.len() / 2;
                out.push(joined(&rows[..half], patches));
                out.push(joined(&rows[half..], patches));
            }
            Some((rows, _)) => out.push(joined(&rows, patches)),
            None => out.extend(
                members
                    .iter()
                    .map(|&f| joined(&[vec![(f, patches[f].net)]], patches)),
            ),
        }
    }
    out
}

/// The rows of patches as one net.
fn joined(rows: &[Vec<(usize, Net)>], patches: &[Patch]) -> Packed {
    let (across, up) = (rows[0].len(), rows.len());
    let mut net = vec![vec![[0.0; 3]; 3 * up + 1]; 3 * across + 1];
    let mut off: f64 = 0.0;
    for (y, row) in rows.iter().enumerate() {
        for (x, (f, patch)) in row.iter().enumerate() {
            off = off.max(patches[*f].off);
            for (a, line) in patch.iter().enumerate() {
                for (b, &p) in line.iter().enumerate() {
                    net[3 * x + a][3 * y + b] = p;
                }
            }
        }
    }
    Packed {
        net,
        spans: [across, up],
        off,
    }
}
