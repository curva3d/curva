//! Rhino `.3dm` files, read and written in Curva's own Rust (spec `docs/specs/3dm-files.md`).
//! The format is the one McNeel documents through the openNURBS source; no code is taken from it.

pub mod archive;
pub mod export;
pub mod import;
pub mod model;
pub mod read;
pub mod write;

pub use archive::{Error, Result};
use archive::{Reader, bad, tcode};

/// The archive version and the chunks after a file's 32-byte start.
///
/// # Errors
/// If it is not a `.3dm` file.
pub fn start(data: &[u8]) -> Result<(u32, Reader<'_>)> {
    let head = data.get(..32).ok_or_else(|| bad("not a .3dm file"))?;
    if &head[..24] != b"3D Geometry File Format " {
        return Err(bad("not a .3dm file"));
    }
    let version: u32 = std::str::from_utf8(&head[24..32])
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| bad("a .3dm file of unknown version"))?;
    if !matches!(version, 1..=5 | 50 | 60 | 70 | 80 | 90) {
        return Err(bad(format!("a .3dm file of unknown version {version}")));
    }
    let mut reader = Reader::new(&data[32..], version);
    reader.version = version;
    Ok((version, reader))
}

/// The chunks of a file as an indented list of type codes and lengths, for looking into files.
///
/// # Errors
/// If it is not a `.3dm` file.
pub fn dump(data: &[u8], depth: usize) -> Result<String> {
    let (version, mut reader) = start(data)?;
    let mut out = format!("version {version}\n");
    walk(&mut reader, 0, depth, &mut out);
    Ok(out)
}

fn walk(reader: &mut Reader<'_>, level: usize, depth: usize, out: &mut String) {
    let pad = "  ".repeat(level);
    while !reader.is_done() {
        let next = reader.peek_tcode();
        let mut chunk = match reader.chunk() {
            Ok(chunk) => chunk,
            Err(e) => {
                out.push_str(&format!("{pad}<damaged: {e}, at {next:x?}>\n"));
                return;
            }
        };
        let short = if chunk.is_short() { " (short)" } else { "" };
        out.push_str(&format!(
            "{pad}{:#010x} {}{short}\n",
            chunk.tcode, chunk.value
        ));
        if level + 1 < depth && !chunk.is_short() && chunk.tcode & tcode::TABLE != 0 {
            walk(&mut chunk.body, level + 1, depth, out);
        }
    }
}
