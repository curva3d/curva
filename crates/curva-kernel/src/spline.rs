//! NURBS curves worked on directly: evaluating, inserting and removing knots, raising the degree,
//! splitting, reparameterizing. The algorithms are those of Piegl and Tiller, *The NURBS Book*
//! (A2.1 span, A4.1 points, A5.1 knot insertion, A5.8 knot removal, degree elevation through
//! Bézier pieces), on homogeneous control points, so rational curves work alike.

// The algorithms keep the book's index loops, so they can be checked against it line by line.
#![allow(
    clippy::needless_range_loop,
    clippy::manual_memcpy,
    clippy::explicit_counter_loop
)]

use crate::{Curve, KernelError, Point};

/// A clamped NURBS curve with homogeneous control points `[x w, y w, z w, w]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Spline {
    /// Its degree, 1 or more.
    pub degree: usize,
    /// Its knots, `points.len() + degree + 1` of them, not decreasing, clamped at both ends.
    pub knots: Vec<f64>,
    /// Its control points, weighted.
    pub points: Vec<[f64; 4]>,
}

fn bad(why: &str) -> KernelError {
    KernelError(format!("not a usable NURBS curve: {why}"))
}

fn lerp(a: [f64; 4], b: [f64; 4], t: f64) -> [f64; 4] {
    std::array::from_fn(|i| a[i] * (1.0 - t) + b[i] * t)
}

