//! Display meshes, STEP files and pictures of results.

use solidcore::Kernel;
use solidcore_convert::brep::to_kernel_body;
use solidcore_ops::Body;
use solidcore_tess::{Mesh, MeshTolerance, plan};
use std::path::{Path, PathBuf};
use std::time::Instant;

const WIDTH: usize = 900;
const HEIGHT: usize = 650;

fn fail(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

fn entry(name: &str, value: String) -> (String, String) {
    (name.to_owned(), value)
}

/// The body's display mesh and how many faces could not be meshed.
pub(crate) fn mesh_of(body: &Body) -> Result<(Mesh, usize), String> {
    let (record, _) = body.to_record().map_err(fail)?;
    let (low, high) = body.bounds().map_err(fail)?;
    let size = (0..3)
        .map(|axis| (high[axis] - low[axis]).powi(2))
        .sum::<f64>()
        .sqrt();
    let (tess, _) = plan(&record, &MeshTolerance::display_of(size), size).map_err(fail)?;
    let (mesh, refused) = tess.evaluate();
    Ok((mesh, refused.len()))
}

/// Triangle count, time and refusals of the display mesh.
pub fn mesh_measures(body: &Body) -> Result<Vec<(String, String)>, String> {
    let start = Instant::now();
    let (mesh, refused) = mesh_of(body)?;
    Ok(vec![
        entry("triangle-count", mesh.triangles.len().to_string()),
        entry(
            "mesh-seconds",
            format!("{:.3}", start.elapsed().as_secs_f64()),
        ),
        entry("faces-not-meshed", refused.to_string()),
    ])
}

/// Writes the body as a STEP file and reads the file back with solidcore's own reader.
pub fn step_measures(body: &Body, folder: &Path, id: u32) -> Result<Vec<(String, String)>, String> {
    let (record, _) = body.to_record().map_err(fail)?;
    let mut kernel = Kernel::default();
    let brep = to_kernel_body(&mut kernel, &record, true).map_err(fail)?;
    let model = step_brep::brep_to_model(&brep).map_err(fail)?;
    let text = step_export::write(&model).map_err(fail)?;
    let path = folder.join(format!("case-{id}.step"));
    std::fs::write(&path, &text).map_err(fail)?;
    let back = step_import::import(text.as_bytes(), &step_import::part21::Limits::default())
        .map_err(fail)?;
    Ok(vec![
        entry(&format!("step-bytes-{id}"), text.len().to_string()),
        entry(
            &format!("step-bodies-read-back-{id}"),
            back.bodies.len().to_string(),
        ),
    ])
}

/// Draws the body as a picture; a failed picture must not change a case's outcome.
pub fn picture(body: &Body, folder: &Path, id: u32) -> Option<PathBuf> {
    let (mesh, _) = mesh_of(body).ok()?;
    let path = folder.join(format!("case-{id}.png"));
    std::fs::write(&path, encode(&draw(&mesh))).ok()?;
    Some(path)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let length = dot(a, a).sqrt().max(1e-12);
    [a[0] / length, a[1] / length, a[2] / length]
}

/// A smooth-shaded view from above and to the front right, using the mesh's vertex normals.
fn draw(mesh: &Mesh) -> Vec<u8> {
    let view = unit([1.0, -1.2, 0.9]);
    let right = unit(cross([0.0, 0.0, 1.0], view));
    let up = cross(view, right);
    let at = |index: u32| {
        let p = mesh.positions[index as usize];
        [p.x, p.y, p.z]
    };
    let (mut low, mut high) = ([f64::MAX; 2], [f64::MIN; 2]);
    for index in 0..mesh.positions.len() {
        let p = at(index as u32);
        for (axis, value) in [dot(p, right), dot(p, up)].into_iter().enumerate() {
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
    }
    let span = (high[0] - low[0]).max(high[1] - low[1]).max(1e-9);
    let scale = (WIDTH.min(HEIGHT) as f64) * 0.9 / span;
    let centre = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0];
    let mut pixels = vec![245_u8; WIDTH * HEIGHT * 3];
    let mut depth = vec![f64::MIN; WIDTH * HEIGHT];
    for triangle in &mesh.triangles {
        let corners = triangle.map(at);
        let shades = triangle.map(|index| {
            let n = mesh.normals[index as usize];
            70.0 + 160.0 * dot([n.x, n.y, n.z], unit([0.5, -0.4, 1.0])).abs()
        });
        let screen = corners.map(|p| {
            [
                WIDTH as f64 / 2.0 + (dot(p, right) - centre[0]) * scale,
                HEIGHT as f64 / 2.0 - (dot(p, up) - centre[1]) * scale,
                dot(p, view),
            ]
        });
        fill(&screen, shades, &mut pixels, &mut depth);
    }
    pixels
}

fn fill(s: &[[f64; 3]; 3], shades: [f64; 3], pixels: &mut [u8], depth: &mut [f64]) {
    let area =
        (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let bound = |axis: usize, limit: usize| {
        let values = [s[0][axis], s[1][axis], s[2][axis]];
        let low = values
            .iter()
            .copied()
            .fold(f64::MAX, f64::min)
            .floor()
            .max(0.0) as usize;
        let high = values
            .iter()
            .copied()
            .fold(f64::MIN, f64::max)
            .ceil()
            .max(0.0) as usize;
        (low, high.min(limit - 1))
    };
    let (left, right) = bound(0, WIDTH);
    let (top, bottom) = bound(1, HEIGHT);
    for y in top..=bottom {
        for x in left..=right {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let w1 = ((px - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (py - s[0][1]))
                / area;
            let w2 = ((s[1][0] - s[0][0]) * (py - s[0][1]) - (px - s[0][0]) * (s[1][1] - s[0][1]))
                / area;
            let w0 = 1.0 - w1 - w2;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * s[0][2] + w1 * s[1][2] + w2 * s[2][2];
            let index = y * WIDTH + x;
            if z > depth[index] {
                depth[index] = z;
                let shade = w0 * shades[0] + w1 * shades[1] + w2 * shades[2];
                pixels[index * 3..index * 3 + 3].fill(shade as u8);
            }
        }
    }
}

fn crc(bytes: &[u8]) -> u32 {
    let mut value = u32::MAX;
    for byte in bytes {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = if value & 1 == 1 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
    }
    !value
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let mut tagged = kind.to_vec();
    tagged.extend_from_slice(data);
    out.extend_from_slice(&tagged);
    out.extend(crc(&tagged).to_be_bytes());
}

/// A PNG with uncompressed data, which keeps this bench free of an image library.
fn encode(pixels: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(pixels.len() + HEIGHT);
    for row in pixels.chunks(WIDTH * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(index + 1 == blocks.len()));
        let length = block.len() as u16;
        zlib.extend(length.to_le_bytes());
        zlib.extend((!length).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend(((b << 16) | a).to_be_bytes());
    let mut header = Vec::new();
    header.extend((WIDTH as u32).to_be_bytes());
    header.extend((HEIGHT as u32).to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}
