"""A small signed-distance raymarcher for Curva logo studies. Y is up, the C faces +Z (the front)."""
import numpy as np
from PIL import Image

def length(*a): return np.sqrt(sum(x*x for x in a))

# ---------- 2D C profile: an arc band, opening to +u (right) ----------
def sd_c2(u, v, R=1.0, w=0.42, open_half=np.radians(45)):
    # arc centred on -u, solid half-angle = 180-open_half
    half = np.pi - open_half
    sc = (np.sin(half), np.cos(half))
    # rotate so the arc's middle points along +v for iq's sdArc: middle at -u -> map (u,v)->(v,-u)
    px, py = np.abs(v), -u
    ra = R
    cond = sc[1]*px > sc[0]*py
    d1 = length(px - sc[0]*ra, py - sc[1]*ra)
    d2 = np.abs(length(px, py) - ra)
    return np.where(cond, d1, d2) - w/2

def extrude(d2, z, h, r=0.0):
    wx, wy = d2 + r, np.abs(z) - h/2 + r
    return np.minimum(np.maximum(wx, wy), 0) + length(np.maximum(wx, 0), np.maximum(wy, 0)) - r

# ---------- shapes (p = x, y, z) ----------
def S_extrude(x, y, z, depth=0.55, w=0.42, r=0.06):
    return extrude(sd_c2(x, y, w=w), z, depth, r)

def S_bent(x, y, z, depth=0.34, w=0.40, amp=0.55, r=0.05):
    # the C's arms lean apart in depth: top arm back, bottom arm forward (domain bend, not exact: scale steps)
    z2 = z + amp * y * np.clip(1.15 - np.abs(np.arctan2(y, -x))/np.pi, 0, 1) * 1.6
    return extrude(sd_c2(x, y, w=w), z2, depth, r) * 0.6

def S_tube(x, y, z, rt=0.21, amp=0.6):
    # round tube bent in depth along the C
    z2 = z + amp * y
    a = np.arctan2(y, -x)  # 0 at the C's back (left), +-pi at the opening
    lim = np.pi - np.radians(45)
    rr = length(x, y)
    # distance to the arc's spine in the xy plane, capped at the ends
    ac = np.clip(a, -lim, lim)
    sx, sy = -np.cos(ac), np.sin(ac)
    d = length(x - sx, y - sy, z2) - rt
    return d * 0.65

def S_twist(x, y, z, w=0.62, t=0.11, r=0.04, turn=np.radians(90)):
    # a flat band on the C's spine whose cross-section turns a quarter along its length
    a = np.arctan2(y, -x)
    lim = np.pi - np.radians(45)
    ac = np.clip(a, -lim, lim)
    sx, sy = -np.cos(ac), np.sin(ac)
    # local frame: radial (in xy) and depth (z)
    rad = (x - sx) * (-np.cos(ac)) + (y - sy) * np.sin(ac)
    tan = (x - sx) * np.sin(ac) + (y - sy) * np.cos(ac)
    ang = turn * (ac / lim) * 0.5 + np.radians(0)
    c, s = np.cos(ang), np.sin(ang)
    u = c * rad - s * z
    v = s * rad + c * z
    qx, qy, qz = np.abs(u) - t/2 + r, np.abs(v) - w/2 + r, np.abs(tan) - 0.0
    d = length(np.maximum(qx, 0), np.maximum(qy, 0)) + np.minimum(np.maximum(qx, qy), 0) - r
    # end caps where the arc stops
    return np.maximum(d, np.where(np.abs(a) > lim, length(x - sx, y - sy) - 0.0, -1)) * 0.8

def S_triplet(x, y, z, w=0.44, r=0.05):
    # intersection of two extrusions: a C seen from the front (xy) and a C seen from above (x, -z)
    front = extrude(sd_c2(x, y, w=w), z, 2.4, r)
    top = extrude(sd_c2(x, -z, w=w*1.55), y, 2.4, r)
    return np.maximum(front, top)

def S_revolve(x, y, z, w=0.40, sweep=np.radians(110), R0=0.0, r=0.05):
    # the C profile revolved part-way about the vertical axis through its open side
    ax = 1.05
    px, pz = x - ax, z
    rr = length(px, pz)
    ang = np.arctan2(pz, -px)
    prof = sd_c2(ax - rr, y, w=w)
    half = sweep / 2
    cap = np.abs(ang) - half
    # convert angular overshoot to a distance at radius rr
    dcap = np.where(cap > 0, rr * np.sin(np.minimum(cap, np.pi/2)), cap * rr)
    return np.maximum(prof, dcap) * 0.9