fn distance4(a: [f64; 4], b: [f64; 4]) -> f64 {
    (0..4).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn binomial(n: usize, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

impl Spline {
    /// The curve as a spline: NURBS curves as they are, other smooth kinds through their exact
    /// NURBS form, lines and polylines as degree 1.
    pub fn of(curve: &Curve) -> Result<Self, KernelError> {
        let nurbs = match curve {
            Curve::Line { start, end } => return Self::polyline(&[*start, *end]),
            Curve::Polyline {
                points,
                closed: false,
            } => return Self::polyline(points),
            Curve::Polyline {
                points,
                closed: true,
            } => {
                let mut points = points.clone();
                if let Some(first) = points.first().copied() {
                    points.push(first);
                }
                return Self::polyline(&points);
            }
            Curve::Nurbs { .. } => curve.clone(),
            Curve::Circle {
                centre,
                radius,
                x,
                y,
            } => crate::conic::conic(
                *centre,
                x.map(|v| v * radius),
                y.map(|v| v * radius),
                std::f64::consts::TAU,
            )?,
            Curve::Arc {
                centre,
                radius,
                x,
                y,
                sweep,
            } => crate::conic::conic(
                *centre,
                x.map(|v| v * radius),
                y.map(|v| v * radius),
                *sweep,
            )?,
            Curve::Compound { segments } => {
                let parts = segments
                    .iter()
                    .map(Self::of)
                    .collect::<Result<Vec<_>, _>>()?;
                return Self::joined(&parts);
            }
            other => other.to_nurbs()?,
        };
        let Curve::Nurbs {
            degree,
            knots,
            points,
            weights,
        } = nurbs
        else {
            return Err(bad("no NURBS form"));
        };
        let points = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let w = weights.get(i).copied().unwrap_or(1.0);
                [p[0] * w, p[1] * w, p[2] * w, w]
            })
            .collect();
        Self {
            degree,
            knots,
            points,
        }
        .clamped()
    }

    /// Parts that meet end to end as one spline, raised to the highest degree among them, each
    /// part's parameter running on from the last.
    pub fn joined(parts: &[Self]) -> Result<Self, KernelError> {
        let degree = parts
            .iter()
            .map(|part| part.degree)
            .max()
            .ok_or_else(|| bad("no parts"))?;
        let mut out: Option<Self> = None;
        for part in parts {
            let part = part.elevated(degree - part.degree);
            out = Some(match out {
                None => part.reparameterized(0.0, 1.0),
                Some(mut so_far) => {
                    let start = so_far.domain().1;
                    let part = part.reparameterized(start, start + 1.0);
                    // The shared knot is kept `degree` times, the shared point once.
                    so_far.knots.truncate(so_far.knots.len() - 1);
                    so_far.knots.extend_from_slice(&part.knots[degree + 1..]);
                    so_far.points.extend_from_slice(&part.points[1..]);
                    so_far
                }
            });
        }
        let out = out.ok_or_else(|| bad("no parts"))?;
        out.check()?;
        Ok(out)
    }
    /// Straight segments through points, as a degree-1 spline.
    pub fn polyline(points: &[Point]) -> Result<Self, KernelError> {
        if points.len() < 2 {
            return Err(bad("too few points"));
        }
        let mut knots = vec![0.0; 2];
        knots.extend((1..points.len() - 1).map(|i| i as f64));
        knots.extend([(points.len() - 1) as f64; 2]);
        Ok(Self {
            degree: 1,
            knots,
            points: points.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
        })
    }

    /// Back to a Curva curve, with weights only when some differ from 1.
    pub fn to_curve(&self) -> Result<Curve, KernelError> {
        let rational = self.points.iter().any(|p| (p[3] - 1.0).abs() > 1e-12);
        let points = self.points.iter().map(|p| self.place(*p)).collect();
        let weights = if rational {
            self.points.iter().map(|p| p[3]).collect()
        } else {
            Vec::new()
        };
        Curve::nurbs(self.degree, self.knots.clone(), points, weights)
    }

    fn place(&self, p: [f64; 4]) -> Point {
        [p[0] / p[3], p[1] / p[3], p[2] / p[3]]
    }

    /// The control points, unweighted.
    #[must_use]
    pub fn control_points(&self) -> Vec<Point> {
        self.points.iter().map(|p| self.place(*p)).collect()
    }

    /// The weights.
    #[must_use]
    pub fn weights(&self) -> Vec<f64> {
        self.points.iter().map(|p| p[3]).collect()
    }

    /// Its parameter range.
    #[must_use]
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.points.len()])
    }

    fn check(&self) -> Result<(), KernelError> {
        if self.degree == 0 || self.points.len() <= self.degree {
            return Err(bad("too few control points for its degree"));
        }
        if self.knots.len() != self.points.len() + self.degree + 1 {
            return Err(bad("wrong number of knots"));
        }
        if self.knots.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(bad("decreasing knots"));
        }
        Ok(())
    }

    /// The same curve with knots repeated `degree + 1` times at both ends of its domain.
    pub fn clamped(mut self) -> Result<Self, KernelError> {
        self.check()?;
        let p = self.degree;
        let (a, b) = self.domain();
        let at_start = self.knots[..=p].iter().all(|k| *k == a);
        if !at_start {
            let have = self.multiplicity(a);
            if have < p {
                self = self.inserted(a, p - have);
            }
            // Drop what lies before the domain's start.
            let first = self.knots.iter().position(|k| *k == a).unwrap_or(0);
            let skip = first.saturating_sub(1);
            self.knots.drain(..skip);
            self.points.drain(..skip.min(self.points.len()));
        }
        let n = self.points.len();
        let at_end = self.knots[n..].iter().all(|k| *k == b);
        if !at_end {
            let have = self.multiplicity(b);
            if have < p {
                self = self.inserted(b, p - have);
            }
            let last = self.knots.iter().rposition(|k| *k == b).unwrap_or(0);
            self.knots.truncate(last + 2);
            let count = self.knots.len() - p - 1;
            self.points.truncate(count);
        }
        self.check()?;
        Ok(self)
    }

    /// How many times `u` is a knot.
    #[must_use]
    pub fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|k| **k == u).count()
    }

    /// The knot span `u` lies in (A2.1).
    fn span(&self, u: f64) -> usize {
        let (p, n) = (self.degree, self.points.len() - 1);
        if u >= self.knots[n + 1] {
            return n;
        }
        if u <= self.knots[p] {
            return p;
        }
        let (mut low, mut high) = (p, n + 1);
        let mut mid = (low + high) / 2;
        while u < self.knots[mid] || u >= self.knots[mid + 1] {
            if u < self.knots[mid] {
                high = mid;
            } else {
                low = mid;
            }
            mid = (low + high) / 2;
        }
        mid
    }

    fn homogeneous_at(&self, u: f64) -> [f64; 4] {
        // De Boor's algorithm.
        let p = self.degree;
        let k = self.span(u);
        let mut d: Vec<[f64; 4]> = (0..=p).map(|j| self.points[j + k - p]).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = j + k - p;
                let span = self.knots[i + p + 1 - r] - self.knots[i];
                let alpha = if span.abs() < f64::EPSILON {
                    0.0
                } else {
                    (u - self.knots[i]) / span
                };
                d[j] = lerp(d[j - 1], d[j], alpha);
            }
        }
        d[p]
    }

    /// The point at parameter `u`.
    #[must_use]
    pub fn point(&self, u: f64) -> Point {
        let (a, b) = self.domain();
        self.place(self.homogeneous_at(u.clamp(a, b)))
    }

    /// The first derivative at `u`, by a small central difference of the exact points.
    #[must_use]
    pub fn derivative(&self, u: f64) -> Point {
        let (a, b) = self.domain();
        let h = (b - a) * 1e-6;
        let (u0, u1) = ((u - h).max(a), (u + h).min(b));
        let (p0, p1) = (self.point(u0), self.point(u1));
        std::array::from_fn(|i| (p1[i] - p0[i]) / (u1 - u0))
    }

    /// The same curve with `u` inserted `times` more times as a knot (A5.1).
    #[must_use]
    pub fn inserted(&self, u: f64, times: usize) -> Self {
        let p = self.degree;
        let s = self.multiplicity(u);
        let times = times.min((p + 1).saturating_sub(s));
        if times == 0 {
            return self.clone();
        }
        let np = self.points.len() - 1;
        let k = self.span(u);
        // At the domain's end the span is the last one, after the end knots: insert before them.
        let k = if u >= self.knots[np + 1] {
            self.knots
                .iter()
                .position(|kn| *kn == u)
                .map_or(k, |i| i.saturating_sub(1))
        } else {
            k
        };
        let mut knots = Vec::with_capacity(self.knots.len() + times);
        knots.extend_from_slice(&self.knots[..=k]);
        knots.extend(std::iter::repeat_n(u, times));
        knots.extend_from_slice(&self.knots[k + 1..]);
        let mut q = vec![[0.0; 4]; np + times + 1];
        q[..=k - p].copy_from_slice(&self.points[..=k - p]);
        q[k - s + times..=np + times].copy_from_slice(&self.points[k - s..=np]);
        let mut r: Vec<[f64; 4]> = (0..=p - s).map(|i| self.points[k - p + i]).collect();
        let mut last_l = k - p;
        for j in 1..=times {
            let l = k - p + j;
            for i in 0..=p - j - s {
                let alpha = (u - self.knots[l + i]) / (self.knots[i + k + 1] - self.knots[l + i]);
                r[i] = lerp(r[i], r[i + 1], alpha);
            }
            q[l] = r[0];
            q[k + times - j - s] = r[p - j - s];
            last_l = l;
        }
        let _ = last_l;
        let l = k - p + times;
        for i in l + 1..k - s {
            q[i] = r[i - l];
        }
        Self {
            degree: p,
            knots,
            points: q,
        }
    }

    /// Removes the knot `u` up to `times` times where the curve stays within `tolerance` (A5.8),
    /// and says how many times it was removed.
    pub fn remove(&mut self, u: f64, times: usize, tolerance: f64) -> usize {
        let p = self.degree;
        let s = self.multiplicity(u);
        let Some(r) = self.knots.iter().rposition(|k| *k == u) else {
            return 0;
        };
        let n = self.points.len() - 1;
        // Only knots inside the domain can be removed.
        if r <= p || r > n {
            return 0;
        }
        let ord = p + 1;
        let fout = (2 * r - s - p) / 2;
        let (mut first, mut last) = (r as isize - p as isize, r as isize - s as isize);
        let mut temp = vec![[0.0; 4]; 2 * p + 1];
        let mut removed = 0;
        let knots = self.knots.clone();
        for t in 0..times.min(s) {
            let off = first - 1;
            if off < 0 {
                break;
            }
            temp[0] = self.points[off as usize];
            temp[(last + 1 - off) as usize] = self.points[(last + 1) as usize];
            let (mut i, mut j) = (first, last);
            let (mut ii, mut jj) = (1isize, last - off);
            while j - i > t as isize {
                let (iu, ju) = (i as usize, j as usize);
                let alfi = (u - knots[iu]) / (knots[iu + ord + t] - knots[iu]);
                let alfj = (u - knots[ju - t]) / (knots[ju + ord] - knots[ju - t]);
                let before = temp[(ii - 1) as usize];
                temp[ii as usize] =
                    std::array::from_fn(|c| (self.points[iu][c] - (1.0 - alfi) * before[c]) / alfi);
                let after = temp[(jj + 1) as usize];
                temp[jj as usize] =
                    std::array::from_fn(|c| (self.points[ju][c] - alfj * after[c]) / (1.0 - alfj));
                i += 1;
                ii += 1;
                j -= 1;
                jj -= 1;
            }
            let fits = if j - i < t as isize {
                distance4(temp[(ii - 1) as usize], temp[(jj + 1) as usize]) <= tolerance
            } else {
                let iu = i as usize;
                let alfi = (u - knots[iu]) / (knots[iu + ord + t] - knots[iu]);
                let guess = lerp(
                    temp[(ii - 1) as usize],
                    temp[(ii + t as isize + 1) as usize],
                    alfi,
                );
                distance4(self.points[iu], guess) <= tolerance
            };
            if !fits {
                break;
            }
            let (mut i, mut j) = (first, last);
            while j - i > t as isize {
                self.points[i as usize] = temp[(i - off) as usize];
                self.points[j as usize] = temp[(j - off) as usize];
                i += 1;
                j -= 1;
            }
            first -= 1;
            last += 1;
            removed = t + 1;
        }
        if removed == 0 {
            return 0;
        }
        let t = removed;
        self.knots.drain(r + 1 - t..=r);
        let (mut i, mut j) = (fout, fout);
        for k in 1..t {
            if k % 2 == 1 {
                i += 1;
            } else {
                j -= 1;
            }
        }
        let mut write = j;
        for k in i + 1..=n {
            self.points[write] = self.points[k];
            write += 1;
        }
        self.points.truncate(self.points.len() - t);
        t
    }

    /// The same curve at a degree `by` higher, exactly: split into Bézier pieces, each raised,
    /// then the knots the split added removed again.
    #[must_use]
    pub fn elevated(&self, by: usize) -> Self {
        if by == 0 {
            return self.clone();
        }
        let p = self.degree;
        let (a, b) = self.domain();
        let mut inner: Vec<(f64, usize)> = Vec::new();
        for k in &self.knots[p + 1..self.points.len()] {
            match inner.last_mut() {
                Some((u, m)) if *u == *k => *m += 1,
                _ => inner.push((*k, 1)),
            }
        }
        let mut split = self.clone();
        for (u, m) in &inner {
            if *m < p {
                split = split.inserted(*u, p - m);
            }
        }
        // Each piece is p + 1 points, the pieces sharing their ends.
        let pieces = (split.points.len() - 1) / p;
        let q = p + by;
        let mut points = Vec::with_capacity(pieces * q + 1);
        for piece in 0..pieces {
            let old = &split.points[piece * p..=piece * p + p];
            for i in 0..=q {
                if piece > 0 && i == 0 {
                    continue;
                }
                let mut sum = [0.0; 4];
                for j in i.saturating_sub(by)..=p.min(i) {
                    let c = binomial(p, j) * binomial(by, i - j) / binomial(q, i);
                    for (s, o) in sum.iter_mut().zip(old[j]) {
                        *s += c * o;
                    }
                }
                points.push(sum);
            }
        }
        let mut knots = vec![a; q + 1];
        for (u, _) in &inner {
            knots.extend(std::iter::repeat_n(*u, q));
        }
        knots.extend(std::iter::repeat_n(b, q + 1));
        let mut raised = Self {
            degree: q,
            knots,
            points,
        };
        let size = self
            .points
            .iter()
            .flat_map(|p| p.iter().map(|v| v.abs()))
            .fold(1.0, f64::max);
        for (u, m) in &inner {
            let keep = (m + by).min(q);
            let extra = q - keep;
            if extra > 0 {
                raised.remove(*u, extra, 1e-9 * size);
            }
        }
        raised
    }

    /// The same curve on a new parameter range, `start` to `end`.
    #[must_use]
    pub fn reparameterized(&self, start: f64, end: f64) -> Self {
        let (a, b) = self.domain();
        let scale = (end - start) / (b - a);
        Self {
            degree: self.degree,
            knots: self.knots.iter().map(|k| start + (k - a) * scale).collect(),
            points: self.points.clone(),
        }
    }

    /// The curve cut at `u` into its two parts, if `u` is inside the domain.
    #[must_use]
    pub fn split(&self, u: f64) -> Option<(Self, Self)> {
        let (a, b) = self.domain();
        if u <= a || u >= b {
            return None;
        }
        let p = self.degree;
        // With `u` a knot `p` times, the curve passes through a control point there.
        let full = self.inserted(u, p.saturating_sub(self.multiplicity(u)));
        let first = full.knots.iter().position(|k| *k == u)?;
        let last = full.knots.iter().rposition(|k| *k == u)?;
        let mut left_knots = full.knots[..=last].to_vec();
        left_knots.push(u);
        let left_count = left_knots.len() - p - 1;
        let mut right_knots = vec![u];
        right_knots.extend_from_slice(&full.knots[first..]);
        let right_count = right_knots.len() - p - 1;
        let left = Self {
            degree: p,
            knots: left_knots,
            points: full.points[..left_count].to_vec(),
        };
        let right = Self {
            degree: p,
            knots: right_knots,
            points: full.points[full.points.len() - right_count..].to_vec(),
        };
        (left.check().is_ok() && right.check().is_ok()).then_some((left, right))
    }

    /// The same control points on evenly spaced knots, which changes the shape a little, as
    /// Rhino's MakeUniform.
    #[must_use]
    pub fn uniform(&self) -> Self {
        let p = self.degree;
        let (a, b) = self.domain();
        let inner = self.points.len() - p;
        let mut knots = vec![a; p + 1];
        knots.extend((1..inner).map(|i| a + (b - a) * i as f64 / inner as f64));
        knots.extend(std::iter::repeat_n(b, p + 1));
        Self {
            degree: p,
            knots,
            points: self.points.clone(),
        }
    }

    /// Evenly spaced points along the parameter, ends included.
    #[must_use]
    pub fn samples(&self, count: usize) -> Vec<Point> {
        let (a, b) = self.domain();
        (0..=count.max(1))
            .map(|i| self.point(a + (b - a) * i as f64 / count.max(1) as f64))
            .collect()
    }

    /// The parameter of the point on the curve nearest `p`, found by sampling, then refined.
    #[must_use]
    pub fn closest(&self, p: Point) -> f64 {
        let (a, b) = self.domain();
        let steps = 64 * self.points.len().max(4);
        let at = |u: f64| crate::gap(self.point(u), p);
        let mut best = (0..=steps)
            .map(|i| a + (b - a) * i as f64 / steps as f64)
            .min_by(|x, y| at(*x).total_cmp(&at(*y)))
            .unwrap_or(a);
        let mut step = (b - a) / steps as f64;
        for _ in 0..60 {
            let (left, right) = ((best - step).max(a), (best + step).min(b));
            if at(left) < at(best) {
                best = left;
            } else if at(right) < at(best) {
                best = right;
            } else {
                step /= 2.0;
            }
        }
        best
    }
}