# ---------- camera and renderer ----------
def rot(az, el):
    az, el = np.radians(az), np.radians(el)
    # camera looks toward origin from azimuth az (around Y, 0 = front +Z) and elevation el
    fwd = -np.array([np.sin(az)*np.cos(el), np.sin(el), np.cos(az)*np.cos(el)])
    right = np.cross(fwd, [0, 1, 0]); right /= np.linalg.norm(right)
    up = np.cross(right, fwd)
    return fwd, right, up

def render(sdf, az=35, el=22, size=512, scale=1.55, steps=120, theme="light", ortho=True, persp=4.5, ss=1):
    N = size * ss
    fwd, right, up = rot(az, el)
    j, i = np.mgrid[0:N, 0:N]
    u = (i + 0.5) / N * 2 - 1; v = 1 - (j + 0.5) / N * 2
    u = u.ravel() * scale; v = v.ravel() * scale
    if ortho:
        ro = (np.outer(u, right) + np.outer(v, up)) - fwd * 6
        rd = np.tile(fwd, (u.size, 1))
    else:
        eye = -fwd * persp
        ro = np.tile(eye, (u.size, 1))
        rd = fwd + np.outer(u / persp * 1.0, right) + np.outer(v / persp * 1.0, up)
        rd /= np.linalg.norm(rd, axis=1)[:, None]
    t = np.zeros(u.size); hit = np.zeros(u.size, bool); alive = np.ones(u.size, bool)
    for _ in range(steps):
        idx = np.where(alive)[0]
        if idx.size == 0: break
        p = ro[idx] + rd[idx] * t[idx, None]
        d = sdf(p[:, 0], p[:, 1], p[:, 2])
        t[idx] += d
        h = d < 1e-3
        hit[idx[h]] = True
        dead = h | (t[idx] > 14)
        alive[idx[dead]] = False
    P = ro + rd * t[:, None]
    hi = np.where(hit)[0]
    p = P[hi]
    e = 1e-3
    k = np.array([[1, -1, -1], [-1, -1, 1], [-1, 1, -1], [1, 1, 1]], float)
    n = np.zeros_like(p)
    for kk in k:
        q = p + kk * e
        n += kk[None, :] * sdf(q[:, 0], q[:, 1], q[:, 2])[:, None]
    n /= np.linalg.norm(n, axis=1)[:, None] + 1e-9
    L = -0.5 * right + 0.8 * up - 0.45 * fwd       # light from the top left of the picture, toward the viewer
    L = L / np.linalg.norm(L)
    diff = np.clip(n @ L, 0, 1)
    # soft shadow
    sh = np.ones(len(hi)); ts = np.full(len(hi), 0.02)
    for _ in range(48):
        q = p + n * 2e-3 + L[None, :] * ts[:, None]
        d = sdf(q[:, 0], q[:, 1], q[:, 2])
        sh = np.minimum(sh, np.clip(10 * d / ts, 0, 1))
        ts += np.clip(d, 0.01, 0.2)
    # ambient occlusion
    ao = np.zeros(len(hi))
    for s in range(1, 6):
        hh = 0.04 * s
        q = p + n * hh
        ao += (hh - sdf(q[:, 0], q[:, 1], q[:, 2])) / (2 ** s)
    ao = np.clip(1 - 3.0 * ao, 0, 1)
    sky = 0.5 + 0.5 * n[:, 1]
    vdir = -rd[hi]
    fres = (1 - np.clip((n * vdir).sum(1), 0, 1)) ** 3
    spec = np.clip((n * ((L + vdir) / np.linalg.norm(L + vdir, axis=1)[:, None])).sum(1), 0, 1) ** 40
    if theme == "light":   # ink solid on white
        base, amb, rim = np.array([0.20, 0.195, 0.19]), 0.10, 0.10
        col = base[None, :] * (0.25 + 1.5 * diff * sh)[:, None] * (0.55 + 0.45 * ao)[:, None] + amb * sky[:, None] * ao[:, None] * 0.6
        col += (0.35 * spec * sh + rim * fres)[:, None]
    else:                  # white solid on graphite
        base = np.array([0.93, 0.925, 0.91])
        col = base[None, :] * (0.30 + 0.75 * diff * sh + 0.18 * sky)[:, None] * (0.45 + 0.55 * ao)[:, None]
        col += (0.15 * spec * sh)[:, None]
    img = np.zeros((u.size, 4)); img[hi, :3] = np.clip(col, 0, 1); img[hi, 3] = 1
    img = (img.reshape(N, N, 4) * 255).astype(np.uint8)
    im = Image.fromarray(img, "RGBA")
    if ss > 1: im = im.resize((size, size), Image.LANCZOS)
    return im

def silhouette(sdf, view, size=256, scale=1.55, ink=(31, 30, 29)):
    """Orthographic silhouette, like a plan or elevation: front (looking -Z), top (looking down), side (looking -X)."""
    az, el = {"front": (0, 0), "top": (0, 89.9), "side": (90, 0)}[view]
    im = render(sdf, az=az, el=el, size=size, scale=scale, steps=90)
    a = np.array(im)[:, :, 3]
    out = np.zeros((size, size, 4), np.uint8); out[..., :3] = ink; out[..., 3] = a
    return Image.fromarray(out, "RGBA")

# ---------- round 2 shapes ----------
def _spine(x, y, open_deg=45):
    a = np.arctan2(y, -x)                     # 0 at the C's back, +- at its tips
    lim = np.pi - np.radians(open_deg)
    ac = np.clip(a, -lim, lim)
    return a, ac, lim, -np.cos(ac), np.sin(ac)

def _rbox(qx, qy, qz, bx, by, bz, r):
    qx, qy, qz = np.abs(qx) - bx + r, np.abs(qy) - by + r, np.abs(qz) - bz + r
    return length(np.maximum(qx, 0), np.maximum(qy, 0), np.maximum(qz, 0)) + np.minimum(np.maximum(qx, np.maximum(qy, qz)), 0) - r

def S_twist2(x, y, z, w=0.56, t=0.17, r=0.05, turn=np.radians(90)):
    a, ac, lim, sx, sy = _spine(x, y)
    rad = (x - sx) * sx + (y - sy) * sy
    tan = (x - sx) * sy - (y - sy) * sx
    ang = turn * (ac / lim) * 0.5
    c, s = np.cos(ang), np.sin(ang)
    u, v = c * rad - s * z, s * rad + c * z
    return _rbox(u, v, tan, t/2, w/2, r, r) * 0.75

def S_curl(x, y, z, rt=0.19, amp=0.75):
    # the sketch read literally: a tube on the C whose two arms swing forward in depth;
    # front: C, top: a crescent, side: an arch
    a, ac, lim, sx, sy = _spine(x, y)
    uu = ac / lim                              # -1 bottom tip .. 0 back .. +1 top tip
    sz = amp * uu * uu * np.sign(uu) * 0 + amp * uu * uu
    return (length(x - sx, y - sy, z - sz) - rt) * 0.6

def S_lean(x, y, z, depth=0.30, w=0.40, amp=0.9, r=0.05):
    # flat band; the top arm swings back, the bottom arm forward, so the arms cross in a 3/4 view
    a, ac, lim, sx, sy = _spine(x, y)
    uu = ac / lim
    off = amp * uu * np.abs(uu)
    return extrude(sd_c2(x, y, w=w), z - off, depth, r) * 0.55

def S_facet(x, y, z, depth=0.6, w=0.44):
    # the extrusion with hard 45-degree chamfers on every edge: a cut, crystalline solid
    d2 = sd_c2(x, y, w=w)
    dz = np.abs(z) - depth/2
    body = np.maximum(d2, dz)
    cham = (d2 + dz) * 0.7071 + 0.07
    return np.maximum(body, cham)

def S_revolve2(x, y, z, w=0.38, sweep=np.radians(100), r=0.05):
    ax = 0.62
    px, pz = x - ax, z
    rr = length(px, pz)
    ang = np.arctan2(pz, -px)
    prof = extrude(sd_c2((ax - rr) * 1.0 - 0.0, y, R=0.62, w=w), 0 * y, 10)  # profile only
    prof = sd_c2(ax - rr, y, R=0.62, w=w)
    half = sweep / 2
    cap = np.abs(ang) - half
    dcap = np.where(cap > 0, rr * np.sin(np.minimum(cap, np.pi/2)), cap * rr)
    return np.maximum(prof, dcap) * 0.9