impl Spline {
    /// The same curve run the other way.
    #[must_use]
    pub fn reversed(&self) -> Self {
        let (a, b) = self.domain();
        let mut knots: Vec<f64> = self.knots.iter().map(|k| a + b - k).collect();
        knots.reverse();
        let mut points = self.points.clone();
        points.reverse();
        Self {
            degree: self.degree,
            knots,
            points,
        }
    }

    /// The point, first and second derivatives at the end of the curve, exact from the control
    /// points: on the weighted points, then by the quotient rule for a weighted curve.
    #[must_use]
    pub fn end_derivatives(&self) -> (Point, Point, Point) {
        let (p, n, u, deg) = (
            self.degree as f64,
            self.points.len() - 1,
            &self.knots,
            self.degree,
        );
        let at = |i: usize| self.points[i];
        // The derivative's control points (on the weighted points).
        let q = |i: usize| -> [f64; 4] {
            let span = u[i + deg + 1] - u[i + 1];
            let (a, b) = (at(i + 1), at(i));
            std::array::from_fn(|k| p * (a[k] - b[k]) / span)
        };
        let h0 = at(n);
        let h1 = q(n - 1);
        let h2: [f64; 4] = if deg >= 2 && n >= 2 {
            let q0 = q(n - 2);
            let span = u[n + deg - 1] - u[n];
            std::array::from_fn(|k| (p - 1.0) * (h1[k] - q0[k]) / span)
        } else {
            [0.0; 4]
        };
        let w = h0[3];
        let c: Point = std::array::from_fn(|k| h0[k] / w);
        let c1: Point = std::array::from_fn(|k| (h1[k] - h1[3] * c[k]) / w);
        let c2: Point = std::array::from_fn(|k| (h2[k] - 2.0 * h1[3] * c1[k] - h2[3] * c[k]) / w);
        (c, c1, c2)
    }