def S_triplet2(x, y, z, w=0.42, r=0.05):
    # front reads C (xy), top reads C (xz, opening right as seen from above)
    front = extrude(sd_c2(x, y, w=w), z, 2.4, r)
    top = extrude(sd_c2(x, -z, w=w), y, 2.4, r)
    return np.maximum(front, top)

# ---------- round 6b: the C swept around a vertical axis ("C of C") ----------
def make_cc(rc=0.95, R=0.55, w=0.24, sweep_gap=90, outward=True, plan_open_deg=0):
    gap = np.radians(sweep_gap) / 2
    po = np.radians(plan_open_deg)
    def f(x, y, z):
        r = length(x, z)
        phi = np.arctan2(z, x) - po
        phi = (phi + np.pi) % (2*np.pi) - np.pi          # 0 = direction of the plan gap
        u = (r - rc) if outward else (rc - r)
        prof = sd_c2(u, y, R=R, w=w)
        over = gap - np.abs(phi)                          # >0 inside the gap
        dcap = np.where(over > 0, r * np.sin(np.minimum(over, np.pi/2)), over * r)
        return np.maximum(prof, dcap) * 0.85
    return f

# ---------- round 6c: two C rings joined by a post (the sketch) ----------
def smin(a, b, k):
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0, 1)
    return b * (1 - h) + a * h - k * h * (1 - h)

def _ring_d(x, z, Rp, gap_deg):
    """In plan: distance from (x,z) to a C arc of radius Rp whose gap faces +x; also the radial offset."""
    a = np.arctan2(z, -x)                     # 0 at the back (-x)
    lim = np.pi - np.radians(gap_deg) / 2
    ac = np.clip(a, -lim, lim)
    px, pz = -Rp * np.cos(ac), Rp * np.sin(ac)
    rad = length(x, z) - Rp
    return length(x - px, z - pz), rad, a, lim, px, pz

def make_rings(Rp=0.62, H=0.62, t=0.13, gap=90, k=0.18, band=None, tilt=0.0):
    """Two horizontal C rings (y=+H and y=-H) joined by a vertical post at the back.
    band=None: round tube of radius t.  band=(width, thickness): flat band, lying flat in the rings, standing in the post."""
    def f(x, y, z):
        out = None
        for sgn in (1, -1):
            yy = y - sgn * H - tilt * x * sgn
            dp, rad, a, lim, px, pz = _ring_d(x, z, Rp, gap)
            if band is None:
                d = length(dp, yy) - t
            else:
                bw, bt = band
                # radial and tangential offsets in plan from the clamped arc point
                tx, tz = x - px, z - pz
                ux, uz = -np.cos(np.clip(a, -lim, lim)), np.sin(np.clip(a, -lim, lim))
                rr = tx * ux + tz * uz
                tt = tx * uz - tz * ux
                d = sdf_rbox3(rr, yy, tt, bw/2, bt/2, 0.0, min(bt/2, 0.05))
            out = d if out is None else np.minimum(out, d)
        # the post: a vertical segment at the back (x=-Rp, z=0) from -H to +H
        qy = np.clip(y, -H, H)
        if band is None:
            post = length(x + Rp, y - qy, z) - t
        else:
            bw, bt = band
            post = sdf_rbox3(x + Rp, y - qy, z, bt/2, 0.0, bw/2, min(bt/2, 0.05))
        return smin(out, post, k) * 0.8
    return f

def sdf_rbox3(qx, qy, qz, bx, by, bz, r):
    qx, qy, qz = np.abs(qx) - bx + r, np.abs(qy) - by + r, np.abs(qz) - bz + r
    return length(np.maximum(qx, 0), np.maximum(qy, 0), np.maximum(qz, 0)) + np.minimum(np.maximum(qx, np.maximum(qy, qz)), 0) - r