    /// The curve carried on past its end by `by` (in its own parameter) as the polynomial of its
    /// last span continues, as Rhino's smooth Extend.
    #[must_use]
    pub fn extended_end(&self, by: f64) -> Self {
        let p = self.degree;
        let (a, b) = self.domain();
        // The last span on its own, as a Bézier piece.
        let last_inner = self.knots[p + 1..self.points.len()]
            .iter()
            .rev()
            .find(|k| **k > a && **k < b)
            .copied();
        let (before, span) = match last_inner {
            Some(u) => match self.split(u) {
                Some((left, right)) => (Some(left), right),
                None => (None, self.clone()),
            },
            None => (None, self.clone()),
        };
        // De Casteljau at t > 1 gives the control points of the span stretched on to t.
        let (s0, s1) = span.domain();
        let t = (s1 - s0 + by) / (s1 - s0);
        let mut work = span.points.clone();
        let mut left = vec![work[0]];
        for r in 1..=p {
            for j in 0..=p - r {
                work[j] = lerp(work[j], work[j + 1], t);
            }
            left.push(work[0]);
        }
        let mut knots = vec![s0; p + 1];
        knots.extend(std::iter::repeat_n(s1 + by, p + 1));
        let stretched = Self {
            degree: p,
            knots,
            points: left,
        };
        match before {
            None => stretched,
            Some(before) => {
                // Joined on, keeping both parts' parameters.
                let mut joined = before.clone();
                joined.knots.truncate(joined.knots.len() - 1);
                joined.knots.extend_from_slice(&stretched.knots[p + 1..]);
                joined.points.extend_from_slice(&stretched.points[1..]);
                match joined.check().map(|()| joined) {
                    Ok(mut joined) => {
                        // Back to the smoothness the curve had at that knot.
                        let size = joined
                            .points
                            .iter()
                            .flat_map(|q| q.iter().map(|v| v.abs()))
                            .fold(1.0, f64::max);
                        let at = joined.knots[before.points.len()];
                        joined.remove(at, p, 1e-9 * size);
                        joined
                    }
                    Err(_) => stretched,
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::Spline;
    use crate::{Curve, gap};

    fn wavy() -> Spline {
        Spline::of(
            &Curve::control_point(
                vec![
                    [0.0, 0.0, 0.0],
                    [2.0, 5.0, 0.0],
                    [5.0, -1.0, 1.0],
                    [8.0, 4.0, 0.0],
                    [10.0, 0.0, 2.0],
                    [13.0, 3.0, 0.0],
                ],
                3,
                false,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn same_shape(a: &Spline, b: &Spline) -> bool {
        a.domain() == b.domain()
            && (0..=200).all(|i| {
                let (s, e) = a.domain();
                let u = s + (e - s) * f64::from(i) / 200.0;
                gap(a.point(u), b.point(u)) < 1e-9
            })
    }

    #[test]
    fn inserting_knots_keeps_the_curve() {
        let curve = wavy();
        let more = curve.inserted(0.4, 2);
        assert_eq!(more.points.len(), curve.points.len() + 2);
        assert!(same_shape(&curve, &more));
    }

    #[test]
    fn removing_an_inserted_knot_gives_the_curve_back() {
        let curve = wavy();
        let mut more = curve.inserted(0.4, 2);
        assert_eq!(more.remove(0.4, 2, 1e-9), 2);
        assert_eq!(more.points.len(), curve.points.len());
        assert!(same_shape(&curve, &more));
    }

    #[test]
    fn raising_the_degree_keeps_the_shape_and_smoothness() {
        let curve = wavy();
        for by in 1..=3 {
            let raised = curve.elevated(by);
            assert_eq!(raised.degree, 3 + by);
            assert!(same_shape(&curve, &raised), "raised by {by}");
            // Each inner knot is one more time per degree, no more (the smoothness kept).
            let inner = curve.points.len() - curve.degree - 1;
            assert_eq!(raised.points.len(), curve.points.len() + by * (inner + 1));
        }
    }

    #[test]
    fn a_circle_raised_stays_a_circle() {
        let circle = Spline::of(&Curve::circle([1.0, 2.0, 0.0], 5.0).unwrap()).unwrap();
        let raised = circle.elevated(1);
        assert!(same_shape(&circle, &raised));
        let (a, b) = raised.domain();
        for i in 0..50 {
            let p = raised.point(a + (b - a) * f64::from(i) / 50.0);
            assert!((gap(p, [1.0, 2.0, 0.0]) - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn splitting_gives_two_parts_that_meet() {
        let curve = wavy();
        let (left, right) = curve.split(0.3).unwrap();
        assert!(gap(left.point(left.domain().1), right.point(right.domain().0)) < 1e-12);
        assert!(gap(left.point(0.15), curve.point(0.15)) < 1e-9);
        assert!(gap(right.point(0.8), curve.point(0.8)) < 1e-9);
    }

    #[test]
    fn reversing_runs_the_same_points_backwards() {
        let curve = wavy();
        let back = curve.reversed();
        for i in 0..=10 {
            let u = f64::from(i) / 10.0;
            assert!(gap(curve.point(u), back.point(1.0 - u)) < 1e-9);
        }
    }

    #[test]
    fn end_derivatives_agree_with_small_differences() {
        let curve = wavy();
        let (end, d1, d2) = curve.end_derivatives();
        let h = 1e-4;
        let (p1, p2) = (curve.point(1.0 - h), curve.point(1.0 - 2.0 * h));
        for k in 0..3 {
            let numeric1 = (3.0 * end[k] - 4.0 * p1[k] + p2[k]) / (2.0 * h);
            let numeric2 = (end[k] - 2.0 * p1[k] + p2[k]) / (h * h);
            assert!(
                (d1[k] - numeric1).abs() < 1e-3 * d1[k].abs().max(1.0),
                "{d1:?}"
            );
            assert!(
                (d2[k] - numeric2).abs() < 1e-2 * d2[k].abs().max(1.0),
                "{d2:?}"
            );
        }
    }

    #[test]
    fn extending_keeps_the_curve_and_carries_on_smoothly() {
        let curve = wavy();
        let longer = curve.extended_end(0.2);
        assert_eq!(longer.domain(), (0.0, 1.2));
        for i in 0..=20 {
            let u = f64::from(i) / 20.0;
            assert!(gap(curve.point(u), longer.point(u)) < 1e-9, "at {u}");
        }
        // No kink where the old end was.
        let (before, after) = (longer.derivative(1.0 - 1e-3), longer.derivative(1.0 + 1e-3));
        assert!(gap(before, after) < 1e-1 * gap(before, [0.0; 3]));
    }
}