def make_loops(Rp=0.6, H=0.62, t=0.13, gap=90, k=0.22, slope=0.35, bulge=0.28, plate=None):
    """Two C rings that tilt toward each other at the front, joined by a back that curves outward.
    plate=(width, thickness): the rings become flat crescent plates instead of tubes."""
    def f(x, y, z):
        out = None
        for sgn in (1, -1):
            ylev = sgn * (H - slope * (x + Rp))           # rings dip toward each other at the front
            yy = (y - ylev) / np.sqrt(1 + slope * slope)
            dp, rad, a, lim, px, pz = _ring_d(x, z, Rp, gap)
            if plate is None:
                d = length(dp, yy) - t
            else:
                pw, pt = plate
                ac = np.clip(a, -lim, lim)
                ux, uz = -np.cos(ac), np.sin(ac)
                tx, tz = x - px, z - pz
                rr, tt = tx * ux + tz * uz, tx * uz - tz * ux
                d = sdf_rbox3(rr, yy, tt, pw/2, pt/2, 0.0, min(pt/2 - 1e-3, 0.06))
            out = d if out is None else np.minimum(out, d)
        qy = np.clip(y, -H, H)
        xs = x + Rp + bulge * (1 - (qy / H) ** 2)          # the back bows outward
        if plate is None:
            post = length(xs, y - qy, z) - t
        else:
            pw, pt = plate
            post = sdf_rbox3(xs, y - qy, z, pt/2, 0.0, pw/2, min(pt/2 - 1e-3, 0.06))
        return smin(out, post, k) * 0.7
    return f

def make_loops2(Rp=0.6, H=0.6, t=0.13, gap=90, k=0.22, dip=0.32, bulge=0.30, plate=None, gap_dir=0.0):
    """As make_loops, but the rings curve down only toward their tips (quadratic), so the front reads as a round C."""
    span = Rp * (1 + np.cos(np.radians(gap) / 2))
    def f(x, y, z):
        out = None
        for sgn in (1, -1):
            s = np.clip((x + Rp) / span, 0, 1.2)
            ylev = sgn * H * (1 - dip * s * s)
            slope = 2 * dip * H * s / span
            yy = (y - ylev) / np.sqrt(1 + slope * slope)
            dp, rad, a, lim, px, pz = _ring_d(x, z, Rp, gap)
            if plate is None:
                d = length(dp, yy) - t
            else:
                pw, pt = plate
                ac = np.clip(a, -lim, lim)
                ux, uz = -np.cos(ac), np.sin(ac)
                tx, tz = x - px, z - pz
                rr, tt = tx * ux + tz * uz, tx * uz - tz * ux
                d = sdf_rbox3(rr, yy, tt, pw/2, pt/2, 0.0, min(pt/2 - 1e-3, 0.06))
            out = d if out is None else np.minimum(out, d)
        qy = np.clip(y, -H, H)
        xs = x + Rp + bulge * (1 - (qy / H) ** 2)
        if plate is None:
            post = length(xs, y - qy, z) - t
        else:
            pw, pt = plate
            post = sdf_rbox3(xs, y - qy, z, pt/2, 0.0, pw/2, min(pt/2 - 1e-3, 0.06))
        return smin(out, post, k) * 0.7
    return f

# ---------- round 7: the owner's model, two slots open to the back ----------
def sd_box(x, y, z, cx, cy, cz, hx, hy, hz, r=0.0):
    qx, qy, qz = np.abs(x - cx) - hx + r, np.abs(y - cy) - hy + r, np.abs(z - cz) - hz + r
    return length(np.maximum(qx, 0), np.maximum(qy, 0), np.maximum(qz, 0)) + np.minimum(np.maximum(qx, np.maximum(qy, qz)), 0) - r

def owner_block(fillet=0.0):
    """The owner's Rhino sketch, rebuilt: cube minus a vertical slot (plan U) minus a horizontal slot (side C), both open to the back.
    Their x -> x, their z (up) -> y, their y (toward the openings) -> -z. Normalised so the cube is about 2 units wide."""
    cx, cy, cz, s = 17.785, 28.575, 35.22, 35.3
    def T(a, b): return (a - b) / s
    # cube
    X0, X1 = T(-17.52, cx), T(53.09, cx); Z0, Z1 = T(0, cz), T(70.44, cz); Y0, Y1 = T(-6.83, cy), T(63.98, cy)
    def box(xa, xb, ya, yb, za, zb, r=0.0):  # their coordinates -> mine
        return sd_box(None, None, None, 0, 0, 0, 0, 0, 0) if False else None
    def f(x, y, z):
        their_x, their_z, their_y = x, y, -z
        cube = sd_box(their_x, their_y, their_z, (X0+X1)/2, (Y0+Y1)/2, (Z0+Z1)/2, (X1-X0)/2, (Y1-Y0)/2, (Z1-Z0)/2, fillet)
        ax0, ax1 = T(-3.22, cx), T(38.79, cx); ay0 = T(4.11, cy)
        slotA = sd_box(their_x, their_y, their_z, (ax0+ax1)/2, (ay0+Y1+1)/2, 0, (ax1-ax0)/2, (Y1+1-ay0)/2, 5, 0)
        by0 = T(49.84, cy); bz0, bz1 = T(16.8, cz), T(58.8, cz)
        slotB = sd_box(their_x, their_y, their_z, 0, (by0+Y1+1)/2, (bz0+bz1)/2, 5, (Y1+1-by0)/2, (bz1-bz0)/2, 0)
        d = np.maximum(cube, -np.minimum(slotA, slotB))
        if fillet: d = np.maximum(cube, -(np.minimum(slotA, slotB) - fillet)) - 0  # soft inner edges
        return d
    return f

def cc_block(R=0.82, w=0.34, open_deg=50, square=0.0, depth_open=True):
    """Two true C extrusions intersected, both opening to the back (-z): a C in plan and a C in side elevation."""
    def prof(u, v):
        return sd_c2(u, v, R=R, w=w, open_half=np.radians(open_deg))
    def f(x, y, z):
        plan = extrude(prof(-z, x), y, 2 * (R + w), 0.03)     # C seen from above, opening to the back
        side = extrude(prof(-z, y), x, 2 * (R + w), 0.03)     # C seen from the side, opening to the back
        return np.maximum(plan, side)
    return f

def sd_rbox2(u, v, a, b, r):
    qx, qy = np.abs(u) - a + r, np.abs(v) - b + r
    return length(np.maximum(qx, 0), np.maximum(qy, 0)) + np.minimum(np.maximum(qx, qy), 0) - r

def sd_csq(u, v, a=0.95, rc=0.42, w=0.34, gap=0.5, cut_r=0.0):
    """A C drawn from a rounded square (half-size a, corner radius rc, stroke w), opening to +u with a gap of height 2*gap."""
    ring = np.abs(sd_rbox2(u, v, a - w/2, a - w/2, max(rc - w/2, 0.001))) - w/2
    slot = sd_rbox2(u - a, v, a * 0.75, gap, 0.0)
    return np.maximum(ring, -slot)

def csq_block(a=0.95, rc=0.42, w=0.34, gap=0.42, edge=0.03, gap_side=None, w_side=None):
    gs = gap if gap_side is None else gap_side
    ws = w if w_side is None else w_side
    def f(x, y, z):
        plan = extrude(sd_csq(-z, x, a, rc, w, gap), y, 2 * a, edge)
        side = extrude(sd_csq(-z, y, a, rc, ws, gs), x, 2 * a, edge)
        return np.maximum(plan, side)
    return f

# ---------- round 8: true trip-let, three silhouettes intersected ----------
def sd_spool(u, v, H=1.0, D=1.0, t=0.30, post=0.22, r=0.06):
    """An I / spool in (u = depth, v = height): two flanges (full depth) at top and bottom, joined by a central post."""
    top = sd_rbox2(u, v - (H - t/2), D, t/2, r)
    bot = sd_rbox2(u, v + (H - t/2), D, t/2, r)
    mid = sd_rbox2(u, v, post, H - t/2, 0.0)
    return np.minimum(np.minimum(top, bot), mid)

def sd_crescent(u, v, R=1.0, w=0.42, open_half=np.radians(45), flip=True):
    return sd_c2(-u if flip else u, v, R=R, w=w, open_half=open_half)

def triplet(front=None, top=None, side=None, edge=0.03, big=4.0):
    """front(x, y), top(x, z), side(z, y): 2D signed distances. The solid is their three extrusions intersected."""
    def f(x, y, z):
        d = extrude(front(x, y), z, big, 0) if front else -1e9
        if top: d = np.maximum(d, extrude(top(x, z), y, big, 0))
        if side: d = np.maximum(d, extrude(side(z, y), x, big, 0))
        return d - 0.0
    return f

def tl_sketch(Rf=0.95, wf=0.40, wt=0.5, ft=0.32, post=0.2, top_flip=True, top_open=45):
    # front: C opening right; top: crescent; side: spool. Sizes matched so the parts line up.
    H = Rf + wf/2
    return triplet(front=lambda x, y: sd_c2(x, y, R=Rf, w=wf),
                   top=lambda x, z: sd_crescent(x, z, R=Rf, w=wt, open_half=np.radians(top_open), flip=top_flip),
                   side=lambda z, y: sd_spool(z, y, H=H, D=H, t=ft, post=post))
